use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Mutex;

use lazy_static::lazy_static;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use tauri::AppHandle;

use crate::storage_manager::internal_read_settings;
use crate::storage_manager::lorebook::{
    set_character_lorebooks, upsert_lorebook, upsert_lorebook_entry, Lorebook, LorebookEntry,
};
use crate::storage_manager::media::{generate_avatar_gradient, storage_save_avatar};
use crate::utils::{log_error, log_info};

const DISCOVERY_BASE_URL: &str = "https://character-tavern.com/api/homepage/cards";
const CARD_DETAIL_BASE_URL: &str = "https://character-tavern.com/api/character";
const CARD_SEARCH_BASE_URL: &str = "https://character-tavern.com/api/search/cards";
const CARD_IMAGE_BASE_URL: &str = "https://cards.character-tavern.com/cdn-cgi/image";
const DISCOVERY_CACHE_TTL_SECS: i64 = 600;

#[derive(Clone)]
struct CacheEntry {
    expires_at: i64,
    value: Value,
}

lazy_static! {
    static ref DISCOVERY_CACHE: Mutex<HashMap<String, CacheEntry>> = Mutex::new(HashMap::new());
}

fn now_epoch() -> i64 {
    chrono::Utc::now().timestamp()
}

fn read_pure_mode_level(app: &AppHandle) -> String {
    if let Ok(Some(raw)) = internal_read_settings(app) {
        if let Ok(json) = serde_json::from_str::<Value>(&raw) {
            let app_state = json.get("appState");
            // Check pureModeLevel first, fall back to pureModeEnabled boolean
            if let Some(level) = app_state
                .and_then(|v| v.get("pureModeLevel"))
                .and_then(|v| v.as_str())
            {
                return level.to_string();
            }
            if let Some(enabled) = app_state
                .and_then(|v| v.get("pureModeEnabled"))
                .and_then(|v| v.as_bool())
            {
                return if enabled {
                    "standard".to_string()
                } else {
                    "off".to_string()
                };
            }
        }
    }
    "standard".to_string()
}

