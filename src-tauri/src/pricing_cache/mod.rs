use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::OptionalExtension;
use tauri::AppHandle;

use crate::models::pricing::OpenRouterProviderPricing;
use crate::storage_manager::db::open_db;
use crate::utils::{log_error, log_info, log_warn};

const CACHE_TTL_HOURS: u64 = 6;
const OPENROUTER_PROVIDER_ID: &str = "openrouter";
const OPENROUTER_ENDPOINTS_REFRESH_KIND: &str = "openrouter_endpoints";
const DEFERRED_REFRESH_DELAY_SECS: u64 = 60;
const DEFERRED_REFRESH_BATCH_SIZE: usize = 6;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn cache_ttl_secs() -> u64 {
    CACHE_TTL_HOURS * 3600
}

fn refresh_guard() -> &'static AtomicBool {
    static GUARD: OnceLock<AtomicBool> = OnceLock::new();
    GUARD.get_or_init(|| AtomicBool::new(false))
}

fn is_retryable_deferred_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("408")
        || lower.contains("timeout")
        || lower.contains("timed out")
        || lower.contains("deadline has elapsed")
}

fn get_cached_model_pricing_internal(
    app: &AppHandle,
    model_id: &str,
    allow_stale: bool,
) -> Result<Option<crate::models::ModelPricing>, String> {
    let conn = open_db(app)?;
    let row: Option<(Option<String>, u64)> = conn
        .query_row(
            "SELECT pricing_json, cached_at FROM model_pricing_cache WHERE model_id = ?1",
            rusqlite::params![model_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some((pricing_json_opt, cached_at)) = row {
        let age = now_secs().saturating_sub(cached_at);
        if allow_stale || age < cache_ttl_secs() {
            if let Some(pricing_json) = pricing_json_opt {
                let pricing: crate::models::ModelPricing = serde_json::from_str(&pricing_json)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                return Ok(Some(pricing));
            }
            return Ok(None);
        }
    }

    Ok(None)
}

fn get_cached_provider_pricings_internal(
    app: &AppHandle,
    model_id: &str,
    allow_stale: bool,
) -> Result<Option<Vec<OpenRouterProviderPricing>>, String> {
    let conn = open_db(app)?;
    let row: Option<(String, u64)> = conn
        .query_row(
            "SELECT provider_pricings_json, cached_at
             FROM openrouter_provider_pricing_cache
             WHERE model_id = ?1",
            rusqlite::params![model_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some((provider_pricings_json, cached_at)) = row {
        let age = now_secs().saturating_sub(cached_at);
        if allow_stale || age < cache_ttl_secs() {
            let provider_pricings: Vec<OpenRouterProviderPricing> =
                serde_json::from_str(&provider_pricings_json)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            return Ok(Some(provider_pricings));
        }
    }

    Ok(None)
}

pub fn get_cached_pricing(
    app: &AppHandle,
    model_id: &str,
) -> Result<Option<crate::models::ModelPricing>, String> {
    get_cached_model_pricing_internal(app, model_id, false)
}

pub fn get_any_cached_pricing(
    app: &AppHandle,
    model_id: &str,
) -> Result<Option<crate::models::ModelPricing>, String> {
    get_cached_model_pricing_internal(app, model_id, true)
}

pub fn cache_model_pricing(
    app: &AppHandle,
    model_id: &str,
    pricing: Option<crate::models::ModelPricing>,
) -> Result<(), String> {
    let conn = open_db(app)?;
    let now = now_secs();
    let pricing_json = match pricing {
        Some(ref pricing) => Some(
            serde_json::to_string(pricing)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
        ),
        None => None,
    };
    conn.execute(
        "INSERT INTO model_pricing_cache (model_id, pricing_json, cached_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(model_id) DO UPDATE
         SET pricing_json = excluded.pricing_json, cached_at = excluded.cached_at",
        rusqlite::params![model_id, pricing_json, now],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

pub fn get_cached_openrouter_provider_pricings(
    app: &AppHandle,
    model_id: &str,
) -> Result<Option<Vec<OpenRouterProviderPricing>>, String> {
    get_cached_provider_pricings_internal(app, model_id, false)
}

pub fn get_any_cached_openrouter_provider_pricings(
    app: &AppHandle,
    model_id: &str,
) -> Result<Option<Vec<OpenRouterProviderPricing>>, String> {
    get_cached_provider_pricings_internal(app, model_id, true)
}

pub fn cache_openrouter_provider_pricings(
    app: &AppHandle,
    model_id: &str,
    provider_pricings: &[OpenRouterProviderPricing],
) -> Result<(), String> {
    let conn = open_db(app)?;
    let now = now_secs();
    let provider_pricings_json = serde_json::to_string(provider_pricings)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute(
        "INSERT INTO openrouter_provider_pricing_cache (model_id, provider_pricings_json, cached_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(model_id) DO UPDATE
         SET provider_pricings_json = excluded.provider_pricings_json, cached_at = excluded.cached_at",
        rusqlite::params![model_id, provider_pricings_json, now],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

pub fn schedule_openrouter_endpoints_refresh(
    app: &AppHandle,
    model_id: &str,
    error: &str,
) -> Result<(), String> {
    let conn = open_db(app)?;
    let now = now_secs();
    let retry_after = now.saturating_add(DEFERRED_REFRESH_DELAY_SECS);
    conn.execute(
        "INSERT INTO deferred_pricing_refreshes
           (provider_id, model_id, refresh_kind, retry_after, last_error, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(provider_id, model_id, refresh_kind) DO UPDATE SET
           retry_after = excluded.retry_after,
           last_error = excluded.last_error,
           updated_at = excluded.updated_at",
        rusqlite::params![
            OPENROUTER_PROVIDER_ID,
            model_id,
            OPENROUTER_ENDPOINTS_REFRESH_KIND,
            retry_after,
            error,
            now,
            now
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

pub fn clear_openrouter_endpoints_refresh(app: &AppHandle, model_id: &str) -> Result<(), String> {
    let conn = open_db(app)?;
    conn.execute(
        "DELETE FROM deferred_pricing_refreshes
         WHERE provider_id = ?1 AND model_id = ?2 AND refresh_kind = ?3",
        rusqlite::params![
            OPENROUTER_PROVIDER_ID,
            model_id,
            OPENROUTER_ENDPOINTS_REFRESH_KIND
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

fn get_due_openrouter_endpoint_refreshes(app: &AppHandle) -> Result<Vec<String>, String> {
    let conn = open_db(app)?;
    let now = now_secs();
    let mut stmt = conn
        .prepare(
            "SELECT model_id
             FROM deferred_pricing_refreshes
             WHERE provider_id = ?1 AND refresh_kind = ?2 AND retry_after <= ?3
             ORDER BY retry_after ASC
             LIMIT ?4",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let rows = stmt
        .query_map(
            rusqlite::params![
                OPENROUTER_PROVIDER_ID,
                OPENROUTER_ENDPOINTS_REFRESH_KIND,
                now,
                DEFERRED_REFRESH_BATCH_SIZE as i64
            ],
            |row| row.get::<_, String>(0),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut model_ids = Vec::new();
    for row in rows {
        model_ids.push(row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?);
    }
    Ok(model_ids)
}

pub fn kick_openrouter_deferred_refreshes(app: AppHandle, api_key: String) {
    if api_key.trim().is_empty() {
        return;
    }

    if refresh_guard()
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    tokio::spawn(async move {
        let result = async {
            let model_ids = get_due_openrouter_endpoint_refreshes(&app)?;
            if model_ids.is_empty() {
                return Ok::<(), String>(());
            }

            log_info(
                &app,
                "cost_calculator",
                format!(
                    "processing deferred OpenRouter pricing refreshes: {} item(s)",
                    model_ids.len()
                ),
            );

            for model_id in model_ids {
                match crate::models::pricing::fetchers::refresh_openrouter_endpoint_caches(
                    app.clone(),
                    &api_key,
                    &model_id,
                )
                .await
                {
                    Ok(_) => {
                        log_info(
                            &app,
                            "cost_calculator",
                            format!("completed deferred pricing refresh for {}", model_id),
                        );
                    }
                    Err(err) => {
                        log_warn(
                            &app,
                            "cost_calculator",
                            format!(
                                "deferred pricing refresh failed for {}: {}",
                                model_id, err
                            ),
                        );
                        if is_retryable_deferred_error(&err) {
                            let _ = schedule_openrouter_endpoints_refresh(&app, &model_id, &err);
                        } else {
                            let _ = clear_openrouter_endpoints_refresh(&app, &model_id);
                        }
                    }
                }
            }

            Ok(())
        }
        .await;

        if let Err(err) = result {
            log_error(
                &app,
                "cost_calculator",
                format!("failed to process deferred pricing refreshes: {}", err),
            );
        }

        refresh_guard().store(false, Ordering::SeqCst);
    });
}
