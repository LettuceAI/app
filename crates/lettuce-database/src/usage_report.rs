use std::collections::BTreeMap;

use lettuce_usage::{UsageReportError, UsageReportEvidence, UsageReportRepository, UsageReportRow};

impl UsageReportRepository for crate::Database {
    fn read_usage_report(&self) -> Result<Vec<UsageReportRow>, UsageReportError> {
        let mut connection = self.connection().map_err(|_| UsageReportError::Storage)?;
        let transaction = connection
            .transaction()
            .map_err(|_| UsageReportError::Storage)?;
        let costs = read_costs(&transaction, "usage_costs")?;
        let conversations = transaction
            .prepare("SELECT id,conversation_id FROM usage_events")
            .map_err(|_| UsageReportError::Storage)?
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|_| UsageReportError::Storage)?
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map_err(|_| UsageReportError::Storage)?;
        let events = crate::usage_adapter::load_all_usage_in(&transaction)
            .map_err(|_| UsageReportError::InvalidData)?
            .into_iter()
            .map(|event| {
                let conversation = conversations
                    .get(&event.id.to_string())
                    .ok_or(UsageReportError::InvalidData)?
                    .parse()
                    .map_err(|_| UsageReportError::InvalidData)?;
                let basis = costs.get(&event.id.to_string()).cloned();
                Ok((event, basis, Some(conversation)))
            })
            .collect::<Result<Vec<_>, UsageReportError>>()?;
        let dispatches = read_dispatches(&transaction)?;
        let legacy = read_legacy(&transaction)?;
        let tombstones = crate::usage_clear::tombstones_in(&transaction)
            .map_err(|_| UsageReportError::InvalidData)?;
        let result = UsageReportEvidence {
            events,
            dispatches,
            legacy,
            tombstones,
        }
        .charge_rows()?;
        transaction
            .commit()
            .map_err(|_| UsageReportError::Storage)?;
        Ok(result)
    }
}

fn read_costs(
    transaction: &rusqlite::Transaction<'_>,
    table: &str,
) -> Result<BTreeMap<String, lettuce_usage::UsageCostBasis>, UsageReportError> {
    let mut statement = transaction
        .prepare(&format!("SELECT event_id,basis_json FROM {table}"))
        .map_err(|_| UsageReportError::Storage)?;
    statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|_| UsageReportError::Storage)?
        .map(|raw| {
            let (id, bytes) = raw.map_err(|_| UsageReportError::Storage)?;
            let basis =
                crate::decode_versioned(&bytes, 1).map_err(|_| UsageReportError::InvalidData)?;
            Ok((id, basis))
        })
        .collect()
}

type DispatchWithCost = (
    lettuce_usage::JobInferenceUsage,
    Option<lettuce_usage::UsageCostBasis>,
);

fn read_dispatches(
    transaction: &rusqlite::Transaction<'_>,
) -> Result<Vec<DispatchWithCost>, UsageReportError> {
    let mut statement = transaction.prepare("SELECT u.id,u.job_id,u.admitted_at,u.record_json,u.result_json,c.basis_json FROM job_inference_usage u LEFT JOIN job_usage_costs c ON c.event_id=u.id")
        .map_err(|_| UsageReportError::Storage)?;
    statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(|_| UsageReportError::Storage)?
        .map(|raw| {
            let (id, job, time, record, result, basis) =
                raw.map_err(|_| UsageReportError::Storage)?;
            let mut evidence: lettuce_usage::JobInferenceUsage =
                crate::decode_versioned(&record, 1).map_err(|_| UsageReportError::InvalidData)?;
            if evidence.id.to_string() != id
                || evidence.job_id.to_string() != job
                || evidence.admitted_at.get() != time
            {
                return Err(UsageReportError::InvalidData);
            }
            evidence.result = result
                .as_deref()
                .map(|bytes| crate::decode_versioned(bytes, 1))
                .transpose()
                .map_err(|_| UsageReportError::InvalidData)?;
            let basis = basis
                .as_deref()
                .map(|bytes| crate::decode_versioned(bytes, 1))
                .transpose()
                .map_err(|_| UsageReportError::InvalidData)?;
            Ok((evidence, basis))
        })
        .collect()
}