fn is_dynamic_memory_enabled(app: &AppHandle) -> bool {
    if let Ok(Some(raw)) = internal_read_settings(app) {
        if let Ok(json) = serde_json::from_str::<Value>(&raw) {
            return json
                .get("advancedSettings")
                .and_then(|v| v.get("dynamicMemory"))
                .and_then(|v| v.get("enabled"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
        }
    }
    false
}

fn cache_get<T: DeserializeOwned>(key: &str) -> Option<T> {
    let now = now_epoch();
    let value = {
        let mut cache = DISCOVERY_CACHE.lock().ok()?;
        match cache.get(key) {
            Some(entry) if entry.expires_at > now => Some(entry.value.clone()),
            Some(_) => {
                cache.remove(key);
                None
            }
            None => None,
        }
    }?;

    serde_json::from_value(value).ok()
}

fn cache_set<T: Serialize>(key: String, value: &T, ttl_secs: i64) {
    let expires_at = now_epoch() + ttl_secs;
    if let Ok(value) = serde_json::to_value(value) {
        if let Ok(mut cache) = DISCOVERY_CACHE.lock() {
            cache.insert(key, CacheEntry { expires_at, value });
        }
    }
}

fn deserialize_optional_string_or_number<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    let output = match value {
        Some(Value::String(text)) => Some(text),
        Some(Value::Number(number)) => Some(number.to_string()),
        Some(Value::Bool(flag)) => Some(flag.to_string()),
        _ => None,
    };
    Ok(output)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryCard {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub in_chat_name: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub tagline: Option<String>,
    #[serde(default)]
    pub page_description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default, rename = "isNSFW", alias = "isNsfw")]
    pub is_nsfw: Option<bool>,
    #[serde(default)]
    pub content_warnings: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub views: Option<i64>,
    #[serde(default)]
    pub downloads: Option<i64>,
    #[serde(default)]
    pub messages: Option<i64>,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub last_update_at: Option<i64>,
    #[serde(default)]
    pub likes: Option<i64>,
    #[serde(default)]
    pub dislikes: Option<i64>,
    #[serde(default)]
    pub total_tokens: Option<i64>,
    #[serde(default)]
    pub has_lorebook: Option<bool>,
    #[serde(default, rename = "isOC", alias = "isOc")]
    pub is_oc: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiscoveryResponse {
    hits: Vec<DiscoveryCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverySearchResponse {
    pub hits: Vec<DiscoveryCard>,
    #[serde(default)]
    pub total_hits: Option<i64>,
    #[serde(default)]
    pub hits_per_page: Option<i64>,
    #[serde(default)]
    pub page: Option<i64>,
    #[serde(default)]
    pub total_pages: Option<i64>,
    #[serde(default)]
    pub processing_time_ms: Option<i64>,
    #[serde(default)]
    pub query: Option<String>,
}

fn strict_suggestive_metadata(card: &DiscoveryCard) -> bool {
    const STRICT_TERMS: [&str; 7] = ["nsfw", "18+", "sexual", "erotic", "porn", "fetish", "kink"];
    let has_tag_term = card.tags.iter().any(|tag| {
        STRICT_TERMS
            .iter()
            .any(|term| tag.to_ascii_lowercase().contains(term))
    });
    let has_warning_term = card.content_warnings.iter().any(|warning| {
        STRICT_TERMS
            .iter()
            .any(|term| warning.to_ascii_lowercase().contains(term))
    });
    has_tag_term || has_warning_term
}

fn filter_nsfw_cards(cards: &mut Vec<DiscoveryCard>, pure_mode_level: &str) {
    match pure_mode_level {
        "off" => {}
        "strict" => {
            cards.retain(|card| !card.is_nsfw.unwrap_or(false) && !strict_suggestive_metadata(card))
        }
        _ => cards.retain(|card| !card.is_nsfw.unwrap_or(false)),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverySections {
    pub newest: Vec<DiscoveryCard>,
    pub popular: Vec<DiscoveryCard>,
    pub trending: Vec<DiscoveryCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryCardDetail {
    pub id: String,
    #[serde(default)]
    pub origin: Option<String>,
    pub name: String,
    #[serde(default)]
    pub in_chat_name: Option<String>,
    #[serde(default)]
    pub author: Option<serde_json::Value>,
    pub path: String,
    #[serde(default)]
    pub tagline: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "isNSFW", alias = "isNsfw")]
    pub is_nsfw: Option<bool>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub has_expression_pack: Option<bool>,
    #[serde(default)]
    pub last_updated_at: Option<String>,
    #[serde(default)]
    pub visibility: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string_or_number")]
    pub lorebook_id: Option<String>,
    #[serde(default, alias = "definition_scenario")]
    pub definition_scenario: Option<String>,
    #[serde(default, alias = "definition_personality")]
    pub definition_personality: Option<String>,
    #[serde(default, alias = "definition_character_description")]
    pub definition_character_description: Option<String>,
    #[serde(default, alias = "definition_first_message")]
    pub definition_first_message: Option<String>,
    #[serde(default, alias = "definition_example_messages")]
    pub definition_example_messages: Option<String>,
    #[serde(default, alias = "definition_system_prompt")]
    pub definition_system_prompt: Option<String>,
    #[serde(default, alias = "definition_post_history_prompt")]
    pub definition_post_history_prompt: Option<String>,
    #[serde(default)]
    pub token_total: Option<i64>,
    #[serde(default)]
    pub token_description: Option<i64>,
    #[serde(default)]
    pub token_personality: Option<i64>,
    #[serde(default)]
    pub token_scenario: Option<i64>,
    #[serde(default)]
    pub token_mes_example: Option<i64>,
    #[serde(default)]
    pub token_first_mes: Option<i64>,
    #[serde(default)]
    pub token_system_prompt: Option<i64>,
    #[serde(default, alias = "token_post_history_instructions")]
    pub token_post_history_instructions: Option<i64>,
    #[serde(default, alias = "analytics_views")]
    pub analytics_views: Option<i64>,
    #[serde(default, alias = "analytics_downloads")]
    pub analytics_downloads: Option<i64>,
    #[serde(default, alias = "analytics_messages")]
    pub analytics_messages: Option<i64>,
    #[serde(default, rename = "isOC", alias = "isOc")]
    pub is_oc: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryCardDetailResponse {
    pub card: DiscoveryCardDetail,
    #[serde(default, alias = "ownerCTId")]
    pub owner_ct_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscoveryLorebook {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub scan_depth: Option<i64>,
    #[serde(default)]
    pub entries: Vec<DiscoveryLorebookEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscoveryLorebookEntry {
    pub id: i64,
    pub name: String,
    pub content: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub insertion_order: Option<i64>,
    #[serde(default)]
    pub constant: bool,
    #[serde(default)]
    pub keys: Vec<String>,
}

#[derive(Clone, Copy)]
enum DiscoverySortKey {
    CreatedAt,
    LastUpdateAt,
    Likes,
    Downloads,
    Messages,
    Views,
    Name,
}

fn normalize_type(card_type: &str) -> Result<&'static str, String> {
    match card_type.trim().to_ascii_lowercase().as_str() {
        "newest" => Ok("newest"),
        "popular" => Ok("popular"),
        "trending" => Ok("trending"),
        other => Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Unsupported card type: {}", other),
        )),
    }
}

fn parse_sort_key(value: Option<&str>) -> Option<DiscoverySortKey> {
    let key = value?.trim().to_ascii_lowercase();
    match key.as_str() {
        "created" | "createdat" | "created_at" => Some(DiscoverySortKey::CreatedAt),
        "updated" | "lastupdateat" | "last_update_at" => Some(DiscoverySortKey::LastUpdateAt),
        "likes" => Some(DiscoverySortKey::Likes),
        "downloads" => Some(DiscoverySortKey::Downloads),
        "messages" => Some(DiscoverySortKey::Messages),
        "views" => Some(DiscoverySortKey::Views),
        "name" => Some(DiscoverySortKey::Name),
        _ => None,
    }
}

fn default_sort_for_type(card_type: &str) -> (DiscoverySortKey, bool) {
    match card_type {
        "newest" => (DiscoverySortKey::CreatedAt, true),
        "popular" => (DiscoverySortKey::Likes, true),
        "trending" => (DiscoverySortKey::LastUpdateAt, true),
        _ => (DiscoverySortKey::CreatedAt, true),
    }
}

fn numeric_value(card: &DiscoveryCard, key: DiscoverySortKey) -> i64 {
    match key {
        DiscoverySortKey::CreatedAt => card.created_at.unwrap_or(0),
        DiscoverySortKey::LastUpdateAt => card.last_update_at.or(card.created_at).unwrap_or(0),
        DiscoverySortKey::Likes => card.likes.unwrap_or(0),
        DiscoverySortKey::Downloads => card.downloads.unwrap_or(0),
        DiscoverySortKey::Messages => card.messages.unwrap_or(0),
        DiscoverySortKey::Views => card.views.unwrap_or(0),
        DiscoverySortKey::Name => 0,
    }
}

fn compare_numeric(
    a: &DiscoveryCard,
    b: &DiscoveryCard,
    key: DiscoverySortKey,
    desc: bool,
) -> Ordering {
    let a_val = numeric_value(a, key);
    let b_val = numeric_value(b, key);
    if desc {
        b_val.cmp(&a_val)
    } else {
        a_val.cmp(&b_val)
    }
}

fn compare_name(a: &DiscoveryCard, b: &DiscoveryCard, desc: bool) -> Ordering {
    let a_name = a.name.to_ascii_lowercase();
    let b_name = b.name.to_ascii_lowercase();
    if desc {
        b_name.cmp(&a_name)
    } else {
        a_name.cmp(&b_name)
    }
}

fn sort_cards(cards: &mut [DiscoveryCard], key: DiscoverySortKey, desc: bool) {
    cards.sort_by(|a, b| {
        let primary = match key {
            DiscoverySortKey::Name => compare_name(a, b, desc),
            _ => compare_numeric(a, b, key, desc),
        };
        if primary != Ordering::Equal {
            return primary;
        }

        compare_numeric(a, b, DiscoverySortKey::Likes, true)
            .then_with(|| compare_numeric(a, b, DiscoverySortKey::Downloads, true))
            .then_with(|| compare_numeric(a, b, DiscoverySortKey::Views, true))
            .then_with(|| compare_numeric(a, b, DiscoverySortKey::Messages, true))
            .then_with(|| compare_numeric(a, b, DiscoverySortKey::CreatedAt, true))
            .then_with(|| compare_name(a, b, false))
    });
}

fn normalize_card_path(raw: &str) -> String {
    let trimmed = raw.trim().trim_start_matches('/');
    let with_ext = if trimmed.ends_with(".png") {
        trimmed.to_string()
    } else {
        format!("{}.png", trimmed)
    };

    let encoded: Vec<String> = with_ext
        .split('/')
        .map(|seg| urlencoding::encode(seg).into_owned())
        .collect();
    encoded.join("/")
}

fn normalize_detail_path(raw: &str) -> Result<(String, String), String> {
    let trimmed = raw.trim().trim_start_matches('/');
    if trimmed.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Card path cannot be empty",
        ));
    }

    let mut parts = trimmed.splitn(2, '/');
    let author = parts
        .next()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "Card path missing author".to_string())?;
    let name_raw = parts
        .next()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "Card path missing name".to_string())?;
    let name = name_raw.strip_suffix(".png").unwrap_or(name_raw);

    let author_encoded = urlencoding::encode(author).into_owned();
    let name_encoded = urlencoding::encode(name).into_owned();
    Ok((author_encoded, name_encoded))
}

