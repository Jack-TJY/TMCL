//! TMCL 应用壳：只做参数转换、状态持有和事件转发，不写业务逻辑。
//!
//! 命令与事件的约定见 `docs/contracts/ipc.md`。

#[cfg(test)]
mod bindings;
mod commands;
mod events;
mod state;
mod tasks;

use tauri::Runtime;
use tauri::ipc::Invoke;

use crate::state::AppState;

/// 所有命令：按模块分组，组内按字母排序，一行一个命令（AGENTS.md §5.2）。
fn handlers<R: Runtime>() -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        // app
        commands::app::app_demo_task,
        commands::app::app_info,
        // appearance
        commands::appearance::appearance_import_background,
        commands::appearance::appearance_read_background,
        // game
        commands::game::game_install,
        commands::game::game_version_list,
        // instance
        commands::instance::instance_create,
        commands::instance::instance_delete,
        commands::instance::instance_duplicate,
        commands::instance::instance_export,
        commands::instance::instance_game_entries,
        commands::instance::instance_get,
        commands::instance::instance_import,
        commands::instance::instance_list,
        commands::instance::instance_open_dir,
        commands::instance::instance_refresh,
        commands::instance::instance_rename,
        commands::instance::instance_save,
        // launch
        commands::launch::launch_kill,
        commands::launch::launch_running,
        commands::launch::launch_start,
        // layout
        commands::layout::layout_get,
        commands::layout::layout_save,
        // loader
        commands::loader::loader_install,
        commands::loader::loader_list,
        // settings
        commands::settings::settings_get,
        commands::settings::settings_get_language,
        commands::settings::settings_open_data_dir,
        commands::settings::settings_set,
        commands::settings::settings_set_language,
        // task
        commands::task::task_cancel,
        commands::task::task_list,
    ]
}

/// 在给定的 builder 上注册状态与命令；测试中配合 mock runtime 使用。
fn setup<R: Runtime>(builder: tauri::Builder<R>, state: AppState) -> tauri::Builder<R> {
    builder.manage(state).invoke_handler(handlers())
}

