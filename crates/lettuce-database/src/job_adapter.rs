use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use lettuce_jobs::{
    Claim, ClaimRef, CreateJobResult, EventSeq, ExpiredClaim, InMemoryJobStore, JobCatalog,
    JobChange, JobListFilter, JobMutation, JobQuery, JobSnapshot, JobStore, NewJob, PruneReport,
    ResourceAvailability, StoreError, StoredJobRecord, Timestamp, WorkerId,
    events::{JobEvent, JobEventEnvelope},
    retention::RetentionPolicy,
};
use lettuce_types::{JobId, Page};
use rusqlite::{
    OptionalExtension, Row, ToSql, Transaction, TransactionBehavior, params, params_from_iter,
};

use crate::{Database, decode_versioned, encode_versioned};

const JOB_FORMAT_VERSION: u32 = 1;
const JOB_EVENT_FORMAT_VERSION: u32 = 1;
const JOB_COLUMNS: &str = "id, idempotency_key, kind, subject_kind, subject_id, state, priority, \
     parent_id, lease_expires_at, created_at, updated_at, spec_json, snapshot_json";
const EVENT_COLUMNS: &str = "job_id, seq, at, correlation_id, event_json";

type JobRecords = BTreeMap<JobId, StoredJobRecord>;

#[derive(Debug)]
pub struct ManualSceneImageAdmission<'a> {
    pub spec: NewJob,
    pub request: lettuce_image_generation::ImageGenerationRequest,
    pub operation_key: &'a str,
    pub request_digest: &'a str,
    pub conversation_id: lettuce_types::ConversationId,
    pub message_id: lettuce_types::MessageId,
    pub target: lettuce_conversations::SceneFollowUpTarget,
    pub prompt: &'a str,
}

