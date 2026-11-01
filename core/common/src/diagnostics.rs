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
/// 替换计算机名后使用的占位符。
pub const HOST_PLACEHOLDER: &str = "<hostname>";
/// 替换 IP 地址后使用的占位符。
pub const IP_PLACEHOLDER: &str = "<ip>";
/// 替换邮箱地址后使用的占位符。
pub const EMAIL_PLACEHOLDER: &str = "<email>";

/// 每个日志文件最多保留的尾部字节数。
pub const LOG_TAIL_BYTES: u64 = 4 * 1024 * 1024;

/// `/Users/<名字>`、`/home/<名字>`、`C:\Users\<名字>`（含 JSON 转义的 `\\`）中的用户名。
static USER_DIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)([\\/](?:users|home)[\\/]+)([^\\/\s"'<>:*?|]+)"#).expect("常量正则")
});

/// 四段都在 0–255 内的点分 IPv4。
static IPV4: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b(?:(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\b",
    )
    .expect("常量正则")
});

/// IPv6 候选：至少两个冒号的十六进制串（可带内嵌 IPv4、`%网卡` 与方括号），再用标准库校验。
static IPV6: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[?[0-9A-Fa-f.]*:[0-9A-Fa-f.:]*:[0-9A-Fa-f.:]*(?:%[0-9A-Za-z]+)?\]?")
        .expect("常量正则")
});

static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}")
        .expect("常量正则")
});

/// 诊断信息的脱敏器。
///
/// 依次处理：令牌（[`redact`]）、登记的敏感值（玩家名、UUID 等，按整词替换为标签）、
/// 用户主目录、路径中的用户名、路径以外出现的系统用户名与计算机名、邮箱。
#[derive(Debug, Default)]
pub struct Scrubber {
    secrets: Vec<(Regex, String)>,
    homes: Vec<String>,
    machine: Vec<(Regex, String)>,
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

    /// 登记本机的系统用户名与计算机名（EasyTier 会把计算机名作为节点主机名写进日志），
    /// 按整词替换为 `<user>` / `<hostname>`。在主目录之后处理，路径仍显示为 `<home>`；
    /// 少于 3 个字符的名字忽略，避免误伤普通文本。
    pub fn with_machine(mut self, user: Option<&str>, host: Option<&str>) -> Self {
        for (value, label) in [(user, USER_PLACEHOLDER), (host, HOST_PLACEHOLDER)] {
            let Some(value) = value.map(str::trim).filter(|v| v.chars().count() >= 3) else {
                continue;
            };
            if let Some(re) = word_regex(&[regex::escape(value)]) {
                self.machine.push((re, label.to_owned()));
            }
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
        if let Some(re) = word_regex(&forms) {
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
        for (re, label) in &self.machine {
            out = re.replace_all(&out, label.as_str()).into_owned();
        }
        EMAIL.replace_all(&out, EMAIL_PLACEHOLDER).into_owned()
    }
}

/// 把文本中的 IPv4 / IPv6 地址替换为 `<ip>`，端口保留（`1.2.3.4:11010` → `<ip>:11010`，
/// `[2001:db8::1]:11010` → `<ip>:11010`）。
///
/// 只用于放进诊断信息包的 EasyTier 日志：本机日志保留 IP 以便排查节点连通性。
/// 版本号（`1.20.1`、`26.3`）不是四段，不受影响；时间（`12:00:00`）与 MAC 地址不是合法 IPv6，原样保留。
pub fn strip_ips(text: &str) -> String {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for found in IPV6.find_iter(text) {
        // 前后紧挨字母数字的不是独立的地址，例如 Rust 模块路径 `easytier::peers`。
        let before = text[..found.start()].chars().next_back();
        // 候选可能吞进句末的冒号或点，校验时去掉，替换后补回。
        let trimmed = found.as_str().trim_end_matches([':', '.']);
        let after = text[found.end()..].chars().next();
        let inner = trimmed.trim_start_matches('[').trim_end_matches(']');
        let addr = inner.split_once('%').map_or(inner, |(addr, _)| addr);
        let balanced = trimmed.starts_with('[') == trimmed.ends_with(']');
        let standalone =
            !before.is_some_and(is_word) && (trimmed.ends_with(']') || !after.is_some_and(is_word));
        if balanced && standalone && addr.parse::<std::net::Ipv6Addr>().is_ok() {
            out.push_str(&text[last..found.start()]);
            out.push_str(IP_PLACEHOLDER);
            last = found.start() + trimmed.len();
        }
    }
    out.push_str(&text[last..]);
    IPV4.replace_all(&out, IP_PLACEHOLDER).into_owned()
}

/// 整词、不区分大小写地匹配 `forms`（已转义）中任一写法。
fn word_regex(forms: &[String]) -> Option<Regex> {
    Regex::new(&format!(r"(?i)\b(?:{})\b", forms.join("|"))).ok()
}

/// 读取单个日志文件尾部的 [`LOG_TAIL_BYTES`]（从第一个完整行开始）；文件不存在时返回 `None`。
pub fn read_log(path: &Path) -> AppResult<Option<String>> {
    if !path.is_file() {
        return Ok(None);
    }
    read_tail(path, LOG_TAIL_BYTES).map(Some)
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
    fn hides_machine_names_outside_paths() {
        let s = Scrubber::new()
            .with_home(Path::new(r"C:\Users\jack"))
            .with_machine(Some("jack"), Some("DESKTOP-JACK01"));
        assert_eq!(
            s.scrub(r"peer desktop-jack01 via C:\Users\jack\tmcl, user=jack, jackson"),
            r"peer <hostname> via <home>\tmcl, user=<user>, jackson"
        );
        // 太短的名字不登记。
        let s = Scrubber::new().with_machine(Some("pc"), Some(" "));
        assert_eq!(s.scrub("pc started"), "pc started");
    }

    #[test]
    fn strips_ipv4_ipv6_and_keeps_ports() {
        assert_eq!(
            strip_ips("connected to 203.0.113.7:11010 via tcp://198.51.100.20:11010"),
            "connected to <ip>:11010 via tcp://<ip>:11010"
        );
        assert_eq!(
            strip_ips("peer 2001:db8::1 at [2001:db8:0:1::42]:11010, link fe80::1%eth0."),
            "peer <ip> at <ip>:11010, link <ip>."
        );
        assert_eq!(strip_ips("mapped ::ffff:192.0.2.1 ok"), "mapped <ip> ok");
        assert_eq!(strip_ips("listen udp://[::]:0"), "listen udp://<ip>:0");
    }

    #[test]
    fn ip_stripping_leaves_versions_times_and_macs() {
        let text = "Minecraft 1.20.1, 26.3, EasyTier 2.4.5 at 2026-11-01 12:34:56.789 mac aa:bb:cc:dd:ee:ff 999.1.1.1";
        assert_eq!(strip_ips(text), text);
        // EasyTier 的 tracing 输出带 Rust 模块路径。
        let text = "INFO easytier::peers::peer_manager: crate::dead::beef";
        assert_eq!(strip_ips(text), text);
    }

    #[test]
    fn read_log_tolerates_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("easytier.log");
        assert_eq!(read_log(&path).unwrap(), None);
        std::fs::write(&path, "peer up\n").unwrap();
        assert_eq!(read_log(&path).unwrap().as_deref(), Some("peer up\n"));
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
