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
use tmcl_resources::{InstallPlan, InstalledList, ModpackInfo, ResourceImportResult, ResourcePage};
use tmcl_sakura::{SakuraPolicy, SakuraUser, StartRequest};
use tmcl_sync::AutoSync;
use ts_rs::{Config, ExportError, TS};

use crate::commands::app::{AppDemoTaskArgs, AppInfo, ReleaseList};
use crate::commands::appearance::{AppearanceImageArgs, AppearanceImportArgs, AppearanceImported};
use crate::commands::auth::{
    AuthAddAuthlibArgs, AuthAddOfflineArgs, AuthAuthlibServerArgs, AuthIdArgs, AuthMsOpenArgs,
};
use crate::commands::diagnostics::{DiagnosticsExportArgs, DiagnosticsExported};
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
use crate::commands::p2p::{LanWorldList, P2pHostArgs, P2pInfo, P2pJoinArgs, P2pStatus};
use crate::commands::resources::{
    DetectedPack, ResourceSources, ResourceUpdateList, ResourceVersionList, ResourceWorldList,
    ResourcesDirArgs, ResourcesImportArgs, ResourcesInstallArgs, ResourcesModpackExportArgs,
    ResourcesModpackInspectArgs, ResourcesModpackInstallArgs, ResourcesModpackSearchArgs,
    ResourcesModpackVersionsArgs, ResourcesOpenPageArgs, ResourcesPackDetectArgs,
    ResourcesRemoveArgs, ResourcesSearchAnyArgs, ResourcesSearchArgs, ResourcesSetEnabledArgs,
    ResourcesSetEnabledResult, ResourcesUpdateArgs, ResourcesVersionsArgs, ResourcesWorldsArgs,
};
use crate::commands::sakura::{
    SakuraKeyArgs, SakuraNodeList, SakuraPing, SakuraPingArgs, SakuraStatus, SakuraTunnelArgs,
    SakuraTunnelList,
};
use crate::commands::settings::{
    SettingsLanguage, SettingsSetArgs, SettingsSetLanguageArgs, SettingsSnapshot,
};
use crate::commands::skin::{
    SkinAccountArgs, SkinAddTextureArgs, SkinFileArgs, SkinIdArgs, SkinImportArgs, SkinLibraryList,
    SkinSetCapeArgs, SkinSite, SkinTextureArgs, SkinUpdateArgs, SkinUploadArgs,
};
use crate::commands::sync::{
    SyncApplyResult, SyncCaptureArgs, SyncMaster, SyncPreview, SyncRestoreArgs, SyncRestoreResult,
    SyncSaveMasterArgs, SyncTargetsArgs,
};
use crate::commands::task::{TaskCancelArgs, TaskCancelResult, TaskList};
use crate::events::{DeviceCode, GameExited, GameLog, TaskStarted};
use tmcl_skin::SkinProfile;

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
    // diagnostics
    DiagnosticsExportArgs::export_all(&cfg)?;
    DiagnosticsExported::export_all(&cfg)?;
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
    // resources（ResourceKind、ResourceSort、ResourceVersion 等随之导出）
    InstallPlan::export_all(&cfg)?;
    InstalledList::export_all(&cfg)?;
    ResourceImportResult::export_all(&cfg)?;
    ModpackInfo::export_all(&cfg)?;
    ResourcePage::export_all(&cfg)?;
    ResourceSources::export_all(&cfg)?;
    ResourceUpdateList::export_all(&cfg)?;
    ResourceVersionList::export_all(&cfg)?;
    ResourceWorldList::export_all(&cfg)?;
    ResourcesDirArgs::export_all(&cfg)?;
    ResourcesImportArgs::export_all(&cfg)?;
    ResourcesInstallArgs::export_all(&cfg)?;
    ResourcesModpackInspectArgs::export_all(&cfg)?;
    ResourcesPackDetectArgs::export_all(&cfg)?;
    DetectedPack::export_all(&cfg)?;
    ResourcesModpackExportArgs::export_all(&cfg)?;
    ResourcesModpackInstallArgs::export_all(&cfg)?;
    ResourcesModpackSearchArgs::export_all(&cfg)?;
    ResourcesModpackVersionsArgs::export_all(&cfg)?;
    ResourcesOpenPageArgs::export_all(&cfg)?;
    ResourcesRemoveArgs::export_all(&cfg)?;
    ResourcesSearchArgs::export_all(&cfg)?;
    ResourcesSearchAnyArgs::export_all(&cfg)?;
    ResourcesSetEnabledArgs::export_all(&cfg)?;
    ResourcesSetEnabledResult::export_all(&cfg)?;
    ResourcesUpdateArgs::export_all(&cfg)?;
    ResourcesVersionsArgs::export_all(&cfg)?;
    ResourcesWorldsArgs::export_all(&cfg)?;
    // p2p（LanWorld 随 LanWorldList，P2pRoom、P2pPlayer、P2pRole 随 P2pStatus 一并导出）
    LanWorldList::export_all(&cfg)?;
    P2pHostArgs::export_all(&cfg)?;
    P2pInfo::export_all(&cfg)?;
    P2pJoinArgs::export_all(&cfg)?;
    P2pStatus::export_all(&cfg)?;
    // sakura（SakuraNodeInfo、SakuraNode 随 SakuraNodeList，SakuraRoom 随 SakuraStatus 一并导出）
    SakuraKeyArgs::export_all(&cfg)?;
    SakuraNodeList::export_all(&cfg)?;
    SakuraPing::export_all(&cfg)?;
    SakuraPingArgs::export_all(&cfg)?;
    SakuraPolicy::export_all(&cfg)?;
    SakuraStatus::export_all(&cfg)?;
    SakuraTunnelArgs::export_all(&cfg)?;
    SakuraTunnelList::export_all(&cfg)?;
    SakuraUser::export_all(&cfg)?;
    StartRequest::export_all(&cfg)?;
    // settings
    SettingsLanguage::export_all(&cfg)?;
    SettingsSetArgs::export_all(&cfg)?;
    SettingsSetLanguageArgs::export_all(&cfg)?;
    SettingsSnapshot::export_all(&cfg)?;
    // skin（SkinEntry、SkinVariant 随 SkinLibraryList，ProfileSkin、ProfileCape 随 SkinProfile 一并导出）
    SkinAccountArgs::export_all(&cfg)?;
    SkinAddTextureArgs::export_all(&cfg)?;
    SkinFileArgs::export_all(&cfg)?;
    SkinIdArgs::export_all(&cfg)?;
    SkinImportArgs::export_all(&cfg)?;
    SkinLibraryList::export_all(&cfg)?;
    SkinProfile::export_all(&cfg)?;
    SkinSetCapeArgs::export_all(&cfg)?;
    SkinSite::export_all(&cfg)?;
    SkinTextureArgs::export_all(&cfg)?;
    SkinUpdateArgs::export_all(&cfg)?;
    SkinUploadArgs::export_all(&cfg)?;
    // sync（SyncProfile、SyncOption、SyncOptionChange、SyncCategory 随之导出）
    SyncApplyResult::export_all(&cfg)?;
    SyncCaptureArgs::export_all(&cfg)?;
    SyncMaster::export_all(&cfg)?;
    SyncPreview::export_all(&cfg)?;
    SyncRestoreArgs::export_all(&cfg)?;
    SyncRestoreResult::export_all(&cfg)?;
    SyncSaveMasterArgs::export_all(&cfg)?;
    SyncTargetsArgs::export_all(&cfg)?;
    AutoSync::export_all(&cfg)?;
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