pub(crate) fn ensure_job_attempt_in(
    connection: &rusqlite::Connection,
    job_id: JobId,
    attempt: Option<u32>,
    at: Timestamp,
    allow_cancellation: bool,
) -> Result<(), StoreError> {
    let Some(attempt) = attempt else {
        return Ok(());
    };
    let encoded = connection
        .query_row(
            "SELECT snapshot_json FROM jobs WHERE id=?1",
            [job_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| StoreError::Storage)?
        .ok_or(StoreError::NotFound)?;
    let snapshot = decode_versioned::<JobSnapshot>(&encoded, JOB_FORMAT_VERSION)
        .map_err(|_| StoreError::Storage)?;
    if snapshot.attempt.get() != attempt
        || snapshot.claim.is_none()
        || (snapshot.state != lettuce_jobs::JobState::Running
            && !(allow_cancellation
                && snapshot.state == lettuce_jobs::JobState::CancellationRequested))
        || (snapshot.cancellation.requested && !allow_cancellation)
        || snapshot.lease_expires_at.is_none_or(|expires| expires < at)
    {
        return Err(StoreError::StaleLease);
    }
    Ok(())
}

impl Database {
    pub fn admit_memory_job_with_detail_result(
        &self,
        conversation_id: lettuce_types::ConversationId,
        branch_id: lettuce_types::ConversationBranchId,
        spec: NewJob,
        operation_key: &str,
        request_digest: &str,
        detail: &serde_json::Value,
    ) -> Result<CreateJobResult, StoreError> {
        let mut connection = self.connection.lock().map_err(|_| StoreError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| StoreError::Storage)?;
        let present: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM conversation_branches branch
                JOIN conversations conversation ON conversation.id = branch.conversation_id
               WHERE branch.conversation_id = ?1 AND branch.id = ?2
                 AND branch.status <> 'tombstoned' AND conversation.lifecycle <> 'tombstoned')",
                params![conversation_id.to_string(), branch_id.to_string()],
                |row| row.get(0),
            )
            .map_err(|_| StoreError::Storage)?;
        if !present {
            return Err(StoreError::NotFound);
        }
        let key = spec
            .idempotency_key
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default();
        let active: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM jobs
                  WHERE kind = 'memory_extraction' AND subject_kind = 'conversation'
                    AND subject_id = ?1
                    AND state NOT IN ('succeeded', 'failed', 'cancelled', 'interrupted')
                    AND coalesce(idempotency_key, '') <> ?2)",
                params![conversation_id.to_string(), key],
                |row| row.get(0),
            )
            .map_err(|_| StoreError::Storage)?;
        if active {
            return Err(StoreError::AlreadyActive);
        }
        let (job, _, created) =
            admit_job_detail_in(&transaction, spec, operation_key, request_digest, detail)?;
        transaction.commit().map_err(|_| StoreError::Storage)?;
        Ok(CreateJobResult { job, created })
    }

    /// Publishes a terminal job and its result together. A crash cannot leave
    /// a known external result attached to a still-running job.
    pub fn settle_job_with_detail(
        &self,
        mutation: JobMutation,
        result: Option<&serde_json::Value>,
        failure: Option<&serde_json::Value>,
    ) -> Result<JobSnapshot, StoreError> {
        if result.is_some_and(|value| !value.is_object())
            || failure.is_some_and(|value| !value.is_object())
        {
            return Err(StoreError::InvalidData);
        }
        let id = mutation.job_id();
        let mut connection = self.connection.lock().map_err(|_| StoreError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| StoreError::Storage)?;
        let before = load_with_children(&transaction, [id])?;
        let job = apply_to_job_set(&transaction, before, |store| {
            store.append_and_transition(mutation)
        })?;
        if !job.state.is_terminal() {
            return Err(StoreError::InvalidData);
        }
        let changed = transaction
            .execute(
                "UPDATE job_details SET result_json=?2, failure_json=?3 WHERE job_id=?1",
                params![
                    id.to_string(),
                    result.map(ToString::to_string),
                    failure.map(ToString::to_string)
                ],
            )
            .map_err(|_| StoreError::Storage)?;
        if changed != 1 {
            return Err(StoreError::InvalidData);
        }
        transaction.commit().map_err(|_| StoreError::Storage)?;
        Ok(job)
    }

    pub fn create_or_get_with_local_model_detail(
        &self,
        spec: NewJob,
        detail: &serde_json::Value,
    ) -> Result<CreateJobResult, StoreError> {
        use rusqlite::OptionalExtension;
        spec.validate()?;
        if !detail.is_object() {
            return Err(StoreError::InvalidData);
        }
        let mut connection = self.connection.lock().map_err(|_| StoreError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| StoreError::Storage)?;
        let before = creation_set(&transaction, &spec)?;
        let created = apply_to_job_set(&transaction, before, |store| store.create_or_get(spec))?;
        let stored: Option<String> = transaction
            .query_row(
                "SELECT detail_json FROM local_model_jobs WHERE job_id=?1",
                [created.job.id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| StoreError::Storage)?;
        if let Some(stored) = stored {
            let stored: serde_json::Value =
                serde_json::from_str(&stored).map_err(|_| StoreError::InvalidData)?;
            if &stored != detail {
                return Err(StoreError::IdempotencyConflict);
            }
        } else {
            if !created.created {
                return Err(StoreError::InvalidData);
            }
            let text = serde_json::to_string(detail).map_err(|_| StoreError::InvalidData)?;
            transaction
                .execute(
                    "INSERT INTO local_model_jobs(job_id,detail_json) VALUES (?1,?2)",
                    params![created.job.id.to_string(), text],
                )
                .map_err(|_| StoreError::Storage)?;
        }
        transaction.commit().map_err(|_| StoreError::Storage)?;
        Ok(created)
    }

    pub fn admit_job_with_detail_result(
        &self,
        spec: NewJob,
        operation_key: &str,
        request_digest: &str,
        detail: &serde_json::Value,
    ) -> Result<CreateJobResult, StoreError> {
        let mut connection = self.connection.lock().map_err(|_| StoreError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| StoreError::Storage)?;
        let (job, _, created) =
            admit_job_detail_in(&transaction, spec, operation_key, request_digest, detail)?;
        transaction.commit().map_err(|_| StoreError::Storage)?;
        Ok(CreateJobResult { job, created })
    }

    pub fn admit_job_with_detail(
        &self,
        spec: NewJob,
        operation_key: &str,
        request_digest: &str,
        detail: &serde_json::Value,
    ) -> Result<JobSnapshot, StoreError> {
        let mut connection = self.connection.lock().map_err(|_| StoreError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| StoreError::Storage)?;
        let (job, _, _) =
            admit_job_detail_in(&transaction, spec, operation_key, request_digest, detail)?;
        transaction.commit().map_err(|_| StoreError::Storage)?;
        Ok(job)
    }

    pub fn admit_speech_synthesis(
        &self,
        spec: NewJob,
        operation_key: &str,
        request_digest: &str,
        request: lettuce_speech::SynthesisRequest,
    ) -> Result<JobSnapshot, StoreError> {
        request.validate().map_err(|_| StoreError::InvalidData)?;
        let mut connection = self.connection.lock().map_err(|_| StoreError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| StoreError::Storage)?;
        let detail = serde_json::json!({"kind": "speech_synthesize"});
        let (job, replayed, _) =
            admit_job_detail_in(&transaction, spec, operation_key, request_digest, &detail)?;
        if !replayed {
            let record = lettuce_speech::SynthesisRecord {
                job_id: job.id,
                request,
                state: lettuce_speech::SynthesisState::Pending,
            };
            crate::media::tts_synthesis_adapter::insert_restored_in(&transaction, &record)
                .map_err(|_| StoreError::Storage)?;
        }
        transaction.commit().map_err(|_| StoreError::Storage)?;
        Ok(job)
    }

    pub fn reuse_speech_synthesis(
        &self,
        operation_key: &str,
        request_digest: &str,
        reuse_key: &lettuce_speech::SynthesisReuseKey,
        now: lettuce_types::TimestampMillis,
    ) -> Result<Option<JobSnapshot>, StoreError> {
        use rusqlite::OptionalExtension;
        if operation_key.trim().is_empty() || request_digest.is_empty() {
            return Err(StoreError::InvalidData);
        }
        let mut connection = self.connection.lock().map_err(|_| StoreError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| StoreError::Storage)?;
        let prior = transaction
            .query_row(
                "SELECT request_digest,job_id FROM job_operations WHERE operation_key=?1",
                [operation_key],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|_| StoreError::Storage)?;
        let job = if let Some((digest, id)) = prior {
            if digest != request_digest {
                return Err(StoreError::IdempotencyConflict);
            }
            let id = id.parse().map_err(|_| StoreError::InvalidData)?;
            Some(
                select_ids(&transaction, [id])?
                    .remove(&id)
                    .ok_or(StoreError::InvalidData)?
                    .snapshot,
            )
        } else if let Some(record) =
            crate::media::tts_synthesis_adapter::find_reusable_in(&transaction, reuse_key, now)
                .map_err(|_| StoreError::Storage)?
        {
            let job = select_ids(&transaction, [record.job_id])?
                .remove(&record.job_id)
                .ok_or(StoreError::InvalidData)?
                .snapshot;
            if job.state == lettuce_jobs::JobState::Succeeded {
                transaction.execute("INSERT INTO job_operations(operation_key,request_digest,job_id) VALUES (?1,?2,?3)",
                    params![operation_key, request_digest, job.id.to_string()]).map_err(|_| StoreError::Storage)?;
                Some(job)
            } else {
                None
            }
        } else {
            None
        };
        transaction.commit().map_err(|_| StoreError::Storage)?;
        Ok(job)
    }

    pub fn admit_speech_transcription(
        &self,
        spec: NewJob,
        operation_key: &str,
        request_digest: &str,
        detail: &serde_json::Value,
        request: lettuce_speech::TranscriptionRequest,
    ) -> Result<JobSnapshot, StoreError> {
        request.validate().map_err(|_| StoreError::InvalidData)?;
        let mut connection = self.connection.lock().map_err(|_| StoreError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| StoreError::Storage)?;
        let (job, replayed, _) =
            admit_job_detail_in(&transaction, spec, operation_key, request_digest, detail)?;
        if !replayed {
            let record = lettuce_speech::TranscriptionRecord {
                job_id: job.id,
                request,
                state: lettuce_speech::TranscriptionState::Pending,
            };
            crate::media::speech_adapter::insert_restored_in(&transaction, &record)
                .map_err(|_| StoreError::Storage)?;
        }
        transaction.commit().map_err(|_| StoreError::Storage)?;
        Ok(job)
    }

    pub fn admit_manual_scene_image(
        &self,
        admission: ManualSceneImageAdmission<'_>,
    ) -> Result<JobSnapshot, StoreError> {
        let ManualSceneImageAdmission {
            spec,
            request,
            operation_key,
            request_digest,
            conversation_id,
            message_id,
            target,
            prompt,
        } = admission;
        let mut connection = self.connection.lock().map_err(|_| StoreError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| StoreError::Storage)?;
        let detail = serde_json::json!({"kind": "scene_image", "conversation_id": conversation_id, "message_id": message_id, "target": target});
        let (job, replayed, _) =
            admit_job_detail_in(&transaction, spec, operation_key, request_digest, &detail)?;
        if !replayed {
            use lettuce_conversations::{SceneFollowUp, SceneFollowUpMode, SceneFollowUpState};
            let current = crate::conversation::scene_follow_up_adapter::one(
                &transaction,
                conversation_id,
                target,
            )
            .map_err(|_| StoreError::Storage)?;
            if current
                .as_ref()
                .is_some_and(|current| current.state.generating())
            {
                return Err(StoreError::ResourceUnavailable);
            }
            let now = request.created_at;
            let follow_up = SceneFollowUp {
                conversation_id,
                message_id,
                target,
                prompt: prompt.to_owned(),
                mode: SceneFollowUpMode::Manual,
                state: SceneFollowUpState::Approved,
                generation: match &current {
                    Some(current) => current
                        .generation
                        .checked_add(1)
                        .ok_or(StoreError::InvalidData)?,
                    None => 1,
                },
                attempt: 1,
                request_id: Some(request.id),
                failure: None,
                created_at: current.as_ref().map_or(now, |current| current.created_at),
                updated_at: current
                    .as_ref()
                    .map_or(now, |current| now.max(current.updated_at)),
            };
            if current.is_some() {
                transaction.execute("UPDATE scene_image_follow_ups SET prompt = ?4, mode = 'manual', state = 'approved', generation = ?5, attempt = 1, request_id = ?6, failure = NULL, updated_at = ?7 WHERE conversation_id = ?1 AND target_kind = ?2 AND target_id = ?3", params![conversation_id.to_string(), target.kind(), target.id(), follow_up.prompt, follow_up.generation, request.id.to_string(), follow_up.updated_at.get()]).map_err(|_| StoreError::Storage)?;
            } else {
                crate::conversation::scene_follow_up_adapter::insert_restored_in(
                    &transaction,
                    &follow_up,
                )
                .map_err(|_| StoreError::Storage)?;
            }
            let record = lettuce_image_generation::ImageGenerationRecord {
                job_id: job.id,
                request,
                state: lettuce_image_generation::ImageGenerationState::Pending,
            };
            crate::media::image_generation_adapter::insert_pending_row(&transaction, &record)
                .map_err(|_| StoreError::Storage)?;
        }
        transaction.commit().map_err(|_| StoreError::Storage)?;
        Ok(job)
    }

    fn read_job_rows<R>(
        &self,
        operation: impl FnOnce(&Transaction<'_>) -> Result<R, StoreError>,
    ) -> Result<R, StoreError> {
        let mut connection = self.connection.lock().map_err(|_| StoreError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(|_| StoreError::Storage)?;
        let result = operation(&transaction)?;
        transaction.commit().map_err(|_| StoreError::Storage)?;
        Ok(result)
    }

    /// Runs `operation` against only the jobs `select` loads, each with its
    /// latest event, and writes back what the operation changed.
    fn write_job_set<R>(
        &self,
        select: impl FnOnce(&Transaction<'_>) -> Result<JobRecords, StoreError>,
        operation: impl FnOnce(&InMemoryJobStore) -> Result<R, StoreError>,
    ) -> Result<R, StoreError> {
        let mut connection = self.connection.lock().map_err(|_| StoreError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| StoreError::Storage)?;
        let before = select(&transaction)?;
        let result = apply_to_job_set(&transaction, before, operation)?;
        transaction.commit().map_err(|_| StoreError::Storage)?;
        Ok(result)
    }
}

fn admit_job_detail_in(
    transaction: &Transaction<'_>,
    spec: NewJob,
    operation_key: &str,
    request_digest: &str,
    detail: &serde_json::Value,
) -> Result<(JobSnapshot, bool, bool), StoreError> {
    spec.validate()?;
    if operation_key.trim().is_empty() || request_digest.is_empty() || !detail.is_object() {
        return Err(StoreError::InvalidData);
    }
    use rusqlite::OptionalExtension;
    let prior: Option<(String, String)> = transaction
        .query_row(
            "SELECT request_digest, job_id FROM job_operations WHERE operation_key = ?1",
            [operation_key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| StoreError::Storage)?;
    if let Some((digest, job_id)) = prior {
        if digest != request_digest {
            return Err(StoreError::IdempotencyConflict);
        }
        let id = job_id.parse().map_err(|_| StoreError::InvalidData)?;
        return Ok((
            select_ids(transaction, [id])?
                .remove(&id)
                .ok_or(StoreError::InvalidData)?
                .snapshot,
            true,
            false,
        ));
    }
    let before = creation_set(transaction, &spec)?;
    let admitted = apply_to_job_set(transaction, before, |store| store.create_or_get(spec))?;
    let stored: Option<String> = transaction
        .query_row(
            "SELECT detail_json FROM job_details WHERE job_id = ?1",
            [admitted.job.id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| StoreError::Storage)?;
    if let Some(stored) = stored {
        let value: serde_json::Value =
            serde_json::from_str(&stored).map_err(|_| StoreError::InvalidData)?;
        if value != *detail {
            return Err(StoreError::IdempotencyConflict);
        }
    }
    transaction.execute("INSERT INTO job_details (job_id, detail_json) VALUES (?1, ?2) ON CONFLICT(job_id) DO NOTHING", params![admitted.job.id.to_string(), detail.to_string()]).map_err(|_| StoreError::Storage)?;
    transaction.execute("INSERT INTO job_operations (operation_key, request_digest, job_id) VALUES (?1, ?2, ?3)", params![operation_key, request_digest, admitted.job.id.to_string()]).map_err(|_| StoreError::Storage)?;
    Ok((admitted.job, false, admitted.created))
}

fn apply_to_job_set<R>(
    transaction: &Transaction<'_>,
    before: JobRecords,
    operation: impl FnOnce(&InMemoryJobStore) -> Result<R, StoreError>,
) -> Result<R, StoreError> {
    let store = InMemoryJobStore::restore_working_set(before.values().cloned().collect())?;
    let result = operation(&store)?;
    persist_changes(transaction, &before, &records_by_id(store.stored_records()))?;
    Ok(result)
}

pub(crate) fn records_by_id(records: Vec<StoredJobRecord>) -> BTreeMap<JobId, StoredJobRecord> {
    records
        .into_iter()
        .map(|record| (record.snapshot.id, record))
        .collect()
}

fn select_rows(
    transaction: &Transaction<'_>,
    filter: &str,
    values: &[&dyn ToSql],
) -> Result<JobRecords, StoreError> {
    let mut statement = transaction
        .prepare(&format!(
            "SELECT {JOB_COLUMNS} FROM jobs WHERE {filter} ORDER BY id"
        ))
        .map_err(|_| StoreError::Storage)?;
    let mut rows = statement.query(values).map_err(|_| StoreError::Storage)?;
    let mut records = BTreeMap::new();
    while let Some(row) = rows.next().map_err(|_| StoreError::Storage)? {
        let record = decode_job_row(row)?;
        if records.insert(record.snapshot.id, record).is_some() {
            return Err(StoreError::InvalidData);
        }
    }
    Ok(records)
}

fn select_ids(
    transaction: &Transaction<'_>,
    ids: impl IntoIterator<Item = JobId>,
) -> Result<JobRecords, StoreError> {
    let ids = ids
        .into_iter()
        .map(|id| id.to_string())
        .collect::<BTreeSet<_>>();
    if ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let mut statement = transaction
        .prepare(&format!(
            "SELECT {JOB_COLUMNS} FROM jobs WHERE id IN ({placeholders}) ORDER BY id"
        ))
        .map_err(|_| StoreError::Storage)?;
    let mut rows = statement
        .query(params_from_iter(ids.iter()))
        .map_err(|_| StoreError::Storage)?;
    let mut records = BTreeMap::new();
    while let Some(row) = rows.next().map_err(|_| StoreError::Storage)? {
        let record = decode_job_row(row)?;
        records.insert(record.snapshot.id, record);
    }
    Ok(records)
}

/// Loads `ids` with the children each loaded job settles against, every
/// loaded job carrying its latest event.
fn load_with_children(
    transaction: &Transaction<'_>,
    ids: impl IntoIterator<Item = JobId>,
) -> Result<JobRecords, StoreError> {
    let mut records = select_ids(transaction, ids)?;
    let children = records
        .values()
        .flat_map(|record| record.snapshot.children.iter().map(|child| child.child_id))
        .filter(|id| !records.contains_key(id))
        .collect::<Vec<_>>();
    records.extend(select_ids(transaction, children)?);
    with_latest_events(transaction, records)
}

fn with_latest_events(
    transaction: &Transaction<'_>,
    mut records: JobRecords,
) -> Result<JobRecords, StoreError> {
    let mut statement = transaction
        .prepare(&format!(
            "SELECT {EVENT_COLUMNS} FROM job_events WHERE job_id=?1 ORDER BY seq DESC LIMIT 1"
        ))
        .map_err(|_| StoreError::Storage)?;
    for (id, record) in &mut records {
        let mut rows = statement
            .query([id.to_string()])
            .map_err(|_| StoreError::Storage)?;
        let row = rows
            .next()
            .map_err(|_| StoreError::Storage)?
            .ok_or(StoreError::InvalidData)?;
        let event = decode_event_row(row)?;
        if event.job_id != *id {
            return Err(StoreError::InvalidData);
        }
        record.events = vec![event];
    }
    Ok(records)
}

fn decode_job_row(row: &Row<'_>) -> Result<StoredJobRecord, StoreError> {
    let id = row.get::<_, String>(0).map_err(|_| StoreError::Storage)?;
    let idempotency_key = row
        .get::<_, Option<String>>(1)
        .map_err(|_| StoreError::Storage)?;
    let kind = row.get::<_, String>(2).map_err(|_| StoreError::Storage)?;
    let subject_kind = row.get::<_, String>(3).map_err(|_| StoreError::Storage)?;
    let subject_id = row.get::<_, String>(4).map_err(|_| StoreError::Storage)?;
    let state = row.get::<_, String>(5).map_err(|_| StoreError::Storage)?;
    let priority = row.get::<_, String>(6).map_err(|_| StoreError::Storage)?;
    let parent_id = row
        .get::<_, Option<String>>(7)
        .map_err(|_| StoreError::Storage)?;
    let lease_expires_at = row
        .get::<_, Option<i64>>(8)
        .map_err(|_| StoreError::Storage)?;
    let created_at = row.get::<_, i64>(9).map_err(|_| StoreError::Storage)?;
    let updated_at = row.get::<_, i64>(10).map_err(|_| StoreError::Storage)?;
    let spec_json = row.get::<_, String>(11).map_err(|_| StoreError::Storage)?;
    let snapshot_json = row.get::<_, String>(12).map_err(|_| StoreError::Storage)?;
    let id = id.parse::<JobId>().map_err(|_| StoreError::InvalidData)?;
    let spec: NewJob =
        decode_versioned(&spec_json, JOB_FORMAT_VERSION).map_err(|()| StoreError::InvalidData)?;
    let snapshot: JobSnapshot = decode_versioned(&snapshot_json, JOB_FORMAT_VERSION)
        .map_err(|()| StoreError::InvalidData)?;
    if snapshot.id != id
        || idempotency_key != snapshot.idempotency_key.as_ref().map(ToString::to_string)
        || kind != enum_name(snapshot.kind)?
        || subject_kind != enum_name(snapshot.subject.kind)?
        || subject_id != snapshot.subject.id.as_str()
        || state != enum_name(snapshot.state)?
        || priority != enum_name(spec.priority)?
        || parent_id != snapshot.parent_id.map(|value| value.to_string())
        || lease_expires_at != snapshot.lease_expires_at.map(Timestamp::get)
        || created_at != snapshot.created_at.get()
        || updated_at != snapshot.updated_at.get()
    {
        return Err(StoreError::InvalidData);
    }
    Ok(StoredJobRecord {
        spec,
        snapshot,
        events: Vec::new(),
    })
}

fn decode_event_row(row: &Row<'_>) -> Result<JobEventEnvelope, StoreError> {
    let job_id = row.get::<_, String>(0).map_err(|_| StoreError::Storage)?;
    let seq = row.get::<_, i64>(1).map_err(|_| StoreError::Storage)?;
    let at = row.get::<_, i64>(2).map_err(|_| StoreError::Storage)?;
    let correlation_id = row.get::<_, String>(3).map_err(|_| StoreError::Storage)?;
    let event_json = row.get::<_, String>(4).map_err(|_| StoreError::Storage)?;
    let job_id = job_id
        .parse::<JobId>()
        .map_err(|_| StoreError::InvalidData)?;
    let seq = u64::try_from(seq).map_err(|_| StoreError::InvalidData)?;
    let correlation_id = uuid::Uuid::parse_str(&correlation_id)
        .map(lettuce_jobs::CorrelationId::from)
        .map_err(|_| StoreError::InvalidData)?;
    let event: JobEvent = decode_versioned(&event_json, JOB_EVENT_FORMAT_VERSION)
        .map_err(|()| StoreError::InvalidData)?;
    Ok(JobEventEnvelope {
        job_id,
        seq: EventSeq::new(seq),
        at: Timestamp::new(at),
        correlation_id,
        event,
    })
}

pub(crate) fn retry_staged_planner_job(
    transaction: &Transaction<'_>,
    previous_job_id: JobId,
    retry_id: lettuce_types::RequestId,
) -> Result<JobId, StoreError> {
    let previous = select_ids(transaction, [previous_job_id])?
        .remove(&previous_job_id)
        .ok_or(StoreError::NotFound)?;
    if previous.snapshot.kind != lettuce_jobs::JobKind::CreationRun
        || previous.snapshot.state != lettuce_jobs::JobState::Failed
    {
        return Err(StoreError::IllegalTransition);
    }
    let mut spec = previous.spec;
    spec.idempotency_key = Some(
        lettuce_jobs::IdempotencyKey::new(format!("staged-planner-retry-{retry_id}"))
            .map_err(|_| StoreError::InvalidData)?,
    );
    let before = creation_set(transaction, &spec)?;
    let created = apply_to_job_set(transaction, before, |store| store.create_or_get(spec))?;
    Ok(created.job.id)
}

pub(crate) fn cancel_creation_project_jobs(
    transaction: &Transaction<'_>,
    project_id: lettuce_types::CreationWorkflowId,
    now: Timestamp,
) -> Result<(), StoreError> {
    use lettuce_jobs::{CancellationReason, JobKind, SubjectKind};
    let subject =
        lettuce_jobs::JobSubject::new(SubjectKind::CreationProject, project_id.to_string())
            .map_err(|_| StoreError::InvalidData)?;
    let rows = select_rows(
        transaction,
        "kind=?1 AND subject_kind=?2 AND subject_id=?3",
        &[
            &enum_name(JobKind::CreationRun)?,
            &enum_name(subject.kind)?,
            &subject.id.as_str(),
        ],
    )?;
    let before = with_latest_events(transaction, rows)?;
    let open = before
        .values()
        .filter(|record| !record.snapshot.is_terminal())
        .map(|record| {
            (
                record.snapshot.id,
                now.max(record.snapshot.updated_at),
                record.snapshot.claim.is_none(),
            )
        })
        .collect::<Vec<_>>();
    apply_to_job_set(transaction, before, |store| {
        for (id, at, unclaimed) in open {
            store.append_and_transition(JobMutation::RequestCancellation {
                id,
                reason: CancellationReason::User,
                at,
            })?;
            if unclaimed {
                store.append_and_transition(JobMutation::FinishQueuedCancellation { id, at })?;
            }
        }
        Ok(())
    })
}

/// Loads the job an idempotent create would return and the parent it would
/// attach to.
fn creation_set(transaction: &Transaction<'_>, spec: &NewJob) -> Result<JobRecords, StoreError> {
    let mut records = match &spec.idempotency_key {
        Some(key) => select_rows(transaction, "idempotency_key=?1", &[&key.to_string()])?,
        None => BTreeMap::new(),
    };
    if let Some(parent_id) = spec.parent_id {
        records.extend(select_ids(transaction, [parent_id])?);
    }
    with_latest_events(transaction, records)
}

/// Loads every job with its full event history.
pub(crate) fn load_store(transaction: &Transaction<'_>) -> Result<InMemoryJobStore, StoreError> {
    let mut records = select_rows(transaction, "1=1", &[])?;
    let mut statement = transaction
        .prepare(&format!(
            "SELECT {EVENT_COLUMNS} FROM job_events ORDER BY job_id, seq"
        ))
        .map_err(|_| StoreError::Storage)?;
    let mut rows = statement.query([]).map_err(|_| StoreError::Storage)?;
    while let Some(row) = rows.next().map_err(|_| StoreError::Storage)? {
        let event = decode_event_row(row)?;
        records
            .get_mut(&event.job_id)
            .ok_or(StoreError::InvalidData)?
            .events
            .push(event);
    }
    InMemoryJobStore::restore(records.into_values().collect())
}

pub(crate) fn persist_changes(
    transaction: &Transaction<'_>,
    before: &BTreeMap<JobId, StoredJobRecord>,
    after: &BTreeMap<JobId, StoredJobRecord>,
) -> Result<(), StoreError> {
    for id in before.keys().filter(|id| !after.contains_key(id)) {
        transaction
            .execute("DELETE FROM jobs WHERE id=?1", [id.to_string()])
            .map_err(|_| StoreError::Storage)?;
    }
    for (id, record) in after {
        if before.get(id) == Some(record) {
            continue;
        }
        let spec_json = encode_versioned(&record.spec, JOB_FORMAT_VERSION)
            .map_err(|_| StoreError::InvalidData)?;
        let snapshot_json = encode_versioned(&record.snapshot, JOB_FORMAT_VERSION)
            .map_err(|_| StoreError::InvalidData)?;
        let snapshot = &record.snapshot;
        transaction
            .execute(
                "INSERT INTO jobs (id, idempotency_key, kind, subject_kind, subject_id, state, \
                 priority, parent_id, lease_expires_at, created_at, updated_at, spec_json, snapshot_json) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13) \
                 ON CONFLICT(id) DO UPDATE SET idempotency_key=excluded.idempotency_key, \
                 kind=excluded.kind, subject_kind=excluded.subject_kind, subject_id=excluded.subject_id, \
                 state=excluded.state, priority=excluded.priority, parent_id=excluded.parent_id, \
                 lease_expires_at=excluded.lease_expires_at, updated_at=excluded.updated_at, \
                 spec_json=excluded.spec_json, snapshot_json=excluded.snapshot_json",
                params![
                    id.to_string(),
                    snapshot.idempotency_key.as_ref().map(ToString::to_string),
                    enum_name(snapshot.kind)?,
                    enum_name(snapshot.subject.kind)?,
                    snapshot.subject.id.to_string(),
                    enum_name(snapshot.state)?,
                    enum_name(record.spec.priority)?,
                    snapshot.parent_id.map(|value| value.to_string()),
                    snapshot.lease_expires_at.map(Timestamp::get),
                    snapshot.created_at.get(),
                    snapshot.updated_at.get(),
                    spec_json,
                    snapshot_json,
                ],
            )
            .map_err(|_| StoreError::Storage)?;
        if snapshot.kind == lettuce_jobs::JobKind::SpeechTranscribe && snapshot.state.is_terminal()
        {
            crate::media::speech_adapter::release_input_in(
                transaction,
                *id,
                snapshot.state,
                snapshot.updated_at,
            )?;
        }
        let persisted_event_count = before.get(id).map_or(0, |stored| stored.events.len());
        if before
            .get(id)
            .is_some_and(|stored| !record.events.starts_with(&stored.events))
        {
            return Err(StoreError::InvalidData);
        }
        for event in record.events.iter().skip(persisted_event_count) {
            let event_json = encode_versioned(&event.event, JOB_EVENT_FORMAT_VERSION)
                .map_err(|_| StoreError::InvalidData)?;
            transaction
                .execute(
                    "INSERT INTO job_events (job_id, seq, at, correlation_id, event_json) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        id.to_string(),
                        i64::try_from(event.seq.get()).map_err(|_| StoreError::InvalidData)?,
                        event.at.get(),
                        event.correlation_id.to_string(),
                        event_json,
                    ],
                )
                .map_err(|_| StoreError::Storage)?;
        }
        let ended_now = snapshot.is_terminal()
            && before
                .get(id)
                .is_some_and(|stored| !stored.snapshot.is_terminal());
        if ended_now && snapshot.kind == lettuce_jobs::JobKind::MemoryExtraction {
            settle_memory_attempts_of_ended_job(transaction, snapshot)?;
        }
    }
    Ok(())
}

/// A memory job that ended (failed, cancelled or interrupted) without its
/// runner settling its attempt leaves no live attempt behind: a created
/// attempt is cancelled, a processing one cancelled with a cancelled job and
/// interrupted otherwise. A live attempt would keep its conversation busy.
fn settle_memory_attempts_of_ended_job(
    transaction: &Transaction<'_>,
    job: &JobSnapshot,
) -> Result<(), StoreError> {
    let processing_to = if job.state == lettuce_jobs::JobState::Cancelled {
        "cancelled"
    } else {
        "interrupted"
    };
    transaction
        .execute(
            "UPDATE dynamic_memory_run_attempts \
             SET status = CASE status WHEN 'created' THEN 'cancelled' ELSE ?2 END, \
                 revision = revision + 1, \
                 finished_at = max(?3, updated_at), \
                 updated_at = max(?3, updated_at) \
             WHERE job_id = ?1 AND status IN ('created', 'processing')",
            params![job.id.to_string(), processing_to, job.updated_at.get()],
        )
        .map_err(|_| StoreError::Storage)?;
    Ok(())
}

impl Database {
    /// Settles every live memory attempt of the conversation whose job has
    /// already ended, as `settle_memory_attempts_of_ended_job` does when the
    /// job ends; answers how many were settled.
    pub fn settle_memory_attempts_of_ended_jobs(
        &self,
        conversation_id: lettuce_types::ConversationId,
    ) -> Result<usize, StoreError> {
        let mut connection = self.connection.lock().map_err(|_| StoreError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| StoreError::Storage)?;
        let terminal = [
            lettuce_jobs::JobState::Succeeded,
            lettuce_jobs::JobState::Failed,
            lettuce_jobs::JobState::Cancelled,
            lettuce_jobs::JobState::Interrupted,
        ]
        .into_iter()
        .map(enum_name)
        .collect::<Result<Vec<_>, _>>()?;
        let jobs = {
            let mut statement = transaction
                .prepare(
                    "SELECT DISTINCT attempt.job_id FROM dynamic_memory_run_attempts attempt \
                     JOIN dynamic_memory_runs run ON run.id = attempt.run_id \
                     JOIN jobs job ON job.id = attempt.job_id \
                     WHERE run.conversation_id = ?1 \
                       AND attempt.status IN ('created', 'processing') \
                       AND job.state IN (?2, ?3, ?4, ?5)",
                )
                .map_err(|_| StoreError::Storage)?;
            statement
                .query_map(
                    params![
                        conversation_id.to_string(),
                        terminal[0],
                        terminal[1],
                        terminal[2],
                        terminal[3],
                    ],
                    |row| row.get::<_, String>(0),
                )
                .and_then(|rows| rows.collect::<Result<Vec<_>, _>>())
                .map_err(|_| StoreError::Storage)?
        };
        let ids = jobs
            .iter()
            .map(|id| id.parse::<JobId>().map_err(|_| StoreError::InvalidData))
            .collect::<Result<Vec<_>, _>>()?;
        let records = select_ids(&transaction, ids)?;
        for record in records.values() {
            settle_memory_attempts_of_ended_job(&transaction, &record.snapshot)?;
        }
        transaction.commit().map_err(|_| StoreError::Storage)?;
        Ok(records.len())
    }
}

fn enum_name(value: impl serde::Serialize) -> Result<String, StoreError> {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or(StoreError::InvalidData)
}

const JOB_BINDING_QUERY: &str = "SELECT job_id FROM speech_transcriptions \
     UNION SELECT job_id FROM speech_syntheses UNION SELECT job_id FROM image_generations \
     UNION SELECT job_id FROM creation_lorebook_entry_runs \
     UNION SELECT job_id FROM creation_lorebook_keyword_runs \
     UNION SELECT job_id FROM creation_staged_lorebook_runs \
     UNION SELECT job_id FROM creation_staged_lorebook_writer_runs \
     UNION SELECT job_id FROM companion_growth_runs \
     UNION SELECT job_id FROM companion_consolidation_runs \
     UNION SELECT job_id FROM companion_soul_writer_runs";

impl JobStore for Database {
    fn create_or_get(&self, spec: NewJob) -> Result<CreateJobResult, StoreError> {
        spec.validate()?;
        let lookup = spec.clone();
        self.write_job_set(
            |transaction| creation_set(transaction, &lookup),
            |store| store.create_or_get(spec),
        )
    }

    fn get(&self, id: JobId) -> Result<Option<JobSnapshot>, StoreError> {
        self.read_job_rows(|transaction| {
            Ok(select_ids(transaction, [id])?
                .remove(&id)
                .map(|record| record.snapshot))
        })
    }

    fn list(&self, query: JobQuery) -> Result<Page<JobSnapshot>, StoreError> {
        let start = query
            .page
            .cursor
            .as_deref()
            .map_or(Ok(0), str::parse::<i64>)
            .map_err(|_| StoreError::InvalidCursor)?;
        if start < 0 {
            return Err(StoreError::InvalidCursor);
        }
        let limit = i64::from(query.page.limit.get());
        let state = query.state.map(enum_name).transpose()?;
        let kind = query.kind.map(enum_name).transpose()?;
        let subject = query.subject.as_ref().map(|subject| subject.as_str());
        self.read_job_rows(|transaction| {
            let mut statement = transaction
                .prepare(&format!(
                    "SELECT {JOB_COLUMNS} FROM jobs WHERE (?1 IS NULL OR state=?1) \
                     AND (?2 IS NULL OR kind=?2) AND (?3 IS NULL OR subject_id=?3) \
                     ORDER BY created_at, id LIMIT ?4 OFFSET ?5"
                ))
                .map_err(|_| StoreError::Storage)?;
            let mut rows = statement
                .query(params![state, kind, subject, limit + 1, start])
                .map_err(|_| StoreError::Storage)?;
            let mut items = Vec::new();
            while let Some(row) = rows.next().map_err(|_| StoreError::Storage)? {
                items.push(decode_job_row(row)?.snapshot);
            }
            let has_more = items.len() > usize::from(query.page.limit.get());
            items.truncate(usize::from(query.page.limit.get()));
            let next_cursor = has_more.then(|| (start + limit).to_string());
            Ok(Page { items, next_cursor })
        })
    }

    fn events_since(
        &self,
        id: JobId,
        after: Option<EventSeq>,
        limit: u32,
    ) -> Result<Vec<JobEventEnvelope>, StoreError> {
        if limit == 0 || limit > 1_000 {
            return Err(StoreError::InvalidLimit);
        }
        let after = after.map_or(Ok(0), |seq| i64::try_from(seq.get()));
        let after = after.map_err(|_| StoreError::InvalidData)?;
        self.read_job_rows(|transaction| {
            if select_ids(transaction, [id])?.is_empty() {
                return Err(StoreError::NotFound);
            }
            let mut statement = transaction
                .prepare(&format!(
                    "SELECT {EVENT_COLUMNS} FROM job_events WHERE job_id=?1 AND seq>?2 \
                     ORDER BY seq LIMIT ?3"
                ))
                .map_err(|_| StoreError::Storage)?;
            let mut rows = statement
                .query(params![id.to_string(), after, i64::from(limit)])
                .map_err(|_| StoreError::Storage)?;
            let mut events = Vec::new();
            while let Some(row) = rows.next().map_err(|_| StoreError::Storage)? {
                events.push(decode_event_row(row)?);
            }
            Ok(events)
        })
    }

    fn claim_next(
        &self,
        worker_id: WorkerId,
        now: Timestamp,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<Claim>, StoreError> {
        let queued = enum_name(lettuce_jobs::JobState::Queued)?;
        self.write_job_set(
            |transaction| {
                let rows = select_rows(transaction, "state=?1", &[&queued])?;
                with_latest_events(transaction, rows)
            },
            |store| store.claim_next(worker_id, now, lease_for, allowed),
        )
    }

    fn claim(
        &self,
        id: JobId,
        worker_id: WorkerId,
        now: Timestamp,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<Claim>, StoreError> {
        self.write_job_set(
            |transaction| load_with_children(transaction, [id]),
            |store| store.claim(id, worker_id, now, lease_for, allowed),
        )
    }

    fn heartbeat(
        &self,
        claim: &ClaimRef,
        now: Timestamp,
        extend_for: Duration,
    ) -> Result<Claim, StoreError> {
        self.write_job_set(
            |transaction| load_with_children(transaction, [claim.job_id]),
            |store| store.heartbeat(claim, now, extend_for),
        )
    }

    fn append_and_transition(&self, mutation: JobMutation) -> Result<JobSnapshot, StoreError> {
        let id = mutation.job_id();
        self.write_job_set(
            |transaction| load_with_children(transaction, [id]),
            |store| store.append_and_transition(mutation),
        )
    }

    fn expired_claims(&self, now: Timestamp, limit: u32) -> Result<Vec<ExpiredClaim>, StoreError> {
        self.write_job_set(
            |transaction| {
                let rows = select_rows(
                    transaction,
                    "lease_expires_at IS NOT NULL AND lease_expires_at < ?1",
                    &[&now.get()],
                )?;
                with_latest_events(transaction, rows)
            },
            |store| store.expired_claims(now, limit),
        )
    }

    fn orphaned_claims(&self, now: Timestamp, limit: u32) -> Result<Vec<ExpiredClaim>, StoreError> {
        self.write_job_set(
            |transaction| {
                let rows = select_rows(transaction, "lease_expires_at IS NOT NULL", &[])?;
                with_latest_events(transaction, rows)
            },
            |store| store.orphaned_claims(now, limit),
        )
    }

    /// Prunes terminal jobs except those a speech transcription or synthesis,
    /// an image generation, a staged lorebook project or a companion growth
    /// run still binds, together with every ancestor such a kept job points at.
    fn prune(&self, policy: RetentionPolicy, now: Timestamp) -> Result<PruneReport, StoreError> {
        let mut connection = self.connection.lock().map_err(|_| StoreError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| StoreError::Storage)?;
        let rows = select_rows(&transaction, "1=1", &[])?;
        let before = with_latest_events(&transaction, rows)?;
        let store = InMemoryJobStore::restore_working_set(before.values().cloned().collect())?;
        let mut report = store.prune(policy, now);
        let bound = transaction
            .prepare(JOB_BINDING_QUERY)
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|_| StoreError::Storage)?
            .into_iter()
            .map(|id| id.parse::<JobId>().map_err(|_| StoreError::InvalidData))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let removed = report.removed.iter().copied().collect::<BTreeSet<_>>();
        let mut kept = removed
            .intersection(&bound)
            .copied()
            .collect::<BTreeSet<_>>();
        loop {
            let parents = kept
                .iter()
                .filter_map(|id| before.get(id).and_then(|record| record.snapshot.parent_id))
                .filter(|parent| removed.contains(parent) && !kept.contains(parent))
                .collect::<Vec<_>>();
            if parents.is_empty() {
                break;
            }
            kept.extend(parents);
        }
        let mut after = records_by_id(store.stored_records());
        for id in &kept {
            if let Some(record) = before.get(id) {
                after.insert(*id, record.clone());
            }
        }
        report.removed.retain(|id| !kept.contains(id));
        persist_changes(&transaction, &before, &after)?;
        transaction.commit().map_err(|_| StoreError::Storage)?;
        Ok(report)
    }
}

impl JobCatalog for Database {
    fn list_jobs(&self, filter: &JobListFilter) -> Result<Page<JobSnapshot>, StoreError> {
        let after = filter
            .page
            .cursor
            .as_deref()
            .map(parse_list_cursor)
            .transpose()?;
        let limit = usize::from(filter.page.limit.get());
        let mut clauses = Vec::new();
        let mut values: Vec<Box<dyn ToSql>> = Vec::new();
        if !filter.kinds.is_empty() {
            clauses.push(format!(
                "kind IN ({})",
                vec!["?"; filter.kinds.len()].join(",")
            ));
            for kind in &filter.kinds {
                values.push(Box::new(enum_name(kind)?));
            }
        }
        if !filter.states.is_empty() {
            clauses.push(format!(
                "state IN ({})",
                vec!["?"; filter.states.len()].join(",")
            ));
            for state in &filter.states {
                values.push(Box::new(enum_name(state)?));
            }
        }
        if let Some((kind, id)) = &filter.subject {
            clauses.push("subject_kind=? AND subject_id=?".to_owned());
            values.push(Box::new(enum_name(kind)?));
            values.push(Box::new(id.to_string()));
        }
        if let Some((created_at, id)) = after {
            clauses.push("(created_at<? OR (created_at=? AND id<?))".to_owned());
            values.push(Box::new(created_at));
            values.push(Box::new(created_at));
            values.push(Box::new(id));
        }
        let filter_sql = if clauses.is_empty() {
            "1=1".to_owned()
        } else {
            clauses.join(" AND ")
        };
        values.push(Box::new(
            i64::try_from(limit + 1).map_err(|_| StoreError::InvalidLimit)?,
        ));
        self.read_job_rows(|transaction| {
            let mut statement = transaction
                .prepare(&format!(
                    "SELECT {JOB_COLUMNS} FROM jobs WHERE {filter_sql} \
                     ORDER BY created_at DESC, id DESC LIMIT ?"
                ))
                .map_err(|_| StoreError::Storage)?;
            let mut rows = statement
                .query(params_from_iter(values.iter()))
                .map_err(|_| StoreError::Storage)?;
            let mut items = Vec::new();
            while let Some(row) = rows.next().map_err(|_| StoreError::Storage)? {
                items.push(decode_job_row(row)?.snapshot);
            }
            let mut next_cursor = None;
            if items.len() > limit {
                items.truncate(limit);
                next_cursor = items
                    .last()
                    .map(|last| format!("{}:{}", last.created_at.get(), last.id));
            }
            Ok(Page { items, next_cursor })
        })
    }

    fn job_change_position(&self) -> Result<u64, StoreError> {
        self.read_job_rows(|transaction| {
            let position: Option<i64> = transaction
                .query_row("SELECT max(position) FROM job_changes", [], |row| {
                    row.get(0)
                })
                .map_err(|_| StoreError::Storage)?;
            u64::try_from(position.unwrap_or(0)).map_err(|_| StoreError::InvalidData)
        })
    }

    fn job_changes_since(&self, after: u64, limit: u32) -> Result<Vec<JobChange>, StoreError> {
        if limit == 0 {
            return Err(StoreError::InvalidLimit);
        }
        let after = i64::try_from(after).map_err(|_| StoreError::InvalidCursor)?;
        self.read_job_rows(|transaction| {
            let mut statement = transaction
                .prepare(
                    "SELECT job_id, position FROM job_changes WHERE position>?1 \
                     ORDER BY position LIMIT ?2",
                )
                .map_err(|_| StoreError::Storage)?;
            let mut rows = statement
                .query(params![after, i64::from(limit)])
                .map_err(|_| StoreError::Storage)?;
            let mut changes = Vec::new();
            while let Some(row) = rows.next().map_err(|_| StoreError::Storage)? {
                let job_id = row
                    .get::<_, String>(0)
                    .map_err(|_| StoreError::Storage)?
                    .parse::<JobId>()
                    .map_err(|_| StoreError::InvalidData)?;
                let position = row.get::<_, i64>(1).map_err(|_| StoreError::Storage)?;
                changes.push(JobChange {
                    job_id,
                    position: u64::try_from(position).map_err(|_| StoreError::InvalidData)?,
                });
            }
            Ok(changes)
        })
    }
}

fn parse_list_cursor(cursor: &str) -> Result<(i64, String), StoreError> {
    let (created_at, id) = cursor.split_once(':').ok_or(StoreError::InvalidCursor)?;
    let created_at = created_at
        .parse::<i64>()
        .map_err(|_| StoreError::InvalidCursor)?;
    let id = id.parse::<JobId>().map_err(|_| StoreError::InvalidCursor)?;
    Ok((created_at, id.to_string()))
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::Arc, thread};

    use lettuce_jobs::{
        CancellationReason, IdempotencyKey, JobKind, JobOutcome, JobPriority, JobSpec, JobState,
        JobSubject, OutcomeRef, ProgressSnapshot, RecoveryPolicy, ResourceClass, SubjectKind,
        UnitsProgress,
    };
    use lettuce_types::{AssetId, PageLimit, PageRequest};
    use uuid::Uuid;

    use super::*;

    fn spec(key: &str) -> JobSpec {
        JobSpec::new(
            JobKind::ArtifactInstall,
            JobSubject::new(SubjectKind::ArtifactInstall, "artifact-1").expect("subject"),
            OutcomeRef::ArtifactInstallation(AssetId::from_uuid(Uuid::nil())),
        )
        .with_resources(vec![ResourceClass::Network, ResourceClass::DiskWrite])
        .with_idempotency_key(IdempotencyKey::new(key).expect("key"))
    }

    fn availability() -> ResourceAvailability {
        ResourceAvailability::all()
    }

    #[test]
    fn job_settlement_and_external_result_roll_back_together() {
        let database = Database::open_in_memory().expect("database");
        let job = database
            .admit_job_with_detail(
                spec("external-result"),
                "external-key",
                "digest",
                &serde_json::json!({"type":"external"}),
            )
            .expect("admit");
        let at = job.updated_at;
        let claim = database
            .claim(
                job.id,
                WorkerId::new(),
                at,
                Duration::from_secs(60),
                &availability(),
            )
            .expect("claim")
            .expect("claimed");
        database
            .append_and_transition(JobMutation::Start {
                claim: claim.claim.clone(),
                at,
            })
            .expect("start");
        database.connection().expect("connection").execute_batch("CREATE TRIGGER reject_external_result BEFORE UPDATE ON job_details BEGIN SELECT RAISE(ABORT,'injected'); END;").expect("inject");
        let mutation = JobMutation::Succeed {
            claim: claim.claim,
            outcome: JobOutcome::Success {
                result_ref: OutcomeRef::ArtifactInstallation(AssetId::new()),
            },
            at,
        };
        assert_eq!(
            database.settle_job_with_detail(
                mutation.clone(),
                Some(&serde_json::json!({"voice_id":"created"})),
                None
            ),
            Err(StoreError::Storage)
        );
        assert_eq!(
            database.get(job.id).expect("get").expect("job").state,
            JobState::Running
        );
        assert!(
            database
                .job_detail(job.id)
                .expect("detail")
                .expect("stored")
                .result
                .is_none()
        );
        database
            .connection()
            .expect("connection")
            .execute_batch("DROP TRIGGER reject_external_result;")
            .expect("remove injection");
        assert_eq!(
            database
                .settle_job_with_detail(
                    mutation,
                    Some(&serde_json::json!({"voice_id":"created"})),
                    None
                )
                .expect("settle")
                .state,
            JobState::Succeeded
        );
        assert_eq!(
            database
                .job_detail(job.id)
                .expect("detail")
                .expect("stored")
                .result,
            Some(serde_json::json!({"voice_id":"created"}))
        );
    }

    #[test]
    fn local_model_detail_admission_rolls_back_the_job_when_detail_storage_fails() {
        let database = Database::open_in_memory().expect("database");
        database.connection().expect("connection").execute_batch(
            "CREATE TRIGGER reject_test_detail BEFORE INSERT ON local_model_jobs BEGIN SELECT RAISE(ABORT,'injected'); END;"
        ).expect("fault injection");
        let detail =
            serde_json::json!({"root": "/models/embedding", "enable_dynamic_memory": true});
        assert_eq!(
            database.create_or_get_with_local_model_detail(spec("atomic-install"), &detail),
            Err(StoreError::Storage)
        );
        assert!(
            database
                .list(JobQuery::default())
                .expect("jobs")
                .items
                .is_empty()
        );
        assert_eq!(database.job_change_position().expect("position"), 0);
        database
            .connection()
            .expect("connection")
            .execute_batch("DROP TRIGGER reject_test_detail")
            .expect("clear fault");
        let admitted = database
            .create_or_get_with_local_model_detail(spec("atomic-install"), &detail)
            .expect("retry");
        assert_eq!(
            database
                .local_model_job(admitted.job.id)
                .expect("detail")
                .expect("stored")
                .detail,
            detail
        );
        assert!(
            !database
                .create_or_get_with_local_model_detail(spec("atomic-install"), &detail)
                .expect("replay")
                .created
        );
        assert_eq!(
            database.create_or_get_with_local_model_detail(
                spec("atomic-install"),
                &serde_json::json!({"enable_dynamic_memory": false})
            ),
            Err(StoreError::IdempotencyConflict)
        );
    }

    fn after(snapshot: &JobSnapshot, millis: i64) -> Timestamp {
        Timestamp::new(
            snapshot
                .updated_at
                .get()
                .checked_add(millis)
                .expect("test time"),
        )
    }

    fn synthesis_fixture() -> (JobSpec, lettuce_speech::SynthesisRequest) {
        use lettuce_speech::{
            AudioProvider, AudioProviderConfig, SynthesisRequest, TtsOutputPolicy,
        };
        let id = lettuce_types::RequestId::new();
        let request = SynthesisRequest {
            id,
            provider: AudioProvider {
                id: lettuce_types::AudioProviderId::new(),
                secret_owner_id: lettuce_settings::SecretOwnerId::new(),
                label: "Local speech".into(),
                api_key_ref: None,
                config: AudioProviderConfig::Kokoro { variant: None },
                revision: lettuce_types::Revision::INITIAL,
                created_at: Timestamp::new(1),
                updated_at: Timestamp::new(1),
            },
            model_id: "int8".into(),
            voice_id: "af_heart".into(),
            prompt: None,
            text: "Hello".into(),
            output_asset_id: AssetId::new(),
            output_policy: TtsOutputPolicy::Retained,
            created_at: Timestamp::new(1),
        };
        let job = JobSpec::new(
            JobKind::SpeechSynthesize,
            JobSubject::new(SubjectKind::SpeechRequest, id.to_string()).expect("subject"),
            OutcomeRef::Request(id),
        )
        .with_idempotency_key(IdempotencyKey::new(id.to_string()).expect("key"))
        .with_resources(vec![
            ResourceClass::ModelLoad,
            ResourceClass::Cpu,
            ResourceClass::DiskWrite,
        ]);
        (job, request)
    }

    #[test]
    fn synthesis_admission_rolls_back_then_replays_across_reopen() {
        use lettuce_speech::SynthesisRepository;
        let path = std::env::temp_dir().join(format!(
            "lettuce-speech-admission-{}.sqlite",
            Uuid::new_v4()
        ));
        let database = Database::open(&path).expect("database");
        let (job, request) = synthesis_fixture();
        database.connection().expect("connection").execute_batch(
            "CREATE TRIGGER reject_synthesis BEFORE INSERT ON speech_syntheses BEGIN SELECT RAISE(ABORT, 'injected speech failure'); END;"
        ).expect("fault injection");
        assert_eq!(
            database.admit_speech_synthesis(
                job.clone(),
                "speech-operation",
                "digest",
                request.clone()
            ),
            Err(StoreError::Storage)
        );
        assert!(
            database
                .job_operation("speech-operation")
                .expect("receipt")
                .is_none()
        );
        let count: i64 = database
            .connection()
            .expect("connection")
            .query_row("SELECT count(*) FROM jobs", [], |row| row.get(0))
            .expect("jobs");
        assert_eq!(count, 0);
        database
            .connection()
            .expect("connection")
            .execute_batch("DROP TRIGGER reject_synthesis")
            .expect("remove fault");
        let admitted = database
            .admit_speech_synthesis(job.clone(), "speech-operation", "digest", request.clone())
            .expect("admit");
        drop(database);
        let database = Database::open(&path).expect("reopen");
        assert_eq!(
            database
                .admit_speech_synthesis(job.clone(), "speech-operation", "digest", request.clone())
                .expect("replay")
                .id,
            admitted.id
        );
        assert_eq!(
            SynthesisRepository::get(&database, admitted.id)
                .expect("record")
                .request,
            request
        );
        assert_eq!(
            database.admit_speech_synthesis(job, "speech-operation", "changed", request),
            Err(StoreError::IdempotencyConflict)
        );
    }

    #[test]
    fn concurrent_synthesis_admission_has_one_record_and_receipt() {
        let path =
            std::env::temp_dir().join(format!("lettuce-speech-race-{}.sqlite", Uuid::new_v4()));
        let first = Database::open(&path).expect("first");
        let second = Database::open(&path).expect("second");
        let (job, request) = synthesis_fixture();
        let barrier = std::sync::Barrier::new(2);
        let (a, b) = thread::scope(|scope| {
            let run = |database: &Database| {
                barrier.wait();
                database
                    .admit_speech_synthesis(
                        job.clone(),
                        "speech-operation",
                        "digest",
                        request.clone(),
                    )
                    .expect("admit")
            };
            let first_ref = &first;
            let second_ref = &second;
            let a = scope.spawn(move || run(first_ref));
            let b = scope.spawn(move || run(second_ref));
            (
                a.join().expect("first thread"),
                b.join().expect("second thread"),
            )
        });
        assert_eq!(a.id, b.id);
        for table in ["jobs", "job_operations", "job_details", "speech_syntheses"] {
            let count: i64 = first
                .connection()
                .expect("connection")
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("count");
            assert_eq!(count, 1, "{table}");
        }
    }

    #[test]
    fn frozen_admission_result_is_atomic_and_replays_across_reopen() {
        let path = std::env::temp_dir().join(format!("lettuce-frozen-{}.sqlite", Uuid::new_v4()));
        let database = Database::open(&path).expect("database");
        let detail = serde_json::json!({"version": 1, "batch": {"branch_id": "frozen-parent"}});
        database.connection().expect("connection").execute_batch("CREATE TRIGGER reject_frozen_detail BEFORE INSERT ON job_details BEGIN SELECT RAISE(ABORT, 'injected'); END;").expect("trigger");
        assert_eq!(
            database.admit_job_with_detail_result(
                spec("frozen-key"),
                "frozen-operation",
                "digest",
                &detail
            ),
            Err(StoreError::Storage)
        );
        for table in ["jobs", "job_details", "job_operations"] {
            let count: i64 = database
                .connection()
                .expect("connection")
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("count");
            assert_eq!(count, 0);
        }
        database
            .connection()
            .expect("connection")
            .execute_batch("DROP TRIGGER reject_frozen_detail")
            .expect("remove trigger");
        let admitted = database
            .admit_job_with_detail_result(spec("frozen-key"), "frozen-operation", "digest", &detail)
            .expect("admit");
        assert!(admitted.created);
        drop(database);
        let database = Database::open(&path).expect("reopen");
        let replay = database
            .admit_job_with_detail_result(spec("frozen-key"), "frozen-operation", "digest", &detail)
            .expect("replay");
        assert!(!replay.created);
        assert_eq!(replay.job, admitted.job);
        assert_eq!(
            database
                .job_detail(replay.job.id)
                .expect("detail")
                .expect("stored")
                .detail,
            detail
        );
        assert_eq!(
            database.admit_job_with_detail_result(
                spec("frozen-key"),
                "frozen-operation",
                "changed",
                &detail
            ),
            Err(StoreError::IdempotencyConflict)
        );
    }

    #[test]
    fn generic_admission_rolls_back_and_replays_across_reopen() {
        let path = std::env::temp_dir().join(format!("lettuce-detail-{}.sqlite", Uuid::new_v4()));
        let database = Database::open(&path).expect("database");
        database.connection().expect("connection").execute_batch("CREATE TRIGGER reject_detail BEFORE INSERT ON job_details BEGIN SELECT RAISE(ABORT, 'injected detail failure'); END;").expect("trigger");
        let detail = serde_json::json!({"kind":"test","text":"first"});
        assert_eq!(
            database.admit_job_with_detail(spec("detail-key"), "operation", "digest", &detail),
            Err(StoreError::Storage)
        );
        let count: i64 = database
            .connection()
            .expect("connection")
            .query_row("SELECT count(*) FROM jobs", [], |row| row.get(0))
            .expect("count");
        assert_eq!(count, 0);
        assert!(
            database
                .job_operation("operation")
                .expect("operation")
                .is_none()
        );
        database
            .connection()
            .expect("connection")
            .execute_batch("DROP TRIGGER reject_detail")
            .expect("remove trigger");
        let admitted = database
            .admit_job_with_detail(spec("detail-key"), "operation", "digest", &detail)
            .expect("admit");
        drop(database);
        let database = Database::open(&path).expect("reopen");
        assert_eq!(
            database
                .admit_job_with_detail(spec("detail-key"), "operation", "digest", &detail)
                .expect("replay")
                .id,
            admitted.id
        );
        assert_eq!(
            database.admit_job_with_detail(spec("detail-key"), "operation", "different", &detail),
            Err(StoreError::IdempotencyConflict)
        );
        assert_eq!(
            database
                .job_detail(admitted.id)
                .expect("detail")
                .expect("stored")
                .detail,
            detail
        );
    }

    #[test]
    fn concurrent_generic_admission_has_one_receipt_and_detail() {
        let path =
            std::env::temp_dir().join(format!("lettuce-detail-race-{}.sqlite", Uuid::new_v4()));
        let first = Database::open(&path).expect("first");
        let second = Database::open(&path).expect("second");
        let barrier = std::sync::Barrier::new(2);
        let detail = serde_json::json!({"kind":"test"});
        let (a, b) = thread::scope(|scope| {
            let run = |database: &Database| {
                barrier.wait();
                database
                    .admit_job_with_detail(
                        spec("same-detail"),
                        "same-operation",
                        "same-digest",
                        &detail,
                    )
                    .expect("admit")
            };
            let first = &first;
            let second = &second;
            let a = scope.spawn(move || run(first));
            let b = scope.spawn(move || run(second));
            (
                a.join().expect("first worker"),
                b.join().expect("second worker"),
            )
        });
        assert_eq!(a.id, b.id);
        for table in ["jobs", "job_details", "job_operations"] {
            let count: i64 = first
                .connection()
                .expect("connection")
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("count");
            assert_eq!(count, 1, "{table}");
        }
    }

    #[test]
    fn committed_job_changes_notify_and_feed_positions_only_grow() {
        let database = Database::open_in_memory().expect("database");
        let notified = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = Arc::clone(&notified);
        database.on_job_change(move || {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        let start = database.job_change_position().expect("position");
        let first = database.create_or_get(spec("feed-1")).expect("first").job;
        assert_eq!(notified.load(std::sync::atomic::Ordering::SeqCst), 1);
        let changes = database.job_changes_since(start, 10).expect("changes");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].job_id, first.id);

        {
            let mut connection = database.connection.lock().expect("connection");
            let transaction = connection.transaction().expect("transaction");
            transaction
                .execute(
                    "INSERT INTO job_events (job_id, seq, at, correlation_id, event_json) \
                     VALUES (?1, 99, 1, 'x', '{}')",
                    [first.id.to_string()],
                )
                .expect("uncommitted change");
        }
        assert_eq!(notified.load(std::sync::atomic::Ordering::SeqCst), 1);

        let second = database.create_or_get(spec("feed-2")).expect("second").job;
        let latest = database.job_change_position().expect("latest");
        database
            .connection
            .lock()
            .expect("connection")
            .execute("DELETE FROM jobs WHERE id = ?1", [second.id.to_string()])
            .expect("remove the newest job");
        let third = database.create_or_get(spec("feed-3")).expect("third").job;
        let changes = database
            .job_changes_since(latest, 10)
            .expect("after delete");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].job_id, third.id);
        assert!(changes[0].position > latest);
        assert_eq!(notified.load(std::sync::atomic::Ordering::SeqCst), 4);
    }

    #[test]
    fn persists_lifecycle_events_and_keyset_pages_across_reopen() {
        let path = std::env::temp_dir().join(format!("lettuce-jobs-{}.sqlite", Uuid::new_v4()));
        let database = Database::open(&path).expect("open database");
        let first = database
            .create_or_get(spec("durable-1"))
            .expect("create first");
        let second = database
            .create_or_get(spec("durable-2"))
            .expect("create second");
        let claimed_at = after(&first.job, 1);
        let claim = database
            .claim(
                first.job.id,
                WorkerId::new(),
                claimed_at,
                Duration::from_secs(5),
                &availability(),
            )
            .expect("claim")
            .expect("eligible");
        database
            .append_and_transition(JobMutation::Start {
                claim: claim.claim.clone(),
                at: Timestamp::new(claimed_at.get() + 1),
            })
            .expect("start");
        drop(database);

        let reopened = Database::open(&path).expect("reopen database");
        assert_eq!(
            reopened.get(first.job.id).expect("get").expect("job").state,
            JobState::Running
        );
        assert_eq!(
            reopened
                .events_since(first.job.id, None, 20)
                .expect("events")
                .len(),
            4
        );
        let first_page = reopened
            .list(JobQuery::page(PageRequest {
                cursor: None,
                limit: PageLimit::new(1),
            }))
            .expect("first page");
        assert_eq!(first_page.items.len(), 1);
        let second_page = reopened
            .list(JobQuery::page(PageRequest {
                cursor: first_page.next_cursor,
                limit: PageLimit::new(1),
            }))
            .expect("second page");
        let mut paged = vec![first_page.items[0].id, second_page.items[0].id];
        paged.sort();
        let mut expected = vec![first.job.id, second.job.id];
        expected.sort();
        assert_eq!(paged, expected);
        drop(reopened);
        fs::remove_file(path).expect("remove database");
    }

    #[test]
    fn serializes_idempotent_creation_and_recovers_expired_claims_across_handles() {
        let path =
            std::env::temp_dir().join(format!("lettuce-jobs-race-{}.sqlite", Uuid::new_v4()));
        drop(Database::open(&path).expect("initialize database"));
        let first = Arc::new(Database::open(&path).expect("first handle"));
        let second = Arc::new(Database::open(&path).expect("second handle"));
        let left = {
            let database = Arc::clone(&first);
            thread::spawn(move || {
                database
                    .create_or_get(spec("same-key"))
                    .expect("left create")
            })
        };
        let right = {
            let database = Arc::clone(&second);
            thread::spawn(move || {
                database
                    .create_or_get(spec("same-key"))
                    .expect("right create")
            })
        };
        let left = left.join().expect("left thread");
        let right = right.join().expect("right thread");
        assert_eq!(left.job.id, right.job.id);
        assert_ne!(left.created, right.created);
        assert_eq!(
            first.create_or_get(spec("same-key").with_priority(JobPriority::Interactive)),
            Err(StoreError::IdempotencyConflict)
        );

        let claimed_at = after(&left.job, 1);
        let claim = first
            .claim(
                left.job.id,
                WorkerId::new(),
                claimed_at,
                Duration::from_secs(1),
                &availability(),
            )
            .expect("claim")
            .expect("eligible");
        first
            .append_and_transition(JobMutation::Start {
                claim: claim.claim,
                at: Timestamp::new(claimed_at.get() + 1),
            })
            .expect("start");
        let expired = second
            .expired_claims(Timestamp::new(claimed_at.get() + 1_001), 10)
            .expect("recover");
        assert_eq!(expired.len(), 1);
        assert_eq!(
            second.get(left.job.id).expect("get").expect("job").state,
            JobState::Queued
        );

        let cancelled = second
            .append_and_transition(JobMutation::RequestCancellation {
                id: left.job.id,
                reason: CancellationReason::User,
                at: Timestamp::new(claimed_at.get() + 1_002),
            })
            .expect("request cancellation");
        assert_eq!(cancelled.state, JobState::CancellationRequested);
        second
            .append_and_transition(JobMutation::FinishQueuedCancellation {
                id: left.job.id,
                at: Timestamp::new(claimed_at.get() + 1_003),
            })
            .expect("finish cancellation");
        drop(first);
        drop(second);
        fs::remove_file(path).expect("remove database");
    }

    #[test]
    fn preserves_retry_and_progress_validation() {
        let database = Database::open_in_memory().expect("open database");
        let created = database
            .create_or_get(spec("retry-progress"))
            .expect("create");
        let claimed_at = after(&created.job, 1);
        let claim = database
            .claim(
                created.job.id,
                WorkerId::new(),
                claimed_at,
                Duration::from_secs(10),
                &availability(),
            )
            .expect("claim")
            .expect("eligible");
        database
            .append_and_transition(JobMutation::Start {
                claim: claim.claim.clone(),
                at: Timestamp::new(claimed_at.get() + 1),
            })
            .expect("start");
        database
            .append_and_transition(JobMutation::Progress {
                claim: claim.claim.clone(),
                progress: ProgressSnapshot {
                    units: Some(UnitsProgress::new(2, Some(4)).expect("progress")),
                    ..ProgressSnapshot::default()
                },
                at: Timestamp::new(claimed_at.get() + 2),
            })
            .expect("progress");
        assert_eq!(
            database.append_and_transition(JobMutation::Progress {
                claim: claim.claim.clone(),
                progress: ProgressSnapshot {
                    units: Some(UnitsProgress::new(1, Some(4)).expect("progress")),
                    ..ProgressSnapshot::default()
                },
                at: Timestamp::new(claimed_at.get() + 3),
            }),
            Err(StoreError::InvalidProgress)
        );
        database
            .append_and_transition(JobMutation::RetryScheduled {
                claim: claim.claim,
                at: Timestamp::new(claimed_at.get() + 4),
            })
            .expect("retry");
        let retried = database
            .claim(
                created.job.id,
                WorkerId::new(),
                Timestamp::new(claimed_at.get() + 5),
                Duration::from_secs(10),
                &availability(),
            )
            .expect("reclaim")
            .expect("eligible");
        assert_eq!(retried.claim.attempt.get(), 2);
        assert_eq!(
            database
                .events_since(created.job.id, None, 20)
                .expect("events")
                .len(),
            7
        );
        assert_eq!(created.job.recovery_policy, RecoveryPolicy::Restart);
    }

    #[test]
    fn preserves_priority_claim_heartbeat_terminal_and_prune_semantics() {
        let database = Database::open_in_memory().expect("open database");
        let background = database
            .create_or_get(spec("background").with_priority(JobPriority::Background))
            .expect("create background");
        let interactive = database
            .create_or_get(spec("interactive").with_priority(JobPriority::Interactive))
            .expect("create interactive");
        let claimed_at = Timestamp::new(
            background
                .job
                .updated_at
                .max(interactive.job.updated_at)
                .get()
                + 1,
        );
        let claim = database
            .claim_next(
                WorkerId::new(),
                claimed_at,
                Duration::from_secs(5),
                &availability(),
            )
            .expect("claim next")
            .expect("eligible");
        assert_eq!(claim.claim.job_id, interactive.job.id);
        database
            .append_and_transition(JobMutation::Start {
                claim: claim.claim.clone(),
                at: Timestamp::new(claimed_at.get() + 1),
            })
            .expect("start");
        let heartbeat = database
            .heartbeat(
                &claim.claim,
                Timestamp::new(claimed_at.get() + 2),
                Duration::from_secs(10),
            )
            .expect("heartbeat");
        assert_eq!(heartbeat.lease_expires_at.get(), claimed_at.get() + 10_002);
        let result_ref = OutcomeRef::ArtifactInstallation(AssetId::from_uuid(Uuid::nil()));
        database
            .append_and_transition(JobMutation::Succeed {
                claim: claim.claim,
                outcome: JobOutcome::Success { result_ref },
                at: Timestamp::new(claimed_at.get() + 3),
            })
            .expect("succeed");
        let report = database
            .prune(
                RetentionPolicy {
                    keep_terminal_for: Some(Duration::ZERO),
                },
                Timestamp::new(claimed_at.get() + 3),
            )
            .expect("prune");
        assert_eq!(report.removed, vec![interactive.job.id]);
        assert!(database.get(interactive.job.id).expect("get").is_none());
        assert!(database.get(background.job.id).expect("get").is_some());
    }

    #[test]
    fn job_operations_by_id_read_only_the_named_job() {
        let database = Database::open_in_memory().expect("open database");
        let created = database.create_or_get(spec("working-set")).expect("create");
        let other = database
            .create_or_get(spec("unrelated"))
            .expect("create other");
        database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE jobs SET snapshot_json='{}' WHERE id=?1",
                [other.job.id.to_string()],
            )
            .expect("corrupt unrelated job");
        let claimed_at = after(&created.job, 1);
        let claim = database
            .claim(
                created.job.id,
                WorkerId::new(),
                claimed_at,
                Duration::from_secs(10),
                &availability(),
            )
            .expect("claim")
            .expect("eligible");
        database
            .append_and_transition(JobMutation::Start {
                claim: claim.claim.clone(),
                at: Timestamp::new(claimed_at.get() + 1),
            })
            .expect("start");
        for step in 1..=5 {
            database
                .append_and_transition(JobMutation::Progress {
                    claim: claim.claim.clone(),
                    progress: ProgressSnapshot {
                        units: Some(UnitsProgress::new(step, Some(5)).expect("progress")),
                        ..ProgressSnapshot::default()
                    },
                    at: Timestamp::new(claimed_at.get() + 1 + i64::try_from(step).expect("step")),
                })
                .expect("progress");
        }
        database
            .heartbeat(
                &claim.claim,
                Timestamp::new(claimed_at.get() + 7),
                Duration::from_secs(10),
            )
            .expect("heartbeat");
        assert_eq!(
            database
                .get(created.job.id)
                .expect("get")
                .expect("job")
                .state,
            JobState::Running
        );
        let events = database
            .events_since(created.job.id, Some(EventSeq::new(3)), 20)
            .expect("events");
        assert_eq!(
            events
                .iter()
                .map(|event| event.seq.get())
                .collect::<Vec<_>>(),
            vec![4, 5, 6, 7, 8, 9]
        );
        assert_eq!(database.get(other.job.id), Err(StoreError::InvalidData));
    }

    #[test]
    fn retention_keeps_a_job_a_staged_lorebook_run_still_binds() {
        let database = Database::open_in_memory().expect("open database");
        let created = database
            .create_or_get(spec("staged-bound"))
            .expect("create");
        let claimed_at = after(&created.job, 1);
        let claim = database
            .claim(
                created.job.id,
                WorkerId::new(),
                claimed_at,
                Duration::from_secs(5),
                &availability(),
            )
            .expect("claim")
            .expect("eligible");
        database
            .append_and_transition(JobMutation::Start {
                claim: claim.claim.clone(),
                at: Timestamp::new(claimed_at.get() + 1),
            })
            .expect("start");
        database
            .append_and_transition(JobMutation::Succeed {
                claim: claim.claim,
                outcome: JobOutcome::Success {
                    result_ref: OutcomeRef::ArtifactInstallation(AssetId::from_uuid(Uuid::nil())),
                },
                at: Timestamp::new(claimed_at.get() + 2),
            })
            .expect("succeed");
        {
            let connection = database.connection().expect("connection");
            connection
                .execute_batch("PRAGMA foreign_keys=OFF")
                .expect("allow a bare run row");
            connection
                .execute(
                    "INSERT INTO creation_staged_lorebook_runs (request_id, project_id, job_id, \
                     model_profile_id, prompt_id, prompt_revision, stage, revision, created_at, \
                     updated_at, run_json) VALUES ('r', 'p', ?1, 'm', 'q', 1, 'committed', 1, 1, 1, '{}')",
                    [created.job.id.to_string()],
                )
                .expect("staged run");
            connection
                .execute_batch("PRAGMA foreign_keys=ON")
                .expect("restore foreign keys");
        }
        let report = database
            .prune(
                RetentionPolicy {
                    keep_terminal_for: Some(Duration::ZERO),
                },
                Timestamp::new(claimed_at.get() + 2),
            )
            .expect("prune");
        assert!(report.removed.is_empty());
        assert!(database.get(created.job.id).expect("get").is_some());
        let runs: i64 = database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT COUNT(*) FROM creation_staged_lorebook_runs",
                [],
                |row| row.get(0),
            )
            .expect("count");
        assert_eq!(runs, 1);
    }

    #[test]
    fn rejects_a_scalar_projection_that_disagrees_with_the_typed_snapshot() {
        let database = Database::open_in_memory().expect("open database");
        let created = database
            .create_or_get(spec("corrupt-projection"))
            .expect("create");
        database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE jobs SET state='failed' WHERE id=?1",
                [created.job.id.to_string()],
            )
            .expect("corrupt projection");
        assert_eq!(database.get(created.job.id), Err(StoreError::InvalidData));
    }
}
