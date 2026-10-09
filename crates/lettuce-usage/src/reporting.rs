use std::collections::{BTreeMap, BTreeSet};

use lettuce_conversations::{InferenceUsage, UsageCounters, UsageOutcome, UsageRecordSnapshot};
use lettuce_types::ConversationId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageReportStatus {
    #[default]
    Pending,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

impl From<UsageOutcome> for UsageReportStatus {
    fn from(value: UsageOutcome) -> Self {
        match value {
            UsageOutcome::Succeeded => Self::Succeeded,
            UsageOutcome::Failed => Self::Failed,
            UsageOutcome::Cancelled => Self::Cancelled,
            UsageOutcome::Interrupted => Self::Interrupted,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UsageReportRow {
    pub id: String,
    pub timestamp: i64,
    pub status: UsageReportStatus,
    pub session_id: Option<String>,
    pub character_id: Option<String>,
    pub character_name: Option<String>,
    pub model_id: Option<String>,
    pub model_name: Option<String>,
    pub provider_kind: Option<String>,
    pub provider_label: Option<String>,
    pub operation_type: Option<String>,
    pub finish_reason: Option<String>,
    pub provider_response_id: Option<String>,
    pub prompt_tokens: Option<u64>,
    pub cached_prompt_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub image_tokens: Option<u64>,
    pub audio_tokens: Option<u64>,
    pub web_search_requests: Option<u64>,
    pub total_tokens: Option<u64>,
    pub memory_tokens: Option<u64>,
    pub summary_tokens: Option<u64>,
    pub input_image_count: Option<u64>,
    pub output_image_count: Option<u64>,
    pub prompt_cost: Option<f64>,
    pub cache_read_cost: Option<f64>,
    pub cache_write_cost: Option<f64>,
    pub completion_cost: Option<f64>,
    pub reasoning_cost: Option<f64>,
    pub request_cost: Option<f64>,
    pub web_search_cost: Option<f64>,
    pub total_cost: Option<f64>,
    pub api_cost: Option<f64>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum UsageReportError {
    #[error("usage report storage failed")]
    Storage,
    #[error("usage report evidence is invalid")]
    InvalidData,
    #[error("usage report exceeds numeric range")]
    Overflow,
    #[error("usage report cursor is invalid")]
    InvalidCursor,
    #[error("usage report date range is invalid")]
    InvalidRange,
    #[error("usage report time zone is invalid")]
    InvalidTimeZone,
}

pub trait UsageReportRepository: Send + Sync {
    fn read_usage_report(&self) -> Result<Vec<UsageReportRow>, UsageReportError>;
}

#[derive(Debug)]
pub struct UsageReportEvidence {
    pub events: Vec<(
        crate::UsageEvent,
        Option<crate::UsageCostBasis>,
        Option<ConversationId>,
    )>,
    pub dispatches: Vec<(crate::JobInferenceUsage, Option<crate::UsageCostBasis>)>,
    pub legacy: Vec<UsageReportRow>,
    pub tombstones: Vec<crate::UsageTombstone>,
}

impl UsageReportEvidence {
    pub fn charge_rows(self) -> Result<Vec<UsageReportRow>, UsageReportError> {
        let dispatch_counts =
            self.dispatches
                .iter()
                .fold(BTreeMap::new(), |mut counts, (event, _)| {
                    *counts.entry(event.logical_attempt_id).or_insert(0usize) += 1;
                    counts
                });
        let aggregate_costs = self
            .events
            .iter()
            .filter_map(|(event, basis, _)| {
                basis
                    .as_ref()
                    .map(|basis| (event.record.attempt_id, (event, basis)))
            })
            .collect::<BTreeMap<_, _>>();
        let mut overlaps = self
            .dispatches
            .iter()
            .map(|(event, _)| event.logical_attempt_id)
            .collect::<BTreeSet<_>>();
        let cleared_attempts = self
            .tombstones
            .iter()
            .filter_map(|proof| match proof {
                crate::UsageTombstone::Dispatch { attempt_id, .. } => Some(*attempt_id),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        overlaps.extend(cleared_attempts.iter().copied());
        let terminals = self
            .events
            .iter()
            .map(|(event, _, conversation)| {
                (event.record.attempt_id, (&event.record, conversation))
            })
            .collect::<BTreeMap<_, _>>();
        let mut rows = Vec::new();
        for (event, basis) in &self.dispatches {
            event
                .validate_snapshot()
                .map_err(|_| UsageReportError::InvalidData)?;
            let terminal = terminals.get(&event.logical_attempt_id);
            let mut row = UsageReportRow {
                id: event.id.to_string(),
                timestamp: event.admitted_at.get(),
                model_id: Some(event.model_profile_id.to_string()),
                session_id: terminal
                    .and_then(|(_, conversation)| conversation.map(|id| id.to_string())),
                ..UsageReportRow::default()
            };
            match &event.result {
                Some(crate::JobInferenceUsageResult::Response {
                    snapshot,
                    usage,
                    provider_response_id,
                }) => {
                    row.status = terminal.map_or(UsageReportStatus::Succeeded, |(record, _)| {
                        record.outcome.into()
                    });
                    row.apply_snapshot(snapshot.as_deref().or(event.snapshot.as_ref()));
                    row.provider_response_id = provider_response_id.clone();
                    if let Some(usage) = usage {
                        row.apply_usage(usage);
                    }
                }
                Some(crate::JobInferenceUsageResult::Failure { cancelled, snapshot }) => {
                    row.status = if *cancelled { UsageReportStatus::Cancelled } else { UsageReportStatus::Failed };
                    row.apply_snapshot(Some(snapshot.as_ref()));
                }
                Some(crate::JobInferenceUsageResult::InferenceFailed) => {
                    row.status = UsageReportStatus::Failed;
                    row.apply_snapshot(event.snapshot.as_ref());
                }
                Some(crate::JobInferenceUsageResult::Cancelled) => {
                    row.status = UsageReportStatus::Cancelled;
                    row.apply_snapshot(event.snapshot.as_ref());
                }
                None => row.apply_snapshot(event.snapshot.as_ref()),
            }
            if let Some((owner, _)) = terminal {
                if let Some(snapshot) = &owner.snapshot {
                    if row.character_id.is_none() && row.character_name.is_none() {
                        row.character_id = snapshot.character_id.map(|id| id.to_string());
                        row.character_name = snapshot.character_name.clone();
                    }
                    if row.error_message.is_none()
                        && row.status == UsageReportStatus::from(owner.outcome)
                    {
                        row.error_message = snapshot.error_message.clone();
                    }
                }
            }
            if let Some(basis) = basis {
                row.apply_cost(
                    basis
                        .calculate_job(event)
                        .map_err(|_| UsageReportError::InvalidData)?,
                );
            } else if dispatch_counts.get(&event.logical_attempt_id) == Some(&1)
                && !cleared_attempts.contains(&event.logical_attempt_id)
                && let Some((aggregate, basis)) = aggregate_costs.get(&event.logical_attempt_id)
                && aggregate.origin == crate::UsageEventOrigin::Live
                && aggregate.record.model_profile_id == Some(event.model_profile_id)
                && aggregate.record.model_revision == Some(event.model_revision)
                && aggregate.record.provider_account_id == Some(event.provider_account_id)
                && aggregate.record.provider_account_revision
                    == Some(event.provider_account_revision)
                && let UsageCounters::Known(terminal_usage) = &aggregate.record.usage
                && let Some(crate::JobInferenceUsageResult::Response {
                    usage: Some(dispatch_usage),
                    ..
                }) = &event.result
                && terminal_usage == dispatch_usage
            {
                row.apply_cost(
                    basis
                        .calculate(aggregate)
                        .map_err(|_| UsageReportError::InvalidData)?,
                );
            }
            rows.push(row);
        }
        for (event, basis, conversation) in &self.events {
            if event.origin == crate::UsageEventOrigin::LegacyImport
                || overlaps.contains(&event.record.attempt_id)
            {
                continue;
            }
            event
                .record
                .validate()
                .map_err(|_| UsageReportError::InvalidData)?;
            let mut row = UsageReportRow {
                id: event.id.to_string(),
                timestamp: event.record.recorded_at.get(),
                status: event.record.outcome.into(),
                session_id: conversation.map(|id| id.to_string()),
                model_id: event.record.model_profile_id.map(|id| id.to_string()),
                ..UsageReportRow::default()
            };
            row.apply_snapshot(event.record.snapshot.as_ref());
            if let UsageCounters::Known(usage) = &event.record.usage {
                row.apply_usage(usage);
            }
            if let Some(basis) = basis {
                row.apply_cost(
                    basis
                        .calculate(event)
                        .map_err(|_| UsageReportError::InvalidData)?,
                );
            }
            rows.push(row);
        }
        rows.extend(self.legacy);
        let mut ids = BTreeSet::new();
        for row in &rows {
            if row.id.is_empty() || !ids.insert(&row.id) {
                return Err(UsageReportError::InvalidData);
            }
            row.validate()?;
        }
        rows.sort_by(|a, b| (a.timestamp, &a.id).cmp(&(b.timestamp, &b.id)));
        Ok(rows)
    }
}

impl UsageReportRow {
    pub fn validate(&self) -> Result<(), UsageReportError> {
        for value in [
            self.prompt_cost,
            self.cache_read_cost,
            self.cache_write_cost,
            self.completion_cost,
            self.reasoning_cost,
            self.request_cost,
            self.web_search_cost,
            self.total_cost,
            self.api_cost,
        ]
        .into_iter()
        .flatten()
        {
            if !value.is_finite() {
                return Err(UsageReportError::InvalidData);
            }
        }
        Ok(())
    }

    fn apply_snapshot(&mut self, snapshot: Option<&UsageRecordSnapshot>) {
        if let Some(snapshot) = snapshot {
            self.character_id = snapshot.character_id.map(|id| id.to_string());
            self.character_name = snapshot.character_name.clone();
            self.model_name = snapshot.model_name.clone();
            self.provider_kind = snapshot.provider_kind.clone();
            self.provider_label = snapshot.provider_label.clone();
            self.operation_type = snapshot.operation_kind.clone();
            self.finish_reason = snapshot.finish_reason.clone();
            self.error_message = snapshot.error_message.clone();
            self.memory_tokens = snapshot.memory_tokens;
            self.summary_tokens = snapshot.summary_tokens;
            self.provider_response_id = snapshot.provider_response_id.clone();
        }
    }

    fn apply_usage(&mut self, usage: &InferenceUsage) {
        self.prompt_tokens = Some(usage.input_tokens);
        self.completion_tokens = Some(usage.output_tokens);
        self.cached_prompt_tokens = usage.cached_input_tokens;
        self.cache_write_tokens = usage.cache_write_tokens;
        self.reasoning_tokens = usage.reasoning_tokens;
        self.image_tokens = usage.image_tokens;
        self.audio_tokens = usage.audio_tokens;
        self.web_search_requests = usage.web_search_requests;
        self.total_tokens = usage.total_tokens;
        self.api_cost = usage.provider_reported_cost.map(|cost| cost.get());
        self.total_cost = self.api_cost;
    }

    fn apply_cost(&mut self, cost: crate::RequestCost) {
        self.prompt_cost = Some(cost.prompt_cost);
        self.cache_read_cost = Some(cost.cache_read_cost);
        self.cache_write_cost = Some(cost.cache_write_cost);
        self.completion_cost = Some(cost.completion_cost);
        self.reasoning_cost = Some(cost.reasoning_cost);
        self.request_cost = Some(cost.request_cost);
        self.web_search_cost = Some(cost.web_search_cost);
        self.total_cost = Some(cost.total_cost);
    }
}

pub const USAGE_CSV_HEADER: &str = "timestamp,session_id,character_name,model_name,provider_label,operation_type,prompt_tokens,cached_prompt_tokens,cache_write_tokens,completion_tokens,reasoning_tokens,image_tokens,audio_tokens,web_search_requests,total_tokens,memory_tokens,summary_tokens,input_image_count,output_image_count,prompt_cost,cache_read_cost,cache_write_cost,completion_cost,reasoning_cost,request_cost,web_search_cost,total_cost,api_cost,success,error_message";

pub fn usage_csv(rows: &[UsageReportRow]) -> Result<String, UsageReportError> {
    let mut csv = format!("{USAGE_CSV_HEADER}\n");
    let mut sorted = rows.iter().collect::<Vec<_>>();
    sorted.sort_by(|a, b| (a.timestamp, &a.id).cmp(&(b.timestamp, &b.id)));
    for row in sorted {
        row.validate()?;
        let text = |value: &Option<String>| value.clone().unwrap_or_default();
        let number = |value: Option<u64>| value.map(|value| value.to_string()).unwrap_or_default();
        let money = |value: Option<f64>| value.map(|value| value.to_string()).unwrap_or_default();
        let success = match row.status {
            UsageReportStatus::Pending => String::new(),
            UsageReportStatus::Succeeded => "yes".into(),
            _ => "no".into(),
        };
        let fields = [
            row.timestamp.to_string(),
            text(&row.session_id),
            text(&row.character_name),
            text(&row.model_name),
            text(&row.provider_label),
            text(&row.operation_type),
            number(row.prompt_tokens),
            number(row.cached_prompt_tokens),
            number(row.cache_write_tokens),
            number(row.completion_tokens),
            number(row.reasoning_tokens),
            number(row.image_tokens),
            number(row.audio_tokens),
            number(row.web_search_requests),
            number(row.total_tokens),
            number(row.memory_tokens),
            number(row.summary_tokens),
            number(row.input_image_count),
            number(row.output_image_count),
            money(row.prompt_cost),
            money(row.cache_read_cost),
            money(row.cache_write_cost),
            money(row.completion_cost),
            money(row.reasoning_cost),
            money(row.request_cost),
            money(row.web_search_cost),
            money(row.total_cost),
            money(row.api_cost),
            success,
            text(&row.error_message),
        ];
        for (index, field) in fields.into_iter().enumerate() {
            if index != 0 {
                csv.push(',');
            }
            if field.contains([',', '"', '\n', '\r']) {
                csv.push('"');
                csv.push_str(&field.replace('"', "\"\""));
                csv.push('"');
            } else {
                csv.push_str(&field);
            }
        }
        csv.push('\n');
    }
    Ok(csv)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_conversations::{InferenceUsage, UsageCounters, UsageOutcome, UsageRecord};
    use lettuce_types::{
        GenerationAttemptId, GenerationTurnId, JobId, ModelProfileId, ProviderAccountId, Revision,
        TimestampMillis, UsageEventId,
    };

    fn evidence() -> UsageReportEvidence {
        let attempt = GenerationAttemptId::new();
        let event = crate::UsageEvent {
            id: UsageEventId::new(),
            origin: crate::UsageEventOrigin::Live,
            record: UsageRecord {
                snapshot: None,
                turn_id: GenerationTurnId::new(),
                attempt_id: attempt,
                outcome: UsageOutcome::Succeeded,
                usage: UsageCounters::Known(InferenceUsage {
                    input_tokens: 10,
                    output_tokens: 5,
                    image_tokens: None,
                    audio_tokens: None,
                    total_tokens: None,
                    cached_input_tokens: None,
                    reasoning_tokens: None,
                    cache_write_tokens: None,
                    web_search_requests: None,
                    provider_reported_cost: None,
                }),
                model_profile_id: Some(ModelProfileId::new()),
                model_revision: Some(Revision::new(1)),
                provider_account_id: Some(ProviderAccountId::new()),
                provider_account_revision: Some(Revision::new(1)),
                recorded_at: TimestampMillis::new(2),
            },
        };
        let dispatch = crate::JobInferenceUsage {
            snapshot: None,
            id: UsageEventId::new(),
            job_id: JobId::new(),
            logical_attempt_id: attempt,
            model_profile_id: event.record.model_profile_id.expect("model"),
            model_revision: Revision::new(1),
            provider_account_id: event.record.provider_account_id.expect("account"),
            provider_account_revision: Revision::new(1),
            admitted_at: TimestampMillis::new(1),
            result: Some(crate::JobInferenceUsageResult::Response {
                snapshot: None,
                usage: match &event.record.usage {
                    UsageCounters::Known(value) => Some(value.clone()),
                    _ => unreachable!(),
                },
                provider_response_id: None,
            }),
        };
        UsageReportEvidence {
            events: vec![(event, None, None)],
            dispatches: vec![(dispatch, None)],
            legacy: Vec::new(),
            tombstones: Vec::new(),
        }
    }

    #[test]
    fn failed_dispatch_retains_its_error_and_finish_snapshot_without_a_terminal_owner() {
        let mut input = evidence();
        input.events.clear();
        let admitted = lettuce_conversations::UsageRecordSnapshot {
            character_name: Some("Ada".into()),
            operation_kind: Some("reply_helper".into()),
            ..Default::default()
        };
        let mut failed = admitted.clone();
        failed.finish_reason = Some("error".into());
        failed.error_message = Some("provider unavailable".into());
        input.dispatches[0].0.snapshot = Some(admitted);
        input.dispatches[0].0.result = Some(serde_json::from_value(serde_json::json!({"Failure": {"cancelled": false, "snapshot": failed}})).expect("persistent failure snapshot"));
        let mut corrupt = UsageReportEvidence { events: Vec::new(), dispatches: input.dispatches.clone(), legacy: Vec::new(), tombstones: Vec::new() };
        let rows = input.charge_rows().expect("failed report");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].character_name.as_deref(), Some("Ada"));
        assert_eq!(rows[0].finish_reason.as_deref(), Some("error"));
        assert_eq!(rows[0].error_message.as_deref(), Some("provider unavailable"));
        assert_eq!(rows[0].status, UsageReportStatus::Failed);
        let snapshot = corrupt.dispatches[0].0.snapshot.as_mut().expect("admission");
        snapshot.character_name = Some("Renamed".into());
        assert_eq!(corrupt.charge_rows(), Err(UsageReportError::InvalidData));
    }

    #[test]
    fn a_chat_charge_and_its_dispatch_are_counted_once_without_collapsing_fallbacks() {
        let mut input = evidence();
        let first = input.dispatches[0].0.id;
        let mut fallback = input.dispatches[0].clone();
        fallback.0.id = UsageEventId::new();
        input.dispatches.push(fallback);
        let rows = input.charge_rows().expect("charges");
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|row| row.id == first.to_string()));
        assert!(rows.iter().all(|row| row.prompt_tokens == Some(10)));
    }

    #[test]
    fn clearing_dispatches_never_reintroduces_their_aggregate_charge() {
        let mut input = evidence();
        let (dispatch, _) = input.dispatches.pop().expect("dispatch");
        input.tombstones.push(crate::UsageTombstone::Dispatch {
            event_id: dispatch.id,
            attempt_id: dispatch.logical_attempt_id,
            job_id: dispatch.job_id,
        });
        assert!(input.charge_rows().expect("cleared charges").is_empty());
    }

    #[test]
    fn only_explicit_legacy_origin_is_excluded_and_real_legacy_rows_remain() {
        let mut input = evidence();
        input.dispatches.clear();
        input.events[0].0.origin = crate::UsageEventOrigin::LegacyImport;
        input.legacy.push(UsageReportRow {
            id: "historical-charge".into(),
            timestamp: 4,
            prompt_tokens: Some(7),
            ..UsageReportRow::default()
        });
        let rows = input.charge_rows().expect("historical charges");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "historical-charge");
        let mut live = evidence();
        live.dispatches.clear();
        assert_eq!(live.charge_rows().expect("live charges").len(), 1);
    }

    #[test]
    fn csv_keeps_the_legacy_header_and_leaves_unknown_values_empty() {
        let row = UsageReportRow {
            id: "charge".into(),
            timestamp: 42,
            character_name: Some("A, \"B\"".into()),
            status: UsageReportStatus::Succeeded,
            ..UsageReportRow::default()
        };
        let csv = usage_csv(&[row]).expect("CSV");
        let mut lines = csv.lines();
        assert_eq!(
            lines.next(),
            Some(
                "timestamp,session_id,character_name,model_name,provider_label,operation_type,prompt_tokens,cached_prompt_tokens,cache_write_tokens,completion_tokens,reasoning_tokens,image_tokens,audio_tokens,web_search_requests,total_tokens,memory_tokens,summary_tokens,input_image_count,output_image_count,prompt_cost,cache_read_cost,cache_write_cost,completion_cost,reasoning_cost,request_cost,web_search_cost,total_cost,api_cost,success,error_message"
            )
        );
        assert_eq!(
            lines.next(),
            Some("42,,\"A, \"\"B\"\"\",,,,,,,,,,,,,,,,,,,,,,,,,,yes,")
        );
        assert!(lines.next().is_none());
        let failed = UsageReportRow {
            id: "failed".into(),
            timestamp: 43,
            status: UsageReportStatus::Failed,
            ..UsageReportRow::default()
        };
        let pending = UsageReportRow {
            id: "pending".into(),
            timestamp: 44,
            status: UsageReportStatus::Pending,
            ..UsageReportRow::default()
        };
        let csv = usage_csv(&[failed, pending]).expect("CSV");
        let mut lines = csv.lines().skip(1);
        assert_eq!(lines.next(), Some("43,,,,,,,,,,,,,,,,,,,,,,,,,,,,no,"));
        assert_eq!(lines.next(), Some("44,,,,,,,,,,,,,,,,,,,,,,,,,,,,,"));
    }

    #[test]
    fn reporting_preserves_finite_historical_completion_adjustments() {
        let row = UsageReportRow {
            id: "legacy-adjustment".into(),
            prompt_cost: Some(0.01),
            completion_cost: Some(-0.002),
            total_cost: Some(0.008),
            ..UsageReportRow::default()
        };
        let csv = usage_csv(&[row]).expect("historical adjustment");
        assert!(csv.contains(",-0.002,"));
    }

    #[test]
    fn dispatch_charge_keeps_its_failed_owners_known_terminal_attribution() {
        let mut input = evidence();
        input.events[0].0.record.outcome = UsageOutcome::Failed;
        input.events[0].0.record.snapshot = Some(UsageRecordSnapshot {
            character_name: Some("Ada".into()),
            operation_kind: Some("chat".into()),
            error_message: Some("invalid reply".into()),
            ..UsageRecordSnapshot::default()
        });
        let rows = input.charge_rows().expect("failed charge");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, UsageReportStatus::Failed);
        assert_eq!(rows[0].character_name.as_deref(), Some("Ada"));
        assert_eq!(rows[0].error_message.as_deref(), Some("invalid reply"));
    }

    #[test]
    fn reported_total_cost_remains_known_without_a_calculated_basis() {
        let mut input = evidence();
        if let Some(crate::JobInferenceUsageResult::Response {
            usage: Some(usage), ..
        }) = &mut input.dispatches[0].0.result
        {
            usage.provider_reported_cost = lettuce_conversations::ProviderReportedCost::new(0.25);
        }
        let rows = input.charge_rows().expect("reported cost");
        assert_eq!(rows[0].api_cost, Some(0.25));
        assert_eq!(rows[0].total_cost, Some(0.25));
        assert_eq!(rows[0].prompt_cost, None);
    }
}
