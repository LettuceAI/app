use crate::chat_manager::prompt_engine;
use crate::chat_manager::types::{
    PromptEntryImageSlot, PromptEntryPayload, PromptEntryPosition, PromptEntryRole, PromptScope,
    SystemPromptEntry, SystemPromptTemplate,
};
use crate::{
    chat_manager::storage::{get_base_prompt, get_base_prompt_entries, PromptType},
    storage_manager::db::open_db,
};
use rusqlite::{params, OptionalExtension};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

pub const APP_DEFAULT_TEMPLATE_ID: &str = "prompt_app_default";
pub const APP_DYNAMIC_SUMMARY_TEMPLATE_ID: &str = "prompt_app_dynamic_summary";
pub const APP_DYNAMIC_MEMORY_TEMPLATE_ID: &str = "prompt_app_dynamic_memory";
pub const APP_HELP_ME_REPLY_TEMPLATE_ID: &str = "prompt_app_help_me_reply";
pub const APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID: &str =
    "prompt_app_help_me_reply_conversational";
pub const APP_GROUP_CHAT_TEMPLATE_ID: &str = "prompt_app_group_chat";
pub const APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID: &str = "prompt_app_group_chat_roleplay";
pub const APP_AVATAR_GENERATION_TEMPLATE_ID: &str = "prompt_app_avatar_generation";
pub const APP_AVATAR_EDIT_TEMPLATE_ID: &str = "prompt_app_avatar_edit";
pub const APP_SCENE_GENERATION_TEMPLATE_ID: &str = "prompt_app_scene_generation";
pub const APP_DESIGN_REFERENCE_TEMPLATE_ID: &str = "prompt_app_design_reference";
const APP_DEFAULT_TEMPLATE_NAME: &str = "App Default";
const APP_DYNAMIC_SUMMARY_TEMPLATE_NAME: &str = "Dynamic Memory: Summarizer";
const APP_DYNAMIC_MEMORY_TEMPLATE_NAME: &str = "Dynamic Memory: Memory Manager";
const APP_HELP_ME_REPLY_TEMPLATE_NAME: &str = "Reply Helper";
const APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_NAME: &str = "Reply Helper (Conversational)";
const APP_AVATAR_GENERATION_TEMPLATE_NAME: &str = "Avatar Generation";
const APP_AVATAR_EDIT_TEMPLATE_NAME: &str = "Avatar Image Edit";
const APP_SCENE_GENERATION_TEMPLATE_NAME: &str = "Scene Generation";
const APP_DESIGN_REFERENCE_TEMPLATE_NAME: &str = "Design Reference Writer";

fn supports_entry_prompts(_id: &str) -> bool {
    true
}

fn single_entry_from_content(content: &str) -> Vec<SystemPromptEntry> {
    vec![SystemPromptEntry {
        id: "entry_system".to_string(),
        name: "System Prompt".to_string(),
        role: PromptEntryRole::System,
        content: content.to_string(),
        enabled: true,
        injection_position: PromptEntryPosition::Relative,
        injection_depth: 0,
        conditional_min_messages: None,
        interval_turns: None,
        system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
    }]
}

fn template_entries_to_content(entries: &[SystemPromptEntry]) -> String {
    let merged = entries
        .iter()
        .filter(|entry| entry.enabled && !entry.content.trim().is_empty())
        .map(|entry| entry.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    if merged.trim().is_empty() {
        String::new()
    } else {
        merged
    }
}

fn prompt_entry_payload_variable(payload: &PromptEntryPayload) -> &'static str {
    match payload {
        PromptEntryPayload::ImageSlot {
            slot: PromptEntryImageSlot::Character,
        } => "{{image[character]}}",
        PromptEntryPayload::ImageSlot {
            slot: PromptEntryImageSlot::Persona,
        } => "{{image[persona]}}",
        PromptEntryPayload::ImageSlot {
            slot: PromptEntryImageSlot::Avatar,
        } => "{{image[avatar]}}",
        PromptEntryPayload::ImageSlot {
            slot: PromptEntryImageSlot::References,
        } => "{{image[references]}}",
    }
}

