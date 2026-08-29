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
    })
}

struct RawUsageEvent {
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
    provider_account_id, provider_account_revision, recorded_at FROM usage_events";

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
                    provider_account_revision, recorded_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
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
        let (database, record) = fixture();
        let first = UsageLedger::record(&database, record.clone()).expect("record");
        let retry = UsageLedger::record(&database, record).expect("retry");
        assert_eq!(retry, first);
        assert_eq!(UsageLedger::get(&database, first.id), Ok(Some(first)));
    }

    #[test]
    fn changed_evidence_conflicts_and_sql_rows_are_immutable() {
        let (database, record) = fixture();
        let event = UsageLedger::record(&database, record.clone()).expect("record");
        let mut changed = record;
        changed.usage = UsageCounters::Known(InferenceUsage {
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