fn push_definition_block(parts: &mut Vec<String>, label: Option<&str>, value: Option<String>) {
    let text = match value {
        Some(value) => value.trim().to_string(),
        None => return,
    };

    if text.is_empty() {
        return;
    }

    let block = match label {
        Some(label) => format!("[{}]\n{}", label, text),
        None => text,
    };

    parts.push(block);
}

#[tauri::command]
pub fn get_card_image(
    path: String,
    format: Option<String>,
    width: Option<u32>,
    quality: Option<u8>,
) -> Result<String, String> {
    if path.trim().is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Card image path cannot be empty",
        ));
    }

    if path.starts_with("http://") || path.starts_with("https://") {
        return Ok(path);
    }

    let format_value = format.unwrap_or_else(|| "auto".to_string());
    let width_value = width.unwrap_or(400).max(1);
    let quality_value = quality.unwrap_or(80).min(100);

    let path = normalize_card_path(&path);
    Ok(format!(
        "{}/format={},width={},quality={}/{}",
        CARD_IMAGE_BASE_URL, format_value, width_value, quality_value, path
    ))
}

#[tauri::command]
pub async fn discovery_fetch_card_detail(
    app: AppHandle,
    path: String,
) -> Result<DiscoveryCardDetailResponse, String> {
    if path.trim().is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Card path cannot be empty",
        ));
    }

    let pure_mode_level = read_pure_mode_level(&app);
    let url = if path.starts_with("http://") || path.starts_with("https://") {
        path
    } else {
        let (author, name) = normalize_detail_path(&path)?;
        format!("{}/{}/{}", CARD_DETAIL_BASE_URL, author, name)
    };
    let cache_key = format!("detail:{}", url);
    if let Some(cached) = cache_get::<DiscoveryCardDetailResponse>(&cache_key) {
        if pure_mode_level != "off" && cached.card.is_nsfw.unwrap_or(false) {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "NSFW content is blocked in Pure Mode",
            ));
        }
        return Ok(cached);
    }

    log_info(
        &app,
        "discovery_card_detail",
        format!("fetching card detail from {}", url),
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        log_error(
            &app,
            "discovery_card_detail",
            format!("detail fetch failed: {} {}", status, text),
        );
        return Err(format!(
            "Discovery detail request failed: {} {}",
            status, text
        ));
    }

    let detail = resp
        .json::<DiscoveryCardDetailResponse>()
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    cache_set(cache_key, &detail, DISCOVERY_CACHE_TTL_SECS);
    if pure_mode_level != "off" && detail.card.is_nsfw.unwrap_or(false) {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "NSFW content is blocked in Pure Mode",
        ));
    }
    Ok(detail)
}

