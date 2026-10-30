//! Tauri 命令，每个模块一个文件：`commands/<模块>.rs`，此处一行一个 `pub mod`，按字母排序。

pub mod app;
pub mod appearance;
pub mod auth;
pub mod diagnostics;
pub mod game;
pub mod instance;
pub mod launch;
pub mod layout;
pub mod loader;
pub mod resources;
pub mod sakura;
pub mod settings;
pub mod skin;
pub mod sync;
pub mod task;
