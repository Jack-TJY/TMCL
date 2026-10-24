//! 数据目录解析（PLAN.md 第 3.5 节，详见 `docs/contracts/data-layout.md`）。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult, ErrorCode};

/// 数据目录相对启动器安装目录的名字。
pub const DATA_DIR_NAME: &str = "tmcl-data";

/// [`DataPaths::resolve_linux`] 用到的环境变量。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinuxEnv {
    /// `$APPIMAGE`：AppImage 运行时指向 `.AppImage` 文件本身。
    pub appimage: Option<PathBuf>,
    /// `$XDG_DATA_HOME`。
    pub xdg_data_home: Option<PathBuf>,
    /// `$HOME`。
    pub home: Option<PathBuf>,
}

/// 启动器数据目录及其中各固定路径。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataPaths {
    root: PathBuf,
}

impl DataPaths {
    /// 按当前 exe 位置解析数据目录：启动器安装目录下的 `tmcl-data/`。
    ///
    /// Linux 上另有两种安装方式，规则见 [`resolve_linux`](Self::resolve_linux)。
    pub fn resolve() -> AppResult<Self> {
        let exe = std::env::current_exe().map_err(|e| {
            AppError::new(ErrorCode::CommonDataDirUnavailable).with_detail(e.to_string())
        })?;
        if cfg!(target_os = "linux") {
            let env = LinuxEnv {
                appimage: std::env::var_os("APPIMAGE").map(PathBuf::from),
                xdg_data_home: std::env::var_os("XDG_DATA_HOME").map(PathBuf::from),
                home: std::env::var_os("HOME").map(PathBuf::from),
            };
            return Self::resolve_linux(&exe, &env);
        }
        Self::resolve_from_exe(&exe)
    }

    /// Linux 版的 [`resolve`](Self::resolve)，环境变量由参数给出，便于测试。
    ///
    /// - AppImage 运行时 exe 位于只读的挂载目录，数据放在 `.AppImage` 文件所在目录
    ///   （`$APPIMAGE` 的父目录）。
    /// - deb 等系统包把 exe 装到 `/usr/` 下，该目录普通用户不可写，数据放在
    ///   `$XDG_DATA_HOME/tmcl-data/`，未设置时为 `~/.local/share/tmcl-data/`。
    /// - 其他情况与 [`resolve_from_exe`](Self::resolve_from_exe) 相同。
    ///
    /// 不是绝对路径的环境变量按未设置处理。
    pub fn resolve_linux(exe: &Path, env: &LinuxEnv) -> AppResult<Self> {
        let absolute = |p: &Option<PathBuf>| p.clone().filter(|p| p.has_root());
        if let Some(appimage) = absolute(&env.appimage) {
            return Self::resolve_from_exe(&appimage);
        }
        if exe.starts_with("/usr") {
            let data_home = absolute(&env.xdg_data_home)
                .or_else(|| absolute(&env.home).map(|home| home.join(".local/share")))
                .ok_or_else(|| {
                    AppError::new(ErrorCode::CommonDataDirUnavailable)
                        .with_param("path", exe.display())
                        .with_detail("系统安装但没有 XDG_DATA_HOME 与 HOME")
                })?;
            return Ok(Self {
                root: data_home.join(DATA_DIR_NAME),
            });
        }
        Self::resolve_from_exe(exe)
    }

    /// [`resolve`](Self::resolve) 的纯函数版本，便于测试。
    ///
    /// 安装目录是 exe 所在目录；macOS 上 exe 位于 `TMCL.app/Contents/MacOS/`，
    /// 为避免写进应用包（会破坏签名），安装目录取 `.app` 所在目录。
    pub fn resolve_from_exe(exe: &Path) -> AppResult<Self> {
        let exe_dir = exe.parent().ok_or_else(|| {
            AppError::new(ErrorCode::CommonDataDirUnavailable).with_detail("exe 没有父目录")
        })?;
        let install_dir = app_bundle_parent(exe_dir).unwrap_or(exe_dir);
        Ok(Self {
            root: install_dir.join(DATA_DIR_NAME),
        })
    }

    /// 直接以给定目录作为数据根目录（测试或自定义位置）。
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// 创建所有固定子目录（已存在时不报错）。
    pub fn ensure_dirs(&self) -> AppResult<()> {
        for dir in [
            self.root.clone(),
            self.shared_versions_dir(),
            self.shared_libraries_dir(),
            self.shared_assets_dir(),
            self.java_dir(),
            self.instances_dir(),
            self.trash_dir(),
            self.ui_backgrounds_dir(),
            self.cache_dir(),
            self.logs_dir(),
        ] {
            fs::create_dir_all(&dir).map_err(|e| {
                AppError::new(ErrorCode::CommonDataDirUnavailable)
                    .with_param("path", dir.display())
                    .with_detail(e.to_string())
            })?;
        }
        Ok(())
    }

