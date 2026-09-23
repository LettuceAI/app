use lettuce_context::{
    PromptBehaviorVersion, PromptDocument, PromptEntryDraft, PromptMetadataDraft, PromptRepository,
    PromptRepositoryError,
};
use lettuce_transfer::{PromptTransfer, PromptTransferEntry, PromptTransferError};
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

/// Which file a prompt template is exported as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptFileFormat {
    Usc,
    SillyTavern,
}

/// A prompt file ready to save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedPromptFile {
    pub filename: String,
    pub content: String,
}

/// Prompt templates read from and written to USC cards and SillyTavern
/// presets.
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

    /// The template as a file of `format`, named like the old app named it.
    pub fn export(
        &self,
        id: PromptDocumentId,
        format: PromptFileFormat,
        now: TimestampMillis,
    ) -> Result<ExportedPromptFile, PromptFileError> {
        let document = self
            .repository
            .get(id)?
            .ok_or(PromptFileError::NotFound(id))?;
        let transfer = PromptTransfer {
            id: document.id.to_string(),
            name: document.name.clone(),
            purpose: document.purpose,
            entries: document
                .entries
                .iter()
                .map(|entry| PromptTransferEntry {
                    id: entry
                        .built_in_entry_key
                        .clone()
                        .unwrap_or_else(|| entry.id.to_string()),
                    draft: PromptEntryDraft {
                        built_in_entry_key: None,
                        name: entry.name.clone(),
                        role: entry.role,
                        content: entry.content.clone(),
                        enabled: entry.enabled,
                        injection_position: entry.injection_position,
                        depth: entry.depth,
                        conditional_min_messages: entry.conditional_min_messages,
                        interval_turns: entry.interval_turns,
                        system_prompt: entry.system_prompt,
                        conditions: entry.conditions.clone(),
                        payload: entry.payload.clone(),
                    },
                })
                .collect(),
            condense: document.condense,
            created_at: document.created_at.get(),
            updated_at: document.updated_at.get(),
        };
        let (content, extension) = match format {
            PromptFileFormat::Usc => (lettuce_transfer::export_prompt_usc(&transfer)?, "usc"),
            PromptFileFormat::SillyTavern => (
                lettuce_transfer::export_prompt_sillytavern(&transfer)?,
                "json",
            ),
        };
        Ok(ExportedPromptFile {
            filename: format!(
                "system_prompts_{}_{}.{extension}",
                export_name(&document.name),
                export_date(now)
            ),
            content,
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
    fn a_prompt_template_round_trips_through_both_formats() {
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
        let usc = files
            .export(
                imported.document.id,
                PromptFileFormat::Usc,
                TimestampMillis::new(86_400_000),
            )
            .expect("usc");
        assert_eq!(usc.filename, "system_prompts_night_shift_1970-01-02.usc");
        let again = files
            .import(
                &usc.content,
                None,
                "Imported",
                "Imported set",
                TimestampMillis::new(6),
            )
            .expect("usc again");
        assert_eq!(again.document.entries.len(), 1);
        assert_eq!(again.document.entries[0].content, "You are {{char}}.");
        let preset = files
            .export(
                imported.document.id,
                PromptFileFormat::SillyTavern,
                TimestampMillis::new(0),
            )
            .expect("preset");
        assert!(preset.filename.ends_with(".json"));
        let from_preset = files
            .import(
                &preset.content,
                Some("night"),
                "Imported",
                "Imported set",
                TimestampMillis::new(7),
            )
            .expect("preset again");
        assert_eq!(from_preset.document.name, "night");
        assert_eq!(from_preset.document.entries[0].content, "You are {{char}}.");
    }

    #[test]
    fn export_names_replace_each_utf16_unit() {
        assert_eq!(export_name("Night 🌙"), "night___");
        assert_eq!(export_name(""), "export");
    }
}
