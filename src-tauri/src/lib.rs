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
        // game
        commands::game::game_install,
        commands::game::game_version_list,
        // settings
        commands::settings::settings_get_language,
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
    setup(tauri::Builder::default(), AppState::new())
        .run(tauri::generate_context!())
        .expect("error while running TMCL");
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use serde_json::{Value, json};
    use tauri::Listener;
    use tauri::test::{
        INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
    };
    use tauri::webview::InvokeRequest;
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
        let (_app, webview, _server, _dir) = game_app();
        let list = invoke(&webview, "game_version_list", json!({})).unwrap();
        assert_eq!(list["latest_release"], "1.20.1");
        assert_eq!(list["versions"][0]["id"], "23w31a");
        assert_eq!(list["versions"][0]["kind"], "snapshot");
        assert_eq!(
            list["versions"][1]["release_time"],
            "2023-06-12T13:25:51+00:00"
        );
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
}
