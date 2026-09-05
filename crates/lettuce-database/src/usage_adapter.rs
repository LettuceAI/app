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
    })
}

struct RawUsageEvent {
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
    provider_account_id, provider_account_revision, recorded_at, cached_input_tokens, reasoning_tokens FROM usage_events";

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
        let (cached, reasoning) = match &record.usage {
            UsageCounters::Known(usage) => (usage.cached_input_tokens, usage.reasoning_tokens),
            UsageCounters::Unavailable(_) => (None, None),
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
                    provider_account_revision, recorded_at, cached_input_tokens, reasoning_tokens)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
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