    /// 数据根目录。
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `config.json`：启动器设置。
    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.json")
    }

    /// `tmcl.db`：SQLite 数据库。
    pub fn db_file(&self) -> PathBuf {
        self.root.join("tmcl.db")
    }

    /// `shared/versions/`：多实例共享的版本文件。
    pub fn shared_versions_dir(&self) -> PathBuf {
        self.root.join("shared").join("versions")
    }

    /// `shared/libraries/`：多实例共享的依赖库。
    pub fn shared_libraries_dir(&self) -> PathBuf {
        self.root.join("shared").join("libraries")
    }

    /// `shared/assets/`：多实例共享的资源文件。
    pub fn shared_assets_dir(&self) -> PathBuf {
        self.root.join("shared").join("assets")
    }

    /// `java/`：启动器下载的 Java 运行时根目录。
    pub fn java_dir(&self) -> PathBuf {
        self.root.join("java")
    }

    /// `java/<runtime-name>/`。
    pub fn java_runtime_dir(&self, runtime_name: &str) -> PathBuf {
        self.java_dir().join(runtime_name)
    }

    /// `instances/`：所有实例。
    pub fn instances_dir(&self) -> PathBuf {
        self.root.join("instances")
    }

    /// `instances/<instance-id>/`。
    pub fn instance_dir(&self, instance_id: &str) -> PathBuf {
        self.instances_dir().join(instance_id)
    }

    /// `instances/<instance-id>/instance.json`。
    pub fn instance_file(&self, instance_id: &str) -> PathBuf {
        self.instance_dir(instance_id).join("instance.json")
    }

    /// `instances/<instance-id>/game/`：作为 `--gameDir` 传给游戏。
    pub fn instance_game_dir(&self, instance_id: &str) -> PathBuf {
        self.instance_dir(instance_id).join("game")
    }

    /// `trash/`：已删除的实例，保留 7 天后清理。
    pub fn trash_dir(&self) -> PathBuf {
        self.root.join("trash")
    }

    /// `ui/`：界面相关的用户数据。
    pub fn ui_dir(&self) -> PathBuf {
        self.root.join("ui")
    }

    /// `ui/layout.json`：主页组件布局。
    pub fn ui_layout_file(&self) -> PathBuf {
        self.ui_dir().join("layout.json")
    }

    /// `ui/backgrounds/`：用户背景图。
    pub fn ui_backgrounds_dir(&self) -> PathBuf {
        self.ui_dir().join("backgrounds")
    }

    /// `cache/`：可随时清空的缓存。
    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    /// `logs/`：启动器日志（已脱敏）。
    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }
}

/// `X.app/Contents/MacOS` → `X.app` 的父目录；不是应用包结构时返回 `None`。
fn app_bundle_parent(exe_dir: &Path) -> Option<&Path> {
    if exe_dir.file_name()? != "MacOS" {
        return None;
    }
    let contents = exe_dir.parent()?;
    if contents.file_name()? != "Contents" {
        return None;
    }
    let bundle = contents.parent()?;
    let is_app = bundle
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("app"));
    if is_app { bundle.parent() } else { None }
}

