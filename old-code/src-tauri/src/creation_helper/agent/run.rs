use serde_json::{json, Value};
use tauri::AppHandle;
use uuid::Uuid;

use crate::chat_manager::request as chat_request;
use crate::chat_manager::sse::accumulate_tool_calls_from_sse;
use crate::chat_manager::tooling::{
    parse_tool_calls, tool_call_message_payload, ToolCall, ToolChoice, ToolConfig,
};
use crate::chat_manager::{
    entries::relative_system_entry, request_builder::system_role_for,
    turn_builder::assemble_prompt_messages,
};
use crate::creation_helper::service::{
    creation_tool_result_message, emit_creation_helper_step, emit_creation_helper_turn_start,
    emit_creation_helper_update, emit_creation_segment_boundary, send_creation_api_request,
};
use crate::creation_helper::types::{CreationMessageRole, CreationSession, CreationToolResult};
use crate::utils::{log_info, log_warn};

use super::exec::dispatch_tool;
use super::llm::load_context;
use super::state::{build_draft_view, AgentTurn, PlanStep, StepStatus, TargetKind};
use super::structured_fallback::{
    fallback_system_prompt, parse as parse_fallback, CreationHelperFallbackFormat,
};
use super::tool_defs::tools_for_target;
use super::verbs::Tool;

const MAX_TURN_ITERATIONS: u32 = 8;

