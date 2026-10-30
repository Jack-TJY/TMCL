//! 诊断信息包（PLAN.md M2-16）：脱敏工具、读取最近的启动器日志、写出 zip。
//!
//! 收集哪些内容由命令层决定；这里只保证写进包里的每段文本都经过 [`Scrubber`]。

use std::io::Write;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::error::{AppError, AppResult, ErrorCode};
use crate::log::{LOG_FILE_PREFIX, redact};

/// 替换用户目录中用户名后使用的占位符。
pub const USER_PLACEHOLDER: &str = "<user>";
/// 替换用户主目录后使用的占位符。
pub const HOME_PLACEHOLDER: &str = "<home>";
/// 替换邮箱地址后使用的占位符。
pub const EMAIL_PLACEHOLDER: &str = "<email>";

/// 每个日志文件最多保留的尾部字节数。
pub const LOG_TAIL_BYTES: u64 = 4 * 1024 * 1024;

/// `/Users/<名字>`、`/home/<名字>`、`C:\Users\<名字>`（含 JSON 转义的 `\\`）中的用户名。
static USER_DIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)([\\/](?:users|home)[\\/]+)([^\\/\s"'<>:*?|]+)"#).expect("常量正则")
});

static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}")
        .expect("常量正则")
});

/// 诊断信息的脱敏器。
///
/// 依次处理：令牌（[`redact`]）、登记的敏感值（玩家名、UUID 等，按整词替换为标签）、
/// 用户主目录、路径中的用户名、邮箱。
#[derive(Debug, Default)]
pub struct Scrubber {
    secrets: Vec<(Regex, String)>,
    homes: Vec<String>,
}

impl Scrubber {
    /// 不带任何登记值的脱敏器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记用户主目录：完整路径替换为 `<home>`（同时匹配 `\` 与 JSON 转义的 `\\` 写法）。
    pub fn with_home(mut self, home: &Path) -> Self {
        let home = home.display().to_string();
        let home = home.trim_end_matches(['/', '\\']);
        // 太短的路径（例如 `/`）替换后会破坏其他内容。
        if home.len() >= 3 {
            self.homes.push(home.replace('\\', r"\\"));
            self.homes.push(home.to_owned());
        }
        self
    }

    /// 登记一个敏感值，按整词（不区分大小写）替换为 `label`。
    ///
    /// 带连字符的 UUID 同时匹配不带连字符的写法；空值忽略。
    pub fn secret(&mut self, value: &str, label: &str) {
        let value = value.trim();
        if value.is_empty() {
            return;
        }
        let mut forms = vec![regex::escape(value)];
        if value.len() == 36 && value.matches('-').count() == 4 {
            forms.push(regex::escape(&value.replace('-', "")));
        }
        let pattern = format!(r"(?i)\b(?:{})\b", forms.join("|"));
        if let Ok(re) = Regex::new(&pattern) {
            self.secrets.push((re, label.to_owned()));
        }
    }

    /// 对一段文本脱敏。
    pub fn scrub(&self, text: &str) -> String {
        let mut out = redact(text).into_owned();
        for (re, label) in &self.secrets {
            if re.is_match(&out) {
                out = re.replace_all(&out, label.as_str()).into_owned();
            }
        }
        for home in &self.homes {
            out = out.replace(home.as_str(), HOME_PLACEHOLDER);
        }
        out = USER_DIR
            .replace_all(&out, format!("${{1}}{USER_PLACEHOLDER}"))
            .into_owned();
        EMAIL.replace_all(&out, EMAIL_PLACEHOLDER).into_owned()
    }
}

/// 读取 `logs_dir` 中的启动器日志（`tmcl.*.log`），按文件名排序，每个最多保留尾部
/// [`LOG_TAIL_BYTES`]。目录不存在时返回空列表。
pub fn read_logs(logs_dir: &Path) -> AppResult<Vec<(String, String)>> {
    let entries = match std::fs::read_dir(logs_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| name.starts_with(&format!("{LOG_FILE_PREFIX}.")) && name.ends_with(".log"))
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let text = read_tail(&logs_dir.join(&name), LOG_TAIL_BYTES)?;
            Ok((name, text))
        })
        .collect()
}

fn read_tail(path: &Path, max: u64) -> AppResult<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    let mut buf = Vec::new();
    if len > max {
        file.seek(SeekFrom::Start(len - max))?;
    }
    file.read_to_end(&mut buf)?;
    let text = String::from_utf8_lossy(&buf);
    // 截断时从第一个完整行开始。
    let text = if len > max {
        text.split_once('\n').map_or("", |(_, rest)| rest)
    } else {
        &text
    };
    Ok(text.to_owned())
}

