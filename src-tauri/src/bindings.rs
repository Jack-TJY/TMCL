//! TS 类型生成：把 IPC 用到的类型导出到 `ui/src/bindings/`（生成物，禁止手改）。
//!
//! `cargo test` 会比对生成结果与仓库中的文件，不一致即失败；
//! 运行 `pnpm gen:bindings`（设置 `TMCL_UPDATE_BINDINGS=1`）重新生成。
//! 新增命令参数、返回值或事件负载时，在 [`export_all`] 里登记根类型，依赖会被一并导出。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use tmcl_auth::AccountList;
use tmcl_common::{AppError, TaskFinished, TaskProgress};
use ts_rs::{Config, ExportError, TS};

use crate::commands::app::{AppDemoTaskArgs, AppInfo, ReleaseList};
use crate::commands::appearance::{AppearanceImageArgs, AppearanceImportArgs, AppearanceImported};
use crate::commands::auth::{
    AuthAddAuthlibArgs, AuthAddOfflineArgs, AuthAuthlibServerArgs, AuthIdArgs, AuthMsOpenArgs,
};
use crate::commands::game::{GameInstallArgs, GameVersionList, GameVersionListArgs};
use crate::commands::instance::{
    InstanceCreateArgs, InstanceDuplicateArgs, InstanceExportArgs, InstanceGameEntries,
    InstanceIdArgs, InstanceImportArgs, InstanceList, InstanceRenameArgs, InstanceSaveArgs,
};
use crate::commands::launch::{
    LaunchInstanceArgs, LaunchKillResult, LaunchRunning, LaunchStartArgs,
};
use crate::commands::layout::{LayoutSaveArgs, LayoutSnapshot};
use crate::commands::loader::{LoaderInstallArgs, LoaderListArgs, LoaderVersionList};
use crate::commands::settings::{
    SettingsLanguage, SettingsSetArgs, SettingsSetLanguageArgs, SettingsSnapshot,
};
use crate::commands::task::{TaskCancelArgs, TaskCancelResult, TaskList};
use crate::events::{DeviceCode, GameExited, GameLog, TaskStarted};

/// 置为 `1` 时测试改为写入生成物。
const UPDATE_ENV: &str = "TMCL_UPDATE_BINDINGS";

/// 仓库中的生成目录。
fn bindings_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/src/bindings")
}

/// 把所有根类型及其依赖导出到 `dir`。
fn export_all(dir: &Path) -> Result<(), ExportError> {
    let cfg = Config::new().with_out_dir(dir);
    // 公共类型
    AppError::export_all(&cfg)?;
    TaskFinished::export_all(&cfg)?;
    TaskProgress::export_all(&cfg)?;
    TaskStarted::export_all(&cfg)?;
    DeviceCode::export_all(&cfg)?;
    GameExited::export_all(&cfg)?;
    GameLog::export_all(&cfg)?;
    // app
    AppDemoTaskArgs::export_all(&cfg)?;
    AppInfo::export_all(&cfg)?;
    ReleaseList::export_all(&cfg)?;
    // appearance
    AppearanceImageArgs::export_all(&cfg)?;
    AppearanceImportArgs::export_all(&cfg)?;
    AppearanceImported::export_all(&cfg)?;
    // auth（Account、AuthlibServer 随 AccountList 一并导出）
    AccountList::export_all(&cfg)?;
    AuthAddAuthlibArgs::export_all(&cfg)?;
    AuthAddOfflineArgs::export_all(&cfg)?;
    AuthAuthlibServerArgs::export_all(&cfg)?;
    AuthIdArgs::export_all(&cfg)?;
    AuthMsOpenArgs::export_all(&cfg)?;
    // game
    GameInstallArgs::export_all(&cfg)?;
    GameVersionList::export_all(&cfg)?;
    GameVersionListArgs::export_all(&cfg)?;
    // instance
    InstanceCreateArgs::export_all(&cfg)?;
    InstanceDuplicateArgs::export_all(&cfg)?;
    InstanceExportArgs::export_all(&cfg)?;
    InstanceGameEntries::export_all(&cfg)?;
    InstanceIdArgs::export_all(&cfg)?;
    InstanceImportArgs::export_all(&cfg)?;
    InstanceList::export_all(&cfg)?;
    InstanceRenameArgs::export_all(&cfg)?;
    InstanceSaveArgs::export_all(&cfg)?;
    // launch
    LaunchInstanceArgs::export_all(&cfg)?;
    LaunchKillResult::export_all(&cfg)?;
    LaunchRunning::export_all(&cfg)?;
    LaunchStartArgs::export_all(&cfg)?;
    // layout
    LayoutSaveArgs::export_all(&cfg)?;
    LayoutSnapshot::export_all(&cfg)?;
    // loader
    LoaderInstallArgs::export_all(&cfg)?;
    LoaderListArgs::export_all(&cfg)?;
    LoaderVersionList::export_all(&cfg)?;
    // settings
    SettingsLanguage::export_all(&cfg)?;
    SettingsSetArgs::export_all(&cfg)?;
    SettingsSetLanguageArgs::export_all(&cfg)?;
    SettingsSnapshot::export_all(&cfg)?;
    // task
    TaskCancelArgs::export_all(&cfg)?;
    TaskCancelResult::export_all(&cfg)?;
    TaskList::export_all(&cfg)?;
    Ok(())
}

/// 读取目录下所有 `.ts` 文件；统一换行符，避免 Windows 检出时的 CRLF 造成误报。
fn read_ts_files(dir: &Path) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return files;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".ts") {
            let text = fs::read_to_string(entry.path()).unwrap_or_default();
            files.insert(name, text.replace("\r\n", "\n"));
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_up_to_date() {
        let tmp = tempfile::tempdir().unwrap();
        export_all(tmp.path()).unwrap();
        let generated = read_ts_files(tmp.path());
        let dir = bindings_dir();

        if std::env::var(UPDATE_ENV).as_deref() == Ok("1") {
            for name in read_ts_files(&dir).keys() {
                fs::remove_file(dir.join(name)).unwrap();
            }
            fs::create_dir_all(&dir).unwrap();
            for (name, text) in &generated {
                fs::write(dir.join(name), text).unwrap();
            }
            return;
        }

        let existing = read_ts_files(&dir);
        let stale: Vec<&String> = generated
            .iter()
            .filter(|(name, text)| existing.get(*name) != Some(*text))
            .map(|(name, _)| name)
            .chain(
                existing
                    .keys()
                    .filter(|name| !generated.contains_key(*name)),
            )
            .collect();
        assert!(
            stale.is_empty(),
            "ui/src/bindings is out of date ({stale:?}); run `pnpm gen:bindings`"
        );
    }
}
