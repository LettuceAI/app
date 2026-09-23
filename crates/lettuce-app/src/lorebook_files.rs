use lettuce_context::{
    DetectionPolicy, KeywordMatchMode, LorebookBehaviorVersion, LorebookDetails,
    LorebookEntryDraft, LorebookMetadataDraft, LorebookRepository, LorebookRepositoryError,
};
use lettuce_transfer::{
    LorebookTransfer, LorebookTransferEntry, LorebookTransferError, PackagedKeywordDetectionMode,
    PackagedKeywordMatchMode,
};
use lettuce_types::{LorebookId, TimestampMillis};

#[derive(Debug, thiserror::Error)]
pub enum LorebookFileError {
    #[error(transparent)]
    Format(#[from] LorebookTransferError),
    #[error("lorebook storage failed: {0}")]
    Repository(#[from] LorebookRepositoryError),
    #[error("Lorebook not found for export")]
    NotFound,
}

/// Lorebooks read from and written to World Info and USC files.
#[derive(Debug)]
pub struct LorebookFileCoordinator<'a, R: ?Sized> {
    repository: &'a R,
}

impl<'a, R: ?Sized> LorebookFileCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }
}

impl<R: LorebookRepository + ?Sized> LorebookFileCoordinator<'_, R> {
    /// A new lorebook from a World Info file.
    pub fn import_world_info(
        &self,
        json: &str,
        now: TimestampMillis,
    ) -> Result<LorebookDetails, LorebookFileError> {
        let imported = lettuce_transfer::parse_world_info(json)?;
        Ok(self.repository.create(
            LorebookMetadataDraft {
                name: imported.name,
                detection_policy: detection_policy(imported.keyword_detection_mode),
                icon_asset_id: None,
                behavior_version: LorebookBehaviorVersion::LegacyV1,
            },
            imported
                .entries
                .into_iter()
                .map(|entry| LorebookEntryDraft {
                    title: entry.title,
                    enabled: entry.enabled,
                    always_active: entry.always_active,
                    keywords: entry.keywords,
                    case_sensitive: entry.case_sensitive,
                    match_mode: KeywordMatchMode::Literal,
                    content: entry.content,
                    priority: entry.priority,
                })
                .collect(),
            now,
        )?)
    }

    pub fn export_world_info(&self, id: LorebookId) -> Result<String, LorebookFileError> {
        Ok(lettuce_transfer::export_world_info(&self.transfer(id)?)?)
    }

    pub fn export_usc(&self, id: LorebookId) -> Result<String, LorebookFileError> {
        Ok(lettuce_transfer::export_lorebook_usc(&self.transfer(id)?)?)
    }

    fn transfer(&self, id: LorebookId) -> Result<LorebookTransfer, LorebookFileError> {
        let details = self
            .repository
            .get(id)?
            .ok_or(LorebookFileError::NotFound)?;
        Ok(lorebook_transfer(&details))
    }
}

pub(crate) fn lorebook_transfer(details: &LorebookDetails) -> LorebookTransfer {
    let mut entries = details.entries.clone();
    entries.sort_by_key(|entry| entry.ordinal);
    LorebookTransfer {
        id: details.book.id.to_string(),
        name: details.book.name.clone(),
        keyword_detection_mode: match details.book.detection_policy {
            DetectionPolicy::RecentMessageWindow => {
                PackagedKeywordDetectionMode::RecentMessageWindow
            }
            DetectionPolicy::LatestUserMessage => PackagedKeywordDetectionMode::LatestUserMessage,
        },
        entries: entries
            .into_iter()
            .map(|entry| LorebookTransferEntry {
                id: entry.id.to_string(),
                title: entry.title,
                enabled: entry.enabled,
                always_active: entry.always_active,
                keywords: entry.keywords,
                case_sensitive: entry.case_sensitive,
                keyword_match_mode: match entry.match_mode {
                    KeywordMatchMode::Literal => PackagedKeywordMatchMode::Literal,
                    KeywordMatchMode::Regex => PackagedKeywordMatchMode::Regex,
                },
                content: entry.content,
                priority: entry.priority,
                display_order: i32::try_from(entry.ordinal).unwrap_or(i32::MAX),
                created_at: entry.created_at.get(),
                updated_at: entry.updated_at.get(),
            })
            .collect(),
        created_at: details.book.created_at.get(),
        updated_at: details.book.updated_at.get(),
    }
}

const fn detection_policy(mode: PackagedKeywordDetectionMode) -> DetectionPolicy {
    match mode {
        PackagedKeywordDetectionMode::RecentMessageWindow => DetectionPolicy::RecentMessageWindow,
        PackagedKeywordDetectionMode::LatestUserMessage => DetectionPolicy::LatestUserMessage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_world_info_file_round_trips_through_a_new_lorebook() {
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let files = backend.lorebook_files();
        let imported = files
            .import_world_info(
                &serde_json::json!({
                    "name": "Harbour",
                    "extensions": {"lettuceai": {"keywordDetectionMode": "latestUserMessage"}},
                    "entries": {
                        "2": {"keys": ["ship"], "content": "A ship", "insertion_order": 1, "priority": 3},
                        "1": {"name": "Key", "keys": ["key"], "content": "Ada keeps it", "insertion_order": 0, "disable": true}
                    }
                })
                .to_string(),
                TimestampMillis::new(10),
            )
            .expect("import");
        assert_eq!(imported.book.name, "Harbour");
        assert_eq!(
            imported.book.detection_policy,
            DetectionPolicy::LatestUserMessage
        );
        let exported: serde_json::Value = serde_json::from_str(
            &files
                .export_world_info(imported.book.id)
                .expect("world info"),
        )
        .expect("json");
        assert_eq!(exported["entries"]["1"]["name"], "Key");
        assert_eq!(exported["entries"]["1"]["enabled"], false);
        assert_eq!(exported["entries"]["2"]["priority"], 3);
        let usc: serde_json::Value =
            serde_json::from_str(&files.export_usc(imported.book.id).expect("usc")).expect("json");
        assert_eq!(usc["payload"]["id"], imported.book.id.to_string());
        assert_eq!(usc["payload"]["entries"][1]["title"], "ship");
        assert!(matches!(
            files.export_usc(LorebookId::new()),
            Err(LorebookFileError::NotFound)
        ));
    }
}
