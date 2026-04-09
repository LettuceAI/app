use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::api::{api_request, ApiRequest};
use crate::chat_manager::attachments::{
    cleanup_attachments, load_attachment_data, persist_attachments,
};
use crate::chat_manager::commands::take_aborted_request;
use crate::chat_manager::execution::{
    build_model_attempts, build_provider_extra_fields, emit_fallback_retry_toast, RequestSettings,
};
use crate::chat_manager::memory::dynamic::{
    context_enrichment_enabled, dynamic_min_similarity, dynamic_retrieval_limit,
    dynamic_retrieval_strategy, dynamic_window_size, ensure_pinned_hot, mark_memories_accessed,
    promote_cold_memories,
};
use crate::chat_manager::memory::flow::{process_dynamic_memory_cycle, select_relevant_memories};
use crate::chat_manager::messages::{
    push_prompt_entry_message, push_system_message, push_user_or_assistant_message_with_context,
    sanitize_placeholders_in_api_messages,
};
use crate::chat_manager::request::{
    extract_error_message, extract_reasoning, extract_text, extract_usage, new_assistant_variant,
};
use crate::chat_manager::service::{
    record_failed_usage, record_usage_if_available, require_api_key, ChatService, PreparedChatTurn,
};
use crate::chat_manager::storage::recent_messages;
use crate::chat_manager::turn_builder::{
    append_image_directive_instructions, build_enriched_query, conversation_window_with_pinned,
    insert_in_chat_prompt_entries, is_dynamic_memory_active, manual_window_size,
    maybe_swap_message_for_api, partition_prompt_entries, role_swap_enabled,
    swapped_prompt_entities,
};
use crate::chat_manager::types::{
    ChatContinueArgs, ContinueResult, ImageAttachment, StoredMessage,
};
use crate::usage::tracking::UsageOperationType;
use crate::utils::{
    emit_debug, emit_error_event, emit_info, emit_warn_event, log_error, log_info, log_warn,
    now_millis,
};

pub struct ContinueFlow {
    app: AppHandle,
}

impl ContinueFlow {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    pub async fn execute(self, args: ChatContinueArgs) -> Result<ContinueResult, String> {
        let app = self.app;
        let ChatContinueArgs {
            session_id,
            character_id,
            persona_id,
            swap_places,
            stream,
            request_id,
        } = args;
        let swap_places = role_swap_enabled(swap_places);

        log_info(
            &app,
            "chat_continue",
            format!(
                "start session={} character={} stream={:?} request_id={:?}",
                &session_id, &character_id, stream, request_id
            ),
        );

        let prepared = ChatService::initialize(app.clone())?.prepare_turn(
            &session_id,
            &character_id,
            persona_id.as_deref(),
        )?;
        let PreparedChatTurn {
            context,
            character,
            mut session,
            persona,
            model,
            credential,
        } = prepared;
        let settings = &context.settings;

        emit_info(
            &app,
            "continue_start",
            json!({
                "sessionId": session.id,
                "characterId": character_id,
                "messageCount": session.messages.len(),
            }),
        );

        let stored_total_messages = session.messages.len();
        let stored_convo_messages = conversation_count(&session.messages);
        log_info(
            &app,
            "chat_continue",
            format!(
                "stored message counts before continue total={} convo={} (no [CONTINUE] prompt persisted)",
                stored_total_messages, stored_convo_messages
            ),
        );

        log_info(
            &app,
            "chat_continue",
            format!(
                "selected provider={} model={} credential={}",
                credential.provider_id.as_str(),
                model.name.as_str(),
                credential.id.as_str()
            ),
        );

        emit_info(
            &app,
            "continue_model_selected",
            json!({
                "providerId": credential.provider_id,
                "model": model.name,
                "credentialId": credential.id,
            }),
        );

        let dynamic_memory_enabled = is_dynamic_memory_active(settings, &character);
        let dynamic_window = dynamic_window_size(settings);

