use crate::sync::manager::{self, SyncStatus};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn share_sync_device(app: AppHandle, port: u16) -> Result<String, String> {
    manager::share_device(app, port).await
}

#[tauri::command]
pub async fn connect_sync_device(
    app: AppHandle,
    ip: String,
    port: u16,
    pin: String,
) -> Result<(), String> {
    manager::connect_device(app, ip, port, pin).await
}

#[tauri::command]
pub async fn stop_sync(app: AppHandle) -> Result<(), String> {
    manager::stop_sync(app).await
}

#[tauri::command]
pub async fn get_sync_status(app: AppHandle) -> Result<SyncStatus, String> {
    let state = app.state::<manager::SyncManagerState>();
    let status = state.status.read().await;
    Ok(status.clone())
}

#[tauri::command]
pub async fn get_local_ip() -> Result<String, String> {
    crate::utils::get_local_ip()
}
#[tauri::command]
pub async fn approve_connection(app: AppHandle, ip: String, allow: bool) -> Result<(), String> {
    manager::approve_connection(app, ip, allow).await
}

#[tauri::command]
pub async fn start_sync_session(app: tauri::AppHandle, ip: String) -> Result<(), String> {
    crate::sync::manager::start_sync_session(app, ip).await
}
