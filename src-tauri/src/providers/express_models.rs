//! Model catalog for the Gemini Agent Platform (Express) provider.
//!
//! The Express endpoint exposes no API-key-listable models endpoint
//! (`ListPublisherModels` requires OAuth2 and 401s on an API key), so we can't
//! discover models at runtime the way other providers do. Instead we ship a
//! curated manifest, optionally refreshed from a remote copy so the list can be
//! updated without an app release. The flow is:
//!
//!   fresh on-disk cache (<1h)  ->  return it, no network
//!   stale / missing            ->  fetch remote, validate, refresh cache
//!   fetch fails / invalid      ->  last-known-good cache, else bundled default
//!
//! Whatever this returns is only a convenience catalog: the model-id field stays
//! free-text, and unsupported ids simply 404 at generation time.

use serde::{Deserialize, Serialize};
use std::fs;

use crate::chat_manager::provider_adapter::ModelInfo;
use crate::utils::{log_error, log_info};

/// Remote manifest URL. Empty = bundled-only (no network). Served from the public
/// repo so the catalog can be updated without an app release.
const MANIFEST_URL: &str =
    "https://raw.githubusercontent.com/rppavan/lettuceai-app/main/src-tauri/manifests/gemini_express_models.json";

/// Refresh at most once per hour.
const CACHE_TTL_MS: u64 = 60 * 60 * 1000;
const CACHE_FILE: &str = "gemini_express_models.cache.json";

/// Compiled-in default, used on first run and whenever fetch/cache are unavailable.
const BUNDLED: &str = include_str!("../../manifests/gemini_express_models.json");

#[derive(Deserialize, Serialize)]
struct Manifest {
    #[serde(default)]
    #[allow(dead_code)] // carried for forward-compat / cache round-trip
    version: u32,
    models: Vec<ManifestModel>,
}

#[derive(Deserialize, Serialize)]
struct ManifestModel {
    id: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    context_length: Option<u64>,
    #[serde(default)]
    input: Option<Vec<String>>,
    #[serde(default)]
    output: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize)]
struct CachedManifest {
    fetched_at: u64,
    manifest: Manifest,
}

/// Resolve the Express model catalog, preferring a fresh remote copy and falling
/// back through cache -> bundled. Never errors: a bad network or corrupt cache
/// still yields the bundled list.
pub async fn express_models(app: &tauri::AppHandle) -> Vec<ModelInfo> {
    if let Some(cached) = read_cache(app) {
        if is_fresh(cached.fetched_at) {
            return to_model_infos(&cached.manifest);
        }
        // stale: try to refresh, but keep serving the cache if the network is down
        if let Some(fresh) = try_fetch(app).await {
            write_cache(app, &fresh);
            return to_model_infos(&fresh);
        }
        return to_model_infos(&cached.manifest);
    }

    if let Some(fresh) = try_fetch(app).await {
        write_cache(app, &fresh);
        return to_model_infos(&fresh);
    }

    to_model_infos(&bundled())
}

fn is_fresh(fetched_at: u64) -> bool {
    crate::infra::utils::now_millis()
        .map(|now| now.saturating_sub(fetched_at) < CACHE_TTL_MS)
        .unwrap_or(false)
}

/// Fetch + validate the remote manifest. Returns None when remote updates are
/// disabled, the request fails, or the payload validates to zero usable models.
async fn try_fetch(app: &tauri::AppHandle) -> Option<Manifest> {
    if MANIFEST_URL.is_empty() {
        return None;
    }
    let client = crate::transport::build_client(
        app,
        Some(5_000),
        false,
        Some("gemini-agent-platform-express"),
        Some(MANIFEST_URL),
    )
    .ok()?;

    let resp = client.get(MANIFEST_URL).send().await.ok()?;
    if !resp.status().is_success() {
        log_error(
            app,
            "express_models",
            format!("manifest fetch returned {}", resp.status()),
        );
        return None;
    }
    let text = resp.text().await.ok()?;
    let manifest: Manifest = serde_json::from_str(&text).ok()?;
    // reject a payload that contains no models with valid Gemini-style ids
    if to_model_infos(&manifest).is_empty() {
        log_error(app, "express_models", "fetched manifest had no valid models");
        return None;
    }
    log_info(app, "express_models", "refreshed model catalog from remote");
    Some(manifest)
}

fn bundled() -> Manifest {
    // the bundled file is authored in-repo, so this parse cannot fail in practice
    serde_json::from_str(BUNDLED).unwrap_or(Manifest {
        version: 0,
        models: Vec::new(),
    })
}

/// Map manifest entries to `ModelInfo`, dropping any id that doesn't look like a
/// Gemini-family model name. This is the trust boundary for remote data: a
/// compromised or garbled manifest can't inject arbitrary ids.
fn to_model_infos(manifest: &Manifest) -> Vec<ModelInfo> {
    let id_pattern = regex::Regex::new(r"^(gemini|imagen|gemma)[a-z0-9.\-]*$")
        .expect("static regex is valid");
    manifest
        .models
        .iter()
        .filter(|m| id_pattern.is_match(&m.id))
        .map(|m| ModelInfo {
            id: m.id.clone(),
            display_name: m.display_name.clone(),
            description: None,
            context_length: m.context_length,
            input_modalities: m.input.clone(),
            output_modalities: m.output.clone(),
            supported_endpoints: None,
            input_price: None,
            output_price: None,
        })
        .collect()
}

fn cache_path(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    crate::infra::utils::ensure_lettuce_dir(app)
        .ok()
        .map(|dir| dir.join(CACHE_FILE))
}

fn read_cache(app: &tauri::AppHandle) -> Option<CachedManifest> {
    let path = cache_path(app)?;
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_cache(app: &tauri::AppHandle, manifest: &Manifest) {
    let Some(path) = cache_path(app) else {
        return;
    };
    let Ok(fetched_at) = crate::infra::utils::now_millis() else {
        return;
    };
    // serialize through borrowed references to avoid cloning the manifest
    let payload = serde_json::json!({
        "fetched_at": fetched_at,
        "manifest": manifest,
    });
    if let Ok(text) = serde_json::to_string(&payload) {
        let _ = fs::write(path, text);
    }
}
