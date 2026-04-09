use tauri::AppHandle;
use uuid::Uuid;

use crate::models::{
    calculate_openrouter_request_cost, fetch_openrouter_generation_details,
    fetch_openrouter_provider_pricings, find_openrouter_provider_pricing, OpenRouterCostInput,
};
use crate::usage::{
    add_usage_record,
    tracking::{RequestUsage, UsageFinishReason, UsageOperationType},
};

use crate::utils::{log_error, log_info, log_warn, now_millis};

use super::repository::ChatRepository;
use super::storage::{build_system_prompt, choose_persona, select_model_with_credential};
use super::types::{
    Character, Model, Persona, ProviderCredential, Session, Settings, SystemPromptEntry,
    UsageSummary,
};

pub struct ChatContext {
    repository: ChatRepository,
    pub settings: Settings,
    pub characters: Vec<Character>,
    pub personas: Vec<Persona>,
}

impl ChatContext {
    pub fn initialize(app: AppHandle) -> Result<Self, String> {
        let repository = ChatRepository::new(app);
        let app = repository.app();
        log_info(&app, "chat_context", "Initializing chat context");
        let settings = repository.load_settings()?;
        let characters = repository.load_characters()?;
        let personas = repository.load_personas()?;

        log_info(
            &app,
            "chat_context",
            format!(
                "Context loaded: {} characters, {} personas",
                characters.len(),
                personas.len()
            ),
        );

        Ok(Self {
            repository,
            settings,
            characters,
            personas,
        })
    }

    pub fn app(&self) -> &AppHandle {
        self.repository.app()
    }

    pub fn find_character(&self, character_id: &str) -> Result<Character, String> {
        self.characters
            .iter()
            .find(|c| c.id == character_id)
            .cloned()
            .ok_or_else(|| "Character not found".to_string())
    }

    pub fn select_model_with_credential<'a>(
        &'a self,
        character: &Character,
    ) -> Result<(&'a Model, &'a ProviderCredential), String> {
        select_model_with_credential(&self.settings, character)
    }

    pub fn load_session(&self, session_id: &str) -> Result<Option<Session>, String> {
        self.repository.load_session(session_id)
    }

    pub fn save_session(&self, session: &Session) -> Result<(), String> {
        self.repository.save_session(session)
    }

    pub fn build_system_prompt(
        &self,
        character: &Character,
        model: &Model,
        persona: Option<&Persona>,
        session: &Session,
    ) -> Vec<SystemPromptEntry> {
        build_system_prompt(
            self.app(),
            character,
            model,
            persona,
            session,
            &self.settings,
        )
    }

    pub fn choose_persona(&self, explicit_persona_id: Option<&str>) -> Option<&Persona> {
        let owned = explicit_persona_id.map(|id| id.to_string());
        choose_persona(&self.personas, owned.as_ref())
    }
}

pub struct ChatService {
    context: ChatContext,
}

pub struct PreparedChatTurn {
    pub context: ChatContext,
    pub character: Character,
    pub session: Session,
    pub persona: Option<Persona>,
    pub model: Model,
    pub credential: ProviderCredential,
}

impl ChatService {
    pub fn initialize(app: AppHandle) -> Result<Self, String> {
        Ok(Self {
            context: ChatContext::initialize(app)?,
        })
    }

    pub fn prepare_turn(
        self,
        session_id: &str,
        character_id: &str,
        persona_id: Option<&str>,
    ) -> Result<PreparedChatTurn, String> {
        let session = self
            .context
            .load_session(session_id)?
            .ok_or_else(|| "Session not found".to_string())?;
        self.prepare_loaded_turn(session, character_id, persona_id, true)
    }

    pub fn prepare_regeneration(self, session_id: &str) -> Result<PreparedChatTurn, String> {
        let session = self
            .context
            .load_session(session_id)?
            .ok_or_else(|| "Session not found".to_string())?;
        let character_id = session.character_id.clone();
        self.prepare_loaded_turn(session, &character_id, None, false)
    }

