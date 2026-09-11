use std::collections::{HashMap, HashSet};

use lettuce_embeddings::{
    EmbeddingDimensions, EmbeddingProjectionError, EmbeddingRequest, MemoryEmbeddingProjection,
    MemoryEmbeddingRepair, MemoryEmbeddingRepository,
};
use lettuce_jobs::{Claim, ResourceClass, handle::JobHandle};
use lettuce_memory::{
    CreateMemoryPreparation, DynamicMemoryToolCallEvidence, MemoryBatchResult, MemorySpaceSnapshot,
    MemoryToolArguments, MemoryToolError, MemoryToolOutcome,
};
use lettuce_types::{MemoryId, MemorySpaceId, TimestampMillis, ToolExecutionId};

use crate::{EmbeddingGenerationError, EmbeddingService, MemoryEmbeddingEngine};

const TOOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedMemoryCreate {
    pub execution_id: ToolExecutionId,
    pub preparation: CreateMemoryPreparation,
    pub projection: Option<PreparedMemoryProjection>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PreparedMemoryProjection {
    Ready(MemoryEmbeddingProjection),
    RepairNeeded(MemoryEmbeddingRepair),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryCreateSeed {
    pub execution_id: ToolExecutionId,
    pub id: MemoryId,
    pub token_count: u32,
    pub created_at: TimestampMillis,
}

pub(crate) fn persist_created_projections<R: MemoryEmbeddingRepository + ?Sized>(
    repository: &R,
    stored: &MemorySpaceSnapshot,
    reduction: &MemoryBatchResult,
    prepared_creates: &[PreparedMemoryCreate],
) -> Vec<MemoryId> {
    let created = reduction
        .results
        .iter()
        .filter_map(|result| match result.outcome {
            MemoryToolOutcome::Created { id } => Some((result.execution_id, id)),
            _ => None,
        })
        .collect::<HashMap<_, _>>();
    let mut pending = Vec::new();
    for prepared in prepared_creates {
        let Some(&created_id) = created.get(&prepared.execution_id) else {
            continue;
        };
        if !stored.items.iter().any(|item| item.id == created_id) {
            continue;
        }
        let Some(projection) = &prepared.projection else {
            continue;
        };
        let result = match projection {
            PreparedMemoryProjection::Ready(projection) => {
                repository.put_ready(projection.clone()).or_else(|_| {
                    repository.mark_repair_needed(MemoryEmbeddingRepair {
                        space_id: projection.space_id,
                        memory_id: projection.memory_id,
                        source_text: projection.source_text.clone(),
                        source_revision: projection.vector.source_revision.clone(),
                        dimensions: projection.dimensions,
                        updated_at: projection.updated_at,
                    })
                })
            }
            PreparedMemoryProjection::RepairNeeded(repair) => {
                repository.mark_repair_needed(repair.clone())
            }
        };
        if result.is_err() {
            pending.push(created_id);
        }
    }
    pending
}

#[derive(Debug)]
pub struct DynamicMemoryCreatePreparer<
    'a,
    E: MemoryEmbeddingEngine + ?Sized,
    R: MemoryEmbeddingRepository + ?Sized,
> {
    engine: &'a E,
    repository: &'a R,
}

impl<'a, E: MemoryEmbeddingEngine + ?Sized, R: MemoryEmbeddingRepository + ?Sized>
    DynamicMemoryCreatePreparer<'a, E, R>
{
    #[must_use]
    pub const fn new(engine: &'a E, repository: &'a R) -> Self {
        Self { engine, repository }
    }

    pub fn prepare_background_calls(
        &self,
        space_id: MemorySpaceId,
        calls: &[DynamicMemoryToolCallEvidence],
        seeds: &[MemoryCreateSeed],
        duplicate_threshold: lettuce_memory::Score,
        claim: &Claim,
        handle: &JobHandle,
    ) -> Result<Vec<PreparedMemoryCreate>, DynamicMemoryPreparationError> {
        if calls.is_empty() {
            return Err(DynamicMemoryPreparationError::InvalidExecution);
        }
        let run_id = calls[0].run_id;
        let attempt_id = calls[0].attempt_id;
        let round_ordinal = calls[0].round_ordinal;
        let mut previous = None;
        let mut ids = HashSet::with_capacity(calls.len());
        for call in calls {
            call.validate()
                .map_err(|_| DynamicMemoryPreparationError::InvalidExecution)?;
            if call.run_id != run_id
                || call.attempt_id != attempt_id
                || call.round_ordinal != round_ordinal
                || call.definition_version != TOOL_VERSION
                || previous.is_some_and(|ordinal| call.ordinal != ordinal + 1)
                || !ids.insert(call.id)
            {
                return Err(DynamicMemoryPreparationError::InvalidExecution);
            }
            previous = Some(call.ordinal);
        }
        self.prepare_call_values(
            space_id,
            calls.iter().map(|call| {
                (
                    call.id,
                    call.definition_version,
                    call.call.name.as_str(),
                    &call.call.arguments,
                )
            }),
            seeds,
            duplicate_threshold,
            claim,
            handle,
        )
    }

    fn prepare_call_values<'b>(
        &self,
        space_id: MemorySpaceId,
        calls: impl Iterator<Item = (ToolExecutionId, u32, &'b str, &'b serde_json::Value)>,
        seeds: &[MemoryCreateSeed],
        duplicate_threshold: lettuce_memory::Score,
        claim: &Claim,
        handle: &JobHandle,
    ) -> Result<Vec<PreparedMemoryCreate>, DynamicMemoryPreparationError> {
        validate_embedding_admission(claim, handle)?;
        let calls = calls.collect::<Vec<_>>();
        let cancellation = handle.cancellation_token();
        let source_revision = self.engine.source_revision();
        let existing = self
            .repository
            .list_ready(space_id, source_revision, EmbeddingDimensions::D128)?
            .into_iter()
            .map(|projection| (projection.memory_id, projection.vector))
            .collect::<Vec<_>>();
        let seed_count = seeds.len();
        let mut seeds = seeds
            .iter()
            .map(|seed| (seed.execution_id, *seed))
            .collect::<HashMap<_, _>>();
        if seeds.len() != seed_count
            || seeds.len()
                != calls
                    .iter()
                    .filter(|(_, _, name, _)| *name == "create_memory")
                    .count()
        {
            return Err(DynamicMemoryPreparationError::InvalidSeeds);
        }
        let mut prepared = Vec::with_capacity(seeds.len());
        for (execution_id, definition_version, name, value) in calls {
            if definition_version != TOOL_VERSION {
                return Err(DynamicMemoryPreparationError::InvalidExecution);
            }
            let arguments = MemoryToolArguments::parse(name, value)?;
            let MemoryToolArguments::CreateMemory { text, .. } = arguments else {
                continue;
            };
            let seed = seeds
                .remove(&execution_id)
                .ok_or(DynamicMemoryPreparationError::InvalidSeeds)?;
            if cancellation.is_cancelled() {
                return Err(DynamicMemoryPreparationError::Cancelled);
            }
            let generated = self.engine.embed_memory(
                &EmbeddingRequest {
                    text: text.clone(),
                    dimensions: EmbeddingDimensions::D128,
                },
                &cancellation,
            );
            let (semantic_duplicate, projection) = match generated {
                Ok(vector) => (
                    EmbeddingService::semantic_duplicate_evidence(
                        &vector,
                        &existing,
                        duplicate_threshold,
                    ),
                    PreparedMemoryProjection::Ready(MemoryEmbeddingProjection {
                        space_id,
                        memory_id: seed.id,
                        source_text: text,
                        vector,
                        dimensions: EmbeddingDimensions::D128,
                        updated_at: seed.created_at,
                    }),
                ),
                Err(EmbeddingGenerationError::Cancelled) => {
                    return Err(DynamicMemoryPreparationError::Cancelled);
                }
                Err(EmbeddingGenerationError::Unavailable) => (
                    None,
                    PreparedMemoryProjection::RepairNeeded(MemoryEmbeddingRepair {
                        space_id,
                        memory_id: seed.id,
                        source_text: text,
                        source_revision: source_revision.to_owned(),
                        dimensions: EmbeddingDimensions::D128,
                        updated_at: seed.created_at,
                    }),
                ),
            };
            prepared.push(PreparedMemoryCreate {
                execution_id,
                preparation: CreateMemoryPreparation {
                    id: seed.id,
                    token_count: seed.token_count,
                    created_at: seed.created_at,
                    semantic_duplicate,
                },
                projection: Some(projection),
            });
        }
        if !seeds.is_empty() {
            return Err(DynamicMemoryPreparationError::InvalidSeeds);
        }
        Ok(prepared)
    }
}

fn validate_embedding_admission(
    claim: &Claim,
    handle: &JobHandle,
) -> Result<(), DynamicMemoryPreparationError> {
    let required = [
        ResourceClass::ModelLoad,
        ResourceClass::DiskRead,
        ResourceClass::Cpu,
    ];
    if claim.claim.job_id != handle.id()
        || claim.cancellation_policy != lettuce_jobs::CancellationPolicy::Cooperative
        || required
            .iter()
            .any(|resource| !claim.resources.contains(resource))
    {
        return Err(DynamicMemoryPreparationError::InvalidAdmission);
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum DynamicMemoryPreparationError {
    #[error("embedding job admission is invalid")]
    InvalidAdmission,
    #[error("dynamic-memory execution is invalid or not running")]
    InvalidExecution,
    #[error("dynamic-memory create seeds are invalid")]
    InvalidSeeds,
    #[error("dynamic-memory preparation was cancelled")]
    Cancelled,
    #[error("dynamic-memory tool call is invalid: {0}")]
    Tool(#[from] MemoryToolError),
    #[error("embedding projection repository failed: {0}")]
    Projection(#[from] EmbeddingProjectionError),
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::ProposedToolCall;
    use lettuce_embeddings::{
        EmbeddingDimensions, EmbeddingRequest, EmbeddingVector, MemoryEmbeddingProjection,
        MemoryEmbeddingRepository,
    };
    use lettuce_jobs::{
        AttemptNo, CancellationPolicy, Claim, ClaimRef, LeaseId, OutcomeRef, RecoveryPolicy,
        ResourceClass, WorkerId, handle::CancellationToken, handle::JobHandle,
    };
    use lettuce_memory::{
        DynamicMemoryToolCallEvidence, MemoryCategory, MemoryItem, MemoryRepository,
        MemorySpaceSnapshot, Score,
    };
    use lettuce_types::{
        DynamicMemoryAttemptId, DynamicMemoryRunId, JobId, MemoryId, MemorySpaceId, Revision,
        TimestampMillis, ToolExecutionId,
    };
    use serde_json::json;

    use super::{
        DynamicMemoryCreatePreparer, DynamicMemoryPreparationError, MemoryCreateSeed,
        PreparedMemoryCreate, PreparedMemoryProjection,
    };
    use crate::{AppBackend, EmbeddingGenerationError, MemoryEmbeddingEngine};

    struct FakeEmbeddingEngine {
        unavailable: bool,
    }

    impl MemoryEmbeddingEngine for FakeEmbeddingEngine {
        fn source_revision(&self) -> &str {
            "v4-test"
        }

        fn count_tokens(&self, text: &str) -> Result<u32, EmbeddingGenerationError> {
            u32::try_from(text.split_whitespace().count())
                .map_err(|_| EmbeddingGenerationError::Unavailable)
        }

        fn embed_memory(
            &self,
            request: &EmbeddingRequest,
            cancellation: &CancellationToken,
        ) -> Result<EmbeddingVector, EmbeddingGenerationError> {
            if cancellation.is_cancelled() {
                return Err(EmbeddingGenerationError::Cancelled);
            }
            if self.unavailable {
                return Err(EmbeddingGenerationError::Unavailable);
            }
            let mut values = vec![0.0; request.dimensions.get()];
            values[0] = 1.0;
            Ok(EmbeddingVector {
                source_revision: self.source_revision().to_owned(),
                values,
            })
        }
    }

    fn admitted_embedding_job() -> (Claim, JobHandle) {
        let id = JobId::new();
        (
            Claim {
                claim: ClaimRef {
                    job_id: id,
                    worker_id: WorkerId::new(),
                    attempt: AttemptNo::new(1),
                    lease_id: LeaseId::new(),
                },
                lease_expires_at: TimestampMillis::new(100),
                input_ref: OutcomeRef::MemoryRun(MemoryId::new()),
                recovery_policy: RecoveryPolicy::Restart,
                cancellation_policy: CancellationPolicy::Cooperative,
                resources: vec![
                    ResourceClass::ModelLoad,
                    ResourceClass::DiskRead,
                    ResourceClass::Cpu,
                ],
            },
            JobHandle::new(id),
        )
    }

    fn space(backend: &AppBackend, items: Vec<MemoryItem>) -> MemorySpaceId {
        let space_id = MemorySpaceId::new();
        MemoryRepository::create(
            backend.database(),
            MemorySpaceSnapshot {
                id: space_id,
                revision: Revision::INITIAL,
                items,
            },
        )
        .expect("space");
        space_id
    }

    fn create_call(text: &str) -> DynamicMemoryToolCallEvidence {
        DynamicMemoryToolCallEvidence {
            id: ToolExecutionId::new(),
            run_id: DynamicMemoryRunId::new(),
            attempt_id: DynamicMemoryAttemptId::new(),
            round_ordinal: 0,
            ordinal: 0,
            definition_version: 1,
            call: ProposedToolCall {
                provider_call_id: Some("call-0".to_owned()),
                name: "create_memory".to_owned(),
                arguments: json!({"text": text, "category": "preference"}),
                raw_arguments: None,
                provider_replay: None,
            },
            admitted_at: TimestampMillis::new(1),
        }
    }

    fn prepare(
        backend: &AppBackend,
        engine: &FakeEmbeddingEngine,
        space_id: MemorySpaceId,
        call: &DynamicMemoryToolCallEvidence,
        cancelled: bool,
    ) -> Result<Vec<PreparedMemoryCreate>, DynamicMemoryPreparationError> {
        let (claim, handle) = admitted_embedding_job();
        if cancelled {
            handle.request_cancel();
        }
        DynamicMemoryCreatePreparer::new(engine, backend.database()).prepare_background_calls(
            space_id,
            std::slice::from_ref(call),
            &[MemoryCreateSeed {
                execution_id: call.id,
                id: MemoryId::new(),
                token_count: 4,
                created_at: TimestampMillis::new(4),
            }],
            Score::from_basis_points(9_000).expect("score"),
            &claim,
            &handle,
        )
    }

    #[test]
    fn available_embedding_prepares_a_ready_projection() {
        let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let space_id = space(&backend, Vec::new());
        let engine = FakeEmbeddingEngine { unavailable: false };
        let prepared = prepare(
            &backend,
            &engine,
            space_id,
            &create_call("Mira prefers tea"),
            false,
        )
        .expect("prepare");
        assert_eq!(prepared.len(), 1);
        assert!(prepared[0].preparation.semantic_duplicate.is_none());
        assert!(matches!(
            &prepared[0].projection,
            Some(PreparedMemoryProjection::Ready(projection))
                if projection.memory_id == prepared[0].preparation.id
                    && projection.source_text == "Mira prefers tea"
        ));
    }

    #[test]
    fn unavailable_embedding_keeps_the_create_and_requests_repair() {
        let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let space_id = space(&backend, Vec::new());
        let engine = FakeEmbeddingEngine { unavailable: true };
        let prepared = prepare(
            &backend,
            &engine,
            space_id,
            &create_call("Mira prefers tea"),
            false,
        )
        .expect("prepare");
        assert!(matches!(
            &prepared[0].projection,
            Some(PreparedMemoryProjection::RepairNeeded(repair))
                if repair.memory_id == prepared[0].preparation.id
                    && repair.source_revision == "v4-test"
        ));
    }

    #[test]
    fn live_same_revision_projection_supplies_duplicate_evidence() {
        let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let existing_id = MemoryId::new();
        let space_id = space(
            &backend,
            vec![MemoryItem {
                id: existing_id,
                text: "Mira likes green tea".to_owned(),
                category: MemoryCategory::Other,
                source_message_id: None,
                source_role: None,
                observed_at: None,
                observed_time_precision: None,
                superseded_by: None,
                superseded_at: None,
                supersedes: Vec::new(),
                token_count: 3,
                is_cold: false,
                is_pinned: false,
                importance: Score::FULL,
                persistence_importance: Score::FULL,
                prompt_importance: Score::FULL,
                volatility: Score::LEGACY_VOLATILITY,
                access_count: 0,
                created_at: TimestampMillis::new(1),
                last_accessed_at: TimestampMillis::new(1),
            }],
        );
        backend
            .database()
            .put_ready(MemoryEmbeddingProjection {
                space_id,
                memory_id: existing_id,
                source_text: "Mira likes green tea".to_owned(),
                vector: EmbeddingVector {
                    source_revision: "v4-test".to_owned(),
                    values: {
                        let mut values = vec![0.0; 128];
                        values[0] = 1.0;
                        values
                    },
                },
                dimensions: EmbeddingDimensions::D128,
                updated_at: TimestampMillis::new(2),
            })
            .expect("existing projection");
        let engine = FakeEmbeddingEngine { unavailable: false };
        let prepared = prepare(
            &backend,
            &engine,
            space_id,
            &create_call("Mira strongly prefers tea"),
            false,
        )
        .expect("prepare");
        assert_eq!(
            prepared[0]
                .preparation
                .semantic_duplicate
                .as_ref()
                .map(|evidence| evidence.existing_id),
            Some(existing_id)
        );
    }

    #[test]
    fn cancelled_embedding_job_stops_before_preparation() {
        let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let space_id = space(&backend, Vec::new());
        let engine = FakeEmbeddingEngine { unavailable: false };
        assert!(matches!(
            prepare(
                &backend,
                &engine,
                space_id,
                &create_call("Mira prefers tea"),
                true
            ),
            Err(DynamicMemoryPreparationError::Cancelled)
        ));
    }
}
