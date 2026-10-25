//! 把游戏以 log4j XMLLayout 输出到控制台的日志事件转换为可读的文本行。
//!
//! 版本 JSON 的 `logging.client` 配置让游戏把日志以 `<log4j:Event>` XML 写到标准输出，
//! 转换后的格式与游戏自己的 `latest.log` 一致：`[HH:MM:SS] [线程/级别]: 消息`，
//! 多行消息与异常堆栈按原样逐行跟在后面。不是 XML 事件的行原样通过。

use std::sync::OnceLock;

use time::{OffsetDateTime, UtcOffset};

/// 一个事件超过这么多行仍未结束时放弃解析，原样输出，避免异常输出把内存占满。
const MAX_EVENT_LINES: usize = 10_000;

static LOCAL_OFFSET: OnceLock<UtcOffset> = OnceLock::new();

/// 记录本地时区偏移，供日志时间换算。
///
/// 在类 Unix 系统上，进程有多个线程后就无法安全读取时区，所以应在 `main` 一开始
/// （创建任何线程之前）调用；没有调用或读取失败时，第一次需要时再尝试，仍失败则使用 UTC。
pub fn init_local_offset() {
    LOCAL_OFFSET.get_or_init(|| UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC));
}

fn local_offset() -> UtcOffset {
    init_local_offset();
    LOCAL_OFFSET.get().copied().unwrap_or(UtcOffset::UTC)
}

/// 逐行输入、按事件输出的转换器；每个输出流各用一个。
#[derive(Debug, Default)]
pub struct Log4jDecoder {
    /// 正在收集的事件原文；`None` 表示不在事件中。
    event: Option<Vec<String>>,
    offset: Option<UtcOffset>,
}

impl Log4jDecoder {
    /// 新建转换器，时间按本地时区显示。
    pub fn new() -> Self {
        Self::default()
    }

    /// 指定时区（测试用）。
    pub fn with_offset(offset: UtcOffset) -> Self {
        Self {
            event: None,
            offset: Some(offset),
        }
    }

    /// 输入一行原始输出，返回可以立即输出的文本行（事件未结束时为空）。
    pub fn push(&mut self, line: String) -> Vec<String> {
        match &mut self.event {
            None if line.trim_start().starts_with("<log4j:Event ") => {
                let done = line.contains("</log4j:Event>");
                self.event = Some(vec![line]);
                if done { self.finish() } else { Vec::new() }
            }
            None => vec![line],
            Some(lines) => {
                let done = line.contains("</log4j:Event>");
                lines.push(line);
                if done {
                    self.finish()
                } else if lines.len() >= MAX_EVENT_LINES {
                    self.flush()
                } else {
                    Vec::new()
                }
            }
        }
    }

    /// 输出流结束时调用：把未结束的事件原样交出。
    pub fn flush(&mut self) -> Vec<String> {
        self.event.take().unwrap_or_default()
    }

    fn finish(&mut self) -> Vec<String> {
        let raw = self.event.take().unwrap_or_default();
        let offset = self.offset.unwrap_or_else(local_offset);
        match decode_event(&raw.join("\n"), offset) {
            Some(lines) => lines,
            None => raw,
        }
    }
}

/// 解析一个完整事件；格式不认识时返回 `None`，由调用方原样输出。
fn decode_event(xml: &str, offset: UtcOffset) -> Option<Vec<String>> {
    let open_end = xml.find('>')?;
    let open = &xml[..open_end];
    let level = attr(open, "level").unwrap_or_default();
    let thread = attr(open, "thread").unwrap_or_default();
    let time = attr(open, "timestamp")
        .and_then(|t| t.parse::<i128>().ok())
        .and_then(|ms| OffsetDateTime::from_unix_timestamp_nanos(ms * 1_000_000).ok())
        .map(|t| {
            let t = t.to_offset(offset);
            format!("{:02}:{:02}:{:02}", t.hour(), t.minute(), t.second())
        })
        .unwrap_or_else(|| "--:--:--".to_owned());
    let body = &xml[open_end + 1..];
    let message = element(body, "log4j:Message")?;
    let mut lines: Vec<String> = Vec::new();
    let mut message_lines = message.split('\n');
    let first = message_lines.next().unwrap_or_default();
    lines.push(format!("[{time}] [{thread}/{level}]: {first}"));
    lines.extend(message_lines.map(str::to_owned));
    if let Some(throwable) = element(body, "log4j:Throwable") {
        lines.extend(
            throwable
                .trim_end_matches('\n')
                .split('\n')
                .map(str::to_owned),
        );
    }
    Some(
        lines
            .into_iter()
            .map(|l| l.trim_end_matches('\r').to_owned())
            .collect(),
    )
}