fn template_entries_to_validation_content(entries: &[SystemPromptEntry]) -> String {
    entries
        .iter()
        .filter(|entry| entry.enabled || entry.system_prompt)
        .flat_map(|entry| {
            let mut parts = Vec::new();
            if !entry.content.trim().is_empty() {
                parts.push(entry.content.clone());
            }
            if let Some(payload) = entry.prompt_entry_payload.as_ref() {
                parts.push(prompt_entry_payload_variable(payload).to_string());
            }
            parts
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn maybe_backfill_entries(
    app: &AppHandle,
    id: &str,
    prompt_type: PromptType,
    entries: Vec<SystemPromptEntry>,
) -> Result<(), String> {
    let template = match get_template(app, id)? {
        Some(template) => template,
        None => return Ok(()),
    };
    if !template.entries.is_empty() {
        return Ok(());
    }
    let base = get_base_prompt(prompt_type);
    if template.content.trim() != base.trim() {
        return Ok(());
    }
    let _ = update_template(
        app,
        id.to_string(),
        None,
        None,
        None,
        Some(template.content),
        Some(entries),
        None,
    )?;
    Ok(())
}

fn append_missing_entry(
    app: &AppHandle,
    id: &str,
    entry_id: &str,
    entry: SystemPromptEntry,
) -> Result<(), String> {
    let template = match get_template(app, id)? {
        Some(template) => template,
        None => return Ok(()),
    };

    if template
        .entries
        .iter()
        .any(|existing| existing.id == entry_id)
    {
        return Ok(());
    }

    let mut next_entries = template.entries;
    next_entries.push(entry);
    let next_content = template_entries_to_content(&next_entries);

    let _ = update_template(
        app,
        id.to_string(),
        None,
        None,
        None,
        Some(next_content),
        Some(next_entries),
        None,
    )?;

    Ok(())
}

fn backfill_missing_entry_conditions(
    app: &AppHandle,
    id: &str,
    defaults: &[SystemPromptEntry],
) -> Result<(), String> {
    let template = match get_template(app, id)? {
        Some(template) => template,
        None => return Ok(()),
    };
    if template.entries.is_empty() {
        return Ok(());
    }

    let mut changed = false;
    let next_entries = template
        .entries
        .into_iter()
        .map(|mut entry| {
            if entry.conditions.is_none() {
                if let Some(default_entry) = defaults
                    .iter()
                    .find(|candidate| candidate.id == entry.id && candidate.conditions.is_some())
                {
                    entry.conditions = default_entry.conditions.clone();
                    changed = true;
                }
            }
            entry
        })
        .collect::<Vec<_>>();

    if !changed {
        return Ok(());
    }

    let next_content = template_entries_to_content(&next_entries);
    let _ = update_template(
        app,
        id.to_string(),
        None,
        None,
        None,
        Some(next_content),
        Some(next_entries),
        None,
    )?;

    Ok(())
}

fn backfill_missing_entry_payloads(
    app: &AppHandle,
    id: &str,
    defaults: &[SystemPromptEntry],
) -> Result<(), String> {
    let template = match get_template(app, id)? {
        Some(template) => template,
        None => return Ok(()),
    };
    if template.entries.is_empty() {
        return Ok(());
    }

    let mut changed = false;
    let next_entries = template
        .entries
        .into_iter()
        .map(|mut entry| {
            if entry.prompt_entry_payload.is_none() {
                if let Some(default_entry) = defaults.iter().find(|candidate| {
                    candidate.id == entry.id && candidate.prompt_entry_payload.is_some()
                }) {
                    entry.prompt_entry_payload = default_entry.prompt_entry_payload.clone();
                    changed = true;
                }
            }
            entry
        })
        .collect::<Vec<_>>();

    if !changed {
        return Ok(());
    }

    let next_content = template_entries_to_content(&next_entries);
    let _ = update_template(
        app,
        id.to_string(),
        None,
        None,
        None,
        Some(next_content),
        Some(next_entries),
        None,
    )?;

    Ok(())
}

fn backfill_legacy_image_entry_content(
    app: &AppHandle,
    id: &str,
    defaults: &[SystemPromptEntry],
) -> Result<(), String> {
    let template = match get_template(app, id)? {
        Some(template) => template,
        None => return Ok(()),
    };
    if template.entries.is_empty() {
        return Ok(());
    }

    let mut changed = false;
    let next_entries = template
        .entries
        .into_iter()
        .map(|mut entry| {
            let Some(default_entry) = defaults.iter().find(|candidate| {
                candidate.id == entry.id && candidate.prompt_entry_payload.is_some()
            }) else {
                return entry;
            };

            let Some(payload) = default_entry.prompt_entry_payload.as_ref() else {
                return entry;
            };

            if default_entry.content.trim().is_empty()
                && entry.content.trim() == prompt_entry_payload_variable(payload)
            {
                entry.content.clear();
                changed = true;
            }

            entry
        })
        .collect::<Vec<_>>();

    if !changed {
        return Ok(());
    }

    let next_content = template_entries_to_content(&next_entries);
    let _ = update_template(
        app,
        id.to_string(),
        None,
        None,
        None,
        Some(next_content),
        Some(next_entries),
        None,
    )?;

    Ok(())
}

fn migrate_legacy_scene_generation_entry_roles(app: &AppHandle) -> Result<(), String> {
    let Some(template) = get_template(app, APP_SCENE_GENERATION_TEMPLATE_ID)? else {
        return Ok(());
    };
    if template.entries.is_empty() {
        return Ok(());
    }

    let mut changed = false;
    let mut next_entries = template.entries.clone();
    for entry in next_entries.iter_mut() {
        let is_scene_user_payload = matches!(
            entry.id.as_str(),
            "scene_gen_context"
                | "scene_gen_character_image"
                | "scene_gen_persona_image"
                | "scene_gen_request"
        );
        let looks_like_legacy_default = matches!(entry.role, PromptEntryRole::System)
            && matches!(entry.injection_position, PromptEntryPosition::Relative);

        if is_scene_user_payload && looks_like_legacy_default {
            entry.role = PromptEntryRole::User;
            entry.injection_position = PromptEntryPosition::InChat;
            entry.injection_depth = 0;
            entry.conditional_min_messages = None;
            entry.interval_turns = None;
            changed = true;
        }
    }

    if !changed {
        return Ok(());
    }

    let content = template_entries_to_content(&next_entries);
    let _ = update_template(
        app,
        APP_SCENE_GENERATION_TEMPLATE_ID.to_string(),
        None,
        None,
        None,
        Some(content),
        Some(next_entries),
        Some(template.condense_prompt_entries),
    )?;

    Ok(())
}

/// Get required variables for a specific template ID
pub fn get_required_variables(template_id: &str) -> Vec<String> {
    match template_id {
        APP_DEFAULT_TEMPLATE_ID => vec![
            "{{scene}}".to_string(),
            "{{scene_direction}}".to_string(),
            "{{char.name}}".to_string(),
            "{{char.desc}}".to_string(),
            "{{context_summary}}".to_string(),
            "{{key_memories}}".to_string(),
        ],
        APP_DYNAMIC_SUMMARY_TEMPLATE_ID => vec!["{{prev_summary}}".to_string()],
        APP_DYNAMIC_MEMORY_TEMPLATE_ID => vec!["{{max_entries}}".to_string()],
        APP_HELP_ME_REPLY_TEMPLATE_ID => vec![
            "{{char.name}}".to_string(),
            "{{char.desc}}".to_string(),
            "{{persona.name}}".to_string(),
            "{{persona.desc}}".to_string(),
            "{{current_draft}}".to_string(),
        ],
        APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID => vec![
            "{{char.name}}".to_string(),
            "{{char.desc}}".to_string(),
            "{{persona.name}}".to_string(),
            "{{persona.desc}}".to_string(),
            "{{current_draft}}".to_string(),
        ],
        APP_GROUP_CHAT_TEMPLATE_ID => vec![
            "{{char.name}}".to_string(),
            "{{char.desc}}".to_string(),
            "{{persona.name}}".to_string(),
            "{{persona.desc}}".to_string(),
            "{{group_characters}}".to_string(),
        ],
        APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID => vec![
            "{{scene}}".to_string(),
            "{{scene_direction}}".to_string(),
            "{{char.name}}".to_string(),
            "{{char.desc}}".to_string(),
            "{{persona.name}}".to_string(),
            "{{persona.desc}}".to_string(),
            "{{group_characters}}".to_string(),
            "{{context_summary}}".to_string(),
            "{{key_memories}}".to_string(),
        ],
        APP_AVATAR_GENERATION_TEMPLATE_ID => vec!["{{avatar_request}}".to_string()],
        APP_AVATAR_EDIT_TEMPLATE_ID => vec![
            "{{current_avatar_prompt}}".to_string(),
            "{{edit_request}}".to_string(),
        ],
        APP_SCENE_GENERATION_TEMPLATE_ID => vec![
            "{{recent_messages}}".to_string(),
            "{{scene_request}}".to_string(),
        ],
        APP_DESIGN_REFERENCE_TEMPLATE_ID => vec![
            "{{subject_name}}".to_string(),
            "{{image[avatar]}}".to_string(),
        ],
        _ => vec![],
    }
}

/// Validate that all required variables exist in the content
pub fn validate_required_variables(template_id: &str, content: &str) -> Result<(), Vec<String>> {
    let required = get_required_variables(template_id);
    if required.is_empty() {
        return Ok(());
    }

    let missing: Vec<String> = required
        .into_iter()
        .filter(|var| !content.contains(var))
        .collect();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

fn generate_id() -> String {
    format!("prompt_{}", uuid::Uuid::new_v4().to_string())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn scope_to_str(scope: &PromptScope) -> &'static str {
    match scope {
        PromptScope::AppWide => "AppWide",
        PromptScope::ModelSpecific => "ModelSpecific",
        PromptScope::CharacterSpecific => "CharacterSpecific",
    }
}

fn str_to_scope(s: &str) -> Result<PromptScope, String> {
    match s {
        "AppWide" => Ok(PromptScope::AppWide),
        "ModelSpecific" => Ok(PromptScope::ModelSpecific),
        "CharacterSpecific" => Ok(PromptScope::CharacterSpecific),
        other => Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Unknown prompt scope: {}", other),
        )),
    }
}

fn row_to_template(row: &rusqlite::Row<'_>) -> Result<SystemPromptTemplate, rusqlite::Error> {
    let id: String = row.get(0)?;
    let name: String = row.get(1)?;
    let scope_str: String = row.get(2)?;
    let target_ids_json: String = row.get(3)?;
    let content: String = row.get(4)?;
    let entries_json: String = row.get(5)?;
    let condense_prompt_entries: bool = row.get(6)?;
    let created_at: u64 = row.get(7)?;
    let updated_at: u64 = row.get(8)?;

    let scope = str_to_scope(&scope_str).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let target_ids: Vec<String> = serde_json::from_str(&target_ids_json).unwrap_or_default();
    let entries: Vec<SystemPromptEntry> = serde_json::from_str(&entries_json).unwrap_or_default();

    Ok(SystemPromptTemplate {
        id,
        name,
        scope,
        target_ids,
        content,
        entries,
        condense_prompt_entries,
        created_at,
        updated_at,
    })
}

pub fn load_templates(app: &AppHandle) -> Result<Vec<SystemPromptTemplate>, String> {
    let conn = open_db(app)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, scope, target_ids, content, entries, condense_prompt_entries, created_at, updated_at FROM prompt_templates ORDER BY created_at ASC",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row_to_template(row))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?);
    }
    if out.is_empty() {
        // Guarantee existence of App Default template even if setup call was skipped
        let _ = ensure_app_default_template(app)?;
        // Reload
        let mut stmt2 = conn
            .prepare(
                "SELECT id, name, scope, target_ids, content, entries, condense_prompt_entries, created_at, updated_at FROM prompt_templates ORDER BY created_at ASC",
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let rows2 = stmt2
            .query_map([], |row| row_to_template(row))
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        out.clear();
        for r in rows2 {
            out.push(r.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?);
        }
    }
    Ok(out)
}

pub fn create_template(
    app: &AppHandle,
    name: String,
    scope: PromptScope,
    target_ids: Vec<String>,
    content: String,
    entries: Option<Vec<SystemPromptEntry>>,
    condense_prompt_entries: Option<bool>,
) -> Result<SystemPromptTemplate, String> {
    let conn = open_db(app)?;
    let id = generate_id();
    let now = now();
    let target_ids_json = serde_json::to_string(&target_ids)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let entries = entries.unwrap_or_else(|| {
        if supports_entry_prompts(&id) && !content.is_empty() {
            single_entry_from_content(&content)
        } else {
            Vec::new()
        }
    });
    let entries_json = serde_json::to_string(&entries)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let condense_prompt_entries = condense_prompt_entries.unwrap_or(false);
    conn.execute(
        "INSERT INTO prompt_templates (id, name, scope, target_ids, content, entries, condense_prompt_entries, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            id,
            name,
            scope_to_str(&scope),
            target_ids_json,
            content,
            entries_json,
            condense_prompt_entries,
            now
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    get_template(app, &id).map(|opt| opt.expect("inserted row should exist"))
}

pub fn update_template(
    app: &AppHandle,
    id: String,
    name: Option<String>,
    scope: Option<PromptScope>,
    target_ids: Option<Vec<String>>,
    content: Option<String>,
    entries: Option<Vec<SystemPromptEntry>>,
    condense_prompt_entries: Option<bool>,
) -> Result<SystemPromptTemplate, String> {
    // Prevent changing scope of app default
    if is_app_default_template(&id) {
        if let Some(s) = &scope {
            // Need the current template to compare, but keeping restriction consistent
            if *s != PromptScope::AppWide {
                return Err(crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    "Cannot change scope of App Default template",
                ));
            }
        }
    }

    let conn = open_db(app)?;
    let current = get_template(app, &id)?.ok_or_else(|| format!("Template not found: {}", id))?;
    let new_name = name.unwrap_or(current.name);
    let new_scope = scope.unwrap_or(current.scope);
    let new_target_ids = target_ids.unwrap_or(current.target_ids);
    let new_content = content.unwrap_or(current.content);
    let new_entries = entries.unwrap_or(current.entries);
    let new_condense_prompt_entries =
        condense_prompt_entries.unwrap_or(current.condense_prompt_entries);

    // Validate required variables for protected templates
    if is_app_default_template(&id) {
        let validation_text = if new_entries.is_empty() {
            new_content.clone()
        } else {
            template_entries_to_validation_content(&new_entries)
        };
        if let Err(missing) = validate_required_variables(&id, &validation_text) {
            return Err(format!(
                "Protected template must contain required variables: {}",
                missing.join(", ")
            ));
        }
    }
    let updated_at = now();
    let target_ids_json = serde_json::to_string(&new_target_ids)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let entries_json = serde_json::to_string(&new_entries)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    conn.execute(
        "UPDATE prompt_templates SET name = ?1, scope = ?2, target_ids = ?3, content = ?4, entries = ?5, condense_prompt_entries = ?6, updated_at = ?7 WHERE id = ?8",
        params![
            new_name,
            scope_to_str(&new_scope),
            target_ids_json,
            new_content,
            entries_json,
            new_condense_prompt_entries,
            updated_at,
            id
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    get_template(app, &id).map(|opt| opt.expect("updated row should exist"))
}

pub fn delete_template(app: &AppHandle, id: String) -> Result<(), String> {
    if is_app_default_template(&id) {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "This template is protected and cannot be deleted",
        ));
    }

    if get_template(app, &id)?.is_none() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Template not found",
        ));
    }

    let conn = open_db(app)?;
    conn.execute("DELETE FROM prompt_templates WHERE id = ?1", params![id])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

