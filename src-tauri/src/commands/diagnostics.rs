//! `diagnostics_*` 命令：导出诊断信息包（PLAN.md M2-16）。
//!
//! 包内容：系统信息、`config.json`、账号概况、实例列表与配置、检测到的 Java、
//! 最近 7 天的启动器日志。写入前全部经过 [`Scrubber`]：令牌、玩家名、玩家 UUID、
//! 账号 ID、用户主目录、路径中的用户名与邮箱都会被替换。

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::State;
use tmcl_common::diagnostics::{Scrubber, read_logs, write_bundle};
use tmcl_common::{AppError, AppResult, DataPaths, ErrorCode};
use tmcl_instance::InstanceStore;
use ts_rs::TS;

use crate::state::AppState;

/// `diagnostics_export` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct DiagnosticsExportArgs {
    /// 目标 `.zip` 文件的绝对路径，已存在时覆盖。
    pub dest: String,
}

/// `diagnostics_export` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct DiagnosticsExported {
    /// 包内的文件名，按写入顺序。
    pub files: Vec<String>,
}

/// 收集诊断信息，脱敏后写成 zip。
///
/// `dest` 不是绝对路径时返回 `COMMON_INTERNAL`；写文件失败返回 `COMMON_IO`。
/// 账号、实例读取失败不会中断导出，错误码写进对应文件。
#[tauri::command]
pub async fn diagnostics_export(
    state: State<'_, AppState>,
    args: DiagnosticsExportArgs,
) -> Result<DiagnosticsExported, AppError> {
    export(&state, &args.dest).await
}

async fn export(state: &AppState, dest: &str) -> AppResult<DiagnosticsExported> {
    let dest = Path::new(dest);
    if !dest.is_absolute() {
        return Err(AppError::new(ErrorCode::CommonInternal)
            .with_param("dest", dest.display())
            .with_detail("diagnostics_export: dest must be an absolute path"));
    }
    let paths = state.data()?.clone();
    let mut scrubber = Scrubber::new();
    if let Some(home) = std::env::home_dir() {
        scrubber = scrubber.with_home(&home);
    }
    let accounts = accounts_json(state, &mut scrubber);
    let instances = instances_json(state);
    let javas = tmcl_java::detect(&tmcl_java::Sources::system(&paths.java_dir())).await;

    let mut files = vec![
        (
            "system.json".to_owned(),
            pretty(&system_json(state, &paths)),
        ),
        ("accounts.json".to_owned(), pretty(&accounts)),
        ("instances.json".to_owned(), pretty(&instances)),
        ("java.json".to_owned(), pretty(&json!(javas))),
    ];
    let config = std::fs::read_to_string(paths.config_file())
        .unwrap_or_else(|e| format!("<{}: {e}>", ErrorCode::CommonIo.as_str()));
    files.push(("config.json".to_owned(), config));

    let dest = dest.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        for (name, text) in read_logs(&paths.logs_dir())? {
            files.push((format!("logs/{name}"), text));
        }
        write_bundle(&dest, &files, &scrubber)?;
        Ok(DiagnosticsExported {
            files: files.into_iter().map(|(name, _)| name).collect(),
        })
    })
    .await
    .map_err(|e| AppError::new(ErrorCode::CommonInternal).with_detail(e.to_string()))?
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

fn error_json(err: &AppError) -> Value {
    json!({ "error": err.code.as_str() })
}

fn system_json(state: &AppState, paths: &DataPaths) -> Value {
    json!({
        "tmcl_version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "os_version": os_version(),
        "arch": std::env::consts::ARCH,
        "cpu_threads": std::thread::available_parallelism().map_or(0, |n| n.get()),
        "webview": tauri::webview_version().ok(),
        "ms_login_enabled": tmcl_auth::ms_login_enabled(),
        "curseforge_enabled": state.curseforge.is_some(),
        "data_dir": paths.root().display().to_string(),
    })
}

/// 账号只导出类型与皮肤站；玩家名、UUID 与账号 ID 登记到脱敏器，在日志等文件中替换为标签。
fn accounts_json(state: &AppState, scrubber: &mut Scrubber) -> Value {
    let list = match state.with_db(|_, db| tmcl_auth::accounts::list(db)) {
        Ok(list) => list,
        Err(err) => return error_json(&err),
    };
    let accounts: Vec<Value> = list
        .accounts
        .iter()
        .enumerate()
        .map(|(i, account)| {
            let n = i + 1;
            scrubber.secret(&account.name, &format!("<player-{n}>"));
            scrubber.secret(&account.uuid, &format!("<uuid-{n}>"));
            scrubber.secret(&account.id, &format!("<account-{n}>"));
            json!({
                "label": format!("<account-{n}>"),
                "kind": account.kind,
                "server": account.server.as_ref().map(|s| &s.api_url),
                "default": list.default_id.as_deref() == Some(account.id.as_str()),
            })
        })
        .collect();
    json!({ "accounts": accounts })
}