fn read_legacy(
    transaction: &rusqlite::Transaction<'_>,
) -> Result<Vec<UsageReportRow>, UsageReportError> {
    let mut statement = transaction.prepare("SELECT u.source_id,u.metadata_json,r.source_fingerprint,
        json_object('timestamp',u.recorded_at,'status',CASE u.success WHEN 1 THEN 'succeeded' ELSE 'failed' END,
        'session_id',u.session_source_id,'character_id',u.character_source_id,'character_name',u.character_name,
        'model_id',u.model_profile_id,'model_name',u.model_name,'provider_kind',u.provider_source_id,
        'provider_label',u.provider_label,'operation_type',u.operation_type,'finish_reason',u.finish_reason,
        'prompt_tokens',u.prompt_tokens,'completion_tokens',u.completion_tokens,'total_tokens',u.total_tokens,
        'memory_tokens',u.memory_tokens,'summary_tokens',u.summary_tokens,'reasoning_tokens',u.reasoning_tokens,
        'image_tokens',u.image_tokens,'audio_tokens',u.audio_tokens,'prompt_cost',u.prompt_cost,
        'completion_cost',u.completion_cost,'total_cost',u.total_cost,'error_message',u.error_message),u.run_id
        FROM legacy_usage_records u LEFT JOIN legacy_import_runs r ON r.id=u.run_id")
        .map_err(|_| UsageReportError::Storage)?;
    statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|_| UsageReportError::Storage)?
        .map(|raw| {
            let (source, metadata, fingerprint, fields, run) =
                raw.map_err(|_| UsageReportError::Storage)?;
            let fingerprint = fingerprint
                .ok_or(UsageReportError::InvalidData)?
                .parse()
                .map_err(|_| UsageReportError::InvalidData)?;
            let scope = lettuce_transfer::LegacyIdScope::new(&fingerprint);
            let mut row: UsageReportRow =
                serde_json::from_str(&fields).map_err(|_| UsageReportError::InvalidData)?;
            let identity =
                serde_json::to_string(&(run, source)).map_err(|_| UsageReportError::InvalidData)?;
            row.id = scope.derived(&identity, "usage-report").to_string();
            row.session_id = row
                .session_id
                .filter(|id| !id.is_empty())
                .map(|id| scope.source(&id).to_string());
            row.character_id = row
                .character_id
                .filter(|id| !id.is_empty())
                .map(|id| scope.source(&id).to_string());
            let metadata: BTreeMap<String, String> =
                serde_json::from_str(&metadata).map_err(|_| UsageReportError::InvalidData)?;
            let count = |keys: &[&str]| {
                keys.iter()
                    .find_map(|key| metadata.get(*key)?.parse::<u64>().ok())
            };
            let money = |keys: &[&str]| {
                keys.iter()
                    .find_map(|key| metadata.get(*key)?.parse::<f64>().ok())
                    .filter(|value| value.is_finite())
            };
            row.cached_prompt_tokens =
                count(&["cached_prompt_tokens", "openrouter_cached_prompt_tokens"]);
            row.cache_write_tokens =
                count(&["cache_write_tokens", "openrouter_cache_write_tokens"]);
            row.web_search_requests =
                count(&["web_search_requests", "openrouter_web_search_requests"]);
            row.input_image_count = count(&["input_image_count"]);
            row.output_image_count = count(&["output_image_count"]);
            row.cache_read_cost = money(&["cost_cache_read"]);
            row.cache_write_cost = money(&["cost_cache_write"]);
            row.reasoning_cost = money(&["cost_reasoning"]);
            row.request_cost = money(&["cost_request"]);
            row.web_search_cost = money(&["cost_web_search"]);
            row.api_cost = money(&["api_cost", "openrouter_api_cost"]);
            if row.total_cost.is_none() {
                row.total_cost = row.api_cost.or_else(|| {
                    row.prompt_cost
                        .zip(row.completion_cost)
                        .map(|(prompt, completion)| prompt + completion)
                });
            }
            Ok(row)
        })
        .collect()
}