async fn fetch_cards(
    app: &AppHandle,
    card_type: &str,
    client: &reqwest::Client,
) -> Result<Vec<DiscoveryCard>, String> {
    let cache_key = format!("cards:{}", card_type);
    if let Some(cached) = cache_get(&cache_key) {
        return Ok(cached);
    }

    let url = format!("{}?type={}", DISCOVERY_BASE_URL, card_type);
    log_info(
        app,
        "discovery_cards",
        format!("fetching {} cards from {}", card_type, url),
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        log_error(
            app,
            "discovery_cards",
            format!("{} cards failed: {} {}", card_type, status, text),
        );
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Discovery request failed: {} {}", status, text),
        ));
    }

    let data: DiscoveryResponse = resp
        .json()
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    cache_set(cache_key, &data.hits, DISCOVERY_CACHE_TTL_SECS);
    Ok(data.hits)
}

#[tauri::command]
pub async fn discovery_fetch_cards(
    app: AppHandle,
    card_type: String,
    sort_by: Option<String>,
    descending: Option<bool>,
) -> Result<Vec<DiscoveryCard>, String> {
    let card_type = normalize_type(&card_type)?;
    let client = reqwest::Client::new();
    let mut cards = fetch_cards(&app, card_type, &client).await?;
    let pure_mode_level = read_pure_mode_level(&app);
    filter_nsfw_cards(&mut cards, &pure_mode_level);

    let (default_key, default_desc) = default_sort_for_type(card_type);
    let key = parse_sort_key(sort_by.as_deref()).unwrap_or(default_key);
    let desc = descending.unwrap_or(default_desc);

    sort_cards(&mut cards, key, desc);
    Ok(cards)
}

