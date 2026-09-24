use lettuce_image_generation::{
    PlaygroundHistoryEntry, PlaygroundHistoryError, PlaygroundHistoryRepository,
    playground_page_size,
};
use lettuce_types::TimestampMillis;

impl crate::AppBackend {
    /// The old app's `playground_history_list`: newest first, 30 by default
    /// (1 to 200), `before` a creation time to page back from.
    pub fn playground_history(
        &self,
        limit: Option<u32>,
        before: Option<TimestampMillis>,
    ) -> Result<Vec<PlaygroundHistoryEntry>, PlaygroundHistoryError> {
        self.database()
            .list_playground_history(playground_page_size(limit), before)
    }

    /// The old app's `playground_history_delete`: removes the entry and,
    /// with `delete_images`, its image assets unless the library keeps them
    /// or something else uses them.
    pub fn delete_playground_history(
        &self,
        id: &str,
        delete_images: bool,
    ) -> Result<Vec<lettuce_types::AssetId>, PlaygroundHistoryError> {
        self.database().delete_playground_history(id, delete_images)
    }
}
