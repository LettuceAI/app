//! Legacy message attachments (`ImageAttachment` JSON on messages and group
//! variants). Legacy persisted every kept attachment as a file under
//! `sessions/` and stored its path with empty inline data.

use serde::Deserialize;

const MAX_LABEL_SCALARS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyMessageAttachment {
    pub id: String,
    #[serde(default)]
    pub data: String,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub storage_path: Option<String>,
}

/// The attachments of one legacy JSON column; a value legacy could not have
/// read yields none.
#[must_use]
pub fn legacy_message_attachments(raw: &str) -> Vec<LegacyMessageAttachment> {
    serde_json::from_str::<Vec<serde_json::Value>>(raw)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect()
}

impl LegacyMessageAttachment {
    /// The stored file of a persisted attachment, relative to legacy's
    /// storage root.
    #[must_use]
    pub fn stored_path(&self) -> Option<&str> {
        self.storage_path
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
    }

    /// Legacy dropped an attachment with neither data nor a stored file.
    #[must_use]
    pub fn is_placeholder(&self) -> bool {
        self.stored_path().is_none() && self.data.is_empty()
    }

    /// The filename (for generated images, their prompt) as a media source
    /// label: control characters become spaces and the text is cut to the
    /// label bound. The flag says whether it changed.
    #[must_use]
    pub fn label(&self) -> Option<(String, bool)> {
        let raw = self.filename.as_deref()?.trim();
        if raw.is_empty() {
            return None;
        }
        let cleaned = raw
            .chars()
            .map(|character| {
                if character.is_control() {
                    ' '
                } else {
                    character
                }
            })
            .take(MAX_LABEL_SCALARS)
            .collect::<String>()
            .trim()
            .to_owned();
        let changed = cleaned != raw;
        (!cleaned.is_empty()).then_some((cleaned, changed))
    }
}