    fn prepare_loaded_turn(
        self,
        mut session: Session,
        character_id: &str,
        persona_id: Option<&str>,
        sync_character_id: bool,
    ) -> Result<PreparedChatTurn, String> {
        let character = self.context.find_character(character_id)?;

        if sync_character_id && session.character_id != character.id {
            session.character_id = character.id.clone();
        }

        let effective_persona_id = resolve_persona_id(&session, persona_id);
        let persona = self.context.choose_persona(effective_persona_id).cloned();
        let (model, credential) = self.context.select_model_with_credential(&character)?;
        let model = model.clone();
        let credential = credential.clone();

        Ok(PreparedChatTurn {
            context: self.context,
            character,
            session,
            persona,
            model,
            credential,
        })
    }
}

fn resolve_persona_id<'a>(session: &'a Session, explicit: Option<&'a str>) -> Option<&'a str> {
    if explicit.is_some() {
        return explicit;
    }
    if session.persona_disabled {
        Some("")
    } else {
        session.persona_id.as_deref()
    }
}

pub fn require_api_key(
    app: &AppHandle,
    credential: &ProviderCredential,
    log_scope: &str,
) -> Result<String, String> {
    if credential.provider_id == "llamacpp" || credential.provider_id == "ollama" {
        return Ok(credential.api_key.clone().unwrap_or_default());
    }
    // Prefer inline api_key on the credential
    if let Some(ref key) = credential.api_key {
        if !key.is_empty() {
            return Ok(key.clone());
        }
    }
    log_error(
        app,
        log_scope,
        format!(
            "provider credential {} missing API key",
            credential.id.as_str()
        ),
    );
    Err(crate::utils::err_msg(
        module_path!(),
        line!(),
        "Provider credential missing API key",
    ))
}

pub fn insert_extended_usage_metadata(
    metadata: &mut std::collections::HashMap<String, String>,
    usage: &UsageSummary,
) {
    if let Some(response_id) = &usage.response_id {
        metadata.insert("openrouter_response_id".to_string(), response_id.clone());
    }
    if let Some(api_cost) = usage.api_cost {
        metadata.insert("openrouter_api_cost".to_string(), api_cost.to_string());
    }
    if let Some(cached_prompt_tokens) = usage.cached_prompt_tokens {
        metadata.insert(
            "openrouter_cached_prompt_tokens".to_string(),
            cached_prompt_tokens.to_string(),
        );
    }
    if let Some(cache_write_tokens) = usage.cache_write_tokens {
        metadata.insert(
            "openrouter_cache_write_tokens".to_string(),
            cache_write_tokens.to_string(),
        );
    }
    if let Some(web_search_requests) = usage.web_search_requests {
        metadata.insert(
            "openrouter_web_search_requests".to_string(),
            web_search_requests.to_string(),
        );
    }
    if let Some(api_cost) = usage.api_cost {
        metadata.insert("api_cost".to_string(), api_cost.to_string());
    }
    if let Some(cached_prompt_tokens) = usage.cached_prompt_tokens {
        metadata.insert(
            "cached_prompt_tokens".to_string(),
            cached_prompt_tokens.to_string(),
        );
    }
    if let Some(cache_write_tokens) = usage.cache_write_tokens {
        metadata.insert(
            "cache_write_tokens".to_string(),
            cache_write_tokens.to_string(),
        );
    }
    if let Some(web_search_requests) = usage.web_search_requests {
        metadata.insert(
            "web_search_requests".to_string(),
            web_search_requests.to_string(),
        );
    }
}

