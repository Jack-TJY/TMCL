//! `options.txt` 的解析与写回。
//!
//! 每行是 `键:值`，以第一个 `:` 分隔（与游戏一致）。解析时保留每一行的原文和换行符，
//! 未修改的文件写回后逐字节相同；修改只替换对应行，新增的键追加在末尾。

/// `options.txt` 中的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    /// `键:值` 行。`raw` 为原文（不含换行符），值被修改后为 `None`。
    Entry {
        key: String,
        value: String,
        raw: Option<String>,
        eol: String,
    },
    /// 空行、注释或无法识别的行，原样保留。
    Other { raw: String, eol: String },
}

/// 解析后的 `options.txt`，保留未知行与顺序。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptionsFile {
    lines: Vec<Line>,
}

impl OptionsFile {
    /// 解析文本。任何文本都能解析；不含 `:` 的行按原样保留。
    pub fn parse(text: &str) -> Self {
        let mut lines = Vec::new();
        let mut rest = text;
        while !rest.is_empty() {
            let (line, eol, next) = match rest.find('\n') {
                Some(i) if rest[..i].ends_with('\r') => (&rest[..i - 1], "\r\n", &rest[i + 1..]),
                Some(i) => (&rest[..i], "\n", &rest[i + 1..]),
                None => (rest, "", ""),
            };
            let eol = eol.to_owned();
            lines.push(match line.split_once(':') {
                Some((key, value)) if !key.is_empty() => Line::Entry {
                    key: key.to_owned(),
                    value: value.to_owned(),
                    raw: Some(line.to_owned()),
                    eol,
                },
                _ => Line::Other {
                    raw: line.to_owned(),
                    eol,
                },
            });
            rest = next;
        }
        Self { lines }
    }

    /// 写回文本。
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for line in &self.lines {
            match line {
                Line::Entry {
                    raw: Some(raw),
                    eol,
                    ..
                }
                | Line::Other { raw, eol } => {
                    out.push_str(raw);
                    out.push_str(eol);
                }
                Line::Entry {
                    key,
                    value,
                    raw: None,
                    eol,
                } => {
                    out.push_str(key);
                    out.push(':');
                    out.push_str(value);
                    out.push_str(eol);
                }
            }
        }
        out
    }

    /// 按文件顺序列出所有 `(键, 值)`；同一个键出现多次时都会列出。
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.lines.iter().filter_map(|line| match line {
            Line::Entry { key, value, .. } => Some((key.as_str(), value.as_str())),
            Line::Other { .. } => None,
        })
    }

    /// 键的值；出现多次时取最后一次（游戏读取时后者覆盖前者）。
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries()
            .filter(|(k, _)| *k == key)
            .last()
            .map(|(_, v)| v)
    }

    /// 设置键的值：已有的行（全部同名行）就地修改，没有则追加到末尾。
    pub fn set(&mut self, key: &str, new_value: &str) {
        let mut found = false;
        for line in &mut self.lines {
            if let Line::Entry {
                key: k, value, raw, ..
            } = line
                && k == key
            {
                found = true;
                if value != new_value {
                    *value = new_value.to_owned();
                    *raw = None;
                }
            }
        }
        if found {
            return;
        }
        let eol = self.newline().to_owned();
        if let Some(Line::Entry { eol: last, .. } | Line::Other { eol: last, .. }) =
            self.lines.last_mut()
            && last.is_empty()
        {
            *last = eol.clone();
        }
        self.lines.push(Line::Entry {
            key: key.to_owned(),
            value: new_value.to_owned(),
            raw: None,
            eol,
        });
    }

    /// 文件使用的换行符：首个换行为 `\r\n` 时沿用，否则为 `\n`。
    fn newline(&self) -> &str {
        self.lines
            .iter()
            .map(|line| match line {
                Line::Entry { eol, .. } | Line::Other { eol, .. } => eol.as_str(),
            })
            .find(|eol| !eol.is_empty())
            .unwrap_or("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_roundtrip_is_byte_identical() {
        for text in [
            "",
            "version:3465\nlang:en_us\n",
            "a:1\r\nb:2\r\n",
            "a:1\nno colon line\n\n:empty key\nc:x:y",
            "key_key.forward:17:CONTROL\r\nmixed:1\nlast",
        ] {
            assert_eq!(OptionsFile::parse(text).to_text(), text);
        }
    }

    #[test]
    fn set_keeps_order_and_unknown_lines() {
        let mut file = OptionsFile::parse("a:1\r\n# note\r\nb:2\r\nb:3");
        file.set("b", "9");
        file.set("a", "1");
        file.set("c", "new");
        assert_eq!(file.to_text(), "a:1\r\n# note\r\nb:9\r\nb:9\r\nc:new\r\n");
        assert_eq!(file.get("b"), Some("9"));
        assert_eq!(file.get("missing"), None);
    }

    #[test]
    fn value_keeps_later_colons_and_last_duplicate_wins() {
        let file = OptionsFile::parse("lastServer:host:25565\nx:1\nx:2\n");
        assert_eq!(file.get("lastServer"), Some("host:25565"));
        assert_eq!(file.get("x"), Some("2"));
        assert_eq!(file.entries().count(), 3);

        let mut empty = OptionsFile::parse("");
        empty.set("k", "v");
        assert_eq!(empty.to_text(), "k:v\n");
    }
}