#[tauri::command]
pub async fn discovery_fetch_sections(
    app: AppHandle,
    sort_by: Option<String>,
    descending: Option<bool>,
) -> Result<DiscoverySections, String> {
    let client = reqwest::Client::new();
    let (mut newest, mut popular, mut trending) = tokio::try_join!(
        fetch_cards(&app, "newest", &client),
        fetch_cards(&app, "popular", &client),
        fetch_cards(&app, "trending", &client),
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let pure_mode_level = read_pure_mode_level(&app);
    filter_nsfw_cards(&mut newest, &pure_mode_level);
    filter_nsfw_cards(&mut popular, &pure_mode_level);
    filter_nsfw_cards(&mut trending, &pure_mode_level);

    let key_override = parse_sort_key(sort_by.as_deref());
    if let Some(key) = key_override {
        let desc = descending.unwrap_or(true);
        sort_cards(&mut newest, key, desc);
        sort_cards(&mut popular, key, desc);
        sort_cards(&mut trending, key, desc);
    } else {
        let (key, desc) = default_sort_for_type("newest");
        sort_cards(&mut newest, key, desc);
        let (key, desc) = default_sort_for_type("popular");
        sort_cards(&mut popular, key, desc);
        let (key, desc) = default_sort_for_type("trending");
        sort_cards(&mut trending, key, desc);
    }

    Ok(DiscoverySections {
        newest,
        popular,
        trending,
    })
}

#[tauri::command]
pub async fn discovery_search_cards(
    app: AppHandle,
    query: Option<String>,
    page: Option<u32>,
    limit: Option<u32>,
) -> Result<DiscoverySearchResponse, String> {
    let pure_mode_level = read_pure_mode_level(&app);
    let query_value = query
        .map(|q| q.trim().to_string())
        .filter(|q| !q.is_empty());
    let page_value = page.filter(|p| *p > 0);
    let limit_value = limit.unwrap_or(30).max(1);

    let cache_key = format!(
        "search:{}:{}:{}",
        query_value.clone().unwrap_or_default(),
        page_value.unwrap_or(0),
        limit_value
    );
    if let Some(mut cached) = cache_get::<DiscoverySearchResponse>(&cache_key) {
        filter_nsfw_cards(&mut cached.hits, &pure_mode_level);
        return Ok(cached);
    }

    let mut params: Vec<(String, String)> = Vec::new();
    if let Some(query) = query_value.clone() {
        params.push(("query".to_string(), query));
    }

    if let Some(page) = page_value {
        params.push(("page".to_string(), page.to_string()));
    }

    params.push(("limit".to_string(), limit_value.to_string()));

    log_info(
        &app,
        "discovery_search",
        format!("fetching search cards with params {:?}", params),
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(CARD_SEARCH_BASE_URL)
        .query(&params)
        .send()
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        log_error(
            &app,
            "discovery_search",
            format!("search fetch failed: {} {}", status, text),
        );
        return Err(format!(
            "Discovery search request failed: {} {}",
            status, text
        ));
    }

    let mut response = resp
        .json::<DiscoverySearchResponse>()
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    cache_set(cache_key, &response, DISCOVERY_CACHE_TTL_SECS);
    filter_nsfw_cards(&mut response.hits, &pure_mode_level);
    Ok(response)
}

#[tauri::command]
pub async fn discovery_fetch_alternate_greetings(
    app: AppHandle,
    card_id: String,
) -> Result<Vec<String>, String> {
    let card_id = card_id.trim().to_string();
    if card_id.is_empty() {
        return Ok(vec![]);
    }

    let cache_key = format!("alt_greetings:{}", card_id);
    if let Some(cached) = cache_get(&cache_key) {
        return Ok(cached);
    }

    let url = format!(
        "https://character-tavern.com/api/character/{}/alternative-greetings",
        card_id
    );

    log_info(
        &app,
        "discovery_alternate_greetings",
        format!("Fetching alternate greetings from: {}", url),
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let status = resp.status();

    if !status.is_success() {
        return Ok(vec![]);
    }

    let greetings = resp
        .json::<Vec<String>>()
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    cache_set(cache_key, &greetings, DISCOVERY_CACHE_TTL_SECS);
    Ok(greetings)
}

#[tauri::command]
pub async fn discovery_fetch_tags(app: AppHandle, card_id: String) -> Result<Vec<String>, String> {
    let card_id = card_id.trim().to_string();
    if card_id.is_empty() {
        return Ok(vec![]);
    }

    let cache_key = format!("tags:{}", card_id);
    if let Some(cached) = cache_get(&cache_key) {
        return Ok(cached);
    }

    let url = format!(
        "https://character-tavern.com/api/character/{}/tags",
        card_id
    );

    log_info(
        &app,
        "discovery_tags",
        format!("Fetching tags from: {}", url),
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let status = resp.status();

    if !status.is_success() {
        return Ok(vec![]);
    }

    let tags = resp
        .json::<Vec<String>>()
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    cache_set(cache_key, &tags, DISCOVERY_CACHE_TTL_SECS);
    Ok(tags)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorInfo {
    pub display_name: String,
    #[serde(default, alias = "avatarURL")]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub followers_count: Option<i64>,
}

#[tauri::command]
pub async fn discovery_fetch_author_info(
    app: AppHandle,
    author_name: String,
) -> Result<AuthorInfo, String> {
    let author = author_name.trim();
    if author.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Author name cannot be empty",
        ));
    }

    let author_encoded = urlencoding::encode(author);
    let cache_key = format!("author:{}", author_encoded);
    if let Some(cached) = cache_get(&cache_key) {
        return Ok(cached);
    }

    let url = format!(
        "https://character-tavern.com/api/author/{}/info",
        author_encoded
    );

    log_info(
        &app,
        "discovery_author_info",
        format!("Fetching author info from: {}", url),
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let status = resp.status();

    if !status.is_success() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to fetch author info: {}", status),
        ));
    }

    let info = resp
        .json::<AuthorInfo>()
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    cache_set(cache_key, &info, DISCOVERY_CACHE_TTL_SECS);
    Ok(info)
}