/// 原子写文件：先写同目录下的临时文件并落盘，再 rename 覆盖目标。
///
/// 进程在任意时刻崩溃，目标文件要么是旧内容，要么是新内容。
pub fn write_atomic(path: &Path, contents: &[u8]) -> AppResult<()> {
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty());
    if let Some(dir) = dir {
        fs::create_dir_all(dir)?;
    }
    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(format!(".{}.tmp", std::process::id()));
    let tmp = path.with_file_name(tmp_name);

    let result = (|| -> AppResult<()> {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_is_next_to_exe() {
        let exe = Path::new("/opt/tmcl/tmcl");
        let paths = DataPaths::resolve_from_exe(exe).unwrap();
        assert_eq!(paths.root(), Path::new("/opt/tmcl").join(DATA_DIR_NAME));
    }

    #[cfg(windows)]
    #[test]
    fn data_dir_is_next_to_windows_exe() {
        let exe = Path::new(r"D:\Games\TMCL\tmcl.exe");
        let paths = DataPaths::resolve_from_exe(exe).unwrap();
        assert_eq!(
            paths.root(),
            Path::new(r"D:\Games\TMCL").join(DATA_DIR_NAME)
        );
    }

    #[test]
    fn macos_bundle_uses_folder_containing_app() {
        let exe = Path::new("/Applications/Games/TMCL.app/Contents/MacOS/tmcl");
        let paths = DataPaths::resolve_from_exe(exe).unwrap();
        assert_eq!(
            paths.root(),
            Path::new("/Applications/Games").join(DATA_DIR_NAME)
        );
    }

    #[test]
    fn non_bundle_macos_like_path_is_left_alone() {
        let exe = Path::new("/opt/Contents/MacOS/tmcl");
        let paths = DataPaths::resolve_from_exe(exe).unwrap();
        assert_eq!(
            paths.root(),
            Path::new("/opt/Contents/MacOS").join(DATA_DIR_NAME)
        );
    }

    #[test]
    fn appimage_uses_folder_containing_appimage_file() {
        let env = LinuxEnv {
            appimage: Some("/home/u/Games/TMCL_0.1.0_amd64.AppImage".into()),
            home: Some("/home/u".into()),
            ..LinuxEnv::default()
        };
        let exe = Path::new("/tmp/.mount_TMCLab12/usr/bin/tmcl");
        let paths = DataPaths::resolve_linux(exe, &env).unwrap();
        assert_eq!(paths.root(), Path::new("/home/u/Games").join(DATA_DIR_NAME));
    }

    #[test]
    fn relative_appimage_is_ignored() {
        let env = LinuxEnv {
            appimage: Some("TMCL.AppImage".into()),
            ..LinuxEnv::default()
        };
        let paths = DataPaths::resolve_linux(Path::new("/opt/tmcl/tmcl"), &env).unwrap();
        assert_eq!(paths.root(), Path::new("/opt/tmcl").join(DATA_DIR_NAME));
    }

    #[test]
    fn system_install_uses_xdg_data_home() {
        let exe = Path::new("/usr/bin/tmcl");
        let env = LinuxEnv {
            xdg_data_home: Some("/data/xdg".into()),
            home: Some("/home/u".into()),
            ..LinuxEnv::default()
        };
        let paths = DataPaths::resolve_linux(exe, &env).unwrap();
        assert_eq!(paths.root(), Path::new("/data/xdg").join(DATA_DIR_NAME));

        let env = LinuxEnv {
            xdg_data_home: Some("relative".into()),
            home: Some("/home/u".into()),
            ..LinuxEnv::default()
        };
        let paths = DataPaths::resolve_linux(exe, &env).unwrap();
        assert_eq!(
            paths.root(),
            Path::new("/home/u/.local/share").join(DATA_DIR_NAME)
        );
    }

    #[test]
    fn system_install_without_home_is_error() {
        let exe = Path::new("/usr/bin/tmcl");
        let err = DataPaths::resolve_linux(exe, &LinuxEnv::default()).unwrap_err();
        assert_eq!(err.code, ErrorCode::CommonDataDirUnavailable);
    }

    #[test]
    fn portable_linux_exe_keeps_exe_dir() {
        let exe = Path::new("/opt/tmcl/tmcl");
        let paths = DataPaths::resolve_linux(exe, &LinuxEnv::default()).unwrap();
        assert_eq!(paths.root(), Path::new("/opt/tmcl").join(DATA_DIR_NAME));
    }

    #[test]
    fn exe_without_parent_is_error() {
        let err = DataPaths::resolve_from_exe(Path::new("")).unwrap_err();
        assert_eq!(err.code, ErrorCode::CommonDataDirUnavailable);
    }

    #[test]
    fn layout_matches_contract() {
        let paths = DataPaths::at("/d");
        let root = Path::new("/d");
        assert_eq!(paths.config_file(), root.join("config.json"));
        assert_eq!(paths.db_file(), root.join("tmcl.db"));
        assert_eq!(paths.shared_libraries_dir(), root.join("shared/libraries"));
        assert_eq!(
            paths.java_runtime_dir("java-runtime-gamma"),
            root.join("java/java-runtime-gamma")
        );
        assert_eq!(
            paths.instance_file("abc"),
            root.join("instances/abc/instance.json")
        );
        assert_eq!(
            paths.instance_game_dir("abc"),
            root.join("instances/abc/game")
        );
        assert_eq!(paths.trash_dir(), root.join("trash"));
        assert_eq!(paths.ui_layout_file(), root.join("ui/layout.json"));
        assert_eq!(paths.logs_dir(), root.join("logs"));
    }

    #[test]
    fn ensure_dirs_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path().join("root"));
        paths.ensure_dirs().unwrap();
        paths.ensure_dirs().unwrap();
        assert!(paths.shared_assets_dir().is_dir());
        assert!(paths.ui_backgrounds_dir().is_dir());
    }

    #[test]
    fn write_atomic_replaces_and_leaves_no_temp() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("sub").join("a.json");
        write_atomic(&target, b"one").unwrap();
        write_atomic(&target, b"two").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"two");
        let entries: Vec<_> = fs::read_dir(target.parent().unwrap()).unwrap().collect();
        assert_eq!(entries.len(), 1);
    }
}