        let relevant_memories = if dynamic_memory_enabled && !session.memory_embeddings.is_empty() {
            let fixed = ensure_pinned_hot(&mut session.memory_embeddings);
            if fixed > 0 {
                log_info(
                    &app,
                    "dynamic_memory",
                    format!("Restored {} pinned memories to hot", fixed),
                );
            }

            let search_query = if context_enrichment_enabled(&context.settings) {
                build_enriched_query(&session.messages)
            } else {
                session
                    .messages
                    .iter()
                    .rev()
                    .find(|m| m.role == "user")
                    .map(|m| m.content.clone())
                    .unwrap_or_default()
            };
            select_relevant_memories(
                &app,
                &session,
                &search_query,
                dynamic_retrieval_limit(&context.settings),
                dynamic_min_similarity(&context.settings),
                dynamic_retrieval_strategy(&context.settings),
            )
            .await
        } else {
            Vec::new()
        };

        if !relevant_memories.is_empty() {
            let memory_ids: Vec<String> = relevant_memories.iter().map(|m| m.id.clone()).collect();
            let now = now_millis().unwrap_or_default();
            let promoted = promote_cold_memories(&mut session.memory_embeddings, &memory_ids, now);
            let accessed = mark_memories_accessed(&mut session.memory_embeddings, &memory_ids, now);
            if promoted > 0 {
                log_info(
                    &app,
                    "dynamic_memory",
                    format!("Promoted {} cold memories to hot", promoted),
                );
            }
            if accessed > 0 {
                log_info(
                    &app,
                    "dynamic_memory",
                    format!("Marked {} memories as accessed", accessed),
                );
            }
        }

        let prompt_entries = if swap_places {
            let (prompt_character, prompt_persona) =
                swapped_prompt_entities(&character, persona.as_ref());
            append_image_directive_instructions(
                context.build_system_prompt(
                    &prompt_character,
                    &model,
                    prompt_persona.as_ref(),
                    &session,
                ),
                settings,
            )
        } else {
            append_image_directive_instructions(
                context.build_system_prompt(&character, &model, persona.as_ref(), &session),
                settings,
            )
        };
        let used_lorebook_entries =
            crate::chat_manager::prompt_engine::resolve_used_lorebook_entries(
                &app,
                &character.id,
                &session,
                &prompt_entries,
            );
        let (relative_entries, in_chat_entries) = partition_prompt_entries(prompt_entries);

        let (pinned_msgs, recent_msgs) = if dynamic_memory_enabled {
            let (pinned, unpinned) =
                conversation_window_with_pinned(&session.messages, dynamic_window);
            (pinned, unpinned)
        } else {
            (
                Vec::new(),
                recent_messages(&session, manual_window_size(settings)),
            )
        };

        let system_role = crate::chat_manager::request_builder::system_role_for(&credential);
        let mut messages_for_api = Vec::new();
        for entry in &relative_entries {
            push_prompt_entry_message(&mut messages_for_api, &system_role, entry);
        }
        if swap_places {
            let persona_title = persona
                .as_ref()
                .map(|p| p.title.clone())
                .unwrap_or_else(|| "the user persona".to_string());
            push_system_message(
                &mut messages_for_api,
                &system_role,
                Some(format!(
                    "Swap places mode is active for this turn. The human is speaking as character '{}' and you must respond as persona '{}'. Keep the response in first person as '{}'.",
                    character.name, persona_title, persona_title
                )),
            );
        }

        let char_name = if swap_places {
            persona.as_ref().map(|p| p.title.as_str()).unwrap_or("User")
        } else {
            character.name.as_str()
        };
        let persona_name = if swap_places {
            character.name.as_str()
        } else {
            persona.as_ref().map(|p| p.title.as_str()).unwrap_or("")
        };
        let allow_image_input = model
            .input_scopes
            .iter()
            .any(|scope| scope.eq_ignore_ascii_case("image"));

        let mut chat_messages = Vec::new();
        for msg in &pinned_msgs {
            let msg_with_data = load_attachment_data(&app, msg);
            let msg_with_data = maybe_swap_message_for_api(&msg_with_data, swap_places);
            push_user_or_assistant_message_with_context(
                &mut chat_messages,
                &msg_with_data,
                char_name,
                persona_name,
                allow_image_input,
            );
        }

