use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use lettuce_jobs::{
    Claim, ClaimRef, CreateJobResult, EventSeq, ExpiredClaim, InMemoryJobStore, JobMutation,
    JobQuery, JobSnapshot, JobStore, NewJob, PruneReport, ResourceAvailability, StoreError,
    StoredJobRecord, Timestamp, WorkerId,
    events::{JobEvent, JobEventEnvelope},
    retention::RetentionPolicy,
};
use lettuce_types::{JobId, Page};
use rusqlite::{Row, ToSql, Transaction, TransactionBehavior, params, params_from_iter};

use crate::{Database, decode_versioned, encode_versioned};

const JOB_FORMAT_VERSION: u32 = 1;
const JOB_EVENT_FORMAT_VERSION: u32 = 1;
const JOB_COLUMNS: &str = "id, idempotency_key, kind, subject_kind, subject_id, state, priority, \
     parent_id, lease_expires_at, created_at, updated_at, spec_json, snapshot_json";
const EVENT_COLUMNS: &str = "job_id, seq, at, correlation_id, event_json";

type JobRecords = BTreeMap<JobId, StoredJobRecord>;

impl Database {
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
    }
    Ok(())
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

    fn after(snapshot: &JobSnapshot, millis: i64) -> Timestamp {
        Timestamp::new(
            snapshot
                .updated_at
                .get()
                .checked_add(millis)
                .expect("test time"),
        )
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
