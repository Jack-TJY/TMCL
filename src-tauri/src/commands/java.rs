//! `java_*` 命令：列出本机可用的 Java（PLAN.md M1-05）。

use serde::Serialize;
use tauri::State;
use tmcl_common::AppError;
use tmcl_java::{JavaInstall, Sources};
use ts_rs::TS;

use crate::state::AppState;

/// `java_list` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct JavaList {
    /// 启动器下载到 `java/` 下的运行时（`managed` 为 `true`）在前，其余按主版本从高到低；
    /// 同一个 Java 经多个来源找到时只出现一次。
    pub javas: Vec<JavaInstall>,
}

/// 探测本机 Java：启动器自己下载的运行时与系统中安装的 Java。
///
/// 数据目录不可用时返回 `COMMON_DATA_DIR_UNAVAILABLE`。
#[tauri::command]
pub async fn java_list(state: State<'_, AppState>) -> Result<JavaList, AppError> {
    let java_dir = state.data()?.java_dir();
    let javas = tmcl_java::detect(&Sources::system(&java_dir)).await;
    Ok(JavaList { javas })
}
