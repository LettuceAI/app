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
use crate::chat_manager::memory::manual::{has_manual_memories, render_manual_memory_lines};
use crate::chat_manager::messages::{
    push_prompt_entry_message, push_system_message, push_user_or_assistant_message_with_context,
    sanitize_placeholders_in_api_messages,
};
use crate::chat_manager::prompts;
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
    ChatCompletionArgs, ChatTurnResult, ImageAttachment, StoredMessage,
};
use crate::usage::tracking::UsageOperationType;
use crate::utils::{emit_debug, emit_error_event, emit_info, log_error, log_info, log_warn, now_millis};

pub struct CompletionFlow {
    app: AppHandle,
}

impl CompletionFlow {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    pub async fn execute(self, args: ChatCompletionArgs) -> Result<ChatTurnResult, String> {
        let app = self.app;
        let ChatCompletionArgs {
            session_id,
            character_id,
            user_message,
            persona_id,
            swap_places,
            stream,
            request_id,
            attachments,
        } = args;
        let swap_places = role_swap_enabled(swap_places);

        log_info(
            &app,
            "chat_completion",
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

        emit_debug(
            &app,
            "loading_character",
            json!({ "characterId": character_id.clone() }),
        );

        emit_debug(
            &app,
            "session_loaded",
            json!({
                "sessionId": session.id,
                "messageCount": session.messages.len(),
                "updatedAt": session.updated_at,
            }),
        );

        let dynamic_memory_enabled = is_dynamic_memory_active(settings, &character);
        let dynamic_window = dynamic_window_size(settings);
        if dynamic_memory_enabled {
            let _ = prompts::ensure_dynamic_memory_templates(&app);
        }

        log_info(
            &app,
            "chat_completion",
            format!(
                "selected provider={} model={} credential={}",
                credential.provider_id.as_str(),
                model.name.as_str(),
                credential.id.as_str()
            ),
        );

        emit_debug(
            &app,
            "model_selected",
            json!({
                "providerId": credential.provider_id,
                "model": model.name,
                "credentialId": credential.id,
            }),
        );

        let now = now_millis()?;
        let user_msg_id = Uuid::new_v4().to_string();

        let persisted_attachments = persist_attachments(
            &app,
            &character_id,
            &session_id,
            &user_msg_id,
            "user",
            attachments,
        )?;

        let user_msg = StoredMessage {
            id: user_msg_id,
            role: "user".into(),
            content: user_message.clone(),
            created_at: now,
            usage: None,
            variants: Vec::new(),
            selected_variant_id: None,
            memory_refs: Vec::new(),
            used_lorebook_entries: Vec::new(),
            is_pinned: false,
            attachments: persisted_attachments,
            reasoning: None,
            model_id: None,
            fallback_from_model_id: None,
        };
        session.messages.push(user_msg.clone());
        session.updated_at = now;
        context.save_session(&session)?;

        emit_debug(
            &app,
            "session_saved",
            json!({
                "stage": "after_user_message",
                "sessionId": session.id,
                "messageCount": session.messages.len(),
                "updatedAt": session.updated_at,
            }),
        );

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

        let relevant_memories = if dynamic_memory_enabled && !session.memory_embeddings.is_empty() {
            let fixed = ensure_pinned_hot(&mut session.memory_embeddings);
            if fixed > 0 {
                log_info(
                    &app,
                    "dynamic_memory",
                    format!("Restored {} pinned memories to hot", fixed),
                );
            }

            let search_query = if context_enrichment_enabled(settings) {
                build_enriched_query(&session.messages)
            } else {
                user_message.clone()
            };

            log_info(
                &app,
                "memory_retrieval",
                format!(
                    "Search query ({} chars, enriched={})",
                    search_query.len(),
                    context_enrichment_enabled(settings)
                ),
            );

            select_relevant_memories(
                &app,
                &session,
                &search_query,
                dynamic_retrieval_limit(settings),
                dynamic_min_similarity(settings),
                dynamic_retrieval_strategy(settings),
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

        let memory_block = if dynamic_memory_enabled {
            if relevant_memories.is_empty() {
                None
            } else {
                Some(
                    relevant_memories
                        .iter()
                        .map(|m| format!("- {}", m.text))
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            }
        } else if has_manual_memories(&session.memories) {
            Some(render_manual_memory_lines(&session.memories))
        } else {
            None
        };
        if let Some(block) = memory_block {
            push_system_message(
                &mut messages_for_api,
                &system_role,
                Some(format!("Relevant memories:\n{}", block)),
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
            "chat_completion",
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

            let attempt_api_key = match require_api_key(&app, attempt_credential, "chat_completion")
            {
                Ok(key) => key,
                Err(err) => {
                    log_error(
                        &app,
                        "chat_completion",
                        format!(
                            "failed to resolve API key for model={} provider={}: {}",
                            attempt_model.name, attempt_credential.provider_id, err
                        ),
                    );
                    last_error = err;
                    if has_next_attempt {
                        emit_fallback_retry_toast(&app, &mut fallback_toast_shown);
                        continue;
                    }
                    return Err(last_error);
                }
            };

            let request_settings = RequestSettings::resolve(&session, attempt_model, settings);
            let extra_body_fields = build_provider_extra_fields(
                &attempt_credential.provider_id,
                &session,
                attempt_model,
                settings,
                &request_settings,
            );

            log_info(
                &app,
                "chat_completion",
                format!(
                    "reasoning settings: enabled={} effort={:?} budget={:?} model_adv={:?}",
                    request_settings.reasoning_enabled,
                    request_settings.reasoning_effort,
                    request_settings.reasoning_budget,
                    attempt_model
                        .advanced_model_settings
                        .as_ref()
                        .map(|a| a.reasoning_enabled)
                ),
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

            log_info(
                &app,
                "chat_completion",
                format!(
                    "request prepared endpoint={} stream={} request_id={:?} model={} fallback_attempt={}",
                    built.url.as_str(),
                    should_stream,
                    &request_id,
                    attempt_model.name,
                    is_fallback_attempt
                ),
            );

            let request_started_at = now_millis().unwrap_or_default();

            emit_info(
                &app,
                "sending_request",
                json!({
                    "operation": "completion",
                    "sessionId": session.id,
                    "providerId": attempt_credential.provider_id,
                    "model": attempt_model.name,
                    "stream": should_stream,
                    "requestId": request_id,
                    "endpoint": built.url,
                    "requestStartedAt": request_started_at,
                    "requestBody": &built.body,
                    "reasoning": built.body.get("reasoning"),
                    "reasoningEffort": built.body.get("reasoning_effort"),
                    "maxCompletionTokens": built.body.get("max_completion_tokens"),
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
                    log_error(
                        &app,
                        "chat_completion",
                        format!(
                            "api_request failed model={} provider={} err={}",
                            attempt_model.name, attempt_credential.provider_id, err
                        ),
                    );
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
                "response",
                json!({
                    "operation": "completion",
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

                if !has_next_attempt {
                    record_failed_usage(
                        &app,
                        &failed_usage,
                        &session,
                        &character,
                        attempt_model,
                        attempt_credential,
                        UsageOperationType::Chat,
                        &err_message,
                        "chat_completion",
                    );
                }

                emit_error_event(
                    &app,
                    "provider_error",
                    json!({
                        "operation": "completion",
                        "sessionId": session.id,
                        "requestId": request_id,
                        "status": api_response.status,
                        "message": err_message,
                        "usage": failed_usage,
                        "model": attempt_model.name,
                    }),
                );

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

            log_error(
                &app,
                "chat_completion",
                format!("empty response from provider: has_reasoning={}", has_reasoning),
            );
            return Err(error_detail.to_string());
        }

        if let Some(filter) = app.try_state::<crate::content_filter::ContentFilter>() {
            if filter.is_enabled() {
                let result = filter.check_text(&text);
                if result.blocked {
                    log_warn(
                        &app,
                        "chat_completion",
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
            "assistant_reply",
            json!({
                "length": text.len(),
                "requestId": request_id,
                "operation": "completion",
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
            cleanup_attachments(&app, &persisted_assistant_attachments, "chat_completion");
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
            cleanup_attachments(&app, &assistant_message.attachments, "chat_completion");
            return Err("Request aborted by user".to_string());
        }
        context.save_session(&session)?;

        log_info(
            &app,
            "chat_completion",
            format!(
                "assistant response saved message_id={} length={} total_messages={}",
                assistant_message.id.as_str(),
                assistant_message.content.len(),
                session.messages.len()
            ),
        );

        emit_debug(
            &app,
            "session_saved",
            json!({
                "stage": "after_assistant_message",
                "sessionId": session.id,
                "requestId": request_id,
                "messageId": assistant_message.id,
                "messageCount": session.messages.len(),
                "updatedAt": session.updated_at,
            }),
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
            UsageOperationType::Chat,
            "chat_completion",
        )
        .await;

        if dynamic_memory_enabled {
            if let Err(err) =
                process_dynamic_memory_cycle(&app, &mut session, settings, &character).await
            {
                log_error(
                    &app,
                    "chat_completion",
                    format!("dynamic memory cycle failed: {}", err),
                );
            }
        }

        Ok(ChatTurnResult {
            session_id: session.id,
            session_updated_at: session.updated_at,
            request_id,
            user_message: user_msg,
            assistant_message,
            usage,
        })
    }
}