/// 读取开始标签中的属性值并反转义。
fn attr(open: &str, name: &str) -> Option<String> {
    let key = format!(" {name}=\"");
    let start = open.find(&key)? + key.len();
    let len = open[start..].find('"')?;
    Some(unescape(&open[start..start + len]))
}

/// 读取元素内容：`<![CDATA[…]]>` 片段拼接（log4j 把内容中的 `]]>` 拆成相邻片段），
/// 否则按转义文本处理。
fn element(body: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = body.find(&open)? + open.len();
    let len = body[start..].find(&close)?;
    let inner = &body[start..start + len];
    if !inner.contains("<![CDATA[") {
        return Some(unescape(inner));
    }
    let mut out = String::new();
    let mut rest = inner;
    while let Some(i) = rest.find("<![CDATA[") {
        let content = &rest[i + "<![CDATA[".len()..];
        let end = content.find("]]>").unwrap_or(content.len());
        out.push_str(&content[..end]);
        rest = content.get(end + "]]>".len()..).unwrap_or_default();
    }
    Some(out)
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#13;", "\r")
        .replace("&#10;", "\n")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(raw: &str) -> Vec<String> {
        let mut d = Log4jDecoder::with_offset(UtcOffset::from_hms(8, 0, 0).unwrap());
        let mut out = Vec::new();
        for line in raw.lines() {
            out.extend(d.push(line.to_owned()));
        }
        out.extend(d.flush());
        out
    }

    #[test]
    fn converts_event_to_text_line() {
        let raw = r#"<log4j:Event logger="net.minecraft.client.Minecraft" timestamp="1792852953950" level="INFO" thread="Render thread">
  <log4j:Message><![CDATA[Setting user: Dev]]></log4j:Message>
</log4j:Event>"#;
        assert_eq!(
            decode(raw),
            ["[22:42:33] [Render thread/INFO]: Setting user: Dev"]
        );
    }

    #[test]
    fn keeps_multiline_message_and_throwable() {
        let raw = r#"<log4j:Event logger="x" timestamp="1792852953950" level="ERROR" thread="main">
  <log4j:Message><![CDATA[Failed:
second line]]></log4j:Message>
  <log4j:Throwable><![CDATA[java.lang.RuntimeException: boom
	at a.b(C.java:1)
]]></log4j:Throwable>
</log4j:Event>"#;
        assert_eq!(
            decode(raw),
            [
                "[22:42:33] [main/ERROR]: Failed:",
                "second line",
                "java.lang.RuntimeException: boom",
                "\tat a.b(C.java:1)",
            ]
        );
    }

    #[test]
    fn passes_plain_lines_and_interleaves_in_order() {
        let raw = r#"plain before
<log4j:Event logger="x" timestamp="0" level="WARN" thread="t &amp; &quot;q&quot;"><log4j:Message><![CDATA[a]]]]><![CDATA[>b]]></log4j:Message></log4j:Event>
plain after"#;
        assert_eq!(
            decode(raw),
            [
                "plain before",
                "[08:00:00] [t & \"q\"/WARN]: a]]>b",
                "plain after",
            ]
        );
    }

    #[test]
    fn unfinished_event_is_flushed_raw() {
        let raw = "<log4j:Event logger=\"x\" timestamp=\"0\" level=\"INFO\" thread=\"t\">\n  <log4j:Message><![CDATA[cut";
        assert_eq!(decode(raw), raw.lines().collect::<Vec<_>>());
    }

    #[test]
    fn crash_signature_survives_decoding() {
        let raw = r#"<log4j:Event logger="STDOUT" timestamp="0" level="INFO" thread="Client thread">
  <log4j:Message><![CDATA[[net.minecraft.client.Minecraft:func_71377_b:359]: #@!@# Game crashed! Crash report saved to: #@!@# C:\x.txt]]></log4j:Message>
</log4j:Event>"#;
        let out = decode(raw);
        assert_eq!(out.len(), 1);
        assert!(crate::is_crash_line(&out[0]));
    }
}
