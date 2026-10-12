use lettuce_context::{
    PromptBehaviorVersion, PromptDocument, PromptMetadataDraft, PromptRepository,
    PromptRepositoryError,
};
use lettuce_transfer::PromptTransferError;
use lettuce_types::{PromptDocumentId, TimestampMillis};

#[derive(Debug, thiserror::Error)]
pub enum PromptFileError {
    #[error(transparent)]
    Format(#[from] PromptTransferError),
    #[error("prompt storage failed: {0}")]
    Repository(#[from] PromptRepositoryError),
    #[error("Template not found: {0}")]
    NotFound(PromptDocumentId),
}

/// A prompt file written as a new prompt template.
#[derive(Debug, Clone)]
pub struct ImportedPromptFile {
    pub document: PromptDocument,
    /// The file named no purpose this app can run; it became a direct chat
    /// prompt.
    pub purpose_defaulted: bool,
}

/// Prompt templates read from USC cards and SillyTavern presets.
#[derive(Debug)]
pub struct PromptFileCoordinator<'a, R: ?Sized> {
    repository: &'a R,
}

impl<'a, R: ?Sized> PromptFileCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }
}

impl<R: PromptRepository + ?Sized> PromptFileCoordinator<'_, R> {
    /// Creates the file's template: a USC card, else a SillyTavern preset
    /// named after `file_stem`. The fallback names are the host's localized
    /// names for an unnamed entry and an unnamed preset.
    pub fn import(
        &self,
        json: &str,
        file_stem: Option<&str>,
        fallback_entry_name: &str,
        fallback_set_name: &str,
        now: TimestampMillis,
    ) -> Result<ImportedPromptFile, PromptFileError> {
        let imported = lettuce_transfer::parse_prompt_import(
            json,
            file_stem,
            fallback_entry_name,
            fallback_set_name,
        )?;
        let document = self.repository.create_user_draft(
            PromptMetadataDraft {
                name: imported.name,
                purpose: imported.purpose,
                condense: imported.condense,
                behavior_version: PromptBehaviorVersion::LegacyV1,
            },
            imported.entries,
            now,
        )?;
        Ok(ImportedPromptFile {
            document,
            purpose_defaulted: imported.purpose_defaulted,
        })
    }
}

/// A name with every UTF-16 unit outside `[A-Za-z0-9_-]` replaced by `_`,
/// lowercased, or `export` when empty.
pub(crate) fn export_name(name: &str) -> String {
    let safe = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character.to_ascii_lowercase().to_string()
            } else {
                "_".repeat(character.len_utf16())
            }
        })
        .collect::<String>();
    if safe.is_empty() {
        "export".to_owned()
    } else {
        safe
    }
}

/// The UTC calendar date of `now` as `YYYY-MM-DD`.
pub(crate) fn export_date(now: TimestampMillis) -> String {
    chrono::DateTime::from_timestamp_millis(now.get())
        .unwrap_or_default()
        .format("%Y-%m-%d")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_usc_card_becomes_a_prompt_template() {
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let files = backend.prompt_files();
        let card = serde_json::json!({
            "schema": {"name": "USC", "version": "1.0"},
            "kind": "system_prompt_template",
            "payload": {
                "name": "Night Shift",
                "promptType": "directChat",
                "content": "",
                "entries": [
                    {"id": "e1", "name": "Rules", "role": "system", "content": "You are {{char}}.", "enabled": true}
                ],
                "condensePromptEntries": false
            }
        });
        let imported = files
            .import(
                &card.to_string(),
                None,
                "Imported",
                "Imported set",
                TimestampMillis::new(5),
            )
            .expect("import");
        assert_eq!(imported.document.name, "Night Shift");
        assert!(!imported.purpose_defaulted);
        assert_eq!(imported.document.entries.len(), 1);
        assert_eq!(imported.document.entries[0].content, "You are {{char}}.");
    }

    #[test]
    fn export_names_replace_each_utf16_unit() {
        assert_eq!(export_name("Night 🌙"), "night___");
        assert_eq!(export_name(""), "export");
    }
}
