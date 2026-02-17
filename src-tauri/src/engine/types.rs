use serde::{Deserialize, Serialize};

// ── Health ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    #[serde(default)]
    pub version: Option<String>,
}

// ── Setup ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct SetupStatusResponse {
    pub needs_setup: bool,
    #[serde(default)]
    pub configured_providers: Vec<String>,
    #[serde(default)]
    pub has_api_key: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SetupCompleteResponse {
    pub status: String,
}

// ── Config ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigLlmRequest {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigLlmDefaultRequest {
    pub provider: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigEngineRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_history: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigBackgroundRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synthesis_interval_minutes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consolidation_interval_minutes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bm25_rebuild_interval_minutes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drip_research_interval_minutes: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigMemoryRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retrieval_results: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dense_weight: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bm25_weight: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_weight: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recency_boost_hours: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub random_surface_probability: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigSafetyRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub honesty_section: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_data_deletion: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigResearchRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_scrape_on_boot: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub periodic_interval_hours: Option<u32>,
}
