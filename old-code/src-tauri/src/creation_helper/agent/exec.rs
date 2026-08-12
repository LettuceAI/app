use std::collections::HashMap;

use serde_json::{json, Value};
use tauri::AppHandle;
use uuid::Uuid;

use super::state::StepResult;
use super::verbs::Tool;
use crate::creation_helper::service::execute_tool;
use crate::creation_helper::types::{CreationSession, CreationToolResult};

pub fn parse_args(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let s = text.trim();
    if s.is_empty() {
        return out;
    }

    let bytes = s.as_bytes();
    let mut i = 0usize;
    let mut had_kv = false;

    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let key_start = i;
        while i < bytes.len() && bytes[i] != b'=' && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            let rest = s[key_start..].trim().to_string();
            if !rest.is_empty() {
                out.entry("note".to_string())
                    .and_modify(|v: &mut String| {
                        v.push(' ');
                        v.push_str(&rest);
                    })
                    .or_insert(rest);
            }
            break;
        }
        let key = s[key_start..i].to_string();
        i += 1; // skip '='

        let value = if i < bytes.len() && bytes[i] == b'"' {
            i += 1;
            let v_start = i;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            let v = s[v_start..i].to_string();
            if i < bytes.len() {
                i += 1; // skip closing quote
            }
            v
        } else {
            let v_start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            s[v_start..i].to_string()
        };
        out.insert(key, value);
        had_kv = true;
    }

    if !had_kv && !s.is_empty() && !out.contains_key("note") {
        out.insert("note".to_string(), s.to_string());
    }
    out
}

fn from_legacy(result: CreationToolResult, default_summary: &str) -> StepResult {
    let payload = result.result.clone();
    let success = result.success;
    let summary = payload
        .get("message")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| default_summary.to_string());
    let error = payload
        .get("error")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    StepResult {
        success,
        summary,
        user_facing: None,
        error,
        terminal: false,
        extra: payload,
    }
}

