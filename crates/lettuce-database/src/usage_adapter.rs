use std::str::FromStr;

use async_trait::async_trait;
use lettuce_conversations::{
    InferenceUsage, PortError, UsageCounters, UsageOutcome, UsagePort, UsageRecord,
    UsageUnavailableReason,
};
use lettuce_types::{Revision, UsageEventId};
use lettuce_usage::{UsageEvent, UsageLedger, UsageLedgerError};
use rusqlite::{OptionalExtension, params};

use crate::Database;

impl lettuce_usage::JobUsageLedger for Database {
    fn admit_job_usage(
        &self,
        record: lettuce_usage::JobInferenceUsage,
    ) -> Result<(), UsageLedgerError> {
        if record.result.is_some()
            || record.model_revision.get() == 0
            || record.provider_account_revision.get() == 0
        {
            return Err(UsageLedgerError::Invalid);
        }
        let encoded = crate::encode_versioned(&record, 1).map_err(|_| UsageLedgerError::Invalid)?;
        let db = self.connection().map_err(|_| UsageLedgerError::Storage)?;
        db.execute("INSERT OR IGNORE INTO job_inference_usage (id,job_id,admitted_at,record_json) SELECT ?1,?2,?3,?4 WHERE EXISTS (SELECT 1 FROM jobs WHERE id=?2)",
            params![record.id.to_string(), record.job_id.to_string(), record.admitted_at.get(), encoded]).map_err(|_| UsageLedgerError::Storage)?;
        let saved: Option<String> = db
            .query_row(
                "SELECT record_json FROM job_inference_usage WHERE id=?1",
                [record.id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| UsageLedgerError::Storage)?;
        if saved.as_deref() != Some(&encoded) {
            return Err(UsageLedgerError::Conflict);
        }
        Ok(())
    }
    fn settle_job_usage(
        &self,
        id: UsageEventId,
        result: lettuce_usage::JobInferenceUsageResult,
    ) -> Result<(), UsageLedgerError> {
        let encoded = crate::encode_versioned(&result, 1).map_err(|_| UsageLedgerError::Invalid)?;
        let db = self.connection().map_err(|_| UsageLedgerError::Storage)?;
        db.execute(
            "UPDATE job_inference_usage SET result_json=?2 WHERE id=?1 AND result_json IS NULL",
            params![id.to_string(), encoded],
        )
        .map_err(|_| UsageLedgerError::Storage)?;
        let saved: Option<String> = db
            .query_row(
                "SELECT result_json FROM job_inference_usage WHERE id=?1",
                [id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| UsageLedgerError::Storage)?
            .flatten();
        let saved = saved
            .map(|value| {
                crate::decode_versioned::<lettuce_usage::JobInferenceUsageResult>(&value, 1)
            })
            .transpose()
            .map_err(|_| UsageLedgerError::Storage)?;
        if saved.as_ref() != Some(&result) {
            return Err(UsageLedgerError::Conflict);
        }
        Ok(())
    }
    fn job_usage(
        &self,
        job_id: lettuce_types::JobId,
    ) -> Result<Vec<lettuce_usage::JobInferenceUsage>, UsageLedgerError> {
        let db = self.connection().map_err(|_| UsageLedgerError::Storage)?;
        let mut query = db.prepare("SELECT record_json,result_json FROM job_inference_usage WHERE job_id=?1 ORDER BY admitted_at,id").map_err(|_| UsageLedgerError::Storage)?;
        query
            .query_map([job_id.to_string()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })
            .map_err(|_| UsageLedgerError::Storage)?
            .map(|row| {
                let (record, result) = row.map_err(|_| UsageLedgerError::Storage)?;
                let mut record =
                    crate::decode_versioned::<lettuce_usage::JobInferenceUsage>(&record, 1)
                        .map_err(|_| UsageLedgerError::Storage)?;
                record.result = result
                    .map(|r| crate::decode_versioned(&r, 1))
                    .transpose()
                    .map_err(|_| UsageLedgerError::Storage)?;
                Ok(record)
            })
            .collect()
    }
}

impl lettuce_usage::UsageCostLedger for Database {
    fn record_cost(
        &self,
        event_id: UsageEventId,
        basis: lettuce_usage::UsageCostBasis,
    ) -> Result<lettuce_usage::UsageCost, UsageLedgerError> {
        let event = UsageLedger::get(self, event_id)?.ok_or(UsageLedgerError::Invalid)?;
        let cost = basis.calculate(&event)?;
        let encoded = crate::encode_versioned(&basis, 1).map_err(|_| UsageLedgerError::Invalid)?;
        let mut connection = self.connection().map_err(|_| UsageLedgerError::Storage)?;
        let tx = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| UsageLedgerError::Storage)?;
        tx.execute(
            "INSERT OR IGNORE INTO usage_costs (event_id, basis_json) VALUES (?1, ?2)",
            params![event_id.to_string(), encoded],
        )
        .map_err(|_| UsageLedgerError::Storage)?;
        let stored: String = tx
            .query_row(
                "SELECT basis_json FROM usage_costs WHERE event_id=?1",
                [event_id.to_string()],
                |row| row.get(0),
            )
            .map_err(|_| UsageLedgerError::Storage)?;
        let stored = crate::decode_versioned::<lettuce_usage::UsageCostBasis>(&stored, 1)
            .map_err(|_| UsageLedgerError::Storage)?;
        if stored != basis {
            return Err(UsageLedgerError::Conflict);
        }
        tx.commit().map_err(|_| UsageLedgerError::Storage)?;
        Ok(lettuce_usage::UsageCost {
            event_id,
            basis,
            cost,
        })
    }

    fn get_cost(
        &self,
        event_id: UsageEventId,
    ) -> Result<Option<lettuce_usage::UsageCost>, UsageLedgerError> {
        let stored: Option<String> = self
            .connection()
            .map_err(|_| UsageLedgerError::Storage)?
            .query_row(
                "SELECT basis_json FROM usage_costs WHERE event_id=?1",
                [event_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| UsageLedgerError::Storage)?;
        let Some(stored) = stored else {
            return Ok(None);
        };
        let basis = crate::decode_versioned::<lettuce_usage::UsageCostBasis>(&stored, 1)
            .map_err(|_| UsageLedgerError::Storage)?;
        let event = UsageLedger::get(self, event_id)?.ok_or(UsageLedgerError::Storage)?;
        let cost = basis
            .calculate(&event)
            .map_err(|_| UsageLedgerError::Storage)?;
        Ok(Some(lettuce_usage::UsageCost {
            event_id,
            basis,
            cost,
        }))
    }
    fn record_job_cost(
        &self,
        event_id: UsageEventId,
        basis: lettuce_usage::UsageCostBasis,
    ) -> Result<lettuce_usage::UsageCost, UsageLedgerError> {
        let event = load_job_usage(self, event_id)?.ok_or(UsageLedgerError::Invalid)?;
        let cost = basis.calculate_job(&event)?;
        let encoded = crate::encode_versioned(&basis, 1).map_err(|_| UsageLedgerError::Invalid)?;
        let mut connection = self.connection().map_err(|_| UsageLedgerError::Storage)?;
        let tx = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| UsageLedgerError::Storage)?;
        tx.execute(
            "INSERT OR IGNORE INTO job_usage_costs (event_id, basis_json) VALUES (?1, ?2)",
            params![event_id.to_string(), encoded],
        )
        .map_err(|_| UsageLedgerError::Storage)?;
        let stored: String = tx
            .query_row(
                "SELECT basis_json FROM job_usage_costs WHERE event_id=?1",
                [event_id.to_string()],
                |row| row.get(0),
            )
            .map_err(|_| UsageLedgerError::Storage)?;
        let stored = crate::decode_versioned::<lettuce_usage::UsageCostBasis>(&stored, 1)
            .map_err(|_| UsageLedgerError::Storage)?;
        if stored != basis {
            return Err(UsageLedgerError::Conflict);
        }
        tx.commit().map_err(|_| UsageLedgerError::Storage)?;
        Ok(lettuce_usage::UsageCost {
            event_id,
            basis,
            cost,
        })
    }

    fn get_job_cost(
        &self,
        event_id: UsageEventId,
    ) -> Result<Option<lettuce_usage::UsageCost>, UsageLedgerError> {
        let stored: Option<String> = self
            .connection()
            .map_err(|_| UsageLedgerError::Storage)?
            .query_row(
                "SELECT basis_json FROM job_usage_costs WHERE event_id=?1",
                [event_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| UsageLedgerError::Storage)?;
        let Some(stored) = stored else {
            return Ok(None);
        };
        let basis = crate::decode_versioned::<lettuce_usage::UsageCostBasis>(&stored, 1)
            .map_err(|_| UsageLedgerError::Storage)?;
        let event = load_job_usage(self, event_id)?.ok_or(UsageLedgerError::Storage)?;
        let cost = basis
            .calculate_job(&event)
            .map_err(|_| UsageLedgerError::Storage)?;
        Ok(Some(lettuce_usage::UsageCost {
            event_id,
            basis,
            cost,
        }))
    }
}

fn load_job_usage(
    database: &Database,
    event_id: UsageEventId,
) -> Result<Option<lettuce_usage::JobInferenceUsage>, UsageLedgerError> {
    let raw: Option<(String, Option<String>)> = database
        .connection()
        .map_err(|_| UsageLedgerError::Storage)?
        .query_row(
            "SELECT record_json, result_json FROM job_inference_usage WHERE id=?1",
            [event_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| UsageLedgerError::Storage)?;
    raw.map(|(record, result)| {
        let mut record = crate::decode_versioned::<lettuce_usage::JobInferenceUsage>(&record, 1)
            .map_err(|_| UsageLedgerError::Storage)?;
        record.result = result
            .map(|result| crate::decode_versioned(&result, 1))
            .transpose()
            .map_err(|_| UsageLedgerError::Storage)?;
        Ok(record)
    })
    .transpose()
}

fn outcome_name(value: UsageOutcome) -> &'static str {
    match value {
        UsageOutcome::Succeeded => "succeeded",
        UsageOutcome::Failed => "failed",
        UsageOutcome::Cancelled => "cancelled",
        UsageOutcome::Interrupted => "interrupted",
    }
}

fn unavailable_name(value: UsageUnavailableReason) -> &'static str {
    match value {
        UsageUnavailableReason::NotAdmitted => "not_admitted",
        UsageUnavailableReason::CancelledBeforeResponse => "cancelled_before_response",
        UsageUnavailableReason::ProviderOmitted => "provider_omitted",
        UsageUnavailableReason::TransportFailed => "transport_failed",
    }
}

fn parse_outcome(value: &str) -> Result<UsageOutcome, UsageLedgerError> {
    match value {
        "succeeded" => Ok(UsageOutcome::Succeeded),
        "failed" => Ok(UsageOutcome::Failed),
        "cancelled" => Ok(UsageOutcome::Cancelled),
        "interrupted" => Ok(UsageOutcome::Interrupted),
        _ => Err(UsageLedgerError::Storage),
    }
}

fn parse_unavailable(value: &str) -> Result<UsageUnavailableReason, UsageLedgerError> {
    match value {
        "not_admitted" => Ok(UsageUnavailableReason::NotAdmitted),
        "cancelled_before_response" => Ok(UsageUnavailableReason::CancelledBeforeResponse),
        "provider_omitted" => Ok(UsageUnavailableReason::ProviderOmitted),
        "transport_failed" => Ok(UsageUnavailableReason::TransportFailed),
        _ => Err(UsageLedgerError::Storage),
    }
}

fn hydrate(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawUsageEvent> {
    Ok(RawUsageEvent {
        id: row.get(0)?,
        turn_id: row.get(1)?,
        attempt_id: row.get(2)?,
        outcome: row.get(3)?,
        counters_kind: row.get(4)?,
        input_tokens: row.get(5)?,
        output_tokens: row.get(6)?,
        unavailable_reason: row.get(7)?,
        model_profile_id: row.get(8)?,
        model_revision: row.get(9)?,
        provider_account_id: row.get(10)?,
        provider_account_revision: row.get(11)?,
        recorded_at: row.get(12)?,
        cached_input_tokens: row.get(13)?,
        reasoning_tokens: row.get(14)?,
        cache_write_tokens: row.get(15)?,
        web_search_requests: row.get(16)?,
        provider_reported_cost: row.get(17)?,
    })
}

struct RawUsageEvent {
    provider_reported_cost: Option<f64>,
    cache_write_tokens: Option<i64>,
    web_search_requests: Option<i64>,
    cached_input_tokens: Option<i64>,
    reasoning_tokens: Option<i64>,
    id: String,
    turn_id: String,
    attempt_id: String,
    outcome: String,
    counters_kind: String,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    unavailable_reason: Option<String>,
    model_profile_id: Option<String>,
    model_revision: Option<i64>,
    provider_account_id: Option<String>,
    provider_account_revision: Option<i64>,
    recorded_at: i64,
}

impl RawUsageEvent {
    fn decode(self) -> Result<UsageEvent, UsageLedgerError> {
        let usage = match self.counters_kind.as_str() {
            "known" => UsageCounters::Known(InferenceUsage {
                provider_reported_cost: self
                    .provider_reported_cost
                    .map(lettuce_conversations::ProviderReportedCost::try_from)
                    .transpose()
                    .map_err(|_| UsageLedgerError::Storage)?,
                cache_write_tokens: self
                    .cache_write_tokens
                    .map(u64::try_from)
                    .transpose()
                    .map_err(|_| UsageLedgerError::Storage)?,
                web_search_requests: self
                    .web_search_requests
                    .map(u64::try_from)
                    .transpose()
                    .map_err(|_| UsageLedgerError::Storage)?,
                cached_input_tokens: self
                    .cached_input_tokens
                    .map(u64::try_from)
                    .transpose()
                    .map_err(|_| UsageLedgerError::Storage)?,
                reasoning_tokens: self
                    .reasoning_tokens
                    .map(u64::try_from)
                    .transpose()
                    .map_err(|_| UsageLedgerError::Storage)?,
                input_tokens: self
                    .input_tokens
                    .and_then(|value| u64::try_from(value).ok())
                    .ok_or(UsageLedgerError::Storage)?,
                output_tokens: self
                    .output_tokens
                    .and_then(|value| u64::try_from(value).ok())
                    .ok_or(UsageLedgerError::Storage)?,
            }),
            "unavailable" => UsageCounters::Unavailable(parse_unavailable(
                self.unavailable_reason
                    .as_deref()
                    .ok_or(UsageLedgerError::Storage)?,
            )?),
            _ => return Err(UsageLedgerError::Storage),
        };
        let event = UsageEvent {
            id: UsageEventId::from_str(&self.id).map_err(|_| UsageLedgerError::Storage)?,
            record: UsageRecord {
                turn_id: self
                    .turn_id
                    .parse()
                    .map_err(|_| UsageLedgerError::Storage)?,
                attempt_id: self
                    .attempt_id
                    .parse()
                    .map_err(|_| UsageLedgerError::Storage)?,
                outcome: parse_outcome(&self.outcome)?,
                usage,
                model_profile_id: self
                    .model_profile_id
                    .map(|value| value.parse().map_err(|_| UsageLedgerError::Storage))
                    .transpose()?,
                model_revision: self
                    .model_revision
                    .map(|value| {
                        u64::try_from(value)
                            .ok()
                            .map(Revision::new)
                            .ok_or(UsageLedgerError::Storage)
                    })
                    .transpose()?,
                provider_account_id: self
                    .provider_account_id
                    .map(|value| value.parse().map_err(|_| UsageLedgerError::Storage))
                    .transpose()?,
                provider_account_revision: self
                    .provider_account_revision
                    .map(|value| {
                        u64::try_from(value)
                            .ok()
                            .map(Revision::new)
                            .ok_or(UsageLedgerError::Storage)
                    })
                    .transpose()?,
                recorded_at: lettuce_types::TimestampMillis::new(self.recorded_at),
            },
        };
        event
            .record
            .validate()
            .map_err(|_| UsageLedgerError::Storage)?;
        Ok(event)
    }
}

const SELECT_EVENT: &str = "SELECT id, turn_id, attempt_id, outcome, counters_kind,
    input_tokens, output_tokens, unavailable_reason, model_profile_id, model_revision,
    provider_account_id, provider_account_revision, recorded_at, cached_input_tokens, reasoning_tokens, cache_write_tokens, web_search_requests, provider_reported_cost FROM usage_events";

impl UsageLedger for Database {
    fn record(&self, record: UsageRecord) -> Result<UsageEvent, UsageLedgerError> {
        record.validate().map_err(|_| UsageLedgerError::Invalid)?;
        let mut connection = self.connection().map_err(|_| UsageLedgerError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| UsageLedgerError::Storage)?;
        let conversation_id = transaction
            .query_row(
                "SELECT conversation_id FROM generation_attempts WHERE turn_id = ?1 AND id = ?2",
                params![record.turn_id.to_string(), record.attempt_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|_| UsageLedgerError::Storage)?
            .ok_or(UsageLedgerError::Conflict)?;
        let existing = transaction
            .query_row(
                &format!(
                    "{SELECT_EVENT} WHERE conversation_id = ?1 AND turn_id = ?2 AND attempt_id = ?3"
                ),
                params![
                    conversation_id,
                    record.turn_id.to_string(),
                    record.attempt_id.to_string()
                ],
                hydrate,
            )
            .optional()
            .map_err(|_| UsageLedgerError::Storage)?
            .map(RawUsageEvent::decode)
            .transpose()?;
        if let Some(existing) = existing {
            if existing.record != record {
                return Err(UsageLedgerError::Conflict);
            }
            transaction
                .commit()
                .map_err(|_| UsageLedgerError::Storage)?;
            return Ok(existing);
        }
        let id = UsageEventId::new();
        let provider_reported_cost = match &record.usage {
            UsageCounters::Known(usage) => usage
                .provider_reported_cost
                .map(lettuce_conversations::ProviderReportedCost::get),
            UsageCounters::Unavailable(_) => None,
        };
        let (cached, reasoning, cache_write, web_search) = match &record.usage {
            UsageCounters::Known(usage) => (
                usage.cached_input_tokens,
                usage.reasoning_tokens,
                usage.cache_write_tokens,
                usage.web_search_requests,
            ),
            UsageCounters::Unavailable(_) => (None, None, None, None),
        };
        let cached = cached
            .map(i64::try_from)
            .transpose()
            .map_err(|_| UsageLedgerError::Invalid)?;
        let reasoning = reasoning
            .map(i64::try_from)
            .transpose()
            .map_err(|_| UsageLedgerError::Invalid)?;
        let (kind, input, output, unavailable) = match record.usage {
            UsageCounters::Known(ref usage) => (
                "known",
                Some(i64::try_from(usage.input_tokens).map_err(|_| UsageLedgerError::Invalid)?),
                Some(i64::try_from(usage.output_tokens).map_err(|_| UsageLedgerError::Invalid)?),
                None,
            ),
            UsageCounters::Unavailable(reason) => {
                ("unavailable", None, None, Some(unavailable_name(reason)))
            }
        };
        transaction
            .execute(
                "INSERT INTO usage_events (id, conversation_id, turn_id, attempt_id, outcome,
                    counters_kind, input_tokens, output_tokens, unavailable_reason,
                    model_profile_id, model_revision, provider_account_id,
                    provider_account_revision, recorded_at, cached_input_tokens, reasoning_tokens, cache_write_tokens, web_search_requests, provider_reported_cost)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                params![
                    id.to_string(),
                    conversation_id,
                    record.turn_id.to_string(),
                    record.attempt_id.to_string(),
                    outcome_name(record.outcome),
                    kind,
                    input,
                    output,
                    unavailable,
                    record.model_profile_id.map(|value| value.to_string()),
                    record
                        .model_revision
                        .map(|value| i64::try_from(value.get())
                            .map_err(|_| UsageLedgerError::Invalid))
                        .transpose()?,
                    record.provider_account_id.map(|value| value.to_string()),
                    record
                        .provider_account_revision
                        .map(|value| i64::try_from(value.get())
                            .map_err(|_| UsageLedgerError::Invalid))
                        .transpose()?,
                    record.recorded_at.get(),
                    cached,
                    reasoning,
                    cache_write.map(i64::try_from).transpose().map_err(|_| UsageLedgerError::Invalid)?,
                    web_search.map(i64::try_from).transpose().map_err(|_| UsageLedgerError::Invalid)?,
                    provider_reported_cost,
                ],
            )
            .map_err(|_| UsageLedgerError::Storage)?;
        transaction
            .commit()
            .map_err(|_| UsageLedgerError::Storage)?;
        Ok(UsageEvent { id, record })
    }

    fn get(&self, id: UsageEventId) -> Result<Option<UsageEvent>, UsageLedgerError> {
        self.connection()
            .map_err(|_| UsageLedgerError::Storage)?
            .query_row(
                &format!("{SELECT_EVENT} WHERE id = ?1"),
                [id.to_string()],
                hydrate,
            )
            .optional()
            .map_err(|_| UsageLedgerError::Storage)?
            .map(RawUsageEvent::decode)
            .transpose()
    }
}

#[async_trait]
impl UsagePort for Database {
    async fn record(&self, record: UsageRecord) -> Result<UsageEventId, PortError> {
        UsageLedger::record(self, record)
            .map(|event| event.id)
            .map_err(|error| match error {
                UsageLedgerError::Invalid | UsageLedgerError::Conflict => PortError::Rejected,
                UsageLedgerError::Storage => PortError::Unavailable,
            })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn job_dispatch_evidence_survives_reopen_and_rejects_mutation() {
        use lettuce_jobs::{JobKind, JobSpec, JobStore, JobSubject, OutcomeRef, SubjectKind};
        use lettuce_types::UsageEventId;
        use lettuce_usage::{
            JobInferenceUsage, JobInferenceUsageResult, JobUsageLedger, ModelPricing,
            OpenRouterCostInput, UsageCostBasis, UsageCostLedger,
        };
        let old: JobInferenceUsageResult =
            serde_json::from_str(r#"{"Response":{"usage":null}}"#).expect("old response");
        assert_eq!(
            old,
            JobInferenceUsageResult::Response {
                usage: None,
                provider_response_id: None
            }
        );
        let path =
            std::env::temp_dir().join(format!("lettuce-job-usage-{}.sqlite", uuid::Uuid::new_v4()));
        let database = Database::open(&path).expect("database");
        let job = database
            .create_or_get(
                JobSpec::new(
                    JobKind::ArtifactInstall,
                    JobSubject::new(SubjectKind::ArtifactInstall, "usage-test").expect("subject"),
                    OutcomeRef::ArtifactInstallation(lettuce_types::AssetId::new()),
                )
                .with_resources(vec![lettuce_jobs::ResourceClass::Network]),
            )
            .expect("job")
            .job;
        let record = JobInferenceUsage {
            id: UsageEventId::new(),
            job_id: job.id,
            logical_attempt_id: GenerationAttemptId::new(),
            model_profile_id: ModelProfileId::new(),
            model_revision: Revision::new(1),
            provider_account_id: ProviderAccountId::new(),
            provider_account_revision: Revision::new(1),
            admitted_at: TimestampMillis::new(1),
            result: None,
        };
        let mut missing = record.clone();
        missing.job_id = lettuce_types::JobId::new();
        assert_eq!(
            database.admit_job_usage(missing),
            Err(UsageLedgerError::Conflict)
        );
        database.admit_job_usage(record.clone()).expect("admission");
        let mut legacy = record.clone();
        legacy.id = UsageEventId::new();
        database
            .admit_job_usage(legacy.clone())
            .expect("legacy admission");
        let encoded = crate::encode_versioned(&old, 1).expect("legacy envelope");
        let encoded = encoded.replace(",\"provider_response_id\":null", "");
        assert!(!encoded.contains("provider_response_id"));
        database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE job_inference_usage SET result_json=?2 WHERE id=?1",
                rusqlite::params![legacy.id.to_string(), encoded],
            )
            .expect("legacy response");
        database
            .settle_job_usage(legacy.id, old.clone())
            .expect("old response replay");
        legacy.result = Some(old);
        let basis = UsageCostBasis {
            openrouter: None,
            model_profile_id: record.model_profile_id,
            provider_account_id: record.provider_account_id,
            source: "OpenRouter endpoint snapshot".into(),
            captured_at: TimestampMillis::new(2),
            pricing: ModelPricing {
                prompt: "0.001".into(),
                completion: "0.002".into(),
                request: String::new(),
                image: String::new(),
                image_output: String::new(),
                web_search: String::new(),
                internal_reasoning: String::new(),
                input_cache_read: String::new(),
                input_cache_write: String::new(),
            },
            input: OpenRouterCostInput {
                prompt_tokens: 120,
                completion_tokens: 30,
                cached_prompt_tokens: 10,
                cache_write_tokens: 5,
                reasoning_tokens: 2,
                web_search_requests: 0,
                authoritative_total_cost: Some(0.25),
            },
        };
        for id in [record.id, UsageEventId::new()] {
            assert!(matches!(
                database.record_job_cost(id, basis.clone()),
                Err(UsageLedgerError::Invalid)
            ));
            assert!(database.get_job_cost(id).expect("no cost").is_none());
        }
        drop(database);
        let database = Database::open(&path).expect("reopen pending");
        let pending = database.job_usage(job.id).expect("pending");
        assert_eq!(pending.len(), 2);
        assert!(pending.contains(&record));
        assert!(pending.contains(&legacy));
        database
            .settle_job_usage(record.id, JobInferenceUsageResult::InferenceFailed)
            .expect("settlement");
        drop(database);
        let database = Database::open(&path).expect("reopen settled");
        let mut settled = record.clone();
        settled.result = Some(JobInferenceUsageResult::InferenceFailed);
        let saved = database.job_usage(job.id).expect("settled");
        assert_eq!(saved.len(), 2);
        assert!(saved.contains(&settled));
        assert!(saved.contains(&legacy));
        database
            .admit_job_usage(record.clone())
            .expect("replay admission after settlement");
        assert!(matches!(
            database.record_job_cost(record.id, basis.clone()),
            Err(UsageLedgerError::Invalid)
        ));
        for result in [
            JobInferenceUsageResult::Cancelled,
            JobInferenceUsageResult::Response {
                usage: None,
                provider_response_id: None,
            },
        ] {
            let mut unavailable = record.clone();
            unavailable.id = UsageEventId::new();
            database
                .admit_job_usage(unavailable.clone())
                .expect("admit");
            database
                .settle_job_usage(unavailable.id, result)
                .expect("settle");
            assert!(matches!(
                database.record_job_cost(unavailable.id, basis.clone()),
                Err(UsageLedgerError::Invalid)
            ));
        }
        let mut retry = record.clone();
        retry.id = UsageEventId::new();
        database
            .admit_job_usage(retry.clone())
            .expect("retry dispatch");
        let result = JobInferenceUsageResult::Response {
            provider_response_id: Some("gen-retry".into()),
            usage: Some(InferenceUsage {
                input_tokens: 120,
                output_tokens: 30,
                cached_input_tokens: Some(10),
                cache_write_tokens: Some(5),
                reasoning_tokens: Some(2),
                web_search_requests: Some(0),
                provider_reported_cost: lettuce_conversations::ProviderReportedCost::new(0.25),
            }),
        };
        database
            .settle_job_usage(retry.id, result.clone())
            .expect("response");
        let mut changed_identity = result.clone();
        if let JobInferenceUsageResult::Response {
            provider_response_id,
            ..
        } = &mut changed_identity
        {
            *provider_response_id = Some("gen-other".into());
        }
        assert_eq!(
            database.settle_job_usage(retry.id, changed_identity),
            Err(UsageLedgerError::Conflict)
        );
        retry.result = Some(result);
        for field in 0..10 {
            let mut wrong = basis.clone();
            match field {
                0 => wrong.model_profile_id = ModelProfileId::new(),
                1 => wrong.provider_account_id = ProviderAccountId::new(),
                2 => wrong.input.prompt_tokens += 1,
                3 => wrong.input.completion_tokens += 1,
                4 => wrong.input.cached_prompt_tokens += 1,
                5 => wrong.input.cache_write_tokens += 1,
                6 => wrong.input.reasoning_tokens += 1,
                7 => wrong.input.web_search_requests += 1,
                8 => wrong.input.authoritative_total_cost = None,
                _ => wrong.source.clear(),
            }
            assert!(matches!(
                database.record_job_cost(retry.id, wrong),
                Err(UsageLedgerError::Invalid)
            ));
        }
        assert!(
            database
                .get_job_cost(retry.id)
                .expect("no invalid cost")
                .is_none()
        );
        let cost = database
            .record_job_cost(retry.id, basis.clone())
            .expect("cost");
        assert_eq!(cost.cost.total_cost, 0.25);
        assert_eq!(
            database
                .record_job_cost(retry.id, basis.clone())
                .expect("replay")
                .basis,
            basis
        );
        let mut changed = basis.clone();
        changed.pricing.prompt = "0.004".into();
        assert!(matches!(
            database.record_job_cost(retry.id, changed),
            Err(UsageLedgerError::Conflict)
        ));
        assert!(
            database
                .get_cost(retry.id)
                .expect("separate conversation ledger")
                .is_none()
        );
        let evidence = database.job_usage(job.id).expect("retained evidence");
        assert!(evidence.contains(&settled));
        assert!(evidence.contains(&retry));
        let db = database.connection().expect("connection");
        assert!(
            db.execute("UPDATE job_usage_costs SET basis_json='{}'", [])
                .is_err()
        );
        assert!(db.execute("DELETE FROM job_usage_costs", []).is_err());
        assert!(
            db.execute("UPDATE job_inference_usage SET result_json=NULL", [])
                .is_err()
        );
        assert!(db.execute("DELETE FROM job_inference_usage", []).is_err());
        db.execute("DELETE FROM jobs WHERE id=?1", [job.id.to_string()])
            .expect("job retention cleanup");
        drop(db);
        assert_eq!(
            database.job_usage(job.id).expect("usage after cleanup"),
            evidence
        );
        drop(database);
        let database = Database::open(&path).expect("reopen costs after cleanup");
        let cost = database
            .get_job_cost(retry.id)
            .expect("read cost")
            .expect("saved cost");
        assert_eq!(cost.basis, basis);
        assert_eq!(cost.cost.total_cost, 0.25);
        assert_eq!(
            database
                .record_job_cost(retry.id, basis)
                .expect("replay after cleanup")
                .basis,
            cost.basis
        );
        assert_eq!(
            database.job_usage(job.id).expect("unchanged evidence"),
            evidence
        );
        assert!(
            database
                .get_job_cost(record.id)
                .expect("failed dispatch")
                .is_none()
        );
        drop(database);
        std::fs::remove_file(path).expect("remove test database");
    }

    use lettuce_conversations::{InferenceUsage, UsageCounters, UsageOutcome, UsageRecord};
    use lettuce_types::{
        ConversationId, GenerationAttemptId, GenerationTurnId, ModelProfileId, ProviderAccountId,
        Revision, TimestampMillis,
    };
    use lettuce_usage::{UsageLedger, UsageLedgerError};

    use crate::Database;

    fn fixture() -> (Database, UsageRecord) {
        let database = Database::open_in_memory().expect("database");
        database
            .connection()
            .expect("connection")
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("fixture mode");
        let conversation_id = ConversationId::new();
        let turn_id = GenerationTurnId::new();
        let attempt_id = GenerationAttemptId::new();
        database
            .connection()
            .expect("connection")
            .execute(
                "INSERT INTO generation_attempts (
                    conversation_id, turn_id, id, ordinal, parent_attempt_id,
                    status, job_idempotency_key, job_id, started_at, finished_at,
                    usage_event_id, usage_outcome, failure
                 ) VALUES (?1, ?2, ?3, 0, NULL, 'running', ?4, NULL, 1, NULL,
                           NULL, NULL, NULL)",
                rusqlite::params![
                    conversation_id.to_string(),
                    turn_id.to_string(),
                    attempt_id.to_string(),
                    format!("generation.{turn_id}.{attempt_id}"),
                ],
            )
            .expect("attempt");
        (
            database,
            UsageRecord {
                turn_id,
                attempt_id,
                outcome: UsageOutcome::Succeeded,
                usage: UsageCounters::Known(InferenceUsage {
                    provider_reported_cost: None,
                    cache_write_tokens: None,
                    web_search_requests: None,
                    cached_input_tokens: None,
                    reasoning_tokens: None,
                    input_tokens: 120,
                    output_tokens: 30,
                }),
                model_profile_id: Some(ModelProfileId::new()),
                model_revision: Some(Revision::new(3)),
                provider_account_id: Some(ProviderAccountId::new()),
                provider_account_revision: Some(Revision::new(4)),
                recorded_at: TimestampMillis::new(10),
            },
        )
    }

    #[test]
    fn usage_event_round_trips_and_exact_retry_keeps_identity() {
        let (database, mut record) = fixture();
        if let UsageCounters::Known(usage) = &mut record.usage {
            usage.cached_input_tokens = Some(0);
            usage.reasoning_tokens = Some(12);
            usage.cache_write_tokens = Some(3);
            usage.web_search_requests = Some(0);
            usage.provider_reported_cost = lettuce_conversations::ProviderReportedCost::new(0.0125);
        }
        let first = UsageLedger::record(&database, record.clone()).expect("record");
        let mut changed = record.clone();
        if let UsageCounters::Known(usage) = &mut changed.usage {
            usage.cached_input_tokens = None;
        }
        assert_eq!(
            UsageLedger::record(&database, changed),
            Err(UsageLedgerError::Conflict)
        );
        let retry = UsageLedger::record(&database, record).expect("retry");
        assert_eq!(retry, first);
        assert_eq!(UsageLedger::get(&database, first.id), Ok(Some(first)));
    }

    #[test]
    fn costs_retain_pricing_and_counters_without_rewriting_usage() {
        use lettuce_usage::{ModelPricing, OpenRouterCostInput, UsageCostBasis, UsageCostLedger};
        let (database, record) = fixture();
        let event = UsageLedger::record(&database, record).expect("event");
        let basis = UsageCostBasis {
            openrouter: None,
            model_profile_id: event.record.model_profile_id.expect("model"),
            provider_account_id: event.record.provider_account_id.expect("provider"),
            source: "OpenRouter endpoint snapshot".into(),
            captured_at: TimestampMillis::new(11),
            pricing: ModelPricing {
                prompt: "0.001".into(),
                completion: "0.002".into(),
                request: String::new(),
                image: String::new(),
                image_output: String::new(),
                web_search: String::new(),
                internal_reasoning: String::new(),
                input_cache_read: String::new(),
                input_cache_write: String::new(),
            },
            input: OpenRouterCostInput {
                prompt_tokens: 120,
                completion_tokens: 30,
                ..Default::default()
            },
        };
        let cost = database.record_cost(event.id, basis.clone()).expect("cost");
        let mut detailed = event.clone();
        if let UsageCounters::Known(usage) = &mut detailed.record.usage {
            usage.cache_write_tokens = Some(0);
            usage.web_search_requests = Some(0);
        }
        assert!(basis.calculate(&detailed).is_ok());
        let mut reported = event.clone();
        if let UsageCounters::Known(usage) = &mut reported.record.usage {
            usage.provider_reported_cost = lettuce_conversations::ProviderReportedCost::new(0.25);
        }
        assert!(matches!(
            basis.calculate(&reported),
            Err(UsageLedgerError::Invalid)
        ));
        let mut reported_basis = basis.clone();
        reported_basis.input.authoritative_total_cost = Some(0.25);
        assert_eq!(
            reported_basis
                .calculate(&reported)
                .expect("reported cost")
                .total_cost,
            0.25
        );
        let mut mismatched = basis.clone();
        mismatched.input.cache_write_tokens = 1;
        assert!(matches!(
            mismatched.calculate(&detailed),
            Err(UsageLedgerError::Invalid)
        ));
        mismatched.input.cache_write_tokens = 0;
        mismatched.input.web_search_requests = 1;
        assert!(matches!(
            mismatched.calculate(&detailed),
            Err(UsageLedgerError::Invalid)
        ));
        assert!((cost.cost.total_cost - 0.18).abs() < 1e-12);
        assert_eq!(
            database
                .record_cost(event.id, basis.clone())
                .expect("retry")
                .basis,
            basis
        );
        assert_eq!(
            database
                .get_cost(event.id)
                .expect("read")
                .expect("cost")
                .basis,
            basis
        );
        let mut changed = basis.clone();
        changed.pricing.prompt = "0.004".into();
        assert!(matches!(
            database.record_cost(event.id, changed),
            Err(UsageLedgerError::Conflict)
        ));
        let mut wrong = basis.clone();
        wrong.input.prompt_tokens = 121;
        assert!(matches!(
            database.record_cost(event.id, wrong),
            Err(UsageLedgerError::Invalid)
        ));
        let mut wrong = basis.clone();
        wrong.model_profile_id = ModelProfileId::new();
        assert!(matches!(
            database.record_cost(event.id, wrong),
            Err(UsageLedgerError::Invalid)
        ));
        assert_eq!(
            UsageLedger::get(&database, event.id).expect("raw event"),
            Some(event.clone())
        );
        assert!(
            database
                .connection()
                .expect("connection")
                .execute(
                    "UPDATE usage_costs SET basis_json='{}' WHERE event_id=?1",
                    [event.id.to_string()]
                )
                .is_err()
        );
        assert!(
            database
                .connection()
                .expect("connection")
                .execute(
                    "DELETE FROM usage_costs WHERE event_id=?1",
                    [event.id.to_string()]
                )
                .is_err()
        );
        let (unknown_database, mut unknown) = fixture();
        unknown.usage = UsageCounters::Unavailable(
            lettuce_conversations::UsageUnavailableReason::ProviderOmitted,
        );
        let unknown = UsageLedger::record(&unknown_database, unknown).expect("unavailable event");
        assert!(matches!(
            unknown_database.record_cost(unknown.id, basis),
            Err(UsageLedgerError::Invalid)
        ));
        assert!(
            unknown_database
                .get_cost(unknown.id)
                .expect("no fabricated cost")
                .is_none()
        );
    }

    #[test]
    fn changed_evidence_conflicts_and_sql_rows_are_immutable() {
        let (database, record) = fixture();
        let event = UsageLedger::record(&database, record.clone()).expect("record");
        let mut changed = record;
        changed.usage = UsageCounters::Known(InferenceUsage {
            provider_reported_cost: None,
            cache_write_tokens: None,
            web_search_requests: None,
            cached_input_tokens: None,
            reasoning_tokens: None,
            input_tokens: 121,
            output_tokens: 30,
        });
        assert_eq!(
            UsageLedger::record(&database, changed),
            Err(UsageLedgerError::Conflict)
        );
        assert!(
            database
                .connection()
                .expect("connection")
                .execute(
                    "UPDATE usage_events SET input_tokens = 1 WHERE id = ?1",
                    [event.id.to_string()],
                )
                .is_err()
        );
    }
}
