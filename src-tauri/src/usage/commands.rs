use super::app_activity::AppActiveUsageService;
use super::repository;
use super::tracking::{RequestUsage, UsageFilter, UsageStats};
use crate::chat_manager::service::apply_openrouter_cost_to_usage;
use crate::chat_manager::types::UsageSummary;
use crate::utils::{log_error, log_info};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tauri::Manager;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppActiveUsageSummary {
    pub total_ms: u64,
    pub started_at_ms: Option<u64>,
    pub last_updated_at_ms: Option<u64>,
    pub by_day_ms: HashMap<String, u64>,
}

#[tauri::command]
pub async fn usage_add_record(app: AppHandle, usage: RequestUsage) -> Result<(), String> {
    repository::add_usage_record(&app, usage)
}

#[tauri::command]
pub async fn usage_query_records(
    app: AppHandle,
    filter: UsageFilter,
) -> Result<Vec<RequestUsage>, String> {
    repository::query_usage_records(&app, filter)
}

#[tauri::command]
pub async fn usage_get_stats(app: AppHandle, filter: UsageFilter) -> Result<UsageStats, String> {
    repository::calculate_usage_stats(&app, filter)
}

#[tauri::command]
pub async fn usage_clear_before(app: AppHandle, timestamp: u64) -> Result<u64, String> {
    repository::clear_usage_records_before(&app, timestamp)
}

#[tauri::command]
pub async fn usage_export_csv(app: AppHandle, filter: UsageFilter) -> Result<String, String> {
    repository::export_usage_csv(&app, filter)
}

#[tauri::command]
pub async fn usage_save_csv(
    app: AppHandle,
    csv_data: String,
    filename: String,
) -> Result<String, String> {
    repository::save_usage_csv(&app, &csv_data, &filename)
}

#[tauri::command]
pub async fn usage_get_app_active_usage(app: AppHandle) -> Result<AppActiveUsageSummary, String> {
    if let Some(state) = app.try_state::<Arc<AppActiveUsageService>>() {
        state.flush(&app);
    }

    let settings_json = crate::storage_manager::settings::internal_read_settings(&app)?;
    let parsed = settings_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .unwrap_or(Value::Null);
    let app_state = parsed.get("appState").and_then(|v| v.as_object());

    let total_ms = app_state
        .and_then(|s| s.get("appActiveUsageMs"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let started_at_ms = app_state
        .and_then(|s| s.get("appActiveUsageStartedAtMs"))
        .and_then(|v| v.as_u64());
    let last_updated_at_ms = app_state
        .and_then(|s| s.get("appActiveUsageLastUpdatedAtMs"))
        .and_then(|v| v.as_u64());
    let by_day_ms = app_state
        .and_then(|s| s.get("appActiveUsageByDayMs"))
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_u64().map(|value| (k.clone(), value)))
                .collect::<HashMap<String, u64>>()
        })
        .unwrap_or_default();

    Ok(AppActiveUsageSummary {
        total_ms,
        started_at_ms,
        last_updated_at_ms,
        by_day_ms,
    })
}

/// Recalculate costs for all usage records using current pricing
/// This is useful when the cost calculation formula was fixed
#[tauri::command]
pub async fn usage_recalculate_costs(app: AppHandle, api_key: String) -> Result<String, String> {
    log_info(&app, "usage_recalculate", "Starting cost recalculation...");

    // Get all records
    let records = repository::query_usage_records(
        &app,
        UsageFilter {
            start_timestamp: None,
            end_timestamp: None,
            provider_id: None,
            model_id: None,
            character_id: None,
            session_id: None,
            success_only: None,
        },
    )?;

    let total_records = records.len();
    log_info(
        &app,
        "usage_recalculate",
        format!("Found {} records to recalculate", total_records),
    );

    let mut updated_count = 0;
    let mut skipped_count = 0;
    let mut error_count = 0;

    for mut record in records {
        // Only recalculate for OpenRouter records that have token counts
        if !record.provider_id.eq_ignore_ascii_case("openrouter") {
            skipped_count += 1;
            continue;
        }

        let prompt_tokens = record.prompt_tokens.unwrap_or(0);
        let completion_tokens = record.completion_tokens.unwrap_or(0);

        if prompt_tokens == 0 && completion_tokens == 0 {
            skipped_count += 1;
            continue;
        }

        let model_name = record.model_name.clone();
        let usage_summary = UsageSummary {
            prompt_tokens: record.prompt_tokens,
            completion_tokens: record.completion_tokens,
            total_tokens: record.total_tokens,
            cached_prompt_tokens: record.cached_prompt_tokens.or_else(|| {
                record
                    .metadata
                    .get("openrouter_cached_prompt_tokens")
                    .and_then(|v| v.parse::<u64>().ok())
            }),
            cache_write_tokens: record.cache_write_tokens.or_else(|| {
                record
                    .metadata
                    .get("openrouter_cache_write_tokens")
                    .and_then(|v| v.parse::<u64>().ok())
            }),
            reasoning_tokens: record.reasoning_tokens,
            image_tokens: record.image_tokens,
            web_search_requests: record.web_search_requests.or_else(|| {
                record
                    .metadata
                    .get("openrouter_web_search_requests")
                    .and_then(|v| v.parse::<u64>().ok())
            }),
            api_cost: record.api_cost.or_else(|| {
                record
                    .metadata
                    .get("openrouter_api_cost")
                    .and_then(|v| v.parse::<f64>().ok())
            }),
            response_id: record.metadata.get("openrouter_response_id").cloned(),
            first_token_ms: None,
            tokens_per_second: None,
            finish_reason: record
                .finish_reason
                .as_ref()
                .map(|r| r.as_str().to_string()),
        };

        apply_openrouter_cost_to_usage(
            &app,
            &mut record,
            &usage_summary,
            &model_name,
            &api_key,
            "usage_recalculate",
        )
        .await;

        if record.cost.is_none() {
            skipped_count += 1;
            continue;
        }

        match repository::add_usage_record(&app, record) {
            Ok(_) => {
                updated_count += 1;
            }
            Err(e) => {
                log_error(
                    &app,
                    "usage_recalculate",
                    format!("Failed to persist recalculated record: {}", e),
                );
                error_count += 1;
            }
        }
    }

    let result = format!(
        "Recalculation complete: {} updated, {} skipped, {} errors (total: {})",
        updated_count, skipped_count, error_count, total_records
    );
    log_info(&app, "usage_recalculate", &result);

    Ok(result)
}
