//! Every `#[tauri::command]`, grouped by area. `lib.rs` registers them in `generate_handler!`.
//! Tauri names a command by its function name, so moving one between modules doesn't change
//! the frontend's `invoke()` string.

pub(crate) mod ai;
pub(crate) mod app_data;
pub(crate) mod files;
pub(crate) mod hosts;
pub(crate) mod monitoring;
pub(crate) mod network;
pub(crate) mod scheduler;
pub(crate) mod ssh;
pub(crate) mod vault;