pub fn get_template(app: &AppHandle, id: &str) -> Result<Option<SystemPromptTemplate>, String> {
    let conn = open_db(app)?;
    conn
        .query_row(
            "SELECT id, name, scope, target_ids, content, entries, condense_prompt_entries, created_at, updated_at FROM prompt_templates WHERE id = ?1",
            params![id],
            |row| row_to_template(row),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

pub fn ensure_app_default_template(app: &AppHandle) -> Result<String, String> {
    // Check existence
    if let Some(existing) = get_template(app, APP_DEFAULT_TEMPLATE_ID)? {
        let _ = maybe_backfill_entries(
            app,
            APP_DEFAULT_TEMPLATE_ID,
            PromptType::SystemPrompt,
            prompt_engine::default_modular_prompt_entries(),
        );
        let _ = append_missing_entry(
            app,
            APP_DEFAULT_TEMPLATE_ID,
            "entry_scene_image_protocol",
            prompt_engine::default_modular_prompt_entries()
                .into_iter()
                .find(|entry| entry.id == "entry_scene_image_protocol")
                .expect("scene image protocol entry should exist"),
        );
        return Ok(existing.id);
    }
    // Insert default
    let conn = open_db(app)?;
    let now = now();
    let content = get_base_prompt(PromptType::SystemPrompt);
    let entries_json = serde_json::to_string(&prompt_engine::default_modular_prompt_entries())
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute(
        "INSERT OR IGNORE INTO prompt_templates (id, name, scope, target_ids, content, entries, created_at, updated_at) VALUES (?1, ?2, ?3, '[]', ?4, ?5, ?6, ?6)",
        params![
            APP_DEFAULT_TEMPLATE_ID,
            APP_DEFAULT_TEMPLATE_NAME,
            scope_to_str(&PromptScope::AppWide),
            content,
            entries_json,
            now
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(APP_DEFAULT_TEMPLATE_ID.to_string())
}

pub fn ensure_dynamic_memory_templates(app: &AppHandle) -> Result<(), String> {
    let conn = open_db(app)?;
    let now = now();

    // Summarizer template
    if get_template(app, APP_DYNAMIC_SUMMARY_TEMPLATE_ID)?.is_none() {
        let content = get_base_prompt(PromptType::DynamicSummaryPrompt);
        let entries = get_base_prompt_entries(PromptType::DynamicSummaryPrompt);
        let entries_json = serde_json::to_string(&entries)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        conn.execute(
            "INSERT OR IGNORE INTO prompt_templates (id, name, scope, target_ids, content, entries, created_at, updated_at) VALUES (?1, ?2, ?3, '[]', ?4, ?5, ?6, ?6)",
            params![
                APP_DYNAMIC_SUMMARY_TEMPLATE_ID,
                APP_DYNAMIC_SUMMARY_TEMPLATE_NAME,
                scope_to_str(&PromptScope::AppWide),
                content,
                entries_json,
                now
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    } else {
        let _ = maybe_backfill_entries(
            app,
            APP_DYNAMIC_SUMMARY_TEMPLATE_ID,
            PromptType::DynamicSummaryPrompt,
            get_base_prompt_entries(PromptType::DynamicSummaryPrompt),
        );
    }

    // Memory manager template
    if get_template(app, APP_DYNAMIC_MEMORY_TEMPLATE_ID)?.is_none() {
        let content = get_base_prompt(PromptType::DynamicMemoryPrompt);
        let entries = get_base_prompt_entries(PromptType::DynamicMemoryPrompt);
        let entries_json = serde_json::to_string(&entries)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        conn.execute(
            "INSERT OR IGNORE INTO prompt_templates (id, name, scope, target_ids, content, entries, created_at, updated_at) VALUES (?1, ?2, ?3, '[]', ?4, ?5, ?6, ?6)",
            params![
                APP_DYNAMIC_MEMORY_TEMPLATE_ID,
                APP_DYNAMIC_MEMORY_TEMPLATE_NAME,
                scope_to_str(&PromptScope::AppWide),
                content,
                entries_json,
                now
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    } else {
        let _ = maybe_backfill_entries(
            app,
            APP_DYNAMIC_MEMORY_TEMPLATE_ID,
            PromptType::DynamicMemoryPrompt,
            get_base_prompt_entries(PromptType::DynamicMemoryPrompt),
        );
    }

    Ok(())
}

pub fn is_app_default_template(id: &str) -> bool {
    id == APP_DEFAULT_TEMPLATE_ID
        || id == APP_DYNAMIC_SUMMARY_TEMPLATE_ID
        || id == APP_DYNAMIC_MEMORY_TEMPLATE_ID
        || id == APP_HELP_ME_REPLY_TEMPLATE_ID
        || id == APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID
        || id == APP_AVATAR_GENERATION_TEMPLATE_ID
        || id == APP_AVATAR_EDIT_TEMPLATE_ID
        || id == APP_SCENE_GENERATION_TEMPLATE_ID
        || id == APP_DESIGN_REFERENCE_TEMPLATE_ID
}

pub fn reset_app_default_template(app: &AppHandle) -> Result<SystemPromptTemplate, String> {
    let content = get_base_prompt(PromptType::SystemPrompt);
    update_template(
        app,
        APP_DEFAULT_TEMPLATE_ID.to_string(),
        None,
        None,
        None,
        Some(content.clone()),
        Some(prompt_engine::default_modular_prompt_entries()),
        None,
    )
}

pub fn reset_dynamic_summary_template(app: &AppHandle) -> Result<SystemPromptTemplate, String> {
    let content = get_base_prompt(PromptType::DynamicSummaryPrompt);
    let entries = get_base_prompt_entries(PromptType::DynamicSummaryPrompt);
    update_template(
        app,
        APP_DYNAMIC_SUMMARY_TEMPLATE_ID.to_string(),
        None,
        None,
        None,
        Some(content.clone()),
        Some(entries),
        None,
    )
}

pub fn reset_dynamic_memory_template(app: &AppHandle) -> Result<SystemPromptTemplate, String> {
    let content = get_base_prompt(PromptType::DynamicMemoryPrompt);
    let entries = get_base_prompt_entries(PromptType::DynamicMemoryPrompt);
    update_template(
        app,
        APP_DYNAMIC_MEMORY_TEMPLATE_ID.to_string(),
        None,
        None,
        None,
        Some(content.clone()),
        Some(entries),
        None,
    )
}

pub fn ensure_help_me_reply_template(app: &AppHandle) -> Result<(), String> {
    if get_template(app, APP_HELP_ME_REPLY_TEMPLATE_ID)?.is_none() {
        let conn = open_db(app)?;
        let now = now();
        let content = get_base_prompt(PromptType::HelpMeReplyPrompt);
        let entries = get_base_prompt_entries(PromptType::HelpMeReplyPrompt);
        let entries_json = serde_json::to_string(&entries)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        conn.execute(
            "INSERT OR IGNORE INTO prompt_templates (id, name, scope, target_ids, content, entries, created_at, updated_at) VALUES (?1, ?2, ?3, '[]', ?4, ?5, ?6, ?6)",
            params![
                APP_HELP_ME_REPLY_TEMPLATE_ID,
                APP_HELP_ME_REPLY_TEMPLATE_NAME,
                scope_to_str(&PromptScope::AppWide),
                content,
                entries_json,
                now
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    } else {
        let _ = maybe_backfill_entries(
            app,
            APP_HELP_ME_REPLY_TEMPLATE_ID,
            PromptType::HelpMeReplyPrompt,
            get_base_prompt_entries(PromptType::HelpMeReplyPrompt),
        );
    }

    // Also ensure conversational template exists
    if get_template(app, APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID)?.is_none() {
        let conn = open_db(app)?;
        let now = now();
        let content = get_base_prompt(PromptType::HelpMeReplyConversationalPrompt);
        let entries = get_base_prompt_entries(PromptType::HelpMeReplyConversationalPrompt);
        let entries_json = serde_json::to_string(&entries)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        conn.execute(
            "INSERT OR IGNORE INTO prompt_templates (id, name, scope, target_ids, content, entries, created_at, updated_at) VALUES (?1, ?2, ?3, '[]', ?4, ?5, ?6, ?6)",
            params![
                APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID,
                APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_NAME,
                scope_to_str(&PromptScope::AppWide),
                content,
                entries_json,
                now
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    } else {
        let _ = maybe_backfill_entries(
            app,
            APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID,
            PromptType::HelpMeReplyConversationalPrompt,
            get_base_prompt_entries(PromptType::HelpMeReplyConversationalPrompt),
        );
    }
    Ok(())
}

pub fn ensure_avatar_image_templates(app: &AppHandle) -> Result<(), String> {
    let conn = open_db(app)?;
    let now = now();
    let avatar_generation_entries = get_base_prompt_entries(PromptType::AvatarGenerationPrompt);
    let avatar_edit_entries = get_base_prompt_entries(PromptType::AvatarEditPrompt);

    if get_template(app, APP_AVATAR_GENERATION_TEMPLATE_ID)?.is_none() {
        let content = get_base_prompt(PromptType::AvatarGenerationPrompt);
        let entries_json = serde_json::to_string(&avatar_generation_entries)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        conn.execute(
            "INSERT OR IGNORE INTO prompt_templates (id, name, scope, target_ids, content, entries, created_at, updated_at) VALUES (?1, ?2, ?3, '[]', ?4, ?5, ?6, ?6)",
            params![
                APP_AVATAR_GENERATION_TEMPLATE_ID,
                APP_AVATAR_GENERATION_TEMPLATE_NAME,
                scope_to_str(&PromptScope::AppWide),
                content,
                entries_json,
                now
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    } else {
        let _ = maybe_backfill_entries(
            app,
            APP_AVATAR_GENERATION_TEMPLATE_ID,
            PromptType::AvatarGenerationPrompt,
            avatar_generation_entries.clone(),
        );
        let _ = backfill_missing_entry_conditions(
            app,
            APP_AVATAR_GENERATION_TEMPLATE_ID,
            &avatar_generation_entries,
        );
    }

    if get_template(app, APP_AVATAR_EDIT_TEMPLATE_ID)?.is_none() {
        let content = get_base_prompt(PromptType::AvatarEditPrompt);
        let entries_json = serde_json::to_string(&avatar_edit_entries)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        conn.execute(
            "INSERT OR IGNORE INTO prompt_templates (id, name, scope, target_ids, content, entries, created_at, updated_at) VALUES (?1, ?2, ?3, '[]', ?4, ?5, ?6, ?6)",
            params![
                APP_AVATAR_EDIT_TEMPLATE_ID,
                APP_AVATAR_EDIT_TEMPLATE_NAME,
                scope_to_str(&PromptScope::AppWide),
                content,
                entries_json,
                now
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    } else {
        let _ = maybe_backfill_entries(
            app,
            APP_AVATAR_EDIT_TEMPLATE_ID,
            PromptType::AvatarEditPrompt,
            avatar_edit_entries.clone(),
        );
        let _ = backfill_missing_entry_conditions(
            app,
            APP_AVATAR_EDIT_TEMPLATE_ID,
            &avatar_edit_entries,
        );
    }

    Ok(())
}

pub fn ensure_scene_generation_template(app: &AppHandle) -> Result<(), String> {
    let conn = open_db(app)?;
    let now = now();
    let scene_entries = get_base_prompt_entries(PromptType::SceneGenerationPrompt);

    if get_template(app, APP_SCENE_GENERATION_TEMPLATE_ID)?.is_none() {
        let content = get_base_prompt(PromptType::SceneGenerationPrompt);
        let entries_json = serde_json::to_string(&scene_entries)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        conn.execute(
            "INSERT OR IGNORE INTO prompt_templates (id, name, scope, target_ids, content, entries, created_at, updated_at) VALUES (?1, ?2, ?3, '[]', ?4, ?5, ?6, ?6)",
            params![
                APP_SCENE_GENERATION_TEMPLATE_ID,
                APP_SCENE_GENERATION_TEMPLATE_NAME,
                scope_to_str(&PromptScope::AppWide),
                content,
                entries_json,
                now
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    } else {
        let _ = maybe_backfill_entries(
            app,
            APP_SCENE_GENERATION_TEMPLATE_ID,
            PromptType::SceneGenerationPrompt,
            scene_entries.clone(),
        );
        if let Some(entry) = scene_entries
            .iter()
            .find(|entry| entry.id == "scene_gen_character_reference")
            .cloned()
        {
            let _ = append_missing_entry(
                app,
                APP_SCENE_GENERATION_TEMPLATE_ID,
                "scene_gen_character_reference",
                entry,
            );
        }
        if let Some(entry) = scene_entries
            .iter()
            .find(|entry| entry.id == "scene_gen_persona_reference")
            .cloned()
        {
            let _ = append_missing_entry(
                app,
                APP_SCENE_GENERATION_TEMPLATE_ID,
                "scene_gen_persona_reference",
                entry,
            );
        }
        let _ = backfill_missing_entry_conditions(
            app,
            APP_SCENE_GENERATION_TEMPLATE_ID,
            &scene_entries,
        );
        let _ =
            backfill_missing_entry_payloads(app, APP_SCENE_GENERATION_TEMPLATE_ID, &scene_entries);
        let _ = backfill_legacy_image_entry_content(
            app,
            APP_SCENE_GENERATION_TEMPLATE_ID,
            &scene_entries,
        );
        let _ = migrate_legacy_scene_generation_entry_roles(app);
    }

    Ok(())
}

pub fn ensure_design_reference_template(app: &AppHandle) -> Result<(), String> {
    let conn = open_db(app)?;
    let now = now();
    let design_reference_entries = get_base_prompt_entries(PromptType::DesignReferencePrompt);

    if get_template(app, APP_DESIGN_REFERENCE_TEMPLATE_ID)?.is_none() {
        let content = get_base_prompt(PromptType::DesignReferencePrompt);
        let entries_json = serde_json::to_string(&design_reference_entries)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        conn.execute(
            "INSERT OR IGNORE INTO prompt_templates (id, name, scope, target_ids, content, entries, created_at, updated_at) VALUES (?1, ?2, ?3, '[]', ?4, ?5, ?6, ?6)",
            params![
                APP_DESIGN_REFERENCE_TEMPLATE_ID,
                APP_DESIGN_REFERENCE_TEMPLATE_NAME,
                scope_to_str(&PromptScope::AppWide),
                content,
                entries_json,
                now
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    } else {
        let _ = maybe_backfill_entries(
            app,
            APP_DESIGN_REFERENCE_TEMPLATE_ID,
            PromptType::DesignReferencePrompt,
            design_reference_entries.clone(),
        );
        let _ = backfill_missing_entry_conditions(
            app,
            APP_DESIGN_REFERENCE_TEMPLATE_ID,
            &design_reference_entries,
        );
        let _ = backfill_missing_entry_payloads(
            app,
            APP_DESIGN_REFERENCE_TEMPLATE_ID,
            &design_reference_entries,
        );
        let _ = backfill_legacy_image_entry_content(
            app,
            APP_DESIGN_REFERENCE_TEMPLATE_ID,
            &design_reference_entries,
        );
    }

    Ok(())
}

pub fn reset_help_me_reply_template(app: &AppHandle) -> Result<SystemPromptTemplate, String> {
    let content = get_base_prompt(PromptType::HelpMeReplyPrompt);
    let entries = get_base_prompt_entries(PromptType::HelpMeReplyPrompt);
    update_template(
        app,
        APP_HELP_ME_REPLY_TEMPLATE_ID.to_string(),
        None,
        None,
        None,
        Some(content.clone()),
        Some(entries),
        None,
    )
}

pub fn reset_help_me_reply_conversational_template(
    app: &AppHandle,
) -> Result<SystemPromptTemplate, String> {
    let content = get_base_prompt(PromptType::HelpMeReplyConversationalPrompt);
    let entries = get_base_prompt_entries(PromptType::HelpMeReplyConversationalPrompt);
    update_template(
        app,
        APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID.to_string(),
        None,
        None,
        None,
        Some(content.clone()),
        Some(entries),
        None,
    )
}

pub fn reset_avatar_generation_template(app: &AppHandle) -> Result<SystemPromptTemplate, String> {
    let content = get_base_prompt(PromptType::AvatarGenerationPrompt);
    let entries = get_base_prompt_entries(PromptType::AvatarGenerationPrompt);
    update_template(
        app,
        APP_AVATAR_GENERATION_TEMPLATE_ID.to_string(),
        None,
        None,
        None,
        Some(content.clone()),
        Some(entries),
        None,
    )
}

pub fn reset_avatar_edit_template(app: &AppHandle) -> Result<SystemPromptTemplate, String> {
    let content = get_base_prompt(PromptType::AvatarEditPrompt);
    let entries = get_base_prompt_entries(PromptType::AvatarEditPrompt);
    update_template(
        app,
        APP_AVATAR_EDIT_TEMPLATE_ID.to_string(),
        None,
        None,
        None,
        Some(content.clone()),
        Some(entries),
        None,
    )
}

pub fn reset_scene_generation_template(app: &AppHandle) -> Result<SystemPromptTemplate, String> {
    let content = get_base_prompt(PromptType::SceneGenerationPrompt);
    let entries = get_base_prompt_entries(PromptType::SceneGenerationPrompt);
    update_template(
        app,
        APP_SCENE_GENERATION_TEMPLATE_ID.to_string(),
        None,
        None,
        None,
        Some(content.clone()),
        Some(entries),
        None,
    )
}

pub fn reset_design_reference_template(app: &AppHandle) -> Result<SystemPromptTemplate, String> {
    let content = get_base_prompt(PromptType::DesignReferencePrompt);
    let entries = get_base_prompt_entries(PromptType::DesignReferencePrompt);
    update_template(
        app,
        APP_DESIGN_REFERENCE_TEMPLATE_ID.to_string(),
        None,
        None,
        None,
        Some(content.clone()),
        Some(entries),
        None,
    )
}

/// Get the Help Me Reply template from DB, falling back to default if not found
pub fn get_help_me_reply_prompt(app: &AppHandle, style: &str) -> String {
    let template_id = if style == "conversational" {
        APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID
    } else {
        APP_HELP_ME_REPLY_TEMPLATE_ID
    };

    let prompt_type = if style == "conversational" {
        PromptType::HelpMeReplyConversationalPrompt
    } else {
        PromptType::HelpMeReplyPrompt
    };

    match get_template(app, template_id) {
        Ok(Some(template)) => {
            let merged = template_entries_to_content(&template.entries);
            if merged.is_empty() {
                template.content
            } else {
                merged
            }
        }
        _ => get_base_prompt(prompt_type),
    }
}

/// Get the Group Chat template from DB, falling back to default if not found
#[allow(dead_code)]
pub fn get_group_chat_prompt(app: &AppHandle) -> String {
    match get_template(app, APP_GROUP_CHAT_TEMPLATE_ID) {
        Ok(Some(template)) => {
            let merged = template_entries_to_content(&template.entries);
            if merged.is_empty() {
                template.content
            } else {
                merged
            }
        }
        _ => get_base_prompt(PromptType::GroupChatPrompt),
    }
}

/// Get the Group Chat Roleplay template from DB, falling back to default if not found
#[allow(dead_code)]
pub fn get_group_chat_roleplay_prompt(app: &AppHandle) -> String {
    match get_template(app, APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID) {
        Ok(Some(template)) => {
            let merged = template_entries_to_content(&template.entries);
            if merged.is_empty() {
                template.content
            } else {
                merged
            }
        }
        _ => get_base_prompt(PromptType::GroupChatRoleplayPrompt),
    }
}
