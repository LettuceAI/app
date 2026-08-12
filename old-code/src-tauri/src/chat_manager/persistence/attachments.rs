use std::fs;

use tauri::AppHandle;

use crate::chat_manager::types::{ImageAttachment, StoredMessage};
use crate::storage_manager::legacy::storage_root;
use crate::storage_manager::media::{
    storage_load_session_attachment, storage_save_session_attachment,
};
use crate::utils::{log_error, log_info};

pub fn persist_attachments(
    app: &AppHandle,
    character_id: &str,
    session_id: &str,
    message_id: &str,
    role: &str,
    attachments: Vec<ImageAttachment>,
) -> Result<Vec<ImageAttachment>, String> {
    let mut persisted = Vec::new();

    for attachment in attachments {
        if attachment.storage_path.is_some() && attachment.data.is_empty() {
            persisted.push(attachment);
            continue;
        }

        if attachment.data.is_empty() {
            continue;
        }

        let storage_path = storage_save_session_attachment(
            app.clone(),
            character_id.to_string(),
            session_id.to_string(),
            message_id.to_string(),
            attachment.id.clone(),
            role.to_string(),
            attachment.data.clone(),
        )?;

        persisted.push(ImageAttachment {
            id: attachment.id,
            data: String::new(),
            mime_type: attachment.mime_type,
            filename: attachment.filename,
            width: attachment.width,
            height: attachment.height,
            storage_path: Some(storage_path),
        });
    }

    Ok(persisted)
}

pub fn load_attachment_data(app: &AppHandle, message: &StoredMessage) -> StoredMessage {
    let mut loaded_message = message.clone();

    loaded_message.attachments = message
        .attachments
        .iter()
        .map(|attachment| {
            if !attachment.data.is_empty() {
                return attachment.clone();
            }

            let storage_path = match &attachment.storage_path {
                Some(path) => path,
                None => return attachment.clone(),
            };

            match storage_load_session_attachment(app.clone(), storage_path.clone()) {
                Ok(data) => ImageAttachment {
                    id: attachment.id.clone(),
                    data,
                    mime_type: attachment.mime_type.clone(),
                    filename: attachment.filename.clone(),
                    width: attachment.width,
                    height: attachment.height,
                    storage_path: attachment.storage_path.clone(),
                },
                Err(_) => attachment.clone(),
            }
        })
        .collect();

    loaded_message
}

pub fn cleanup_attachments(app: &AppHandle, attachments: &[ImageAttachment], scope: &str) {
    for attachment in attachments {
        let Some(storage_path) = &attachment.storage_path else {
            continue;
        };

        let full_path = match storage_root(app) {
            Ok(root) => root.join(storage_path),
            Err(err) => {
                log_error(
                    app,
                    scope,
                    format!(
                        "failed to resolve storage root while cleaning attachment {}: {}",
                        storage_path, err
                    ),
                );
                return;
            }
        };

        if !full_path.exists() {
            continue;
        }

        if let Err(err) = fs::remove_file(&full_path) {
            log_error(
                app,
                scope,
                format!("failed to remove attachment {}: {}", storage_path, err),
            );
            continue;
        }

        log_info(app, scope, format!("removed attachment {}", storage_path));
    }
}