pub async fn dispatch_tool(
    app: &AppHandle,
    session: &mut CreationSession,
    tool: Tool,
    args_text: &str,
) -> StepResult {
    let args = parse_args(args_text);
    let call_id = format!("agent-{}", Uuid::new_v4());

    match tool {
        Tool::SetName => {
            let Some(name) = args.get("name").or_else(|| args.get("note")) else {
                return StepResult::failed("SET_NAME requires args: name=<text>");
            };
            let payload = json!({ "name": name });
            from_legacy(
                execute_tool(app, session, &call_id, "set_character_name", &payload).await,
                "name set",
            )
        }
        Tool::SetModel => {
            let Some(id) = args.get("id").or_else(|| args.get("model_id")) else {
                return StepResult::failed("SET_MODEL requires args: id=<model_id>");
            };
            let payload = json!({ "model_id": id });
            from_legacy(
                execute_tool(app, session, &call_id, "set_default_model", &payload).await,
                "model set",
            )
        }
        Tool::SetPrompt => {
            let Some(id) = args.get("id").or_else(|| args.get("prompt_id")) else {
                return StepResult::failed("SET_PROMPT requires args: id=<prompt_id>");
            };
            let payload = json!({ "prompt_id": id });
            from_legacy(
                execute_tool(app, session, &call_id, "set_system_prompt", &payload).await,
                "prompt set",
            )
        }
        Tool::SetAvatarGradient => {
            let enabled = args
                .get("enabled")
                .map(|s| s.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            let payload = json!({ "enabled": enabled });
            from_legacy(
                execute_tool(app, session, &call_id, "toggle_avatar_gradient", &payload).await,
                "gradient toggled",
            )
        }
        Tool::AttachLorebooks => {
            let Some(character_id) = session.target_id.clone() else {
                return StepResult::failed(
                    "ATTACH_LOREBOOKS only valid for an existing character (no target_id)",
                );
            };
            let ids: Vec<String> = args
                .get("ids")
                .map(|s| {
                    s.split(',')
                        .map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            let payload = json!({ "character_id": character_id, "lorebook_ids": ids });
            from_legacy(
                execute_tool(app, session, &call_id, "set_character_lorebooks", &payload).await,
                "lorebooks attached",
            )
        }

        Tool::EditScene => {
            let Some(scene_id) = args.get("id").or_else(|| args.get("scene_id")) else {
                return StepResult::failed("EDIT_SCENE requires args: id=<scene_id>");
            };
            let Some(content) = args.get("content") else {
                return StepResult::failed(
                    "EDIT_SCENE requires content (writer should produce it)",
                );
            };
            let mut payload = json!({ "scene_id": scene_id, "content": content });
            if let Some(direction) = args.get("direction") {
                payload["direction"] = json!(direction);
            }
            from_legacy(
                execute_tool(app, session, &call_id, "update_scene", &payload).await,
                "scene updated",
            )
        }
        Tool::EditLoreEntry => {
            let Some(entry_id) = args.get("id") else {
                return StepResult::failed("EDIT_LORE_ENTRY requires args: id=<entry_id>");
            };
            let Some(lorebook_id) = args.get("lorebook_id").or(session.target_id.as_ref()) else {
                return StepResult::failed("EDIT_LORE_ENTRY requires lorebook_id (no target_id)");
            };
            let Some(title) = args.get("title") else {
                return StepResult::failed("EDIT_LORE_ENTRY requires title=<text>");
            };
            let Some(content) = args.get("content") else {
                return StepResult::failed("EDIT_LORE_ENTRY requires content=<text>");
            };
            let payload = json!({
                "id": entry_id,
                "lorebook_id": lorebook_id,
                "title": title,
                "content": content,
            });
            from_legacy(
                execute_tool(app, session, &call_id, "upsert_lorebook_entry", &payload).await,
                "entry updated",
            )
        }
        Tool::DeleteScene => {
            let Some(scene_id) = args.get("id") else {
                return StepResult::failed("DELETE_SCENE requires args: id=<scene_id>");
            };
            let before = session.draft.scenes.len();
            session.draft.scenes.retain(|s| &s.id != scene_id);
            if session.draft.default_scene_id.as_deref() == Some(scene_id.as_str()) {
                session.draft.default_scene_id = None;
            }
            if session.draft.scenes.len() < before {
                StepResult::ok(format!("scene {} deleted", scene_id))
            } else {
                StepResult::failed(format!("scene {} not found", scene_id))
            }
        }
        Tool::DeleteLoreEntry => {
            let Some(id) = args.get("id") else {
                return StepResult::failed("DELETE_LORE_ENTRY requires args: id=<entry_id>");
            };
            let payload = json!({ "entry_id": id });
            from_legacy(
                execute_tool(app, session, &call_id, "delete_lorebook_entry", &payload).await,
                "entry deleted",
            )
        }
        Tool::DeletePersona => {
            let Some(id) = args.get("id") else {
                return StepResult::failed("DELETE_PERSONA requires args: id=<persona_id>");
            };
            let payload = json!({ "id": id });
            from_legacy(
                execute_tool(app, session, &call_id, "delete_persona", &payload).await,
                "persona deleted",
            )
        }
        Tool::DeleteLorebook => {
            let Some(id) = args.get("id") else {
                return StepResult::failed("DELETE_LOREBOOK requires args: id=<lorebook_id>");
            };
            let payload = json!({ "lorebook_id": id });
            from_legacy(
                execute_tool(app, session, &call_id, "delete_lorebook", &payload).await,
                "lorebook deleted",
            )
        }
        Tool::ReorderLoreEntries => {
            let Some(order) = args.get("order") else {
                return StepResult::failed("REORDER_LORE_ENTRIES requires args: order=<id,id,...>");
            };
            let updates: Vec<Value> = order
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .enumerate()
                .map(|(i, id)| json!({ "entry_id": id, "display_order": i as i64 }))
                .collect();
            let payload = json!({ "updates": updates });
            from_legacy(
                execute_tool(app, session, &call_id, "reorder_lorebook_entries", &payload).await,
                "entries reordered",
            )
        }

        Tool::GenerateImage => {
            let Some(request) = args.get("prompt").or_else(|| args.get("note")) else {
                return StepResult::failed("generate_image requires args: prompt=<text>");
            };
            let role = args
                .get("role")
                .map(|s| s.as_str())
                .unwrap_or("avatar")
                .to_string();
            let subject_name = session.draft.name.clone().unwrap_or_default();
            let subject_description = session
                .draft
                .definition
                .clone()
                .or(session.draft.description.clone())
                .unwrap_or_default();
            let polished = super::avatar::build_generation_prompt(
                app,
                &subject_name,
                &subject_description,
                request,
            );
            let payload = json!({ "prompt": polished });
            let mut result = from_legacy(
                execute_tool(app, session, &call_id, "generate_image", &payload).await,
                "image generated",
            );
            if result.success {
                if let Some(image_id) =
                    crate::creation_helper::service::last_generated_image_for_session(&session.id)
                {
                    let mut extra = match result.extra.clone() {
                        Value::Object(m) => m,
                        _ => serde_json::Map::new(),
                    };
                    extra.insert(
                        "rendered_prompt".to_string(),
                        Value::String(polished.clone()),
                    );
                    extra.insert("image_id".to_string(), Value::String(image_id.clone()));
                    extra.insert("role".to_string(), Value::String(role.clone()));
                    result.extra = Value::Object(extra);
                    let use_payload = json!({ "image_id": image_id });
                    let tool_name = match role.as_str() {
                        "background" => "use_uploaded_image_as_chat_background",
                        "persona_avatar" => "use_uploaded_image_as_persona_avatar",
                        _ => "use_uploaded_image_as_avatar",
                    };
                    let apply = execute_tool(app, session, &call_id, tool_name, &use_payload).await;
                    if !apply.success {
                        result.success = false;
                        result.error = apply
                            .result
                            .get("error")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                    }
                }
            }
            result
        }
        Tool::EditAvatarImage => {
            let Some(image_id) = args.get("image_id").or_else(|| args.get("id")) else {
                return StepResult::failed(
                    "edit_avatar_image requires args: image_id=<id> edit_prompt=<text>",
                );
            };
            let Some(edit_request) = args.get("edit_prompt").or_else(|| args.get("prompt")) else {
                return StepResult::failed("edit_avatar_image requires args: edit_prompt=<text>");
            };
            let role = args
                .get("role")
                .map(|s| s.as_str())
                .unwrap_or("avatar")
                .to_string();
            let subject_name = session.draft.name.clone().unwrap_or_default();
            let subject_description = session
                .draft
                .definition
                .clone()
                .or(session.draft.description.clone())
                .unwrap_or_default();
            let polished = super::avatar::build_edit_prompt(
                app,
                &subject_name,
                &subject_description,
                "",
                edit_request,
            );
            let session_id = session.id.clone();
            match crate::creation_helper::service::run_avatar_edit(
                app,
                &session_id,
                &subject_name,
                image_id,
                polished.clone(),
            )
            .await
            {
                Ok((new_image_id, used_source_image)) => {
                    let use_payload = json!({ "image_id": new_image_id });
                    let tool_name = match role.as_str() {
                        "background" => "use_uploaded_image_as_chat_background",
                        "persona_avatar" => "use_uploaded_image_as_persona_avatar",
                        _ => "use_uploaded_image_as_avatar",
                    };
                    let apply = execute_tool(app, session, &call_id, tool_name, &use_payload).await;
                    let mut extra = serde_json::Map::new();
                    extra.insert("image_id".to_string(), Value::String(new_image_id.clone()));
                    extra.insert("rendered_prompt".to_string(), Value::String(polished));
                    extra.insert(
                        "source_image_id".to_string(),
                        Value::String(image_id.clone()),
                    );
                    extra.insert("role".to_string(), Value::String(role));
                    extra.insert(
                        "used_source_image".to_string(),
                        Value::Bool(used_source_image),
                    );
                    if !apply.success {
                        return StepResult {
                            success: false,
                            summary: "image edited (apply failed)".to_string(),
                            user_facing: None,
                            error: apply
                                .result
                                .get("error")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            terminal: false,
                            extra: Value::Object(extra),
                        };
                    }
                    StepResult {
                        success: true,
                        summary: "image edited".to_string(),
                        user_facing: None,
                        error: None,
                        terminal: false,
                        extra: Value::Object(extra),
                    }
                }
                Err(err) => StepResult::failed(err),
            }
        }
        Tool::UseUploadedImage => {
            let Some(image_id) = args.get("id").or_else(|| args.get("image_id")) else {
                return StepResult::failed("USE_IMAGE requires args: id=<image_id>");
            };
            let role = args
                .get("role")
                .map(|s| s.as_str())
                .unwrap_or("avatar")
                .to_string();
            let tool_name = match role.as_str() {
                "background" => "use_uploaded_image_as_chat_background",
                "persona_avatar" => "use_uploaded_image_as_persona_avatar",
                _ => "use_uploaded_image_as_avatar",
            };
            let payload = json!({ "image_id": image_id });
            from_legacy(
                execute_tool(app, session, &call_id, tool_name, &payload).await,
                "image applied",
            )
        }

        Tool::ListModels => {
            let result = execute_tool(app, session, &call_id, "get_model_list", &json!({})).await;
            read_to_step(result, "fetched model list")
        }
        Tool::ListPrompts => {
            let result =
                execute_tool(app, session, &call_id, "get_system_prompt_list", &json!({})).await;
            read_to_step(result, "fetched prompt list")
        }
        Tool::ListPersonas => {
            let result = execute_tool(app, session, &call_id, "list_personas", &json!({})).await;
            read_to_step(result, "fetched personas")
        }
        Tool::ListLorebooks => {
            let result = execute_tool(app, session, &call_id, "list_lorebooks", &json!({})).await;
            read_to_step(result, "fetched lorebooks")
        }
        Tool::ListLoreEntries => {
            let Some(id) = args.get("id").or_else(|| args.get("lorebook_id")) else {
                return StepResult::failed("READ_LORE_ENTRIES requires args: id=<lorebook_id>");
            };
            let payload = json!({ "lorebook_id": id });
            let result =
                execute_tool(app, session, &call_id, "list_lorebook_entries", &payload).await;
            read_to_step(result, "fetched entries")
        }

        Tool::ShowPreview => {
            let result = execute_tool(
                app,
                session,
                &call_id,
                "show_preview",
                &json!({ "message": args.get("note").cloned().unwrap_or_default() }),
            )
            .await;
            from_legacy(result, "preview shown")
        }
        Tool::RequestConfirmation => {
            let result = execute_tool(
                app,
                session,
                &call_id,
                "request_confirmation",
                &json!({ "message": args.get("note").cloned().unwrap_or_default() }),
            )
            .await;
            from_legacy(result, "confirmation requested")
        }

        Tool::WriteDefinition => {
            let Some(text) = args.get("definition").or_else(|| args.get("text")) else {
                return StepResult::failed("write_definition requires `definition`");
            };
            let payload = json!({ "definition": text });
            from_legacy(
                execute_tool(app, session, &call_id, "set_character_definition", &payload).await,
                "definition written",
            )
        }
        Tool::WriteScene => {
            let Some(content) = args.get("content").or_else(|| args.get("text")) else {
                return StepResult::failed("write_scene requires `content`");
            };
            let mut payload = json!({ "content": content });
            if let Some(direction) = args.get("direction") {
                payload["direction"] = json!(direction);
            }
            from_legacy(
                execute_tool(app, session, &call_id, "add_scene", &payload).await,
                "scene added",
            )
        }
        Tool::WriteLoreEntry => {
            let Some(content) = args.get("content").or_else(|| args.get("text")) else {
                return StepResult::failed("write_lore_entry requires `content`");
            };
            let title = args
                .get("title")
                .cloned()
                .unwrap_or_else(|| "New entry".to_string());
            let Some(lorebook_id) = args.get("lorebook_id").or(session.target_id.as_ref()) else {
                return StepResult::failed("write_lore_entry requires lorebook_id (no target_id)");
            };
            let payload = json!({
                "lorebook_id": lorebook_id,
                "title": title,
                "content": content,
            });
            from_legacy(
                execute_tool(app, session, &call_id, "upsert_lorebook_entry", &payload).await,
                "entry written",
            )
        }
    }
}

fn read_to_step(result: CreationToolResult, summary: &str) -> StepResult {
    if result.success {
        StepResult::ok(summary)
    } else {
        let err = result
            .result
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("read failed")
            .to_string();
        StepResult::failed(err)
    }
}
