use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use crate::api::ApiContext;
use crate::api::error::{IntoApiError, api_error, invalid_field};

fn validate(document: &dto::SettingsThemeDocument) -> Result<(), ApiError> {
    if document.format_version != 1 || document.name.trim().is_empty() {
        return Err(invalid_field("theme", "theme version or name is invalid"));
    }
    let colors = serde_json::to_value(&document.colors).map_err(|_| invalid_field("theme.colors", "theme colors are invalid"))?;
    serde_json::from_value::<dto::SettingsUiCustomColors>(colors).map_err(|_| invalid_field("theme.colors", "theme colors are invalid"))?;
    for color in document.colors.values() {
        let valid = color.starts_with('#') && matches!(color.len(), 4 | 7)
            && color[1..].bytes().all(|byte| byte.is_ascii_hexdigit());
        if !valid { return Err(invalid_field("theme.colors", "theme color must be hexadecimal")); }
    }
    if ["surface", "fg", "accent"].iter().any(|key| !document.colors.contains_key(*key)) {
        return Err(invalid_field("theme.colors", "theme requires background, text and accent"));
    }
    Ok(())
}

pub async fn settings_theme_read(context: &ApiContext, source: dto::FileSource) -> Result<dto::SettingsThemeDocument, ApiError> {
    context.blocking(move |context| {
        if source.uri.trim().is_empty() { return Err(invalid_field("source", "source is empty")); }
        let reader = context.files().open(&source.uri).map_err(IntoApiError::into_api_error)?;
        let value: serde_json::Value = serde_json::from_reader(reader).map_err(|_| api_error(ApiErrorCode::Malformed, "theme document is invalid"))?;
        let document = if value.get("type").and_then(serde_json::Value::as_str) == Some("color-preset") && value.get("version").and_then(serde_json::Value::as_u64) == Some(1) {
            dto::SettingsThemeDocument {
                format_version: 1,
                name: value["preset"]["name"].as_str().unwrap_or_default().to_owned(),
                colors: serde_json::from_value(value["preset"]["colors"].clone()).map_err(|_| invalid_field("theme.colors", "theme colors are invalid"))?,
                radius: None,
                density: None,
            }
        } else {
            serde_json::from_value(value).map_err(|_| api_error(ApiErrorCode::Malformed, "theme document is invalid"))?
        };
        validate(&document)?;
        Ok(document)
    }).await
}

pub async fn settings_theme_export(context: &ApiContext, request: dto::SettingsThemeExportRequest) -> Result<dto::EntityExportResult, ApiError> {
    validate(&request.document)?;
    context.blocking(move |context| {
        crate::api::entity_files::publish(context, request.target, "theme.json".to_owned(), |writer| {
            serde_json::to_writer_pretty(writer, &request.document)
                .map_err(|_| api_error(ApiErrorCode::Unavailable, "theme could not be written"))
        }, Vec::new())
    }).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn customization_theme_validation_rejects_unsafe_colors_and_versions() {
        let mut document: dto::SettingsThemeDocument = serde_json::from_value(serde_json::json!({"format_version":1,"name":"Ocean","colors":{"surface":"#08141e","fg":"#e5edf3","accent":"#72bed5"},"radius":null,"density":null})).expect("theme");
        validate(&document).expect("valid theme");
        document.colors.insert("accent".to_owned(), "url(https://example.invalid)".to_owned());
        assert_eq!(validate(&document).expect_err("invalid color").code, ApiErrorCode::InvalidInput);
        document.colors.insert("accent".to_owned(), "#72bed5".to_owned());
        document.format_version = 2;
        assert_eq!(validate(&document).expect_err("unknown version").code, ApiErrorCode::InvalidInput);
    }
}