pub async fn run_agent_turn(
    app: &AppHandle,
    session: &mut CreationSession,
    _user_message: &str,
    stream_request_id: &str,
    fallback_format: CreationHelperFallbackFormat,
) -> Result<AgentTurn, String> {
    let mut turn = AgentTurn::new(MAX_TURN_ITERATIONS);
    let ctx = load_context(app)?;
    let session_id = session.id.clone();
    let target = TargetKind::from_goal(&session.creation_goal);

    log_info(
        app,
        "creation_helper.agent",
        format!(
            "starting turn: target={:?} fallback={} model={}",
            target,
            fallback_format.label(),
            ctx.model_name
        ),
    );

    emit_creation_helper_turn_start(app, &session_id, stream_request_id);

    let system_role = system_role_for(&ctx.cred);
    let mut api_messages = build_initial_messages(session, target, fallback_format, &system_role);

    let tool_config = if fallback_format.is_native() {
        Some(ToolConfig {
            tools: tools_for_target(target),
            choice: Some(ToolChoice::Auto),
        })
    } else {
        None
    };

    let mut iteration = 0u32;
    let mut step_index = 0usize;

    loop {
        iteration += 1;
        turn.iterations = iteration;
        if iteration > MAX_TURN_ITERATIONS {
            log_warn(
                app,
                "creation_helper.agent",
                format!("hit max iterations ({})", MAX_TURN_ITERATIONS),
            );
            break;
        }

        let resp = send_creation_api_request(
            app,
            &session_id,
            stream_request_id,
            &ctx.provider_id,
            &ctx.cred,
            &ctx.api_key,
            &ctx.model_name,
            &api_messages,
            ctx.streaming_enabled,
            tool_config.as_ref(),
        )
        .await?;

        let character_name = session.draft.name.as_deref().unwrap_or("");

        if !resp.ok {
            let err = resp
                .data()
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("LLM call failed")
                .to_string();
            crate::creation_helper::service::record_creation_usage(
                app,
                resp.data(),
                &session_id,
                &ctx.model_id,
                &ctx.model_name,
                &ctx.provider_id,
                &ctx.provider_label,
                &ctx.api_key,
                character_name,
                false,
                Some(err.clone()),
            )
            .await;
            return Err(err);
        }

        let response_data = resp.data();
        let assistant_text =
            chat_request::extract_text(response_data, Some(&ctx.provider_id)).unwrap_or_default();
        crate::creation_helper::service::record_creation_usage(
            app,
            response_data,
            &session_id,
            &ctx.model_id,
            &ctx.model_name,
            &ctx.provider_id,
            &ctx.provider_label,
            &ctx.api_key,
            character_name,
            true,
            None,
        )
        .await;

        let mut tool_calls: Vec<ToolCall> = if response_data.is_string() {
            accumulate_tool_calls_from_sse(response_data.as_str().unwrap(), &ctx.provider_id)
        } else {
            parse_tool_calls(&ctx.provider_id, response_data)
        };

        let mut visible_reply: String = assistant_text.clone();

        if tool_calls.is_empty() && !fallback_format.is_native() {
            match parse_fallback(fallback_format, &assistant_text) {
                Ok(parsed) => {
                    log_info(
                        app,
                        "creation_helper.agent",
                        format!(
                            "fallback parsed {} calls, reply={}",
                            parsed.calls.len(),
                            parsed.reply.is_some()
                        ),
                    );
                    tool_calls = parsed.calls;
                    visible_reply = parsed.reply.unwrap_or_default();
                }
                Err(err) => {
                    log_warn(
                        app,
                        "creation_helper.agent",
                        format!("fallback parse failed: {}", err),
                    );
                    push_user_segment(&mut turn, assistant_text);
                    break;
                }
            }
        }

        log_info(
            app,
            "creation_helper.agent",
            format!(
                "iter {}: tool_calls={} text_len={}",
                iteration,
                tool_calls.len(),
                visible_reply.len()
            ),
        );

        if tool_calls.is_empty() {
            push_user_segment(&mut turn, visible_reply);
            break;
        }

        if !visible_reply.trim().is_empty() {
            push_user_segment(&mut turn, visible_reply.clone());
        }

        let mut creation_calls: Vec<ToolCall> = Vec::with_capacity(tool_calls.len());
        let mut creation_results: Vec<CreationToolResult> = Vec::with_capacity(tool_calls.len());
        let mut terminal_hit = false;

        for tc in &tool_calls {
            let tool = match tc.name.parse::<Tool>() {
                Ok(v) => v,
                Err(_) => {
                    log_warn(
                        app,
                        "creation_helper.agent",
                        format!("ignoring unknown tool: {}", tc.name),
                    );
                    creation_calls.push(ToolCall {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        arguments: tc.arguments.clone(),
                        raw_arguments: None,
                        thought_signature: tc.thought_signature.clone(),
                    });
                    creation_results.push(CreationToolResult {
                        tool_call_id: tc.id.clone(),
                        result: json!({ "success": false, "error": format!("unknown tool: {}", tc.name) }),
                        success: false,
                    });
                    continue;
                }
            };

            let args_text = json_args_to_argstring(&tc.arguments);

            step_index += 1;
            emit_creation_helper_step(
                app,
                &session_id,
                stream_request_id,
                step_index,
                &tc.name,
                &tc.arguments,
                "running",
                None,
            );

            let outcome = dispatch_tool(app, session, tool, &args_text).await;
            let final_status = if outcome.success {
                StepStatus::Completed
            } else {
                StepStatus::Failed
            };
            turn.plan.push(PlanStep {
                index: step_index,
                tool_call_id: tc.id.clone(),
                verb: tool.as_str().to_string(),
                raw_args: tc.arguments.clone(),
                args_text: args_text.clone(),
                status: final_status,
                before_summary: None,
                after_summary: Some(outcome.summary.clone()),
                note: outcome.error.clone(),
                extra: outcome.extra.clone(),
            });

            let mut result_payload: serde_json::Map<String, Value> = match outcome.extra.clone() {
                Value::Object(m) => m,
                _ => serde_json::Map::new(),
            };
            result_payload.insert("success".to_string(), Value::Bool(outcome.success));
            if !result_payload.contains_key("message") {
                result_payload.insert(
                    "message".to_string(),
                    Value::String(outcome.summary.clone()),
                );
            }
            if let Some(err) = &outcome.error {
                result_payload
                    .entry("error".to_string())
                    .or_insert(Value::String(err.clone()));
            }
            match tool {
                Tool::ShowPreview => {
                    result_payload
                        .entry("action".to_string())
                        .or_insert(Value::String("show_preview".to_string()));
                }
                Tool::RequestConfirmation => {
                    result_payload
                        .entry("action".to_string())
                        .or_insert(Value::String("request_confirmation".to_string()));
                }
                _ => {}
            }

            let result_value = Value::Object(result_payload);

            emit_creation_helper_step(
                app,
                &session_id,
                stream_request_id,
                step_index,
                &tc.name,
                &tc.arguments,
                if outcome.success {
                    "completed"
                } else {
                    "failed"
                },
                Some(&result_value),
            );

            creation_calls.push(ToolCall {
                id: tc.id.clone(),
                name: tc.name.clone(),
                arguments: tc.arguments.clone(),
                raw_arguments: None,
                thought_signature: tc.thought_signature.clone(),
            });
            creation_results.push(CreationToolResult {
                tool_call_id: tc.id.clone(),
                result: result_value,
                success: outcome.success,
            });

            turn.push_tool(tc.id.clone());

            if let Some(prose) = outcome.user_facing.clone() {
                push_user_segment(&mut turn, prose);
            }
            if outcome.terminal {
                terminal_hit = true;
            }
        }

        emit_creation_helper_update(app, &session_id, session, None, None);
        if ctx.streaming_enabled {
            emit_creation_segment_boundary(app, stream_request_id);
        }

        let tool_calls_json: Vec<Value> = creation_calls
            .iter()
            .enumerate()
            .map(|(i, c)| tool_call_message_payload(&ctx.provider_id, c, i))
            .collect();
        api_messages.push(json!({
            "role": "assistant",
            "content": if visible_reply.is_empty() { Value::Null } else { json!(visible_reply) },
            "tool_calls": tool_calls_json,
        }));
        for r in &creation_results {
            let tool_name = creation_calls
                .iter()
                .find(|c| c.id == r.tool_call_id)
                .map(|c| c.name.as_str());
            api_messages.push(creation_tool_result_message(
                &ctx.provider_id,
                &r.tool_call_id,
                tool_name,
                &r.result,
            ));
        }

        if terminal_hit {
            log_info(app, "creation_helper.agent", "terminal verb reached");
            break;
        }
    }

    turn.finished = true;
    Ok(turn)
}