        for msg in &recent_msgs {
            let msg_with_data = load_attachment_data(&app, msg);
            let msg_with_data = maybe_swap_message_for_api(&msg_with_data, swap_places);
            push_user_or_assistant_message_with_context(
                &mut chat_messages,
                &msg_with_data,
                char_name,
                persona_name,
                allow_image_input,
            );
        }
        insert_in_chat_prompt_entries(&mut chat_messages, &system_role, &in_chat_entries);
        messages_for_api.extend(chat_messages);
        sanitize_placeholders_in_api_messages(&mut messages_for_api, char_name, persona_name);

        let should_inject_continue_prompt = session
            .messages
            .iter()
            .rev()
            .find(|message| message.role == "user" || message.role == "assistant")
            .map(|message| message.role != "user")
            .unwrap_or(true);

        if should_inject_continue_prompt {
            messages_for_api.push(json!({
                "role": "user",
                "content": "[CONTINUE] You were in the middle of a response. Continue writing from exactly where you left off. Do NOT restart, regenerate, or rewrite what you already said. Simply pick up the narrative thread and continue the scene forward with new content."
            }));
        }

        let should_stream = stream.unwrap_or(true);
        let request_id = if should_stream {
            request_id.or_else(|| Some(Uuid::new_v4().to_string()))
        } else {
            None
        };
        let attempts = build_model_attempts(
            &app,
            settings,
            &character,
            &model,
            &credential,
            "chat_continue",
        );

        let mut selected_model = &model;
        let mut selected_credential = &credential;
        let mut selected_api_key = String::new();
        let mut fallback_from_model_id: Option<String> = None;
        let mut successful_response = None;
        let mut last_error = "request failed".to_string();
        let mut fallback_toast_shown = false;

