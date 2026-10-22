//! TMCL 应用壳：只做参数转换、状态持有和事件转发，不写业务逻辑。
//!
//! 命令与事件的约定见 `docs/contracts/ipc.md`。

#[cfg(test)]
mod bindings;
mod commands;
mod events;
mod state;

use tauri::Runtime;
use tauri::ipc::Invoke;

use crate::state::AppState;

/// 所有命令：按模块分组，组内按字母排序，一行一个命令（AGENTS.md §5.2）。
fn handlers<R: Runtime>() -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        // app
        commands::app::app_demo_task,
        commands::app::app_info,
        // settings
        commands::settings::settings_get_language,
        commands::settings::settings_set_language,
    ]
}

/// 在给定的 builder 上注册状态与命令；测试中配合 mock runtime 使用。
fn setup<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.manage(AppState::new()).invoke_handler(handlers())
}

/// 构建并运行 Tauri 应用。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    setup(tauri::Builder::default())
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
    use tmcl_common::{AppError, ErrorCode, TaskFinished, TaskProgress, TaskState};

    use super::*;
    use crate::events::{TASK_FINISHED, TASK_PROGRESS};

    fn mock_app() -> (tauri::App<MockRuntime>, tauri::WebviewWindow<MockRuntime>) {
        let app = setup(mock_builder())
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
}