fn insert_cost_breakdown_metadata(
    metadata: &mut std::collections::HashMap<String, String>,
    cost: &crate::models::RequestCost,
) {
    metadata.insert(
        "cost_regular_prompt_tokens".to_string(),
        cost.regular_prompt_tokens.to_string(),
    );
    metadata.insert(
        "cost_cached_prompt_tokens".to_string(),
        cost.cached_prompt_tokens.to_string(),
    );
    metadata.insert(
        "cost_cache_write_tokens".to_string(),
        cost.cache_write_tokens.to_string(),
    );
    metadata.insert(
        "cost_reasoning_tokens".to_string(),
        cost.reasoning_tokens.to_string(),
    );
    metadata.insert(
        "cost_web_search_requests".to_string(),
        cost.web_search_requests.to_string(),
    );
    metadata.insert(
        "cost_prompt_base".to_string(),
        cost.prompt_base_cost.to_string(),
    );
    metadata.insert(
        "cost_cache_read".to_string(),
        cost.cache_read_cost.to_string(),
    );
    metadata.insert(
        "cost_cache_write".to_string(),
        cost.cache_write_cost.to_string(),
    );
    metadata.insert(
        "cost_completion_base".to_string(),
        cost.completion_base_cost.to_string(),
    );
    metadata.insert(
        "cost_reasoning".to_string(),
        cost.reasoning_cost.to_string(),
    );
    metadata.insert("cost_request".to_string(), cost.request_cost.to_string());
    metadata.insert(
        "cost_web_search".to_string(),
        cost.web_search_cost.to_string(),
    );
    if let Some(authoritative_total_cost) = cost.authoritative_total_cost {
        metadata.insert(
            "cost_authoritative_total".to_string(),
            authoritative_total_cost.to_string(),
        );
    }
}

pub async fn apply_openrouter_cost_to_usage(
    app: &AppHandle,
    request_usage: &mut RequestUsage,
    usage_info: &UsageSummary,
    model_name: &str,
    api_key: &str,
    log_scope: &str,
) {
    insert_extended_usage_metadata(&mut request_usage.metadata, usage_info);

    let provider_pricings =
        match fetch_openrouter_provider_pricings(app.clone(), api_key, model_name).await {
            Ok(pricings) if !pricings.is_empty() => pricings,
            Ok(_) => {
                log_warn(
                    app,
                    log_scope,
                    format!("no OpenRouter provider pricing found for {}", model_name),
                );
                return;
            }
            Err(err) => {
                log_error(
                    app,
                    log_scope,
                    format!("failed to fetch OpenRouter provider pricing: {}", err),
                );
                return;
            }
        };

    let generation = match usage_info.response_id.as_deref() {
        Some(response_id) => {
            match fetch_openrouter_generation_details(app.clone(), api_key, response_id).await {
                Ok(details) => details,
                Err(err) => {
                    log_warn(
                        app,
                        log_scope,
                        format!("failed to fetch OpenRouter generation details: {}", err),
                    );
                    None
                }
            }
        }
        None => None,
    };

    let provider_name = generation
        .as_ref()
        .and_then(|details| details.provider_name.clone());

    if let Some(provider_name) = &provider_name {
        request_usage.metadata.insert(
            "openrouter_provider_name".to_string(),
            provider_name.clone(),
        );
    }

    let Some(pricing) = provider_name
        .as_deref()
        .and_then(|provider_name| {
            find_openrouter_provider_pricing(&provider_pricings, provider_name)
        })
        .map(|entry| entry.pricing.clone())
        .or_else(|| provider_pricings.first().map(|entry| entry.pricing.clone()))
    else {
        log_warn(
            app,
            log_scope,
            format!(
                "unable to resolve OpenRouter provider pricing for {}",
                model_name
            ),
        );
        return;
    };

    if provider_name.is_none() {
        request_usage.metadata.insert(
            "openrouter_pricing_match".to_string(),
            "fallback_first_provider".to_string(),
        );
    } else {
        request_usage.metadata.insert(
            "openrouter_pricing_match".to_string(),
            "matched_provider_name".to_string(),
        );
    }

    let prompt_tokens = generation
        .as_ref()
        .and_then(|details| details.native_prompt_tokens)
        .or(usage_info.prompt_tokens)
        .unwrap_or(0);
    let completion_tokens = generation
        .as_ref()
        .and_then(|details| details.native_completion_tokens)
        .or(usage_info.completion_tokens)
        .unwrap_or(0);
    let authoritative_total_cost = generation
        .as_ref()
        .and_then(|details| details.total_cost)
        .or(usage_info.api_cost);

    if let Some(total_cost) = generation.as_ref().and_then(|details| details.total_cost) {
        request_usage.metadata.insert(
            "openrouter_generation_total_cost".to_string(),
            total_cost.to_string(),
        );
    }

    let cost_input = OpenRouterCostInput {
        prompt_tokens,
        completion_tokens,
        cached_prompt_tokens: usage_info.cached_prompt_tokens.unwrap_or(0),
        cache_write_tokens: usage_info.cache_write_tokens.unwrap_or(0),
        reasoning_tokens: usage_info.reasoning_tokens.unwrap_or(0),
        web_search_requests: usage_info.web_search_requests.unwrap_or(0),
        authoritative_total_cost,
    };

    if let Some(cost) = calculate_openrouter_request_cost(&cost_input, &pricing) {
        insert_cost_breakdown_metadata(&mut request_usage.metadata, &cost);
        request_usage.cost = Some(cost.clone());
        request_usage.prompt_tokens = Some(prompt_tokens);
        request_usage.completion_tokens = Some(completion_tokens);
        request_usage.total_tokens = Some(prompt_tokens + completion_tokens);
        request_usage.cached_prompt_tokens = usage_info.cached_prompt_tokens;
        request_usage.cache_write_tokens = usage_info.cache_write_tokens;
        request_usage.web_search_requests = usage_info.web_search_requests;
        request_usage.api_cost = authoritative_total_cost.or(usage_info.api_cost);

        if let Some(authoritative_total_cost) = authoritative_total_cost {
            request_usage.metadata.insert(
                "openrouter_authoritative_total_cost".to_string(),
                authoritative_total_cost.to_string(),
            );
        }

        log_info(
            app,
            log_scope,
            format!(
                "calculated OpenRouter routed cost for provider={:?}: ${:.6}",
                provider_name, cost.total_cost
            ),
        );
    } else {
        log_error(
            app,
            log_scope,
            "failed to calculate OpenRouter routed cost".to_string(),
        );
    }
}