        for (idx, (attempt_model, attempt_credential, is_fallback_attempt)) in
            attempts.iter().enumerate()
        {
            let has_next_attempt = idx + 1 < attempts.len();

            let attempt_api_key = match require_api_key(&app, attempt_credential, "chat_continue") {
                Ok(key) => key,
                Err(err) => {
                    last_error = err;
                    if has_next_attempt {
                        emit_fallback_retry_toast(&app, &mut fallback_toast_shown);
                        continue;
                    }
                    return Err(last_error);
                }
            };

            let request_settings =
                RequestSettings::resolve(&session, attempt_model, &context.settings);
            let extra_body_fields = build_provider_extra_fields(
                &attempt_credential.provider_id,
                &session,
                attempt_model,
                &context.settings,
                &request_settings,
            );

            let built = crate::chat_manager::request_builder::build_chat_request(
                attempt_credential,
                &attempt_api_key,
                &attempt_model.name,
                &messages_for_api,
                None,
                request_settings.temperature,
                request_settings.top_p,
                request_settings.max_tokens,
                request_settings.context_length,
                should_stream,
                request_id.clone(),
                request_settings.frequency_penalty,
                request_settings.presence_penalty,
                request_settings.top_k,
                None,
                request_settings.reasoning_enabled,
                request_settings.reasoning_effort.clone(),
                request_settings.reasoning_budget,
                request_settings.prompt_caching_enabled.unwrap_or(false),
                extra_body_fields,
            );

            let request_started_at = now_millis().unwrap_or_default();

            emit_info(
                &app,
                "continue_request",
                json!({
                    "operation": "continue",
                    "sessionId": session.id,
                    "providerId": attempt_credential.provider_id,
                    "model": attempt_model.name,
                    "stream": should_stream,
                    "requestId": request_id,
                    "endpoint": built.url,
                    "requestStartedAt": request_started_at,
                    "requestBody": &built.body,
                    "requestSettings": {
                        "temperature": request_settings.temperature,
                        "topP": request_settings.top_p,
                        "maxTokens": request_settings.max_tokens,
                        "contextLength": request_settings.context_length,
                        "frequencyPenalty": request_settings.frequency_penalty,
                        "presencePenalty": request_settings.presence_penalty,
                        "topK": request_settings.top_k,
                        "reasoningEnabled": request_settings.reasoning_enabled,
                        "reasoningEffort": request_settings.reasoning_effort,
                        "reasoningBudget": request_settings.reasoning_budget,
                    },
                    "fallbackAttempt": is_fallback_attempt,
                }),
            );

            let api_request_payload = ApiRequest {
                url: built.url,
                method: Some("POST".into()),
                headers: Some(built.headers),
                query: None,
                body: Some(built.body),
                timeout_ms: Some(crate::transport::DEFAULT_REQUEST_TIMEOUT_MS),
                stream: Some(built.stream),
                request_id: built.request_id.clone(),
                provider_id: Some(attempt_credential.provider_id.clone()),
            };

            let api_response = match api_request(app.clone(), api_request_payload).await {
                Ok(resp) => resp,
                Err(err) => {
                    last_error = err;
                    if has_next_attempt {
                        emit_fallback_retry_toast(&app, &mut fallback_toast_shown);
                        continue;
                    }
                    return Err(last_error);
                }
            };

            emit_info(
                &app,
                "continue_response",
                json!({
                    "operation": "continue",
                    "sessionId": session.id,
                    "requestId": request_id,
                    "status": api_response.status,
                    "ok": api_response.ok,
                    "model": attempt_model.name,
                    "elapsedMs": now_millis().unwrap_or_default().saturating_sub(request_started_at),
                }),
            );

            if !api_response.ok {
                let fallback = format!("Provider returned status {}", api_response.status);
                let err_message =
                    extract_error_message(api_response.data()).unwrap_or(fallback.clone());
                let failed_usage = extract_usage(api_response.data());
                emit_error_event(
                    &app,
                    "continue_provider_error",
                    json!({
                        "operation": "continue",
                        "sessionId": session.id,
                        "requestId": request_id,
                        "status": api_response.status,
                        "message": err_message,
                        "usage": failed_usage,
                        "model": attempt_model.name,
                    }),
                );
                if !has_next_attempt {
                    record_failed_usage(
                        &app,
                        &failed_usage,
                        &session,
                        &character,
                        attempt_model,
                        attempt_credential,
                        UsageOperationType::Continue,
                        &err_message,
                        "chat_continue",
                    );
                }
                last_error = if err_message == fallback {
                    err_message
                } else {
                    format!("{} (status {})", err_message, api_response.status)
                };
                if has_next_attempt {
                    emit_fallback_retry_toast(&app, &mut fallback_toast_shown);
                    continue;
                }
                return Err(last_error);
            }

            selected_model = attempt_model;
            selected_credential = attempt_credential;
            selected_api_key = attempt_api_key;
            fallback_from_model_id = if *is_fallback_attempt {
                Some(model.id.clone())
            } else {
                None
            };
            successful_response = Some(api_response);
            break;
        }

        let api_response = match successful_response {
            Some(resp) => resp,
            None => return Err(last_error),
        };

        if take_aborted_request(&app, request_id.as_deref()) {
            return Err("Request aborted by user".to_string());
        }

        let images_from_sse = match api_response.data() {
            Value::String(s) if s.contains("data:") => {
                crate::chat_manager::sse::accumulate_image_data_urls_from_sse(s)
            }
            _ => Vec::new(),
        };

        let text = extract_text(api_response.data(), Some(&selected_credential.provider_id))
            .unwrap_or_default();
        let usage = extract_usage(api_response.data());
        let reasoning =
            extract_reasoning(api_response.data(), Some(&selected_credential.provider_id));

        if text.trim().is_empty() && images_from_sse.is_empty() {
            let has_reasoning = reasoning.as_ref().is_some_and(|r| !r.trim().is_empty());
            let error_detail = if has_reasoning {
                "Model completed reasoning but generated no response text. This may indicate the model ran out of tokens or encountered an issue during generation."
            } else {
                "Empty response from provider"
            };

            log_warn(
                &app,
                "chat_continue",
                format!(
                    "empty response from provider, has_reasoning={}",
                    has_reasoning
                ),
            );
            emit_warn_event(
                &app,
                "continue_empty_response",
                json!({ "hasReasoning": has_reasoning, "requestId": request_id }),
            );
            return Err(error_detail.to_string());
        }