fn instances_json(state: &AppState) -> Value {
    let result = state.with_db(|paths, db| {
        let store = InstanceStore::new(paths, db);
        Ok(store
            .list()?
            .into_iter()
            .map(|summary| {
                let config = match store.get(&summary.id) {
                    Ok(config) => json!(config),
                    Err(err) => error_json(&err),
                };
                json!({ "summary": summary, "config": config })
            })
            .collect::<Vec<_>>())
    });
    match result {
        Ok(instances) => json!({ "instances": instances }),
        Err(err) => error_json(&err),
    }
}

/// 操作系统版本，取不到时为 `null`。
#[cfg(windows)]
fn os_version() -> Option<String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;
    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
        .ok()?;
    let product: String = key.get_value("ProductName").unwrap_or_default();
    let display: String = key.get_value("DisplayVersion").unwrap_or_default();
    let build: String = key.get_value("CurrentBuild").unwrap_or_default();
    let ubr: u32 = key.get_value("UBR").unwrap_or_default();
    // Windows 11 的 ProductName 仍写作 “Windows 10”，以内部版本号为准（≥ 22000 为 11）。
    Some(format!("{product} {display} (build {build}.{ubr})"))
}

/// 操作系统版本，取不到时为 `null`。
#[cfg(target_os = "linux")]
fn os_version() -> Option<String> {
    let text = std::fs::read_to_string("/etc/os-release").ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME="))
        .map(|v| v.trim_matches('"').to_owned())
}

/// 操作系统版本，取不到时为 `null`。
#[cfg(not(any(windows, target_os = "linux")))]
fn os_version() -> Option<String> {
    let out = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&out.stdout).trim().to_owned()).filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use tmcl_auth::{AccountKind, MemorySecretStore, NewAccount};
    use tmcl_common::DataPaths;

    use super::*;

    fn read_zip(path: &Path) -> Vec<(String, String)> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        (0..zip.len())
            .map(|i| {
                let mut file = zip.by_index(i).unwrap();
                let mut text = String::new();
                file.read_to_string(&mut text).unwrap();
                (file.name().unwrap().into_owned(), text)
            })
            .collect()
    }

    #[test]
    fn rejects_relative_dest() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::with_data(Ok(DataPaths::at(dir.path())));
        let err = tauri::async_runtime::block_on(export(&state, "diag.zip")).unwrap_err();
        assert_eq!(err.code, ErrorCode::CommonInternal);
    }

    #[test]
    fn bundle_hides_account_details() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path().join("data"));
        paths.ensure_dirs().unwrap();
        let state = AppState::with_data(Ok(paths.clone()));
        let uuid = uuid::Uuid::parse_str("069a79f4-44e9-4726-a5be-fca90e38aaf5").unwrap();
        let account = state
            .with_db(|_, db| {
                tmcl_auth::accounts::upsert(
                    db,
                    &MemorySecretStore::default(),
                    NewAccount {
                        kind: AccountKind::Microsoft,
                        name: "Jack_TJY".into(),
                        uuid,
                        server: None,
                    },
                    None,
                )
            })
            .unwrap();
        state
            .with_db(|paths, db| {
                let store = InstanceStore::new(paths, db);
                store.create("Survival", "1.20.1", None).map(drop)
            })
            .unwrap();
        std::fs::write(
            paths.logs_dir().join("tmcl.2026-10-30.log"),
            format!(
                "launch --username Jack_TJY --uuid {} --accessToken eyJ0 id={}\n",
                uuid.simple(),
                account.id
            ),
        )
        .unwrap();

        let dest = dir.path().join("diag.zip");
        let exported =
            tauri::async_runtime::block_on(export(&state, dest.to_str().unwrap())).unwrap();
        assert!(
            exported
                .files
                .contains(&"logs/tmcl.2026-10-30.log".to_owned())
        );

        let files = read_zip(&dest);
        let all: String = files.iter().map(|(_, text)| text.as_str()).collect();
        assert!(!all.contains("Jack_TJY"), "{all}");
        assert!(!all.contains("069a79f4"), "{all}");
        assert!(!all.contains(&account.id), "{all}");
        assert!(!all.contains("eyJ0"), "{all}");
        let log = &files
            .iter()
            .find(|(name, _)| name == "logs/tmcl.2026-10-30.log")
            .unwrap()
            .1;
        assert_eq!(
            log,
            "launch --username <player-1> --uuid <uuid-1> --accessToken *** id=<account-1>\n"
        );
        let instances = &files.iter().find(|(n, _)| n == "instances.json").unwrap().1;
        assert!(instances.contains("Survival"), "{instances}");
    }
}