pub async fn record_usage_if_available(
    context: &ChatContext,
    usage: &Option<UsageSummary>,
    session: &Session,
    character: &Character,
    model: &Model,
    credential: &ProviderCredential,
    api_key: &str,
    created_at: u64,
    operation_type: UsageOperationType,
    log_scope: &str,
) {
    let Some(usage_info) = usage else {
        return;
    };

    let mut request_usage = RequestUsage {
        id: Uuid::new_v4().to_string(),
        timestamp: now_millis().unwrap_or(created_at),
        session_id: session.id.clone(),
        character_id: character.id.clone(),
        character_name: character.name.clone(),
        model_id: model.id.clone(),
        model_name: model.name.clone(),
        provider_id: credential.provider_id.clone(),
        provider_label: credential.provider_id.clone(),
        operation_type,
        finish_reason: usage_info
            .finish_reason
            .as_ref()
            .and_then(|s| UsageFinishReason::from_str(s)),
        prompt_tokens: usage_info.prompt_tokens,
        completion_tokens: usage_info.completion_tokens,
        total_tokens: usage_info.total_tokens,
        cached_prompt_tokens: usage_info.cached_prompt_tokens,
        cache_write_tokens: usage_info.cache_write_tokens,
        memory_tokens: None,
        summary_tokens: None,
        reasoning_tokens: usage_info.reasoning_tokens,
        image_tokens: usage_info.image_tokens,
        web_search_requests: usage_info.web_search_requests,
        api_cost: usage_info.api_cost,
        cost: None,
        success: true,
        error_message: None,
        metadata: Default::default(),
    };

    // Calculate memory and summary token counts only when dynamic memory is active.
    let dynamic_memory_active = context
        .settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.dynamic_memory.as_ref())
        .map(|dm| dm.enabled)
        .unwrap_or(false)
        && character.memory_type.eq_ignore_ascii_case("dynamic");

    if dynamic_memory_active {
        let mut memory_token_count = 0u64;
        for emb in &session.memory_embeddings {
            memory_token_count += emb.token_count as u64;
        }

        let summary_token_count = session.memory_summary_token_count as u64;

        if memory_token_count > 0 {
            request_usage.memory_tokens = Some(memory_token_count);
        }

        if summary_token_count > 0 {
            request_usage.summary_tokens = Some(summary_token_count);
        }
    }

    if credential.provider_id.eq_ignore_ascii_case("openrouter") {
        apply_openrouter_cost_to_usage(
            context.app(),
            &mut request_usage,
            usage_info,
            &model.name,
            api_key,
            log_scope,
        )
        .await;
    } else if usage_info.cached_prompt_tokens.is_some()
        || usage_info.cache_write_tokens.is_some()
        || usage_info.web_search_requests.is_some()
        || usage_info.api_cost.is_some()
    {
        insert_extended_usage_metadata(&mut request_usage.metadata, usage_info);
    }

    if let Err(err) = add_usage_record(context.app(), request_usage) {
        log_error(
            context.app(),
            log_scope,
            format!("failed to save usage record: {}", err),
        );
    }
}

