//! 应用全局状态：在 `lib.rs` 中用 `manage` 注册，命令通过 `State<'_, AppState>` 取用。

/// 命令层共享的状态。
///
/// 目前没有字段；M1-10 会在这里加入全局 `TaskManager`（任务列表、取消、历史），
/// 后续步骤按需加入数据目录、数据库连接等。
#[derive(Debug, Default)]
pub struct AppState {}