/// 构建并运行 Tauri 应用。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 文件对话框：导入导出实例时由前端经 `ui/src/api/dialog.ts` 调用，只开放打开与保存两项权限。
    let builder = tauri::Builder::default().plugin(tauri_plugin_dialog::init());
    setup(builder, AppState::new())
        .run(tauri::generate_context!())
        .expect("error while running TMCL");
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use serde_json::{Value, json};
    use tauri::test::{
        INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
    };
    use tauri::webview::InvokeRequest;
    use tauri::{Listener, Manager};
    use tmcl_common::{AppError, DataPaths, ErrorCode, TaskFinished, TaskProgress, TaskState};
    use tmcl_net::testing::TestServer;

    use super::*;
    use crate::events::{TASK_FINISHED, TASK_PROGRESS};

    fn mock_app() -> (tauri::App<MockRuntime>, tauri::WebviewWindow<MockRuntime>) {
        mock_app_with(AppState::new())
    }

    fn mock_app_with(
        state: AppState,
    ) -> (tauri::App<MockRuntime>, tauri::WebviewWindow<MockRuntime>) {
        let app = setup(mock_builder(), state)
            .build(mock_context(noop_assets()))
            .unwrap();
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        (app, webview)
    }

    fn invoke(
        webview: &tauri::WebviewWindow<MockRuntime>,
        cmd: &str,
        body: Value,
    ) -> Result<Value, Value> {
        get_ipc_response(
            webview,
            InvokeRequest {
                cmd: cmd.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                // mock runtime 按平台使用不同的本地来源。
                url: if cfg!(windows) {
                    "http://tauri.localhost"
                } else {
                    "tauri://localhost"
                }
                .parse()
                .unwrap(),
                body: body.into(),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_owned(),
            },
        )
        .map(|body| body.deserialize::<Value>().unwrap())
    }

    #[test]
    fn app_info_over_ipc() {
        let (_app, webview) = mock_app();
        let info = invoke(&webview, "app_info", json!({})).unwrap();
        assert_eq!(info["os"], std::env::consts::OS);
        assert_eq!(info["ms_login_enabled"], tmcl_auth::ms_login_enabled());
    }

    #[test]
    fn invalid_args_return_structured_error() {
        let (_app, webview) = mock_app();
        let body = json!({ "args": { "steps": 0, "step_ms": 0 } });
        let err = invoke(&webview, "app_demo_task", body).unwrap_err();
        let err: AppError = serde_json::from_value(err).unwrap();
        assert_eq!(err.code, ErrorCode::CommonInternal);
    }

    #[test]
    fn demo_task_emits_progress_and_finished() {
        let (app, webview) = mock_app();
        let (progress_tx, progress_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = mpsc::channel();
        app.listen(TASK_PROGRESS, move |event| {
            let progress: TaskProgress = serde_json::from_str(event.payload()).unwrap();
            progress_tx.send(progress).unwrap();
        });
        app.listen(TASK_FINISHED, move |event| {
            let finished: TaskFinished = serde_json::from_str(event.payload()).unwrap();
            finished_tx.send(finished).unwrap();
        });

        let body = json!({ "args": { "steps": 2, "step_ms": 0, "fail_at": 2 } });
        let started = invoke(&webview, "app_demo_task", body).unwrap();
        let finished = finished_rx.recv_timeout(Duration::from_secs(10)).unwrap();

        assert_eq!(started["task_id"], json!(finished.task_id));
        assert_eq!(finished.state, TaskState::Failed);
        assert_eq!(finished.error.unwrap().params["step"], "2");
        let first = progress_rx.try_recv().unwrap();
        assert_eq!(
            (first.task_id, first.done, first.total),
            (finished.task_id, 0, 2)
        );
    }

    /// 订阅 `task:finished`，返回接收端。
    fn finished_events(app: &tauri::App<MockRuntime>) -> mpsc::Receiver<TaskFinished> {
        let (tx, rx) = mpsc::channel();
        app.listen(TASK_FINISHED, move |event| {
            let finished: TaskFinished = serde_json::from_str(event.payload()).unwrap();
            let _ = tx.send(finished);
        });
        rx
    }

    #[test]
    fn task_list_and_cancel_over_ipc() {
        let (app, webview) = mock_app();
        let finished_rx = finished_events(&app);

        let body = json!({ "args": { "steps": 100, "step_ms": 50 } });
        let started = invoke(&webview, "app_demo_task", body).unwrap();
        let task_id = started["task_id"].clone();

        let list = invoke(&webview, "task_list", json!({})).unwrap();
        assert_eq!(list["tasks"][0]["task_id"], task_id);
        assert_eq!(list["tasks"][0]["status"], "running");
        assert_eq!(list["tasks"][0]["kind"], "app_demo");

        let cancel = json!({ "args": { "task_id": task_id } });
        let result = invoke(&webview, "task_cancel", cancel.clone()).unwrap();
        assert_eq!(result["cancelled"], true);
        let finished = finished_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(finished.state, TaskState::Cancelled);

        let list = invoke(&webview, "task_list", json!({})).unwrap();
        assert_eq!(list["tasks"][0]["status"], "cancelled");
        assert!(list["tasks"][0]["finished_at"].is_number());
        let again = invoke(&webview, "task_cancel", cancel).unwrap();
        assert_eq!(again["cancelled"], false);
    }

    const MANIFEST: &str = r#"{
        "latest": {"release": "1.20.1", "snapshot": "23w31a"},
        "versions": [
            {"id": "23w31a", "type": "snapshot", "url": "http://127.0.0.1:1/23w31a.json",
             "releaseTime": "2023-08-01T00:00:00+00:00",
             "sha1": "1111111111111111111111111111111111111111"},
            {"id": "1.20.1", "type": "release", "url": "http://127.0.0.1:1/1.20.1.json",
             "releaseTime": "2023-06-12T13:25:51+00:00",
             "sha1": "38d412af3b247e485c7f7e50ac25febc4e79ab1d"}
        ]
    }"#;

    /// 本地清单服务器 + 临时数据目录。
    fn game_app() -> (
        tauri::App<MockRuntime>,
        tauri::WebviewWindow<MockRuntime>,
        TestServer,
        tempfile::TempDir,
    ) {
        let server = tauri::async_runtime::block_on(TestServer::start([(
            "/manifest.json",
            MANIFEST.as_bytes().to_vec(),
        )]));
        let dir = tempfile::tempdir().unwrap();
        let mut state = AppState::with_data(Ok(DataPaths::at(dir.path())));
        state.game.manifest_url = server.url("/manifest.json");
        let (app, webview) = mock_app_with(state);
        (app, webview, server, dir)
    }

    #[test]
    fn game_version_list_over_ipc() {
        let (_app, webview, server, _dir) = game_app();
        let list = invoke(&webview, "game_version_list", json!({})).unwrap();
        assert_eq!(list["latest_release"], "1.20.1");
        assert_eq!(list["versions"][0]["id"], "23w31a");
        assert_eq!(list["versions"][0]["kind"], "snapshot");
        assert_eq!(
            list["versions"][1]["release_time"],
            "2023-06-12T13:25:51+00:00"
        );
        // 第二次读取命中 SQLite 缓存，不再请求服务器。
        let again = invoke(&webview, "game_version_list", json!({})).unwrap();
        assert_eq!(again, list);
        assert_eq!(server.hits(), 1);
    }

    #[test]
    fn game_install_reports_failure_through_task() {
        let (app, webview, _server, _dir) = game_app();
        let finished_rx = finished_events(&app);

        let bad = invoke(
            &webview,
            "game_install",
            json!({ "args": { "version_id": "../x" } }),
        );
        let err: AppError = serde_json::from_value(bad.unwrap_err()).unwrap();
        assert_eq!(err.code, ErrorCode::CommonInternal);

        let body = json!({ "args": { "version_id": "9.9.9" } });
        let started = invoke(&webview, "game_install", body).unwrap();
        let finished = finished_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(started["task_id"], json!(finished.task_id));
        assert_eq!(finished.state, TaskState::Failed);
        assert_eq!(finished.error.unwrap().code, ErrorCode::GameVersionNotFound);
    }

    /// 调用一个返回 `TaskStarted` 的命令并等它结束。
    fn run_task(
        webview: &tauri::WebviewWindow<MockRuntime>,
        finished_rx: &mpsc::Receiver<TaskFinished>,
        cmd: &str,
        body: Value,
    ) -> TaskFinished {
        let started = invoke(webview, cmd, body).unwrap();
        let finished = finished_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(started["task_id"], json!(finished.task_id));
        finished
    }

    fn instance_ids(webview: &tauri::WebviewWindow<MockRuntime>) -> Vec<String> {
        let list = invoke(webview, "instance_list", json!({})).unwrap();
        list["instances"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["id"].as_str().unwrap().to_owned())
            .collect()
    }

    #[test]
    fn instance_crud_over_ipc() {
        let dir = tempfile::tempdir().unwrap();
        let (_app, webview) = mock_app_with(AppState::with_data(Ok(DataPaths::at(dir.path()))));
        assert!(instance_ids(&webview).is_empty());

        let body = json!({ "args": { "name": " 生存 ", "game_version": "1.20.1",
            "loader": { "kind": "fabric", "version": "0.16.5" } } });
        let created = invoke(&webview, "instance_create", body).unwrap();
        let id = created["id"].clone();
        assert_eq!(created["name"], "生存");
        assert_eq!(instance_ids(&webview), [id.as_str().unwrap()]);

        let renamed = invoke(
            &webview,
            "instance_rename",
            json!({ "args": { "id": id, "name": "新名字" } }),
        )
        .unwrap();
        assert_eq!(renamed["name"], "新名字");

        let mut config = invoke(&webview, "instance_get", json!({ "args": { "id": id } })).unwrap();
        config["memory"]["max_mb"] = json!(8192);
        let saved = invoke(
            &webview,
            "instance_save",
            json!({ "args": { "config": config } }),
        )
        .unwrap();
        assert_eq!(saved["memory"]["max_mb"], 8192);

        let bad = invoke(
            &webview,
            "instance_rename",
            json!({ "args": { "id": id, "name": "" } }),
        );
        let err: AppError = serde_json::from_value(bad.unwrap_err()).unwrap();
        assert_eq!(err.code, ErrorCode::InstanceInvalidName);

        invoke(&webview, "instance_delete", json!({ "args": { "id": id } })).unwrap();
        assert!(instance_ids(&webview).is_empty());
        let missing = invoke(&webview, "instance_get", json!({ "args": { "id": id } }));
        let err: AppError = serde_json::from_value(missing.unwrap_err()).unwrap();
        assert_eq!(err.code, ErrorCode::InstanceNotFound);
    }

    #[test]
    fn instance_tasks_over_ipc() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path().join("tmcl-data"));
        let (app, webview) = mock_app_with(AppState::with_data(Ok(paths.clone())));
        let finished_rx = finished_events(&app);

        let body = json!({ "args": { "name": "源", "game_version": "1.20.1" } });
        let id = invoke(&webview, "instance_create", body).unwrap()["id"].clone();
        let game = paths.instance_game_dir(id.as_str().unwrap());
        std::fs::create_dir_all(game.join("saves/world")).unwrap();
        std::fs::create_dir_all(game.join("mods")).unwrap();
        std::fs::write(game.join("mods/a.jar"), b"jar").unwrap();
        std::fs::write(game.join("saves/world/level.dat"), b"dat").unwrap();

        let entries = invoke(
            &webview,
            "instance_game_entries",
            json!({ "args": { "id": id } }),
        )
        .unwrap();
        assert_eq!(entries["entries"], json!(["mods", "saves"]));

        let body = json!({ "args": { "id": id, "name": "副本", "include_saves": false } });
        let finished = run_task(&webview, &finished_rx, "instance_duplicate", body);
        assert_eq!(finished.state, TaskState::Succeeded);
        assert_eq!(instance_ids(&webview).len(), 2);

        let pack = dir.path().join("out.tmclpack");
        let body = json!({ "args": { "id": id, "dest": pack, "include": ["mods"] } });
        let finished = run_task(&webview, &finished_rx, "instance_export", body);
        assert_eq!(finished.state, TaskState::Succeeded);
        assert!(pack.is_file());

        let body = json!({ "args": { "pack": pack } });
        let finished = run_task(&webview, &finished_rx, "instance_import", body);
        assert_eq!(finished.state, TaskState::Succeeded);
        assert_eq!(instance_ids(&webview).len(), 3);

        let not_pack = dir.path().join("bad.tmclpack");
        std::fs::write(&not_pack, b"not a zip").unwrap();
        let body = json!({ "args": { "pack": not_pack } });
        let finished = run_task(&webview, &finished_rx, "instance_import", body);
        assert_eq!(finished.error.unwrap().code, ErrorCode::InstancePackInvalid);

        let relative = invoke(
            &webview,
            "instance_import",
            json!({ "args": { "pack": "x.tmclpack" } }),
        );
        assert!(relative.is_err());
    }

    #[test]
    fn instance_refresh_marks_corrupted() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path());
        let (_app, webview) = mock_app_with(AppState::with_data(Ok(paths.clone())));
        let body = json!({ "args": { "name": "a", "game_version": "1.20.1" } });
        let id = invoke(&webview, "instance_create", body).unwrap()["id"].clone();

        std::fs::write(paths.instance_file(id.as_str().unwrap()), b"{ broken").unwrap();
        let list = invoke(&webview, "instance_refresh", json!({})).unwrap();
        assert_eq!(list["instances"][0]["id"], id);
        assert_eq!(list["instances"][0]["error"], "INSTANCE_CORRUPTED");
    }

    #[test]
    fn instance_open_dir_rejects_unknown_instance() {
        let dir = tempfile::tempdir().unwrap();
        let (_app, webview) = mock_app_with(AppState::with_data(Ok(DataPaths::at(dir.path()))));
        let err = invoke(
            &webview,
            "instance_open_dir",
            json!({ "args": { "id": "missing" } }),
        )
        .unwrap_err();
        assert_eq!(err["code"], "INSTANCE_NOT_FOUND");
    }

    #[test]
    fn layout_get_and_save_over_ipc() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path());
        let (_app, webview) = mock_app_with(AppState::with_data(Ok(paths.clone())));

        let empty = invoke(&webview, "layout_get", json!({})).unwrap();
        assert_eq!(empty["widgets"], Value::Null);

        let widgets = json!([{ "uid": "a", "type": "start-game", "x": 0, "y": 0, "w": 6, "h": 4,
            "settings": { "instance": null } }]);
        invoke(
            &webview,
            "layout_save",
            json!({ "args": { "widgets": widgets } }),
        )
        .unwrap();
        assert!(paths.ui_layout_file().is_file());
        let saved = invoke(&webview, "layout_get", json!({})).unwrap();
        assert_eq!(saved["widgets"], widgets);
        assert_eq!(saved["skipped"], 0);

        let bad = json!([{ "uid": "a", "type": "x", "x": 11, "y": 0, "w": 2, "h": 1 }]);
        let err = invoke(
            &webview,
            "layout_save",
            json!({ "args": { "widgets": bad } }),
        );
        assert_eq!(err.unwrap_err()["code"], "COMMON_INTERNAL");
    }

    #[test]
    fn appearance_import_and_missing_background_over_ipc() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path().join("data"));
        let (_app, webview) = mock_app_with(AppState::with_data(Ok(paths.clone())));
        let source = dir.path().join("photo.png");
        std::fs::write(&source, b"\x89PNG\r\n\x1a\n").unwrap();

        let body = json!({ "args": { "path": source.display().to_string() } });
        let imported = invoke(&webview, "appearance_import_background", body).unwrap();
        let image = imported["image"].as_str().unwrap();
        assert!(paths.ui_backgrounds_dir().join(image).is_file());

        let body = json!({ "args": { "image": "bg-0.png" } });
        let err = invoke(&webview, "appearance_read_background", body).unwrap_err();
        assert_eq!(err["code"], "APPEARANCE_IMAGE_MISSING");
    }

    #[test]
    fn settings_get_and_set_over_ipc() {
        let dir = tempfile::tempdir().unwrap();
        let (app, webview) = mock_app_with(AppState::with_data(Ok(DataPaths::at(dir.path()))));

        let snapshot = invoke(&webview, "settings_get", json!({})).unwrap();
        assert_eq!(snapshot["data_dir"], dir.path().display().to_string());
        assert_eq!(snapshot["config_backup"], Value::Null);
        let mut settings = snapshot["settings"].clone();
        assert_eq!(settings["theme"], "system");
        assert_eq!(settings["appearance"]["window_material"], true);

        settings["theme"] = json!("dark");
        settings["appearance"]["accent"] = json!("#0078D4");
        settings["download"]["max_concurrency"] = json!(4);
        let saved = invoke(
            &webview,
            "settings_set",
            json!({ "args": { "settings": settings } }),
        )
        .unwrap();
        assert_eq!(saved, settings);
        let state = app.state::<AppState>();
        assert_eq!(state.downloader().unwrap().options().concurrency, 4);

        let again = invoke(&webview, "settings_get", json!({})).unwrap();
        assert_eq!(again["settings"], settings);

        settings["memory"]["max_mb"] = json!(1);
        let bad = invoke(
            &webview,
            "settings_set",
            json!({ "args": { "settings": settings } }),
        );
        let err: AppError = serde_json::from_value(bad.unwrap_err()).unwrap();
        assert_eq!(err.code, ErrorCode::CommonInternal);
        assert_eq!(err.params["max_mb"], "false");
    }

    #[test]
    fn launch_rejects_unknown_instance_and_bad_name() {
        let dir = tempfile::tempdir().unwrap();
        let (_app, webview) = mock_app_with(AppState::with_data(Ok(DataPaths::at(dir.path()))));
        let body = json!({ "args": { "instance_id": "00000000-0000-0000-0000-000000000000",
            "account": { "kind": "offline", "name": "Dev" } } });
        let err: AppError =
            serde_json::from_value(invoke(&webview, "launch_start", body).unwrap_err()).unwrap();
        assert_eq!(err.code, ErrorCode::InstanceNotFound);

        let body = json!({ "args": { "name": "x", "game_version": "1.20.1" } });
        let id = invoke(&webview, "instance_create", body).unwrap()["id"].clone();
        let body = json!({ "args": { "instance_id": id,
            "account": { "kind": "offline", "name": "a b" } } });
        let err: AppError =
            serde_json::from_value(invoke(&webview, "launch_start", body).unwrap_err()).unwrap();
        assert_eq!(err.code, ErrorCode::AuthOfflineInvalidName);

        let kill = json!({ "args": { "instance_id": id } });
        assert_eq!(
            invoke(&webview, "launch_kill", kill).unwrap()["killed"],
            false
        );
        let running = invoke(&webview, "launch_running", json!({})).unwrap();
        assert_eq!(running["games"], json!([]));
    }

    #[test]
    fn failed_launch_task_releases_the_instance() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path());
        let (app, webview) = mock_app_with(AppState::with_data(Ok(paths.clone())));
        let finished_rx = finished_events(&app);
        // 无需下载任何文件的自定义版本 + 无效的手动 Java：任务在选择 Java 时失败。
        let version_dir = paths.shared_versions_dir().join("custom");
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(
            version_dir.join("custom.json"),
            r#"{"id":"custom","type":"release","mainClass":"Main","libraries":[]}"#,
        )
        .unwrap();
        let body = json!({ "args": { "name": "x", "game_version": "custom" } });
        let mut config = invoke(&webview, "instance_create", body).unwrap();
        config["java"] = json!({ "mode": "path", "path": dir.path().join("no-java") });
        invoke(
            &webview,
            "instance_save",
            json!({ "args": { "config": config } }),
        )
        .unwrap();

        let body = json!({ "args": { "instance_id": config["id"],
            "account": { "kind": "offline", "name": "Dev" } } });
        for _ in 0..2 {
            let started = invoke(&webview, "launch_start", body.clone()).unwrap();
            let finished = finished_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            assert_eq!(started["task_id"], json!(finished.task_id));
            assert_eq!(finished.state, TaskState::Failed);
            assert_eq!(finished.error.unwrap().code, ErrorCode::JavaProbeFailed);
        }
        let list = invoke(&webview, "task_list", json!({})).unwrap();
        assert_eq!(list["tasks"][0]["kind"], "launch");
    }
    /// 本地加载器元数据服务器 + 临时数据目录，带一个继承自本地自定义版本 `custom` 的实例。
    fn loader_app() -> (
        tauri::App<MockRuntime>,
        tauri::WebviewWindow<MockRuntime>,
        TestServer,
        tempfile::TempDir,
        Value,
    ) {
        let server = tauri::async_runtime::block_on(TestServer::start([(
            "/fabric/v2/versions/loader/custom",
            br#"[{"loader":{"version":"0.17.0","stable":false}},
                 {"loader":{"version":"0.16.5","stable":true}}]"#
                .to_vec(),
        )]));
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path());
        let version_dir = paths.shared_versions_dir().join("custom");
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(
            version_dir.join("custom.json"),
            r#"{"id":"custom","type":"release","mainClass":"Main","libraries":[]}"#,
        )
        .unwrap();
        let mut state = AppState::with_data(Ok(paths));
        state.game.manifest_url = server.url("/manifest.json");
        state.loader.fabric_meta = server.url("/fabric");
        let (app, webview) = mock_app_with(state);
        let body = json!({ "args": { "name": "x", "game_version": "custom" } });
        let config = invoke(&webview, "instance_create", body).unwrap();
        (app, webview, server, dir, config)
    }

    #[test]
    fn loader_list_over_ipc() {
        let (_app, webview, _server, _dir, _config) = loader_app();
        let body = json!({ "args": { "kind": "fabric", "game_version": "custom" } });
        let list = invoke(&webview, "loader_list", body).unwrap();
        assert_eq!(list["versions"][0]["version"], "0.17.0");
        assert_eq!(list["versions"][1]["recommended"], true);
        // 元数据服务器不认识的版本（404）视为不支持：空列表。
        let body = json!({ "args": { "kind": "fabric", "game_version": "9.9" } });
        let list = invoke(&webview, "loader_list", body).unwrap();
        assert_eq!(list["versions"], json!([]));
        let body = json!({ "args": { "kind": "forge", "game_version": "1.5.2" } });
        let err: AppError =
            serde_json::from_value(invoke(&webview, "loader_list", body).unwrap_err()).unwrap();
        assert_eq!(err.code, ErrorCode::LoaderUnsupportedVersion);
        let body = json!({ "args": { "kind": "fabric", "game_version": "../x" } });
        let err: AppError =
            serde_json::from_value(invoke(&webview, "loader_list", body).unwrap_err()).unwrap();
        assert_eq!(err.code, ErrorCode::CommonInternal);
    }

    #[test]
    fn loader_install_sets_loader_only_on_success() {
        let (app, webview, server, _dir, config) = loader_app();
        let finished_rx = finished_events(&app);
        let id = config["id"].clone();

        let body = json!({ "args": { "instance_id": "missing", "kind": "fabric",
            "version": "0.16.5" } });
        let err: AppError =
            serde_json::from_value(invoke(&webview, "loader_install", body).unwrap_err()).unwrap();
        assert_eq!(err.code, ErrorCode::InstanceNotFound);

        // profile 不存在：任务失败，实例仍是原版。
        let body = json!({ "args": { "instance_id": id, "kind": "fabric", "version": "0.16.5" } });
        let finished = run_task(&webview, &finished_rx, "loader_install", body.clone());
        assert_eq!(finished.state, TaskState::Failed);
        assert_eq!(
            finished.error.unwrap().code,
            ErrorCode::LoaderVersionNotFound
        );
        let get = json!({ "args": { "id": id } });
        assert_eq!(
            invoke(&webview, "instance_get", get.clone()).unwrap()["loader"],
            Value::Null
        );

        server.route(
            "/fabric/v2/versions/loader/custom/0.16.5/profile/json",
            br#"{"id":"fabric-loader-0.16.5-custom","inheritsFrom":"custom","type":"release",
                 "mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient","libraries":[]}"#
                .to_vec(),
        );
        let finished = run_task(&webview, &finished_rx, "loader_install", body);
        assert_eq!(finished.state, TaskState::Succeeded, "{:?}", finished.error);
        let loader = invoke(&webview, "instance_get", get).unwrap()["loader"].clone();
        assert_eq!(loader, json!({ "kind": "fabric", "version": "0.16.5" }));
        let list = invoke(&webview, "task_list", json!({})).unwrap();
        assert_eq!(list["tasks"][0]["kind"], "loader_install");
    }
}