pub fn record_failed_usage(
    app: &tauri::AppHandle,
    usage: &Option<UsageSummary>,
    session: &Session,
    character: &Character,
    model: &Model,
    credential: &ProviderCredential,
    operation_type: UsageOperationType,
    error_message: &str,
    log_scope: &str,
) {
    let Some(usage_info) = usage else {
        return;
    };

    let request_usage = RequestUsage {
        id: Uuid::new_v4().to_string(),
        timestamp: now_millis().unwrap_or(0),
        session_id: session.id.clone(),
        character_id: character.id.clone(),
        character_name: character.name.clone(),
        model_id: model.id.clone(),
        model_name: model.name.clone(),
        provider_id: credential.provider_id.clone(),
        provider_label: credential.provider_id.clone(),
        operation_type,
        finish_reason: usage_info
            .finish_reason
            .as_ref()
            .and_then(|s| UsageFinishReason::from_str(s)),
        prompt_tokens: usage_info.prompt_tokens,
        completion_tokens: usage_info.completion_tokens,
        total_tokens: usage_info.total_tokens,
        cached_prompt_tokens: usage_info.cached_prompt_tokens,
        cache_write_tokens: usage_info.cache_write_tokens,
        memory_tokens: None,
        summary_tokens: None,
        reasoning_tokens: usage_info.reasoning_tokens,
        image_tokens: usage_info.image_tokens,
        web_search_requests: usage_info.web_search_requests,
        api_cost: usage_info.api_cost,
        cost: None,
        success: false,
        error_message: Some(error_message.to_string()),
        metadata: Default::default(),
    };

    if credential.provider_id.eq_ignore_ascii_case("openrouter")
        || usage_info.cached_prompt_tokens.is_some()
        || usage_info.cache_write_tokens.is_some()
        || usage_info.web_search_requests.is_some()
        || usage_info.api_cost.is_some()
    {
        let mut request_usage = request_usage;
        insert_extended_usage_metadata(&mut request_usage.metadata, usage_info);

        log_info(
            app,
            log_scope,
            format!(
                "recording failed usage: tokens={:?} error={}",
                usage_info.total_tokens, error_message
            ),
        );

        if let Err(err) = add_usage_record(app, request_usage) {
            log_error(
                app,
                log_scope,
                format!("failed to save failed usage record: {}", err),
            );
        }
        return;
    }

    log_info(
        app,
        log_scope,
        format!(
            "recording failed usage: tokens={:?} error={}",
            usage_info.total_tokens, error_message
        ),
    );

    if let Err(err) = add_usage_record(app, request_usage) {
        log_error(
            app,
            log_scope,
            format!("failed to save failed usage record: {}", err),
        );
    }
}
