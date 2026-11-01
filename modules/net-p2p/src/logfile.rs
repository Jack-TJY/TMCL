//! EasyTier 输出的日志文件：`<data_root>/p2p/easytier.log`，带大小上限与轮转。
//!
//! 只保留当前文件与上一份（`easytier.old.log`）：每次启动 `easytier-core` 时轮转一次，
//! 写入超过 [`MAX_LOG_BYTES`] 时再轮转，所以两份合计不超过约 2 × [`MAX_LOG_BYTES`]。
//! 写入的行已由调用方去掉网络密钥；导出诊断信息时还会再经过 `tmcl-common` 的脱敏器。

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// 当前日志文件名。
pub const EASYTIER_LOG: &str = "easytier.log";
/// 上一份日志文件名。
pub const EASYTIER_LOG_OLD: &str = "easytier.old.log";
/// 单个日志文件的大小上限（字节）。
pub const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

/// 按行写入、超过上限即轮转的日志文件。
#[derive(Debug)]
pub struct RotatingLog {
    path: PathBuf,
    old: PathBuf,
    max_bytes: u64,
    file: File,
    written: u64,
}

impl RotatingLog {
    /// 在 `dir` 下打开 [`EASYTIER_LOG`]：已有的当前文件先改名为 [`EASYTIER_LOG_OLD`]（覆盖更旧的）。
    pub fn open(dir: &Path) -> io::Result<Self> {
        Self::with_limit(dir, MAX_LOG_BYTES)
    }

    /// 同 [`RotatingLog::open`]，指定大小上限（测试用）。
    pub fn with_limit(dir: &Path, max_bytes: u64) -> io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(EASYTIER_LOG);
        let old = dir.join(EASYTIER_LOG_OLD);
        rotate(&path, &old)?;
        let file = create(&path)?;
        Ok(Self {
            path,
            old,
            max_bytes,
            file,
            written: 0,
        })
    }

    /// 写入一行（自动补换行）。写入后会超过上限时先轮转；单行本身超过上限时截断。
    pub fn write_line(&mut self, line: &str) -> io::Result<()> {
        let max_line = usize::try_from(self.max_bytes.saturating_sub(1)).unwrap_or(usize::MAX);
        let line = truncate(line, max_line);
        let len = line.len() as u64 + 1;
        if self.written > 0 && self.written + len > self.max_bytes {
            self.file.flush()?;
            rotate(&self.path, &self.old)?;
            self.file = create(&self.path)?;
            self.written = 0;
        }
        self.file.write_all(line.as_bytes())?;
        self.file.write_all(b"\n")?;
        self.written += len;
        Ok(())
    }
}

/// 把 `path` 改名为 `old`（覆盖），`path` 不存在时什么也不做。
fn rotate(path: &Path, old: &Path) -> io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    // Windows 上 rename 不能覆盖已有文件。
    match std::fs::remove_file(old) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    std::fs::rename(path, old)
}

fn create(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
}

/// 截到不超过 `max` 字节的字符边界。
fn truncate(line: &str, max: usize) -> &str {
    if line.len() <= max {
        return line;
    }
    let mut end = max;
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    &line[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(dir: &Path, name: &str) -> String {
        std::fs::read_to_string(dir.join(name)).unwrap_or_default()
    }

    #[test]
    fn open_keeps_one_previous_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = RotatingLog::open(dir.path()).unwrap();
        log.write_line("first run").unwrap();
        drop(log);
        let mut log = RotatingLog::open(dir.path()).unwrap();
        log.write_line("second run").unwrap();
        drop(log);
        let mut log = RotatingLog::open(dir.path()).unwrap();
        log.write_line("third run").unwrap();
        drop(log);

        assert_eq!(read(dir.path(), EASYTIER_LOG), "third run\n");
        assert_eq!(read(dir.path(), EASYTIER_LOG_OLD), "second run\n");
        let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn rotates_when_exceeding_limit() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = RotatingLog::with_limit(dir.path(), 16).unwrap();
        for line in ["aaaaaa", "bbbbbb", "cccccc", "dddddd", "eeeeee"] {
            log.write_line(line).unwrap();
        }
        drop(log);
        // 每行 7 字节，上限 16：每两行一个文件，只留最近两份。
        assert_eq!(read(dir.path(), EASYTIER_LOG), "eeeeee\n");
        assert_eq!(read(dir.path(), EASYTIER_LOG_OLD), "cccccc\ndddddd\n");
        for name in [EASYTIER_LOG, EASYTIER_LOG_OLD] {
            assert!(std::fs::metadata(dir.path().join(name)).unwrap().len() <= 16);
        }
    }

    #[test]
    fn overlong_lines_are_truncated_on_char_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = RotatingLog::with_limit(dir.path(), 8).unwrap();
        log.write_line("节点节点节点").unwrap();
        drop(log);
        let text = read(dir.path(), EASYTIER_LOG);
        assert_eq!(text, "节点\n");
        assert!(text.len() <= 8);
    }
}