fn json_args_to_argstring(args: &Value) -> String {
    let Some(obj) = args.as_object() else {
        return String::new();
    };
    let mut parts = Vec::new();
    for (k, v) in obj {
        let value_str = match v {
            Value::String(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::Null => continue,
            Value::Array(arr) => arr
                .iter()
                .filter_map(|x| {
                    x.as_str()
                        .map(|s| s.to_string())
                        .or_else(|| Some(x.to_string()))
                })
                .collect::<Vec<_>>()
                .join(","),
            Value::Object(_) => v.to_string(),
        };
        if value_str.is_empty() {
            continue;
        }
        if value_str.contains(char::is_whitespace) || value_str.contains('"') {
            parts.push(format!("{}=\"{}\"", k, value_str.replace('"', "'")));
        } else {
            parts.push(format!("{}={}", k, value_str));
        }
    }
    parts.join(" ")
}

fn push_user_segment(turn: &mut AgentTurn, text: String) {
    turn.push_text(text);
}

fn build_initial_messages(
    session: &CreationSession,
    target: TargetKind,
    fallback: CreationHelperFallbackFormat,
    system_role: &str,
) -> Vec<Value> {
    let mut conversation_messages = Vec::new();
    let view = build_draft_view(session);

    let mut prompt_entries =
        crate::chat_manager::prompting::prompt_engine::default_creation_helper_system_entries();
    for entry in &mut prompt_entries {
        entry.content = entry
            .content
            .replace("{{target_label}}", target.label())
            .replace("{{draft_state}}", &view.rendered);
    }

    let fallback_addendum = fallback_system_prompt(fallback, target);
    if !fallback_addendum.is_empty() {
        prompt_entries.push(relative_system_entry(
            "creation_agent_fallback_protocol",
            "Creation Agent Fallback Protocol",
            fallback_addendum,
        ));
    }

    for msg in &session.messages {
        let role = match msg.role {
            CreationMessageRole::User => "user",
            CreationMessageRole::Assistant => "assistant",
            CreationMessageRole::System => "system",
        };
        if msg.content.is_empty() {
            continue;
        }
        conversation_messages.push(json!({ "role": role, "content": msg.content }));
    }

    assemble_prompt_messages(prompt_entries, conversation_messages, system_role)
}

#[allow(dead_code)]
fn _force_uuid_link() {
    let _ = Uuid::new_v4();
}
