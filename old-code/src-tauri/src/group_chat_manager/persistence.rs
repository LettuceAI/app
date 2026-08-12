use tauri::AppHandle;
use uuid::Uuid;

use crate::chat_manager::attachments::persist_attachments;
use crate::chat_manager::types::ImageAttachment;
use crate::storage_manager::media::storage_load_session_attachment;

pub(super) fn load_attachment_data(
    app: &AppHandle,
    attachments: &[ImageAttachment],
) -> Vec<ImageAttachment> {
    attachments
        .iter()
        .map(|attachment| {
            if !attachment.data.is_empty() {
                return attachment.clone();
            }
            let Some(storage_path) = &attachment.storage_path else {
                return attachment.clone();
            };
            match storage_load_session_attachment(app.clone(), storage_path.clone()) {
                Ok(data) => ImageAttachment {
                    data,
                    ..attachment.clone()
                },
                Err(_) => attachment.clone(),
            }
        })
        .collect()
}

pub(super) fn generated_image_attachments(data_urls: Vec<String>) -> Vec<ImageAttachment> {
    data_urls
        .into_iter()
        .map(|data| {
            let mime_type = data
                .split_once(";base64,")
                .and_then(|(prefix, _)| prefix.strip_prefix("data:"))
                .unwrap_or("image/png")
                .to_string();
            ImageAttachment {
                id: Uuid::new_v4().to_string(),
                data,
                mime_type,
                filename: None,
                width: None,
                height: None,
                storage_path: None,
            }
        })
        .collect()
}

pub(super) fn persist_group_attachments(
    app: &AppHandle,
    owner_id: &str,
    session_id: &str,
    message_id: &str,
    role: &str,
    attachments: Vec<ImageAttachment>,
) -> Result<Vec<ImageAttachment>, String> {
    persist_attachments(app, owner_id, session_id, message_id, role, attachments)
}
