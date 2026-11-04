use lettuce_image_generation::ProviderImageRequest;
use lettuce_models::{CustomImageConfig, CustomImageEditFormat};
use lettuce_network::MultipartField;
use serde_json::Value;

use super::adapters::{Adapter, ImageCall, ImagePayload, ImageResponseData};
use crate::common::{custom_config, select_path};

pub(super) fn call(base: &str, request: &ProviderImageRequest) -> Result<ImageCall, String> {
    let config = custom_config(&request.account.config).map_err(|_| "Invalid custom provider configuration")?;
    let image = &config.image;
    let editing = !request.input_images.is_empty();
    let path = if editing {
        image.edit_path.as_deref().ok_or("This provider has no image editing endpoint")?
    } else { &image.generation_path };
    let mut call = if editing && image.edit_format == CustomImageEditFormat::JsonDataUrls {
        Adapter::NanoGpt.call(base, request)?
    } else { Adapter::OpenAi.call(base, request)? };
    let (endpoint, path) = crate::common::request_target(std::borrow::Cow::Borrowed(base), std::borrow::Cow::Owned(path.to_owned()));
    call.endpoint = endpoint.into_owned();
    call.path = path.into_owned();
    match &mut call.payload {
        ImagePayload::Json(body) => {
            let body = body.as_object_mut().ok_or("Invalid image request")?;
            for (name, value) in &image.extra_body { body.entry(name.clone()).or_insert_with(|| value.clone()); }
        }
        ImagePayload::Multipart(fields) => {
            for (name, value) in &image.extra_body {
                if !matches!(name.as_str(), "model" | "prompt" | "n" | "image" | "image[]" | "mask") {
                    fields.push(MultipartField::Text { name: name.clone(), value: value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string()) });
                }
            }
            if let Some(mask) = &request.mask_image {
                fields.push(MultipartField::File { name: "mask".into(), filename: "mask.png".into(), mime_type: mask.mime_type.clone(), bytes: mask.bytes.clone() });
            }
        }
    }
    Ok(call)
}

pub(super) fn parse(config: &CustomImageConfig, body: &Value) -> Result<Vec<ImageResponseData>, String> {
    let selected = match &config.list_path {
        Some(path) => select_path(body, path.as_str()).ok_or("Image response list path was not found")?,
        None => body,
    };
    let entries = selected.as_array().map_or_else(|| vec![selected], |items| items.iter().collect());
    if entries.is_empty() { return Err("Provider returned no images".into()); }
    entries.into_iter().map(|item| {
        let read = |path: &Option<lettuce_models::JsonPath>| path.as_ref().and_then(|path| select_path(item, path.as_str())).and_then(Value::as_str).filter(|value| !value.is_empty()).map(str::to_owned);
        let (url, b64_json) = (read(&config.url_path), read(&config.b64_path));
        if url.is_none() && b64_json.is_none() { return Err("Image response has no image at the configured paths".into()); }
        Ok(ImageResponseData { url, b64_json, text: None })
    }).collect()
}