async fn discovery_fetch_lorebook(
    app: &AppHandle,
    card_id: &str,
) -> Result<Option<DiscoveryLorebook>, String> {
    let card_id = card_id.trim();
    if card_id.is_empty() {
        return Ok(None);
    }

    let cache_key = format!("lorebook:{}", card_id);
    if let Some(cached) = cache_get::<Option<DiscoveryLorebook>>(&cache_key) {
        return Ok(cached);
    }

    let url = format!(
        "https://character-tavern.com/api/character/{}/lorebook",
        card_id
    );

    log_info(
        app,
        "discovery_lorebook",
        format!("Fetching lorebook from: {}", url),
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let status = resp.status();

    if !status.is_success() {
        return Ok(None);
    }

    let lorebook = resp
        .json::<Option<DiscoveryLorebook>>()
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    cache_set(cache_key, &lorebook, DISCOVERY_CACHE_TTL_SECS);
    Ok(lorebook)
}

#[tauri::command]
pub async fn discovery_import_character(app: AppHandle, path: String) -> Result<String, String> {
    log_info(
        &app,
        "discovery_import",
        format!("Importing character from path: {}", path),
    );

    // Fetch card detail from API
    let detail = discovery_fetch_card_detail(app.clone(), path.clone()).await?;
    let card = detail.card;

    let alternate_greetings = discovery_fetch_alternate_greetings(app.clone(), card.id.clone())
        .await
        .unwrap_or_default();
    if !alternate_greetings.is_empty() {
        log_info(
            &app,
            "discovery_import",
            format!("Found {} alternate greetings", alternate_greetings.len()),
        );
    }

    let lorebook = match discovery_fetch_lorebook(&app, &card.id).await {
        Ok(lorebook) => lorebook,
        Err(err) => {
            log_error(
                &app,
                "discovery_import",
                format!("Failed to fetch lorebook: {}", err),
            );
            None
        }
    };

    // Fetch avatar image from CDN
    let client = reqwest::Client::new();

    // Save avatar image locally using CDN URL
    let avatar_cdn_url = get_card_image(
        card.path.clone(),
        Some("webp".to_string()),
        Some(400),
        Some(85),
    )?;

    log_info(
        &app,
        "discovery_import",
        format!("Downloading avatar from CDN: {}", avatar_cdn_url),
    );

    let avatar_response = client.get(&avatar_cdn_url).send().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to download avatar: {}", e),
        )
    })?;

    let avatar_data = avatar_response.bytes().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to read avatar data: {}", e),
        )
    })?;

    // Generate unique UUID for character
    let character_id = uuid::Uuid::new_v4().to_string();
    let avatar_entity_id = format!("character-{}", character_id);

    // Convert bytes to base64 for storage_save_avatar
    let avatar_base64 =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &avatar_data);

    let avatar_path =
        storage_save_avatar(app.clone(), avatar_entity_id.clone(), avatar_base64, None).map_err(
            |e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to save avatar: {}", e),
                )
            },
        )?;

    if let Err(err) =
        generate_avatar_gradient(app.clone(), avatar_entity_id, "avatar_base.webp".into())
    {
        log_error(
            &app,
            "discovery_import",
            format!("Failed to generate avatar gradient: {}", err),
        );
    }

    log_info(
        &app,
        "discovery_import",
        format!("Avatar saved to: {}", avatar_path),
    );

    // Build character JSON
    let now = chrono::Utc::now().timestamp_millis();
    let memory_type = if is_dynamic_memory_enabled(&app) {
        "dynamic"
    } else {
        "manual"
    };

    // Create scenes: first_message + alternate_greetings
    let mut scenes = vec![];

    // Add primary scene from definition_first_message
    if let Some(first_msg) = card.definition_first_message.clone() {
        let scene_id = uuid::Uuid::new_v4().to_string();
        scenes.push(serde_json::json!({
            "id": scene_id,
            "content": first_msg,
            "createdAt": now,
            "variants": []
        }));
    }

    // Add alternate greetings as additional scenes
    for alt_greeting in alternate_greetings {
        let scene_id = uuid::Uuid::new_v4().to_string();
        scenes.push(serde_json::json!({
            "id": scene_id,
            "content": alt_greeting,
            "createdAt": now,
            "variants": []
        }));
    }

    // Build definition from available fields
    let mut definition_parts = vec![];

    push_definition_block(
        &mut definition_parts,
        None,
        card.definition_character_description.clone(),
    );
    push_definition_block(
        &mut definition_parts,
        Some("Personality"),
        card.definition_personality.clone(),
    );
    push_definition_block(
        &mut definition_parts,
        Some("Scenario"),
        card.definition_scenario.clone(),
    );
    push_definition_block(
        &mut definition_parts,
        Some("System Prompt"),
        card.definition_system_prompt.clone(),
    );
    push_definition_block(
        &mut definition_parts,
        Some("Post History Instructions"),
        card.definition_post_history_prompt.clone(),
    );
    push_definition_block(
        &mut definition_parts,
        None,
        card.definition_example_messages.clone().map(|examples| {
            format!(
                "<example_dialogue>\n{}\n</example_dialogue>",
                examples.trim()
            )
        }),
    );

    let definition = if definition_parts.is_empty() {
        None
    } else {
        Some(definition_parts.join("\n\n"))
    };

    let character = serde_json::json!({
        "id": character_id,
        "name": card.in_chat_name.clone().unwrap_or(card.name.clone()),
        "description": card.description.clone().or(card.tagline.clone()).unwrap_or_default(),
        "definition": definition,
        "avatarPath": avatar_path,
        "backgroundImagePath": null,
        "rules": [],
        "defaultSceneId": if !scenes.is_empty() { scenes[0]["id"].as_str() } else { None },
        "defaultModelId": null,
        "memoryType": memory_type,
        "promptTemplateId": null,
        "voiceConfig": null,
        "voiceAutoplay": false,
        "disableAvatarGradient": false,
        "customGradientEnabled": false,
        "customGradientColors": null,
        "customTextColor": null,
        "customTextSecondary": null,
        "scenes": scenes,
        "createdAt": now,
        "updatedAt": now
    });

    let character_json = serde_json::to_string(&character).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize character: {}", e),
        )
    })?;

    // Save to database using existing character_upsert command
    crate::storage_manager::characters::character_upsert(app.clone(), character_json).map_err(
        |e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to save character to database: {}", e),
            )
        },
    )?;

    if let Some(lorebook) = lorebook {
        match crate::storage_manager::db::open_db(&app) {
            Ok(mut conn) => {
                let now = crate::utils::now_millis().unwrap_or(0) as i64;
                let lorebook_id = uuid::Uuid::new_v4().to_string();
                let lorebook_name = if lorebook.name.trim().is_empty() {
                    format!("{} Lorebook", card.name)
                } else {
                    lorebook.name.clone()
                };

                let lorebook_record = Lorebook {
                    id: lorebook_id.clone(),
                    name: lorebook_name,
                    created_at: now,
                    updated_at: now,
                };

                if let Err(err) = upsert_lorebook(&conn, &lorebook_record) {
                    log_error(
                        &app,
                        "discovery_import",
                        format!("Failed to save lorebook: {}", err),
                    );
                } else {
                    for (index, entry) in lorebook.entries.iter().enumerate() {
                        let display_order = entry
                            .insertion_order
                            .and_then(|value| i32::try_from(value).ok())
                            .unwrap_or(index as i32);

                        let always_active = entry.constant && entry.keys.is_empty();
                        let entry_record = LorebookEntry {
                            id: uuid::Uuid::new_v4().to_string(),
                            lorebook_id: lorebook_id.clone(),
                            title: entry.name.clone(),
                            enabled: entry.enabled,
                            always_active,
                            keywords: entry.keys.clone(),
                            case_sensitive: false,
                            content: entry.content.clone(),
                            priority: 0,
                            display_order,
                            created_at: now,
                            updated_at: now,
                        };

                        if let Err(err) = upsert_lorebook_entry(&conn, &entry_record) {
                            log_error(
                                &app,
                                "discovery_import",
                                format!("Failed to save lorebook entry: {}", err),
                            );
                        }
                    }

                    if let Err(err) =
                        set_character_lorebooks(&mut conn, &character_id, &[lorebook_id.clone()])
                    {
                        log_error(
                            &app,
                            "discovery_import",
                            format!("Failed to link lorebook to character: {}", err),
                        );
                    }
                }
            }
            Err(err) => {
                log_error(
                    &app,
                    "discovery_import",
                    format!("Failed to open database for lorebook import: {}", err),
                );
            }
        }
    }

    log_info(
        &app,
        "discovery_import",
        format!("Successfully imported character: {}", character_id),
    );

    Ok(character_id)
}
