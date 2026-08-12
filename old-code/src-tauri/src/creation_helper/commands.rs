use tauri::AppHandle;

use super::service;
use super::types::{
    CreationGoal, CreationMode, CreationSession, CreationSessionSummary, DraftCharacter,
    UploadedImage,
};

#[tauri::command]
pub fn creation_helper_start(
    app: AppHandle,
    creation_goal: Option<CreationGoal>,
    creation_mode: Option<CreationMode>,
    target_type: Option<CreationGoal>,
    target_id: Option<String>,
) -> Result<CreationSession, String> {
    service::start_session(
        &app,
        creation_goal.unwrap_or(CreationGoal::Character),
        creation_mode.unwrap_or(CreationMode::Create),
        target_type,
        target_id,
    )
}

#[tauri::command]
pub fn creation_helper_get_session(
    app: AppHandle,
    session_id: String,
) -> Result<Option<CreationSession>, String> {
    service::get_session(&app, &session_id)
}

#[tauri::command]
pub fn creation_helper_get_latest_session(
    app: AppHandle,
    creation_goal: Option<CreationGoal>,
) -> Result<Option<CreationSession>, String> {
    service::get_latest_resumable_session(&app, creation_goal)
}

#[tauri::command]
pub fn creation_helper_list_sessions(
    app: AppHandle,
    creation_goal: Option<CreationGoal>,
) -> Result<Vec<CreationSessionSummary>, String> {
    service::list_sessions(&app, creation_goal)
}

#[tauri::command]
pub async fn creation_helper_send_message(
    app: AppHandle,
    session_id: String,
    message: String,
    uploaded_images: Option<Vec<UploadedImageArg>>,
    request_id: Option<String>,
) -> Result<CreationSession, String> {
    let images = uploaded_images.map(|imgs| {
        imgs.into_iter()
            .map(|img| (img.id, img.data, img.mime_type))
            .collect()
    });
    service::send_message(app, session_id, message, images, request_id).await
}

#[tauri::command]
pub async fn creation_helper_regenerate(
    app: AppHandle,
    session_id: String,
    request_id: Option<String>,
) -> Result<CreationSession, String> {
    service::regenerate_response(app, session_id, request_id).await
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadedImageArg {
    pub id: String,
    pub data: String,
    pub mime_type: String,
}

#[tauri::command]
pub fn creation_helper_get_draft(
    app: AppHandle,
    session_id: String,
) -> Result<Option<DraftCharacter>, String> {
    service::get_draft(&app, &session_id)
}

#[tauri::command]
pub fn creation_helper_cancel(app: AppHandle, session_id: String) -> Result<(), String> {
    service::cancel_session(&app, &session_id)
}

#[tauri::command]
pub fn creation_helper_complete(
    app: AppHandle,
    session_id: String,
) -> Result<DraftCharacter, String> {
    service::complete_session(&app, &session_id)
}

#[tauri::command]
pub fn creation_helper_get_uploaded_image(
    app: AppHandle,
    session_id: String,
    image_id: String,
) -> Result<Option<UploadedImage>, String> {
    let _ = service::get_session(&app, &session_id)?;
    service::get_uploaded_image_metadata(&session_id, &image_id)
}

#[tauri::command]
pub fn creation_helper_get_images(
    app: AppHandle,
    session_id: String,
) -> Result<Vec<UploadedImage>, String> {
    let _ = service::get_session(&app, &session_id)?;
    service::get_all_uploaded_images_metadata(&session_id)
}
