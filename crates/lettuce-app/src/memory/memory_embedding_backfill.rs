//! Embeds a memory space's missing vectors when retrieval needs them. Vectors
//! are keyed by the model's vector-space label and dimension, so after a
//! model or dimension change a space's memories are re-embedded the next time
//! it is retrieved from; the old vectors stay stored and become current again
//! if the user switches back.

use std::collections::HashSet;

use lettuce_embeddings::{
    EmbeddingRequest, MemoryEmbeddingProjection, MemoryEmbeddingRepository, ProjectionWrite,
};
use lettuce_jobs::handle::CancellationToken;
use lettuce_memory::MemorySpaceSnapshot;
use lettuce_types::TimestampMillis;

use crate::{EmbeddingGenerationError, MemoryEmbeddingEngine};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemoryEmbeddingBackfill {
    pub embedded: usize,
    /// Memories whose text changed while they were embedded; the new text
    /// is embedded by the next retrieval.
    pub superseded: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MemoryEmbeddingBackfillError {
    #[error("memory embedding backfill was cancelled")]
    Cancelled,
    #[error("memory embedding storage failed")]
    Repository,
}

/// Embeds every active memory of `memory` whose vector for the engine's
/// vector space, dimension and the memory's current text is missing or
/// repair needed, recounting its tokens with the embedding tokenizer. A memory
/// that fails stays pending for the next pass while the rest still get
/// their vectors; superseded memories are never retrieved and are skipped.
pub(crate) fn embed_missing_memories<E, R>(
    engine: &E,
    repository: &R,
    memory: &MemorySpaceSnapshot,
    cancellation: &CancellationToken,
    now: TimestampMillis,
) -> Result<MemoryEmbeddingBackfill, MemoryEmbeddingBackfillError>
where
    E: MemoryEmbeddingEngine + ?Sized,
    R: MemoryEmbeddingRepository + ?Sized,
{
    let pending = repository
        .list_repairs(memory.id, engine.source_revision(), engine.dimensions())
        .map_err(|_| MemoryEmbeddingBackfillError::Repository)?
        .into_iter()
        .map(|repair| (repair.memory_id, repair.source_text))
        .collect::<HashSet<_>>();
    let mut outcome = MemoryEmbeddingBackfill::default();
    for item in &memory.items {
        if item.superseded_by.is_some() || !pending.contains(&(item.id, item.text.clone())) {
            continue;
        }
        let vector = match engine.embed_memory(
            &EmbeddingRequest {
                text: item.text.clone(),
                dimensions: engine.dimensions(),
            },
            cancellation,
        ) {
            Ok(vector) => vector,
            Err(EmbeddingGenerationError::Cancelled) => {
                return Err(MemoryEmbeddingBackfillError::Cancelled);
            }
            Err(EmbeddingGenerationError::Unavailable) => {
                outcome.failed += 1;
                continue;
            }
        };
        let projection = MemoryEmbeddingProjection {
            space_id: memory.id,
            memory_id: item.id,
            source_text: item.text.clone(),
            vector,
            dimensions: engine.dimensions(),
            updated_at: now,
        };
        let written = match engine.count_tokens(&item.text) {
            Ok(token_count) => repository.put_reembedded(projection, token_count),
            Err(_) => repository.put_ready(projection),
        };
        match written {
            Ok(ProjectionWrite::Stored) => outcome.embedded += 1,
            Ok(ProjectionWrite::Superseded) => outcome.superseded += 1,
            Err(_) => outcome.failed += 1,
        }
    }
    if outcome.failed > 0 {
        tracing::info!(
            failed = outcome.failed,
            "pending memory embeddings wait for the next pass"
        );
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use lettuce_database::Database;
    use lettuce_embeddings::{EmbeddingDimensions, EmbeddingVector};
    use lettuce_memory::{MemoryCategory, MemoryItem, MemoryRepository, Score};
    use lettuce_types::{MemoryId, Revision};

    use super::*;

    type EmbedHook<'a> = std::sync::Mutex<Option<Box<dyn FnOnce() + Send + 'a>>>;

    struct Engine<'a> {
        space: &'static str,
        dimensions: EmbeddingDimensions,
        calls: AtomicUsize,
        fail_text: Option<&'static str>,
        during_embed: EmbedHook<'a>,
    }

    impl MemoryEmbeddingEngine for Engine<'_> {
        fn source_revision(&self) -> &str {
            self.space
        }

        fn dimensions(&self) -> EmbeddingDimensions {
            self.dimensions
        }

        fn count_tokens(&self, _text: &str) -> Result<u32, EmbeddingGenerationError> {
            Ok(1)
        }

        fn embed_memory(
            &self,
            request: &EmbeddingRequest,
            cancellation: &CancellationToken,
        ) -> Result<EmbeddingVector, EmbeddingGenerationError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if cancellation.is_cancelled() {
                return Err(EmbeddingGenerationError::Cancelled);
            }
            if let Some(hook) = self.during_embed.lock().expect("hook").take() {
                hook();
            }
            if self.fail_text == Some(request.text.as_str()) {
                return Err(EmbeddingGenerationError::Unavailable);
            }
            Ok(EmbeddingVector {
                source_revision: self.space.to_owned(),
                values: vec![0.25; request.dimensions.get()],
            })
        }
    }

    fn item(text: &str) -> MemoryItem {
        let id = MemoryId::new();
        MemoryItem {
            id,
            short_id: lettuce_memory::MemoryShortId::derived(id),
            text: text.to_owned(),
            category: MemoryCategory::Other,
            source_message_id: None,
            source_role: None,
            observed_at: None,
            observed_time_precision: None,
            superseded_by: None,
            superseded_at: None,
            supersedes: Vec::new(),
            token_count: 2,
            is_cold: false,
            is_pinned: false,
            importance: Score::FULL,
            persistence_importance: Score::FULL,
            prompt_importance: Score::FULL,
            volatility: Score::LEGACY_VOLATILITY,
            access_count: 0,
            created_at: TimestampMillis::new(1),
            last_accessed_at: TimestampMillis::new(1),
        }
    }

    fn engine(space: &'static str, fail_text: Option<&'static str>) -> Engine<'static> {
        Engine {
            space,
            dimensions: EmbeddingDimensions::D768,
            calls: AtomicUsize::new(0),
            fail_text,
            during_embed: std::sync::Mutex::new(None),
        }
    }

    fn space(database: &Database, texts: &[&str]) -> MemorySpaceSnapshot {
        database
            .create(MemorySpaceSnapshot {
                id: lettuce_types::MemorySpaceId::new(),
                revision: Revision::INITIAL,
                items: texts.iter().map(|text| item(text)).collect(),
            })
            .expect("space")
    }

    fn backfill(
        engine: &Engine<'_>,
        database: &Database,
        memory: &MemorySpaceSnapshot,
    ) -> MemoryEmbeddingBackfill {
        embed_missing_memories(
            engine,
            database,
            memory,
            &CancellationToken::new(),
            TimestampMillis::new(2),
        )
        .expect("backfill")
    }

    #[test]
    fn a_model_switch_re_embeds_a_space_when_it_is_needed_and_keeps_the_old_vectors() {
        let database = Database::open_in_memory().expect("database");
        let memory = space(&database, &["tea by the harbor", "a quiet mill"]);
        let v4 = engine("v4", None);
        assert_eq!(backfill(&v4, &database, &memory).embedded, 2);
        assert_eq!(
            backfill(&v4, &database, &memory),
            MemoryEmbeddingBackfill::default()
        );

        let eidos = engine("v5", Some("a quiet mill"));
        let outcome = backfill(&eidos, &database, &memory);
        assert_eq!((outcome.embedded, outcome.failed), (1, 1));
        assert_eq!(
            database
                .list_repairs(memory.id, "v5", EmbeddingDimensions::D768)
                .expect("pending")
                .len(),
            1
        );
        assert_eq!(
            database
                .list_ready(memory.id, "v4", EmbeddingDimensions::D768)
                .expect("v4 kept")
                .len(),
            2
        );
        assert_eq!(
            backfill(&engine("v5", None), &database, &memory).embedded,
            1
        );
        let recounted = MemoryRepository::get(&database, memory.id)
            .expect("space")
            .expect("present");
        assert!(recounted.items.iter().all(|item| item.token_count == 1));
        assert_eq!(recounted.revision, memory.revision);

        let cancelled = CancellationToken::new();
        cancelled.cancel();
        let fresh = space(&database, &["unembedded"]);
        assert_eq!(
            embed_missing_memories(
                &eidos,
                &database,
                &fresh,
                &cancelled,
                TimestampMillis::new(3)
            ),
            Err(MemoryEmbeddingBackfillError::Cancelled)
        );
    }

    #[test]
    fn a_memory_edited_while_it_is_embedded_is_skipped_and_embedded_by_the_next_retrieval() {
        let database = Database::open_in_memory().expect("database");
        let snapshot = space(&database, &["the first wording"]);
        let memory_id = snapshot.items[0].id;
        let space = snapshot.id;
        let v5 = Engine {
            during_embed: std::sync::Mutex::new(Some(Box::new(|| {
                let current = MemoryRepository::get(&database, space)
                    .expect("space")
                    .expect("present");
                let mut items = current.items.clone();
                items[0].text = "the edited wording".to_owned();
                database
                    .compare_and_apply(lettuce_memory::MemoryChangeSet {
                        space_id: space,
                        expected_revision: current.revision,
                        items,
                    })
                    .expect("edit");
            }))),
            ..engine("v5", None)
        };
        assert_eq!(backfill(&v5, &database, &snapshot).superseded, 1);
        assert!(
            database
                .list_ready(space, "v5", EmbeddingDimensions::D768)
                .expect("ready")
                .iter()
                .all(|projection| projection.source_text == "the edited wording")
        );
        let edited = MemoryRepository::get(&database, space)
            .expect("space")
            .expect("present");
        assert_eq!(edited.items[0].token_count, 2);
        assert_eq!(
            backfill(&engine("v5", None), &database, &edited).embedded,
            1
        );
        let ready = database
            .list_ready(space, "v5", EmbeddingDimensions::D768)
            .expect("ready");
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].memory_id, memory_id);
        assert_eq!(ready[0].source_text, "the edited wording");
        assert_eq!(
            MemoryRepository::get(&database, space)
                .expect("space")
                .expect("present")
                .items[0]
                .token_count,
            1
        );
    }
}
