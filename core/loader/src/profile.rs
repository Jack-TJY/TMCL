//! 各加载器共用的安装步骤：补全原版、整理加载器版本 JSON、下载其依赖库、最后写入版本 JSON。
//!
//! Fabric 与 Quilt 的元数据直接给出完整的继承版本 JSON（profile），由 [`install_profile`] 一步完成。

use serde_json::Value;
use tmcl_common::{AppError, AppResult, ErrorCode, ProgressUpdate, TaskCtx, write_atomic};
use tmcl_game::{
    GameFiles, InstallReport, RuleEnv, VersionJson, install_version, library_files, parse_version,
    resolve,
};
use tmcl_net::{DownloadReport, is_valid};

use crate::{InstalledLoader, LoaderInstaller, LoaderKind, display_name, version_id};

/// 下载加载器 profile 并安装：先补全原版，再下载 profile 中的库，最后写入版本 JSON。
///
/// profile 不存在（HTTP 400/404）时返回 `LOADER_VERSION_NOT_FOUND`。
pub(crate) async fn install_profile(
    installer: &LoaderInstaller,
    kind: LoaderKind,
    game_version: &str,
    loader_version: &str,
    url: &str,
    ctx: &TaskCtx,
) -> AppResult<InstalledLoader> {
    let id = version_id(kind, game_version, loader_version);
    let mut report = install_game(installer, game_version, ctx).await?;

    ctx.report(ProgressUpdate::new("loader_profile").items(0, 1));
    let bytes = installer
        .downloader()
        .get_bytes(url, ctx)
        .await
        .map_err(|e| not_found_on_missing(e, kind, game_version, loader_version))?;
    let (value, version) = prepare_json(kind, game_version, loader_version, &id, &bytes)?;
    ctx.report(ProgressUpdate::new("loader_profile").items(1, 1));

    add(
        &mut report,
        download_libraries(installer, &version, "loader_libraries", ctx).await?,
    );
    check_local_files(installer, &version).await?;
    write_version(installer, &id, &value)?;
    Ok(InstalledLoader {
        version_id: id,
        report,
    })
}

/// 补全加载器所继承的原版（版本 JSON、客户端 jar、库与资源）。
pub(crate) async fn install_game(
    installer: &LoaderInstaller,
    game_version: &str,
    ctx: &TaskCtx,
) -> AppResult<InstallReport> {
    install_version(
        installer.downloader(),
        installer.paths(),
        game_version,
        &installer.game_options,
        ctx,
    )
    .await
}

/// 把元数据服务器的 400/404 映射为 `LOADER_VERSION_NOT_FOUND`，其他错误原样返回。
pub(crate) fn not_found_on_missing(
    err: AppError,
    kind: LoaderKind,
    game_version: &str,
    loader_version: &str,
) -> AppError {
    let missing = err.code == ErrorCode::NetHttpStatus
        && matches!(
            err.params.get("status").map(String::as_str),
            Some("400" | "404")
        );
    if missing {
        AppError::new(ErrorCode::LoaderVersionNotFound)
            .with_param("kind", display_name(kind))
            .with_param("game", game_version)
            .with_param("version", loader_version)
            .with_detail(err.to_string())
    } else {
        err
    }
}

/// `LOADER_INSTALLER_INVALID`。
pub(crate) fn invalid(kind: LoaderKind, loader_version: &str, detail: impl AsRef<str>) -> AppError {
    AppError::new(ErrorCode::LoaderInstallerInvalid)
        .with_param("kind", display_name(kind))
        .with_param("version", loader_version)
        .with_detail(detail)
}

/// 整理加载器给出的版本 JSON：`id` 改为 `id`，`inheritsFrom` 必须为（或补为）`game_version`。
///
/// 返回要写入磁盘的 JSON 与解析结果；格式不对时返回 `LOADER_INSTALLER_INVALID`。
pub(crate) fn prepare_json(
    kind: LoaderKind,
    game_version: &str,
    loader_version: &str,
    id: &str,
    bytes: &[u8],
) -> AppResult<(Value, VersionJson)> {
    let mut value: Value =
        serde_json::from_slice(bytes).map_err(|e| invalid(kind, loader_version, e.to_string()))?;
    let Some(object) = value.as_object_mut() else {
        return Err(invalid(
            kind,
            loader_version,
            "version JSON is not an object",
        ));
    };
    match object.get("inheritsFrom").and_then(Value::as_str) {
        Some(parent) if parent != game_version => {
            return Err(invalid(
                kind,
                loader_version,
                format!("inheritsFrom {parent}, expected {game_version}"),
            ));
        }
        Some(_) => {}
        None => {
            object.insert("inheritsFrom".to_owned(), game_version.into());
        }
    }
    object.insert("id".to_owned(), id.into());
    let bytes = serde_json::to_vec(&value)?;
    let version =
        parse_version(id, &bytes).map_err(|e| invalid(kind, loader_version, e.to_string()))?;
    Ok((value, version))
}

/// 与本地父版本合并后的完整版本。
fn resolved(installer: &LoaderInstaller, version: &VersionJson) -> AppResult<VersionJson> {
    let files = GameFiles::new(installer.paths().clone());
    resolve(&version.id, |id| {
        if id == version.id {
            Ok(version.clone())
        } else {
            files.load_version(id)
        }
    })
}

/// 下载 `version`（尚未写入磁盘的加载器版本）合并父版本后、有下载地址的全部文件。
pub(crate) async fn download_libraries(
    installer: &LoaderInstaller,
    version: &VersionJson,
    stage: &str,
    ctx: &TaskCtx,
) -> AppResult<DownloadReport> {
    let files = GameFiles::new(installer.paths().clone());
    let full = resolved(installer, version)?;
    let items = library_files(&files, &full, &RuleEnv::current())
        .iter()
        .filter_map(|f| f.download_item())
        .collect();
    installer.downloader().download_all(items, stage, ctx).await
}

/// 没有下载地址（应由安装器生成）的文件必须已在本地且校验通过，否则返回 `GAME_FILE_MISSING`。
pub(crate) async fn check_local_files(
    installer: &LoaderInstaller,
    version: &VersionJson,
) -> AppResult<()> {
    let files = GameFiles::new(installer.paths().clone());
    let full = resolved(installer, version)?;
    for file in library_files(&files, &full, &RuleEnv::current()) {
        if file.url.is_none() && !is_valid(&file.path, file.size, file.sha1.as_deref()).await? {
            return Err(AppError::new(ErrorCode::GameFileMissing)
                .with_param("version", &version.id)
                .with_param("path", file.path.display()));
        }
    }
    Ok(())
}

/// 原子写入 `shared/versions/<id>/<id>.json`。安装的最后一步。
pub(crate) fn write_version(installer: &LoaderInstaller, id: &str, value: &Value) -> AppResult<()> {
    let files = GameFiles::new(installer.paths().clone());
    let bytes = serde_json::to_vec_pretty(value)?;
    write_atomic(&files.version_json(id), &bytes)
}

/// 累加下载统计。
pub(crate) fn add(report: &mut InstallReport, r: DownloadReport) {
    report.downloaded += r.downloaded;
    report.skipped += r.skipped;
    report.bytes += r.bytes;
}