        if let Some(filter) = app.try_state::<crate::content_filter::ContentFilter>() {
            if filter.is_enabled() {
                let result = filter.check_text(&text);
                if result.blocked {
                    log_warn(
                        &app,
                        "chat_continue",
                        format!(
                            "Content blocked by Pure Mode (score={:.1}, terms={:?})",
                            result.score, result.matched_terms
                        ),
                    );
                    return Err(
                        "Response blocked by Pure Mode. Try rephrasing your message.".to_string(),
                    );
                }
            }
        }

        emit_info(
            &app,
            "continue_assistant_reply",
            json!({
                "length": text.len(),
                "requestId": request_id,
                "operation": "continue",
            }),
        );

        let assistant_created_at = now_millis()?;
        let variant = new_assistant_variant(text.clone(), usage.clone(), assistant_created_at);
        let variant_id = variant.id.clone();

        let assistant_message_id = Uuid::new_v4().to_string();

        let mut assistant_generated_attachments: Vec<ImageAttachment> = Vec::new();
        for data_url in images_from_sse {
            let mime_type = data_url
                .split_once(";base64,")
                .and_then(|(prefix, _)| prefix.strip_prefix("data:"))
                .unwrap_or("image/png")
                .to_string();
            assistant_generated_attachments.push(ImageAttachment {
                id: Uuid::new_v4().to_string(),
                data: data_url,
                mime_type,
                filename: None,
                width: None,
                height: None,
                storage_path: None,
            });
        }

        let persisted_assistant_attachments = persist_attachments(
            &app,
            &character_id,
            &session_id,
            &assistant_message_id,
            "assistant",
            assistant_generated_attachments,
        )?;

        if take_aborted_request(&app, request_id.as_deref()) {
            cleanup_attachments(&app, &persisted_assistant_attachments, "chat_continue");
            return Err("Request aborted by user".to_string());
        }

        let assistant_message = StoredMessage {
            id: assistant_message_id,
            role: "assistant".into(),
            content: text.clone(),
            created_at: assistant_created_at,
            usage: usage.clone(),
            variants: vec![variant],
            selected_variant_id: Some(variant_id),
            memory_refs: if dynamic_memory_enabled {
                relevant_memories
                    .iter()
                    .map(|m| {
                        if let Some(score) = m.match_score {
                            format!("{}::{}", score, m.text)
                        } else {
                            m.text.clone()
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            },
            used_lorebook_entries,
            is_pinned: false,
            attachments: persisted_assistant_attachments,
            reasoning,
            model_id: Some(selected_model.id.clone()),
            fallback_from_model_id: fallback_from_model_id.clone(),
        };

        session.messages.push(assistant_message.clone());
        session.updated_at = now_millis()?;
        if take_aborted_request(&app, request_id.as_deref()) {
            cleanup_attachments(&app, &assistant_message.attachments, "chat_continue");
            return Err("Request aborted by user".to_string());
        }
        context.save_session(&session)?;

        emit_debug(
            &app,
            "continue_session_saved",
            json!({
                "sessionId": session.id,
                "messageId": assistant_message.id,
                "requestId": request_id,
                "messageCount": session.messages.len(),
                "updatedAt": session.updated_at,
            }),
        );

        log_info(
            &app,
            "chat_continue",
            format!(
                "assistant continuation saved message_id={} total_messages={} convo_messages={} request_id={:?}",
                assistant_message.id.as_str(),
                session.messages.len(),
                conversation_count(&session.messages),
                &request_id
            ),
        );

        record_usage_if_available(
            &context,
            &usage,
            &session,
            &character,
            selected_model,
            selected_credential,
            &selected_api_key,
            assistant_created_at,
            UsageOperationType::Continue,
            "chat_continue",
        )
        .await;

        if dynamic_memory_enabled {
            if let Err(err) =
                process_dynamic_memory_cycle(&app, &mut session, settings, &character).await
            {
                log_error(
                    &app,
                    "chat_continue",
                    format!("dynamic memory cycle failed: {}", err),
                );
            }
        }

        Ok(ContinueResult {
            session_id: session.id,
            session_updated_at: session.updated_at,
            request_id,
            assistant_message,
        })
    }
}

fn conversation_count(messages: &[StoredMessage]) -> usize {
    messages
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .count()
}