/// 把若干文本文件写成 zip（`dest` 已存在时覆盖），每个文件写入前都经过 `scrubber`。
pub fn write_bundle(dest: &Path, files: &[(String, String)], scrubber: &Scrubber) -> AppResult<()> {
    let zip_err = |e: zip::result::ZipError| {
        AppError::new(ErrorCode::CommonIo)
            .with_param("path", dest.display())
            .with_detail(e.to_string())
    };
    let file = std::fs::File::create(dest)
        .map_err(|e| AppError::from(e).with_param("path", dest.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, text) in files {
        zip.start_file(name.as_str(), options).map_err(zip_err)?;
        zip.write_all(scrubber.scrub(text).as_bytes())?;
    }
    zip.finish().map_err(zip_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;

    fn scrubber() -> Scrubber {
        let mut s = Scrubber::new().with_home(Path::new(r"C:\Users\Jack"));
        s.secret("Jack_TJY", "<player-1>");
        s.secret("069a79f4-44e9-4726-a5be-fca90e38aaf5", "<uuid-1>");
        s
    }

    #[test]
    fn replaces_registered_secrets_as_words() {
        let s = scrubber();
        assert_eq!(
            s.scrub("--username Jack_TJY --uuid 069a79f444e94726a5befca90e38aaf5"),
            "--username <player-1> --uuid <uuid-1>"
        );
        assert_eq!(
            s.scrub("id 069A79F4-44E9-4726-A5BE-FCA90E38AAF5"),
            "id <uuid-1>"
        );
        // 只替换整词。
        assert_eq!(s.scrub("Jack_TJY2"), "Jack_TJY2");
    }

    #[test]
    fn hides_home_and_user_names_in_paths() {
        let s = scrubber();
        assert_eq!(
            s.scrub(r"C:\Users\Jack\Desktop\TMCL"),
            r"<home>\Desktop\TMCL"
        );
        assert_eq!(
            s.scrub(r#"{"path": "C:\\Users\\Jack\\x"}"#),
            r#"{"path": "<home>\\x"}"#
        );
        assert_eq!(
            s.scrub("/home/tjy/.local/share"),
            "/home/<user>/.local/share"
        );
        assert_eq!(s.scrub("/Users/alice/Library"), "/Users/<user>/Library");
        assert_eq!(s.scrub(r"D:\users\Bob\a"), r"D:\users\<user>\a");
    }

    #[test]
    fn keeps_existing_token_redaction_and_hides_emails() {
        let s = Scrubber::new();
        assert_eq!(
            s.scrub("--accessToken eyJ0 by someone@example.com"),
            "--accessToken *** by <email>"
        );
    }

    #[test]
    fn ignores_empty_secrets_and_root_home() {
        let mut s = Scrubber::new().with_home(Path::new("/"));
        s.secret("  ", "<x>");
        assert_eq!(s.scrub("/opt/a b"), "/opt/a b");
    }

    #[test]
    fn reads_only_launcher_logs_sorted() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("tmcl.2026-10-30.log"), "b\n").unwrap();
        std::fs::write(dir.path().join("tmcl.2026-10-29.log"), "a\n").unwrap();
        std::fs::write(dir.path().join("other.log"), "x").unwrap();
        let logs = read_logs(dir.path()).unwrap();
        let names: Vec<&str> = logs.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["tmcl.2026-10-29.log", "tmcl.2026-10-30.log"]);
        assert!(read_logs(&dir.path().join("missing")).unwrap().is_empty());
    }

    #[test]
    fn long_logs_keep_whole_lines_from_the_tail() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tmcl.log");
        std::fs::write(&path, "first line\nsecond\nthird\n").unwrap();
        assert_eq!(read_tail(&path, 10).unwrap(), "third\n");
        assert_eq!(
            read_tail(&path, 100).unwrap(),
            "first line\nsecond\nthird\n"
        );
    }

    #[test]
    fn bundle_contents_are_scrubbed() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("diag.zip");
        let files = vec![(
            "logs/tmcl.log".to_owned(),
            r"launch C:\Users\Jack\x --username Jack_TJY".to_owned(),
        )];
        write_bundle(&dest, &files, &scrubber()).unwrap();
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&dest).unwrap()).unwrap();
        let mut text = String::new();
        zip.by_name("logs/tmcl.log")
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, r"launch <home>\x --username <player-1>");
    }
}
