use base64::{engine::general_purpose, Engine};
use tauri::AppHandle;

use crate::storage_manager::media::storage_write_image_bytes;

pub struct SavedGeneratedImage {
    pub asset_id: String,
    pub file_path: String,
    pub mime_type: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

pub async fn save_image(app: &AppHandle, image_data: &str) -> Result<SavedGeneratedImage, String> {
    let bytes = if image_data.starts_with("http://") || image_data.starts_with("https://") {
        download_image_from_url(image_data).await?
    } else if image_data.starts_with("data:image") {
        decode_data_url(image_data)?
    } else {
        decode_raw_base64(image_data)?
    };

    save_image_bytes(app, &bytes)
}

pub fn save_image_bytes(app: &AppHandle, bytes: &[u8]) -> Result<SavedGeneratedImage, String> {
    let asset_id = uuid::Uuid::new_v4().to_string();
    let stored = storage_write_image_bytes(app, &asset_id, bytes)?;
    let (width, height) = image::load_from_memory(bytes)
        .map(|img| (Some(img.width()), Some(img.height())))
        .unwrap_or((None, None));

    Ok(SavedGeneratedImage {
        asset_id,
        file_path: stored.file_path,
        mime_type: stored.mime_type,
        width,
        height,
    })
}

async fn download_image_from_url(url: &str) -> Result<Vec<u8>, String> {
    let response = reqwest::get(url).await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to download image: {}", e),
        )
    })?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to download image: HTTP {}",
            response.status()
        ));
    }

    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to read image bytes: {}", e),
            )
        })
}

fn decode_data_url(data_url: &str) -> Result<Vec<u8>, String> {
    let base64_data = data_url
        .split(',')
        .nth(1)
        .ok_or_else(|| "Invalid data URL format".to_string())?;

    decode_raw_base64(base64_data)
}

fn decode_raw_base64(base64_data: &str) -> Result<Vec<u8>, String> {
    general_purpose::STANDARD.decode(base64_data).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to decode base64: {}", e),
        )
    })
}
