//! TMCL 应用壳：只做参数转换、状态持有和事件转发，不写业务逻辑。

mod commands;

/// 构建并运行 Tauri 应用。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running TMCL");
}
