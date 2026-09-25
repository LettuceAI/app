use std::path::Path;

use lettuce_transfer::{
    LegacyBackupDocument, LegacyBackupDocumentKind, LegacyDatabasePreflightError,
};
use rusqlite::{Connection, Row, params};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use zeroize::Zeroizing;

use crate::legacy::legacy_database_preflight::{open_validated, require_table};

const REQUIRED_TABLES: [&str; 29] = [
    "meta",
    "audio_providers",
    "user_voices",
    "model_pricing_cache",
    "secrets",
    "prompt_templates",
    "chat_template_messages",
    "character_rules",
    "scenes",
    "scene_variants",
    "companion_scheduled_notes",
    "companion_shared_memory_state",
    "memory_embeddings",
    "messages",
    "message_variants",
    "group_participation",
    "group_messages",
    "group_message_variants",
    "usage_records",
    "usage_metadata",
    "lorebook_entries",
    "creation_helper_sessions",
    "asr_vocabulary_terms",
    "asr_corrections",
    "asr_ignored_suggestions",
    "personas",
    "characters",
    "chat_templates",
    "group_characters",
];

/// Reads a version-92 legacy database into the documents a legacy backup
/// archive of the same data contains, so both sources share one planner.
pub fn read_legacy_database_documents(
    path: impl AsRef<Path>,
) -> Result<Vec<LegacyBackupDocument>, LegacyDatabasePreflightError> {
    let connection = open_validated(path)?;
    for table in REQUIRED_TABLES {
        require_table(&connection, table)?;
    }
    let mut documents = vec![
        document(
            LegacyBackupDocumentKind::Meta,
            Value::Array(meta(&connection)?),
        )?,
        document(LegacyBackupDocumentKind::Settings, settings(&connection)?)?,
        document(
            LegacyBackupDocumentKind::ProviderCredentials,
            Value::Array(provider_credentials(&connection)?),
        )?,
        document(
            LegacyBackupDocumentKind::Models,
            Value::Array(models(&connection)?),
        )?,
        document(
            LegacyBackupDocumentKind::AudioProviders,
            Value::Array(audio_providers(&connection)?),
        )?,
        document(
            LegacyBackupDocumentKind::UserVoices,
            Value::Array(user_voices(&connection)?),
        )?,
        document(
            LegacyBackupDocumentKind::ModelPricingCache,
            Value::Array(model_pricing_cache(&connection)?),
        )?,
        document(
            LegacyBackupDocumentKind::Secrets,
            Value::Array(secrets(&connection)?),
        )?,
        document(
            LegacyBackupDocumentKind::PromptTemplates,
            Value::Array(prompt_templates(&connection)?),
        )?,
        document(
            LegacyBackupDocumentKind::ChatTemplates,
            Value::Array(chat_templates(&connection)?),
        )?,
        document(
            LegacyBackupDocumentKind::Personas,
            Value::Array(personas(&connection)?),
        )?,
        document(
            LegacyBackupDocumentKind::Characters,
            Value::Array(characters(&connection)?),
        )?,
        document(
            LegacyBackupDocumentKind::CompanionScheduledNotes,
            Value::Array(companion_scheduled_notes(&connection)?),
        )?,
        document(
            LegacyBackupDocumentKind::CompanionSharedMemory,
            Value::Array(companion_shared_memory(&connection)?),
        )?,
        document(
            LegacyBackupDocumentKind::MemoryEmbeddings,
            Value::Array(memory_embedding_owners(&connection)?),
        )?,
        document(
            LegacyBackupDocumentKind::Sessions,
            Value::Array(sessions(&connection)?),
        )?,
        document(
            LegacyBackupDocumentKind::CreationHelperSessions,
            Value::Array(creation_helper_sessions(&connection)?),
        )?,
        document(
            LegacyBackupDocumentKind::AsrLearning,
            asr_learning(&connection)?,
        )?,
        document(
            LegacyBackupDocumentKind::GroupCharacters,
            Value::Array(group_characters(&connection)?),
        )?,
        document(
            LegacyBackupDocumentKind::GroupSessions,
            Value::Array(group_sessions(&connection)?),
        )?,
        document(
            LegacyBackupDocumentKind::UsageRecords,
            Value::Array(usage_records(&connection)?),
        )?,
        document(
            LegacyBackupDocumentKind::Lorebooks,
            Value::Array(lorebooks(&connection)?),
        )?,
    ];
    if table_exists(&connection, "image_loras")? {
        documents.push(document(
            LegacyBackupDocumentKind::ImageLoras,
            Value::Array(image_loras(&connection)?),
        )?);
    }
    if table_exists(&connection, "playground_generations")? {
        documents.push(document(
            LegacyBackupDocumentKind::PlaygroundGenerations,
            Value::Array(playground_generations(&connection)?),
        )?);
    }
    if table_exists(&connection, "llm_generation_metrics")? {
        documents.push(document(
            LegacyBackupDocumentKind::LlmGenerationMetrics,
            Value::Array(llm_generation_metrics(&connection)?),
        )?);
    }
    documents.sort_by_key(|document| document.kind);
    Ok(documents)
}

fn document(
    kind: LegacyBackupDocumentKind,
    value: Value,
) -> Result<LegacyBackupDocument, LegacyDatabasePreflightError> {
    let bytes = serde_json::to_vec_pretty(&value)
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    Ok(LegacyBackupDocument {
        kind,
        bytes: Zeroizing::new(bytes),
    })
}

fn rows<P, F>(
    connection: &Connection,
    sql: &str,
    params: P,
    map: F,
) -> Result<Vec<Value>, LegacyDatabasePreflightError>
where
    P: rusqlite::Params,
    F: FnMut(&Row<'_>) -> rusqlite::Result<Value>,
{
    let mut statement = connection
        .prepare(sql)
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    statement
        .query_map(params, map)
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)
}

/// Builds a select list for a table whose optional columns an accepted source
/// may lack; a missing one reads as the given default, the value the column
/// holds once it exists. A column without a default is required.
fn projection(
    connection: &Connection,
    table: &str,
    columns: &[(&str, Option<&str>)],
) -> Result<String, LegacyDatabasePreflightError> {
    let mut statement = connection
        .prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    let present = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?
        .collect::<Result<std::collections::BTreeSet<_>, _>>()
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    columns
        .iter()
        .map(|(name, default)| {
            if present.contains(*name) {
                Ok((*name).to_owned())
            } else {
                default
                    .map(|value| format!("{value} AS {name}"))
                    .ok_or(LegacyDatabasePreflightError::InvalidSchema)
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|columns| columns.join(", "))
}

const MESSAGE_COLUMNS: &[(&str, Option<&str>)] = &[
    ("id", None),
    ("role", None),
    ("content", None),
    ("created_at", None),
    ("visible_in_chat", Some("0")),
    ("scene_edited", Some("0")),
    ("prompt_tokens", Some("NULL")),
    ("completion_tokens", Some("NULL")),
    ("total_tokens", Some("NULL")),
    ("first_token_ms", Some("NULL")),
    ("tokens_per_second", Some("NULL")),
    ("mtp_stats", Some("NULL")),
    ("model_id", Some("NULL")),
    ("selected_variant_id", Some("NULL")),
    ("is_pinned", Some("0")),
    ("memory_refs", Some("'[]'")),
    ("used_lorebook_entries", Some("'[]'")),
    ("attachments", Some("'[]'")),
    ("reasoning", Some("NULL")),
    ("parent_message_id", Some("NULL")),
    ("effective_at", Some("NULL")),
];

const MESSAGE_VARIANT_COLUMNS: &[(&str, Option<&str>)] = &[
    ("id", None),
    ("content", None),
    ("created_at", None),
    ("prompt_tokens", Some("NULL")),
    ("completion_tokens", Some("NULL")),
    ("total_tokens", Some("NULL")),
    ("first_token_ms", Some("NULL")),
    ("tokens_per_second", Some("NULL")),
    ("mtp_stats", Some("NULL")),
    ("reasoning", Some("NULL")),
];

const GROUP_MESSAGE_COLUMNS: &[(&str, Option<&str>)] = &[
    ("id", None),
    ("role", None),
    ("content", None),
    ("speaker_character_id", Some("NULL")),
    ("turn_number", None),
    ("created_at", None),
    ("prompt_tokens", Some("NULL")),
    ("completion_tokens", Some("NULL")),
    ("total_tokens", Some("NULL")),
    ("first_token_ms", Some("NULL")),
    ("tokens_per_second", Some("NULL")),
    ("mtp_stats", Some("NULL")),
    ("selected_variant_id", Some("NULL")),
    ("is_pinned", Some("0")),
    ("attachments", Some("'[]'")),
    ("used_lorebook_entries", Some("'[]'")),
    ("memory_refs", Some("'[]'")),
    ("reasoning", Some("NULL")),
    ("selection_reasoning", Some("NULL")),
    ("model_id", Some("NULL")),
    ("gemini_content", Some("NULL")),
    ("usage_json", Some("NULL")),
    ("parent_message_id", Some("NULL")),
];

const GROUP_MESSAGE_VARIANT_COLUMNS: &[(&str, Option<&str>)] = &[
    ("id", None),
    ("content", None),
    ("speaker_character_id", Some("NULL")),
    ("created_at", None),
    ("prompt_tokens", Some("NULL")),
    ("completion_tokens", Some("NULL")),
    ("total_tokens", Some("NULL")),
    ("first_token_ms", Some("NULL")),
    ("tokens_per_second", Some("NULL")),
    ("mtp_stats", Some("NULL")),
    ("reasoning", Some("NULL")),
    ("selection_reasoning", Some("NULL")),
    ("model_id", Some("NULL")),
    ("attachments", Some("'[]'")),
    ("gemini_content", Some("NULL")),
    ("usage_json", Some("NULL")),
];

fn flag(row: &Row<'_>, index: usize) -> rusqlite::Result<bool> {
    Ok(row.get::<_, i64>(index)? != 0)
}

fn meta(connection: &Connection) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    rows(
        connection,
        "SELECT key, value FROM meta ORDER BY key ASC",
        [],
        |r| {
            Ok(json!({
                "key": r.get::<_, String>(0)?,
                "value": r.get::<_, Option<String>>(1)?,
            }))
        },
    )
}

fn settings(connection: &Connection) -> Result<Value, LegacyDatabasePreflightError> {
    let parsed = |raw: Option<String>| {
        raw.map(|value| serde_json::from_str::<Value>(&value).unwrap_or(Value::String(value)))
    };
    let mut values = rows(
        connection,
        "SELECT default_provider_credential_id, default_model_id, app_state, advanced_model_settings, prompt_template_id, system_prompt, migration_version, advanced_settings, created_at, updated_at FROM settings WHERE id = 1",
        [],
        |r| {
            let app_state: String = r.get(2)?;
            Ok(json!({
                "default_provider_credential_id": r.get::<_, Option<String>>(0)?,
                "default_model_id": r.get::<_, Option<String>>(1)?,
                "app_state": serde_json::from_str::<Value>(&app_state).unwrap_or(Value::String(app_state)),
                "advanced_model_settings": parsed(r.get(3)?),
                "prompt_template_id": r.get::<_, Option<String>>(4)?,
                "system_prompt": r.get::<_, Option<String>>(5)?,
                "migration_version": r.get::<_, i64>(6)?,
                "advanced_settings": parsed(r.get(7)?),
                "created_at": r.get::<_, i64>(8)?,
                "updated_at": r.get::<_, i64>(9)?,
            }))
        },
    )?;
    Ok(values.pop().unwrap_or_else(|| json!({})))
}

fn provider_credentials(
    connection: &Connection,
) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    rows(
        connection,
        "SELECT id, provider_id, label, api_key_ref, api_key, base_url, default_model, headers, config FROM provider_credentials",
        [],
        |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "provider_id": r.get::<_, String>(1)?,
                "label": r.get::<_, String>(2)?,
                "api_key_ref": r.get::<_, Option<String>>(3)?,
                "api_key": r.get::<_, Option<String>>(4)?,
                "base_url": r.get::<_, Option<String>>(5)?,
                "default_model": r.get::<_, Option<String>>(6)?,
                "headers": r.get::<_, Option<String>>(7)?,
                "config": r.get::<_, Option<String>>(8)?,
            }))
        },
    )
}

fn models(connection: &Connection) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    rows(
        connection,
        "SELECT id, name, provider_id, provider_credential_id, provider_label, display_name, created_at, model_type, input_scopes, output_scopes, advanced_model_settings, prompt_template_id, system_prompt FROM models",
        [],
        |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "name": r.get::<_, String>(1)?,
                "provider_id": r.get::<_, String>(2)?,
                "provider_credential_id": r.get::<_, Option<String>>(3)?,
                "provider_label": r.get::<_, String>(4)?,
                "display_name": r.get::<_, String>(5)?,
                "created_at": r.get::<_, i64>(6)?,
                "model_type": r.get::<_, Option<String>>(7)?,
                "input_scopes": r.get::<_, Option<String>>(8)?,
                "output_scopes": r.get::<_, Option<String>>(9)?,
                "advanced_model_settings": r.get::<_, Option<String>>(10)?,
                "prompt_template_id": r.get::<_, Option<String>>(11)?,
                "system_prompt": r.get::<_, Option<String>>(12)?,
            }))
        },
    )
}

fn audio_providers(connection: &Connection) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    rows(
        connection,
        "SELECT id, provider_type, label, api_key, project_id, location, base_url, request_path, kokoro_variant, asset_root, created_at, updated_at FROM audio_providers",
        [],
        |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "provider_type": r.get::<_, String>(1)?,
                "label": r.get::<_, String>(2)?,
                "api_key": r.get::<_, Option<String>>(3)?,
                "project_id": r.get::<_, Option<String>>(4)?,
                "location": r.get::<_, Option<String>>(5)?,
                "base_url": r.get::<_, Option<String>>(6)?,
                "request_path": r.get::<_, Option<String>>(7)?,
                "kokoro_variant": r.get::<_, Option<String>>(8)?,
                "asset_root": r.get::<_, Option<String>>(9)?,
                "created_at": r.get::<_, i64>(10)?,
                "updated_at": r.get::<_, i64>(11)?,
            }))
        },
    )
}

fn user_voices(connection: &Connection) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    rows(
        connection,
        "SELECT id, provider_id, name, model_id, voice_id, prompt, created_at, updated_at FROM user_voices",
        [],
        |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "provider_id": r.get::<_, String>(1)?,
                "name": r.get::<_, String>(2)?,
                "model_id": r.get::<_, String>(3)?,
                "voice_id": r.get::<_, String>(4)?,
                "prompt": r.get::<_, Option<String>>(5)?,
                "created_at": r.get::<_, i64>(6)?,
                "updated_at": r.get::<_, i64>(7)?,
            }))
        },
    )
}

fn model_pricing_cache(
    connection: &Connection,
) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    rows(
        connection,
        "SELECT model_id, pricing_json, cached_at FROM model_pricing_cache ORDER BY cached_at DESC",
        [],
        |r| {
            Ok(json!({
                "model_id": r.get::<_, String>(0)?,
                "pricing_json": r.get::<_, Option<String>>(1)?,
                "cached_at": r.get::<_, i64>(2)?,
            }))
        },
    )
}

fn secrets(connection: &Connection) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    rows(
        connection,
        "SELECT service, account, value, created_at, updated_at FROM secrets",
        [],
        |r| {
            Ok(json!({
                "service": r.get::<_, String>(0)?,
                "account": r.get::<_, String>(1)?,
                "value": r.get::<_, String>(2)?,
                "created_at": r.get::<_, i64>(3)?,
                "updated_at": r.get::<_, i64>(4)?,
            }))
        },
    )
}

fn prompt_templates(connection: &Connection) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    rows(
        connection,
        "SELECT id, name, prompt_type, content, entries, condense_prompt_entries, created_at, updated_at FROM prompt_templates",
        [],
        |r| {
            let entries: String = r.get(4)?;
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "name": r.get::<_, String>(1)?,
                "prompt_type": r.get::<_, String>(2)?,
                "content": r.get::<_, String>(3)?,
                "entries": serde_json::from_str::<Value>(&entries).unwrap_or_else(|_| Value::Array(Vec::new())),
                "condense_prompt_entries": flag(r, 5)?,
                "created_at": r.get::<_, i64>(6)?,
                "updated_at": r.get::<_, i64>(7)?,
            }))
        },
    )
}

fn chat_templates(connection: &Connection) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    let mut templates = rows(
        connection,
        "SELECT id, character_id, name, scene_id, prompt_template_id, lorebook_ids_override, created_at FROM chat_templates ORDER BY created_at ASC",
        [],
        |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "character_id": r.get::<_, String>(1)?,
                "name": r.get::<_, String>(2)?,
                "scene_id": r.get::<_, Option<String>>(3)?,
                "prompt_template_id": r.get::<_, Option<String>>(4)?,
                "lorebook_ids_override": r.get::<_, Option<String>>(5)?,
                "created_at": r.get::<_, i64>(6)?,
            }))
        },
    )?;
    for template in &mut templates {
        let id = string_field(template, "id")?;
        let messages = rows(
            connection,
            "SELECT id, idx, role, content FROM chat_template_messages WHERE template_id = ?1 ORDER BY idx ASC",
            params![id],
            |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?,
                    "idx": r.get::<_, i64>(1)?,
                    "role": r.get::<_, String>(2)?,
                    "content": r.get::<_, String>(3)?,
                }))
            },
        )?;
        template["messages"] = Value::Array(messages);
    }
    Ok(templates)
}

fn personas(connection: &Connection) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    rows(
        connection,
        "SELECT id, title, description, nickname, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, design_description, design_reference_image_ids, lora_name, lora_strength, COALESCE(active_lorebook_ids, '[]'), is_default, created_at, updated_at FROM personas",
        [],
        |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "title": r.get::<_, String>(1)?,
                "description": r.get::<_, String>(2)?,
                "nickname": r.get::<_, Option<String>>(3)?,
                "avatar_path": r.get::<_, Option<String>>(4)?,
                "avatar_crop_x": r.get::<_, Option<f64>>(5)?,
                "avatar_crop_y": r.get::<_, Option<f64>>(6)?,
                "avatar_crop_scale": r.get::<_, Option<f64>>(7)?,
                "design_description": r.get::<_, Option<String>>(8)?,
                "design_reference_image_ids": r.get::<_, Option<String>>(9)?,
                "lora_name": r.get::<_, Option<String>>(10)?,
                "lora_strength": r.get::<_, Option<f64>>(11)?,
                "active_lorebook_ids": r.get::<_, String>(12)?,
                "is_default": flag(r, 13)?,
                "created_at": r.get::<_, i64>(14)?,
                "updated_at": r.get::<_, i64>(15)?,
            }))
        },
    )
}

fn characters(connection: &Connection) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    let mut characters = rows(
        connection,
        "SELECT id, name, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, banner_crop_x, banner_crop_y, banner_crop_scale, COALESCE(card_type, 'circle'), design_description, design_reference_image_ids, lora_name, lora_strength, background_image_path, description, definition, nickname, scenario, creator_notes, creator, creator_notes_multilingual, source, tags, default_scene_id, default_model_id, COALESCE(mode, 'roleplay'), companion, memory_type, COALESCE(active_lorebook_ids, '[]'), prompt_template_id, group_chat_prompt_template_id, group_chat_roleplay_prompt_template_id, system_prompt, voice_config, voice_autoplay, disable_avatar_gradient, COALESCE(avatar_gradient_source, 'base'), custom_gradient_enabled, custom_gradient_colors, custom_text_color, custom_text_secondary, chat_appearance, default_chat_template_id, created_at, updated_at FROM characters",
        [],
        |r| {
            let mut value = Map::new();
            let mut put = |key: &str, field: Value| {
                value.insert(key.to_owned(), field);
            };
            put("id", json!(r.get::<_, String>(0)?));
            put("name", json!(r.get::<_, String>(1)?));
            put("avatar_path", json!(r.get::<_, Option<String>>(2)?));
            put("avatar_crop_x", json!(r.get::<_, Option<f64>>(3)?));
            put("avatar_crop_y", json!(r.get::<_, Option<f64>>(4)?));
            put("avatar_crop_scale", json!(r.get::<_, Option<f64>>(5)?));
            put("banner_crop_x", json!(r.get::<_, Option<f64>>(6)?));
            put("banner_crop_y", json!(r.get::<_, Option<f64>>(7)?));
            put("banner_crop_scale", json!(r.get::<_, Option<f64>>(8)?));
            put("card_type", json!(r.get::<_, String>(9)?));
            put("design_description", json!(r.get::<_, Option<String>>(10)?));
            put(
                "design_reference_image_ids",
                json!(r.get::<_, Option<String>>(11)?),
            );
            put("lora_name", json!(r.get::<_, Option<String>>(12)?));
            put("lora_strength", json!(r.get::<_, Option<f64>>(13)?));
            put(
                "background_image_path",
                json!(r.get::<_, Option<String>>(14)?),
            );
            put("description", json!(r.get::<_, Option<String>>(15)?));
            put("definition", json!(r.get::<_, Option<String>>(16)?));
            put("nickname", json!(r.get::<_, Option<String>>(17)?));
            put("scenario", json!(r.get::<_, Option<String>>(18)?));
            put("creator_notes", json!(r.get::<_, Option<String>>(19)?));
            put("creator", json!(r.get::<_, Option<String>>(20)?));
            put(
                "creator_notes_multilingual",
                json!(r.get::<_, Option<String>>(21)?),
            );
            put("source", json!(r.get::<_, Option<String>>(22)?));
            put("tags", json!(r.get::<_, Option<String>>(23)?));
            put("default_scene_id", json!(r.get::<_, Option<String>>(24)?));
            put("default_model_id", json!(r.get::<_, Option<String>>(25)?));
            put("mode", json!(r.get::<_, String>(26)?));
            put("companion", json!(r.get::<_, Option<String>>(27)?));
            put("memory_type", json!(r.get::<_, String>(28)?));
            put("active_lorebook_ids", json!(r.get::<_, String>(29)?));
            put("prompt_template_id", json!(r.get::<_, Option<String>>(30)?));
            put(
                "group_chat_prompt_template_id",
                json!(r.get::<_, Option<String>>(31)?),
            );
            put(
                "group_chat_roleplay_prompt_template_id",
                json!(r.get::<_, Option<String>>(32)?),
            );
            put("system_prompt", json!(r.get::<_, Option<String>>(33)?));
            put("voice_config", json!(r.get::<_, Option<String>>(34)?));
            put(
                "voice_autoplay",
                json!(r.get::<_, Option<i64>>(35)?.unwrap_or(0) != 0),
            );
            put("disable_avatar_gradient", json!(flag(r, 36)?));
            put("avatar_gradient_source", json!(r.get::<_, String>(37)?));
            put("custom_gradient_enabled", json!(flag(r, 38)?));
            put(
                "custom_gradient_colors",
                json!(r.get::<_, Option<String>>(39)?),
            );
            put("custom_text_color", json!(r.get::<_, Option<String>>(40)?));
            put(
                "custom_text_secondary",
                json!(r.get::<_, Option<String>>(41)?),
            );
            put("chat_appearance", json!(r.get::<_, Option<String>>(42)?));
            put(
                "default_chat_template_id",
                json!(r.get::<_, Option<String>>(43)?),
            );
            put("created_at", json!(r.get::<_, i64>(44)?));
            put("updated_at", json!(r.get::<_, i64>(45)?));
            Ok(Value::Object(value))
        },
    )?;
    for character in &mut characters {
        let id = string_field(character, "id")?;
        let rules = rows(
            connection,
            "SELECT rule FROM character_rules WHERE character_id = ?1 ORDER BY idx",
            params![id],
            |r| Ok(json!(r.get::<_, String>(0)?)),
        )?;
        let mut scenes = rows(
            connection,
            "SELECT id, content, direction, background_image_path, created_at, selected_variant_id FROM scenes WHERE character_id = ?1",
            params![id],
            |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?,
                    "content": r.get::<_, String>(1)?,
                    "direction": r.get::<_, Option<String>>(2)?,
                    "background_image_path": r.get::<_, Option<String>>(3)?,
                    "created_at": r.get::<_, i64>(4)?,
                    "selected_variant_id": r.get::<_, Option<String>>(5)?,
                }))
            },
        )?;
        for scene in &mut scenes {
            let scene_id = string_field(scene, "id")?;
            let variants = rows(
                connection,
                "SELECT id, content, direction, created_at FROM scene_variants WHERE scene_id = ?1 ORDER BY created_at ASC, rowid ASC",
                params![scene_id],
                |r| {
                    Ok(json!({
                        "id": r.get::<_, String>(0)?,
                        "content": r.get::<_, String>(1)?,
                        "direction": r.get::<_, Option<String>>(2)?,
                        "created_at": r.get::<_, i64>(3)?,
                    }))
                },
            )?;
            scene["variants"] = Value::Array(variants);
        }
        character["rules"] = Value::Array(rules);
        character["scenes"] = Value::Array(scenes);
    }
    Ok(characters)
}

fn companion_scheduled_notes(
    connection: &Connection,
) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    rows(
        connection,
        "SELECT id, character_id, label, content, available_at, expires_at, recurrence, recurrence_window_ms, enabled, created_at, updated_at FROM companion_scheduled_notes ORDER BY character_id ASC, available_at ASC, id ASC",
        [],
        |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "character_id": r.get::<_, String>(1)?,
                "label": r.get::<_, String>(2)?,
                "content": r.get::<_, String>(3)?,
                "available_at": r.get::<_, i64>(4)?,
                "expires_at": r.get::<_, Option<i64>>(5)?,
                "recurrence": r.get::<_, String>(6)?,
                "recurrence_window_ms": r.get::<_, Option<i64>>(7)?,
                "enabled": flag(r, 8)?,
                "created_at": r.get::<_, i64>(9)?,
                "updated_at": r.get::<_, i64>(10)?,
            }))
        },
    )
}

fn companion_shared_memory(
    connection: &Connection,
) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    let mut states = rows(
        connection,
        "SELECT character_id, memories, memory_summary, memory_summary_token_count, memory_tool_events, memory_status, memory_error, memory_progress_step, soul_growth, relationship_states, created_at, updated_at FROM companion_shared_memory_state ORDER BY character_id ASC",
        [],
        |r| {
            Ok(json!({
                "character_id": r.get::<_, String>(0)?,
                "memories": r.get::<_, String>(1)?,
                "memory_summary": r.get::<_, Option<String>>(2)?,
                "memory_summary_token_count": r.get::<_, i64>(3)?,
                "memory_tool_events": r.get::<_, String>(4)?,
                "memory_status": r.get::<_, Option<String>>(5)?,
                "memory_error": r.get::<_, Option<String>>(6)?,
                "memory_progress_step": r.get::<_, Option<i64>>(7)?,
                "soul_growth": r.get::<_, String>(8)?,
                "relationship_states": r.get::<_, String>(9)?,
                "created_at": r.get::<_, i64>(10)?,
                "updated_at": r.get::<_, i64>(11)?,
            }))
        },
    )?;
    let episodes_exist = table_exists(connection, "companion_episodes")?;
    if episodes_exist {
        states.extend(rows(
            connection,
            "SELECT episode.character_id, MIN(episode.started_at), MAX(episode.updated_at) FROM companion_episodes episode JOIN characters character ON character.id = episode.character_id AND character.mode = 'companion' WHERE episode.character_id NOT IN (SELECT character_id FROM companion_shared_memory_state) GROUP BY episode.character_id",
            [],
            |r| {
                let created_at = r.get::<_, i64>(1)?;
                Ok(json!({
                    "character_id": r.get::<_, String>(0)?,
                    "memories": "[]",
                    "memory_summary": null,
                    "memory_summary_token_count": 0,
                    "memory_tool_events": "[]",
                    "memory_status": null,
                    "memory_error": null,
                    "memory_progress_step": null,
                    "soul_growth": "[]",
                    "relationship_states": "{}",
                    "created_at": created_at,
                    "updated_at": r.get::<_, i64>(2)?.max(created_at),
                }))
            },
        )?);
        states.sort_by(|left, right| {
            left["character_id"]
                .as_str()
                .cmp(&right["character_id"].as_str())
        });
    }
    for state in &mut states {
        let character_id = string_field(state, "character_id")?;
        if episodes_exist {
            let episodes = rows(
                connection,
                "SELECT session_id, persona_key, episode_index, previous_session_id, started_at, ended_at, updated_at FROM companion_episodes WHERE character_id = ?1 ORDER BY persona_key ASC, episode_index ASC",
                params![character_id],
                |r| {
                    Ok(json!({
                        "session_id": r.get::<_, String>(0)?,
                        "persona_key": r.get::<_, String>(1)?,
                        "episode_index": r.get::<_, i64>(2)?,
                        "previous_session_id": r.get::<_, Option<String>>(3)?,
                        "started_at": r.get::<_, i64>(4)?,
                        "ended_at": r.get::<_, Option<i64>>(5)?,
                        "updated_at": r.get::<_, i64>(6)?,
                    }))
                },
            )?;
            state["episodes"] = Value::Array(episodes);
        }
        let embeddings = canonical_embeddings(connection, &character_id, "companion_shared", "[]")?;
        state["memory_embeddings"] = Value::String(embeddings);
    }
    Ok(states)
}

fn memory_embedding_owners(
    connection: &Connection,
) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    let owners = rows(
        connection,
        "SELECT DISTINCT session_id, session_kind FROM memory_embeddings ORDER BY session_kind ASC, session_id ASC",
        [],
        |r| Ok(json!([r.get::<_, String>(0)?, r.get::<_, String>(1)?])),
    )?;
    owners
        .into_iter()
        .map(|owner| {
            let session_id = owner[0].as_str().unwrap_or_default().to_owned();
            let session_kind = owner[1].as_str().unwrap_or_default().to_owned();
            if !matches!(
                session_kind.as_str(),
                "session" | "group_session" | "companion_shared"
            ) {
                return Err(LegacyDatabasePreflightError::InvalidSchema);
            }
            let memory_embeddings =
                canonical_embeddings(connection, &session_id, &session_kind, "[]")?;
            Ok(json!({
                "session_id": session_id,
                "session_kind": session_kind,
                "memory_embeddings": memory_embeddings,
            }))
        })
        .collect()
}

/// The normalized memory rows serialized in the legacy `MemoryEmbedding` JSON
/// shape, or the session's legacy column when no normalized rows exist.
fn canonical_embeddings(
    connection: &Connection,
    session_id: &str,
    session_kind: &str,
    legacy_json: &str,
) -> Result<String, LegacyDatabasePreflightError> {
    let mut statement = connection
        .prepare(
            "SELECT memory_id, embedding, embedding_model, text, token_count, category, importance_score, persistence_importance, prompt_importance, volatility, is_cold, is_pinned, access_count, fact_signature, fact_polarity, source_role, source_message_id, superseded_by, superseded_at, supersedes_json, canonical_entities_json, observed_at, observed_time_precision, created_at, last_accessed_at FROM memory_embeddings WHERE session_id = ?1 AND session_kind = ?2 ORDER BY created_at ASC",
        )
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    let memories = statement
        .query_map(params![session_id, session_kind], |r| {
            let blob: Vec<u8> = r.get(1)?;
            let embedding = blob
                .chunks_exact(4)
                .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
                .collect::<Vec<_>>();
            let embedding_dimensions = (!embedding.is_empty()).then_some(embedding.len());
            Ok(LegacyMemoryEmbedding {
                id: r.get(0)?,
                text: r.get(3)?,
                embedding,
                created_at: r.get::<_, i64>(23)? as u64,
                token_count: r.get::<_, i64>(4)? as u32,
                is_cold: flag(r, 10)?,
                last_accessed_at: r.get::<_, i64>(24)? as u64,
                importance_score: r.get::<_, f64>(6)? as f32,
                persistence_importance: r.get::<_, f64>(7)? as f32,
                prompt_importance: r.get::<_, f64>(8)? as f32,
                volatility: r.get::<_, f64>(9)? as f32,
                is_pinned: flag(r, 11)?,
                access_count: r.get::<_, i64>(12)? as u32,
                embedding_source_version: r.get(2)?,
                embedding_dimensions,
                category: r.get(5)?,
                observed_at: r.get::<_, Option<i64>>(21)?.map(|value| value as u64),
                observed_time_precision: r.get(22)?,
                canonical_entities: r
                    .get::<_, Option<String>>(20)?
                    .and_then(|raw| serde_json::from_str(&raw).ok())
                    .unwrap_or_default(),
                fact_signature: r.get(13)?,
                fact_polarity: r.get::<_, Option<i64>>(14)?.map(|value| value as i8),
                source_role: r.get(15)?,
                source_message_id: r.get(16)?,
                superseded_by: r.get(17)?,
                superseded_at: r.get::<_, Option<i64>>(18)?.map(|value| value as u64),
                supersedes: r
                    .get::<_, Option<String>>(19)?
                    .and_then(|raw| serde_json::from_str(&raw).ok())
                    .unwrap_or_default(),
            })
        })
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    if memories.is_empty() {
        return Ok(legacy_json.to_owned());
    }
    serde_json::to_string(&memories).map_err(|_| LegacyDatabasePreflightError::InvalidSchema)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LegacyMemoryEmbedding {
    id: String,
    text: String,
    embedding: Vec<f32>,
    created_at: u64,
    token_count: u32,
    is_cold: bool,
    last_accessed_at: u64,
    importance_score: f32,
    persistence_importance: f32,
    prompt_importance: f32,
    volatility: f32,
    is_pinned: bool,
    access_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    embedding_source_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    embedding_dimensions: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    observed_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    observed_time_precision: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    canonical_entities: Vec<LegacyMemoryEntityAnchor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fact_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fact_polarity: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    superseded_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    superseded_at: Option<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    supersedes: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyMemoryEntityAnchor {
    label: String,
    surface: String,
    canonical_key: String,
    canonical_name: String,
    #[serde(default)]
    confidence: f32,
}

fn sessions(connection: &Connection) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    let mut sessions = rows(
        connection,
        "SELECT id, character_id, title, parent_session_id, branched_from_message_id, root_session_id, background_image_path, system_prompt, mode, selected_scene_id, author_note, persona_id, persona_disabled, voice_autoplay, prompt_template_id, lorebook_ids_override, temperature, top_p, max_output_tokens, frequency_penalty, presence_penalty, top_k, advanced_model_settings, companion_state, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, memory_status, memory_error, memory_progress_step, archived, created_at, updated_at FROM sessions",
        [],
        |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "character_id": r.get::<_, String>(1)?,
                "title": r.get::<_, String>(2)?,
                "parent_session_id": r.get::<_, Option<String>>(3)?,
                "branched_from_message_id": r.get::<_, Option<String>>(4)?,
                "root_session_id": r.get::<_, Option<String>>(5)?,
                "background_image_path": r.get::<_, Option<String>>(6)?,
                "system_prompt": r.get::<_, Option<String>>(7)?,
                "mode": r.get::<_, String>(8)?,
                "selected_scene_id": r.get::<_, Option<String>>(9)?,
                "author_note": r.get::<_, Option<String>>(10)?,
                "persona_id": r.get::<_, Option<String>>(11)?,
                "persona_disabled": flag(r, 12)?,
                "voice_autoplay": r.get::<_, Option<i64>>(13)?.map(|value| value != 0),
                "prompt_template_id": r.get::<_, Option<String>>(14)?,
                "lorebook_ids_override": r.get::<_, Option<String>>(15)?,
                "temperature": r.get::<_, Option<f64>>(16)?,
                "top_p": r.get::<_, Option<f64>>(17)?,
                "max_output_tokens": r.get::<_, Option<i64>>(18)?,
                "frequency_penalty": r.get::<_, Option<f64>>(19)?,
                "presence_penalty": r.get::<_, Option<f64>>(20)?,
                "top_k": r.get::<_, Option<i64>>(21)?,
                "advanced_model_settings": r.get::<_, Option<String>>(22)?,
                "companion_state": r.get::<_, Option<String>>(23)?,
                "memories": r.get::<_, String>(24)?,
                "memory_embeddings": r.get::<_, String>(25)?,
                "memory_summary": r.get::<_, Option<String>>(26)?,
                "memory_summary_token_count": r.get::<_, i64>(27)?,
                "memory_tool_events": r.get::<_, String>(28)?,
                "memory_status": r.get::<_, Option<String>>(29)?,
                "memory_error": r.get::<_, Option<String>>(30)?,
                "memory_progress_step": r.get::<_, Option<i64>>(31)?,
                "archived": flag(r, 32)?,
                "created_at": r.get::<_, i64>(33)?,
                "updated_at": r.get::<_, i64>(34)?,
            }))
        },
    )?;
    let message_sql = format!(
        "SELECT {} FROM messages WHERE session_id = ?1 ORDER BY created_at ASC",
        projection(connection, "messages", MESSAGE_COLUMNS)?
    );
    let variant_sql = format!(
        "SELECT {} FROM message_variants WHERE message_id = ?1 ORDER BY created_at ASC, rowid ASC",
        projection(connection, "message_variants", MESSAGE_VARIANT_COLUMNS)?
    );
    for session in &mut sessions {
        let id = string_field(session, "id")?;
        let legacy = session["memory_embeddings"]
            .as_str()
            .unwrap_or("[]")
            .to_owned();
        session["memory_embeddings"] =
            Value::String(canonical_embeddings(connection, &id, "session", &legacy)?);
        let mut messages = rows(connection, &message_sql, params![id], |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "role": r.get::<_, String>(1)?,
                "content": r.get::<_, String>(2)?,
                "created_at": r.get::<_, i64>(3)?,
                "visible_in_chat": flag(r, 4)?,
                "scene_edited": flag(r, 5)?,
                "prompt_tokens": r.get::<_, Option<i64>>(6)?,
                "completion_tokens": r.get::<_, Option<i64>>(7)?,
                "total_tokens": r.get::<_, Option<i64>>(8)?,
                "first_token_ms": r.get::<_, Option<i64>>(9)?,
                "tokens_per_second": r.get::<_, Option<f64>>(10)?,
                "mtp_stats": r.get::<_, Option<String>>(11)?,
                "model_id": r.get::<_, Option<String>>(12)?,
                "selected_variant_id": r.get::<_, Option<String>>(13)?,
                "is_pinned": flag(r, 14)?,
                "memory_refs": r.get::<_, String>(15)?,
                "used_lorebook_entries": r.get::<_, String>(16)?,
                "attachments": r.get::<_, String>(17)?,
                "reasoning": r.get::<_, Option<String>>(18)?,
                "parent_message_id": r.get::<_, Option<String>>(19)?,
                "effective_at": r.get::<_, Option<i64>>(20)?,
            }))
        })?;
        for message in &mut messages {
            let message_id = string_field(message, "id")?;
            let variants = rows(connection, &variant_sql, params![message_id], |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?,
                    "content": r.get::<_, String>(1)?,
                    "created_at": r.get::<_, i64>(2)?,
                    "prompt_tokens": r.get::<_, Option<i64>>(3)?,
                    "completion_tokens": r.get::<_, Option<i64>>(4)?,
                    "total_tokens": r.get::<_, Option<i64>>(5)?,
                    "first_token_ms": r.get::<_, Option<i64>>(6)?,
                    "tokens_per_second": r.get::<_, Option<f64>>(7)?,
                    "mtp_stats": r.get::<_, Option<String>>(8)?,
                    "reasoning": r.get::<_, Option<String>>(9)?,
                }))
            })?;
            message["variants"] = Value::Array(variants);
        }
        session["messages"] = Value::Array(messages);
    }
    Ok(sessions)
}

fn group_sessions(connection: &Connection) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    let mut sessions = rows(
        connection,
        "SELECT id, group_character_id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived, chat_type, starting_scene, background_image_path, lorebook_ids, disable_character_lorebooks, author_note, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, memory_status, memory_error, memory_progress_step, speaker_selection_method, memory_type, config_overrides, parent_session_id, branched_from_message_id, root_session_id, character_model_overrides, group_chat_prompt_template_id, group_chat_roleplay_prompt_template_id FROM group_sessions",
        [],
        |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "group_character_id": r.get::<_, Option<String>>(1)?,
                "name": r.get::<_, String>(2)?,
                "character_ids": r.get::<_, String>(3)?,
                "muted_character_ids": r.get::<_, String>(4)?,
                "persona_id": r.get::<_, Option<String>>(5)?,
                "created_at": r.get::<_, i64>(6)?,
                "updated_at": r.get::<_, i64>(7)?,
                "archived": flag(r, 8)?,
                "chat_type": r.get::<_, String>(9)?,
                "starting_scene": r.get::<_, Option<String>>(10)?,
                "background_image_path": r.get::<_, Option<String>>(11)?,
                "lorebook_ids": r.get::<_, String>(12)?,
                "disable_character_lorebooks": flag(r, 13)?,
                "author_note": r.get::<_, Option<String>>(14)?,
                "memories": r.get::<_, String>(15)?,
                "memory_embeddings": r.get::<_, String>(16)?,
                "memory_summary": r.get::<_, String>(17)?,
                "memory_summary_token_count": r.get::<_, i64>(18)?,
                "memory_tool_events": r.get::<_, String>(19)?,
                "memory_status": r.get::<_, Option<String>>(20)?,
                "memory_error": r.get::<_, Option<String>>(21)?,
                "memory_progress_step": r.get::<_, Option<i64>>(22)?,
                "speaker_selection_method": r.get::<_, Option<String>>(23)?,
                "memory_type": r.get::<_, Option<String>>(24)?,
                "config_overrides": r.get::<_, Option<String>>(25)?.unwrap_or_else(|| "{\"version\":1}".to_owned()),
                "parent_session_id": r.get::<_, Option<String>>(26)?,
                "branched_from_message_id": r.get::<_, Option<String>>(27)?,
                "root_session_id": r.get::<_, Option<String>>(28)?,
                "character_model_overrides": r.get::<_, Option<String>>(29)?.unwrap_or_else(|| "{}".to_owned()),
                "group_chat_prompt_template_id": r.get::<_, Option<String>>(30)?,
                "group_chat_roleplay_prompt_template_id": r.get::<_, Option<String>>(31)?,
            }))
        },
    )?;
    let message_sql = format!(
        "SELECT {} FROM group_messages WHERE session_id = ?1 ORDER BY created_at ASC",
        projection(connection, "group_messages", GROUP_MESSAGE_COLUMNS)?
    );
    let variant_sql = format!(
        "SELECT {} FROM group_message_variants WHERE message_id = ?1 ORDER BY created_at ASC, rowid ASC",
        projection(
            connection,
            "group_message_variants",
            GROUP_MESSAGE_VARIANT_COLUMNS
        )?
    );
    for session in &mut sessions {
        let id = string_field(session, "id")?;
        let legacy = session["memory_embeddings"]
            .as_str()
            .unwrap_or("[]")
            .to_owned();
        session["memory_embeddings"] = Value::String(canonical_embeddings(
            connection,
            &id,
            "group_session",
            &legacy,
        )?);
        let participation = rows(
            connection,
            "SELECT id, character_id, speak_count, last_spoke_turn, last_spoke_at FROM group_participation WHERE session_id = ?1",
            params![id],
            |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?,
                    "character_id": r.get::<_, String>(1)?,
                    "speak_count": r.get::<_, i64>(2)?,
                    "last_spoke_turn": r.get::<_, Option<i64>>(3)?,
                    "last_spoke_at": r.get::<_, Option<i64>>(4)?,
                }))
            },
        )?;
        let mut messages = rows(connection, &message_sql, params![id], |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "role": r.get::<_, String>(1)?,
                "content": r.get::<_, String>(2)?,
                "speaker_character_id": r.get::<_, Option<String>>(3)?,
                "turn_number": r.get::<_, i64>(4)?,
                "created_at": r.get::<_, i64>(5)?,
                "prompt_tokens": r.get::<_, Option<i64>>(6)?,
                "completion_tokens": r.get::<_, Option<i64>>(7)?,
                "total_tokens": r.get::<_, Option<i64>>(8)?,
                "first_token_ms": r.get::<_, Option<i64>>(9)?,
                "tokens_per_second": r.get::<_, Option<f64>>(10)?,
                "mtp_stats": r.get::<_, Option<String>>(11)?,
                "selected_variant_id": r.get::<_, Option<String>>(12)?,
                "is_pinned": flag(r, 13)?,
                "attachments": r.get::<_, String>(14)?,
                "used_lorebook_entries": r.get::<_, String>(15)?,
                "memory_refs": r.get::<_, String>(16)?,
                "reasoning": r.get::<_, Option<String>>(17)?,
                "selection_reasoning": r.get::<_, Option<String>>(18)?,
                "model_id": r.get::<_, Option<String>>(19)?,
                "gemini_content": r.get::<_, Option<String>>(20)?,
                "usage_json": r.get::<_, Option<String>>(21)?,
                "parent_message_id": r.get::<_, Option<String>>(22)?,
            }))
        })?;
        for message in &mut messages {
            let message_id = string_field(message, "id")?;
            let variants = rows(connection, &variant_sql, params![message_id], |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?,
                    "content": r.get::<_, String>(1)?,
                    "speaker_character_id": r.get::<_, Option<String>>(2)?,
                    "created_at": r.get::<_, i64>(3)?,
                    "prompt_tokens": r.get::<_, Option<i64>>(4)?,
                    "completion_tokens": r.get::<_, Option<i64>>(5)?,
                    "total_tokens": r.get::<_, Option<i64>>(6)?,
                    "first_token_ms": r.get::<_, Option<i64>>(7)?,
                    "tokens_per_second": r.get::<_, Option<f64>>(8)?,
                    "mtp_stats": r.get::<_, Option<String>>(9)?,
                    "reasoning": r.get::<_, Option<String>>(10)?,
                    "selection_reasoning": r.get::<_, Option<String>>(11)?,
                    "model_id": r.get::<_, Option<String>>(12)?,
                    "attachments": r.get::<_, Option<String>>(13)?.unwrap_or_else(|| "[]".to_owned()),
                    "gemini_content": r.get::<_, Option<String>>(14)?,
                    "usage_json": r.get::<_, Option<String>>(15)?,
                }))
            })?;
            message["variants"] = Value::Array(variants);
        }
        session["participation"] = Value::Array(participation);
        session["messages"] = Value::Array(messages);
    }
    Ok(sessions)
}

fn group_characters(connection: &Connection) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    rows(
        connection,
        "SELECT id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived, chat_type, starting_scene, background_image_path, lorebook_ids, disable_character_lorebooks, chat_appearance, speaker_selection_method, memory_type, character_model_overrides, group_chat_prompt_template_id, group_chat_roleplay_prompt_template_id FROM group_characters ORDER BY updated_at DESC",
        [],
        |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "name": r.get::<_, String>(1)?,
                "character_ids": r.get::<_, String>(2)?,
                "muted_character_ids": r.get::<_, String>(3)?,
                "persona_id": r.get::<_, Option<String>>(4)?,
                "created_at": r.get::<_, i64>(5)?,
                "updated_at": r.get::<_, i64>(6)?,
                "archived": flag(r, 7)?,
                "chat_type": r.get::<_, String>(8)?,
                "starting_scene": r.get::<_, Option<String>>(9)?,
                "background_image_path": r.get::<_, Option<String>>(10)?,
                "lorebook_ids": r.get::<_, String>(11)?,
                "disable_character_lorebooks": flag(r, 12)?,
                "chat_appearance": r.get::<_, Option<String>>(13)?,
                "speaker_selection_method": r.get::<_, Option<String>>(14)?,
                "memory_type": r.get::<_, Option<String>>(15)?,
                "character_model_overrides": r.get::<_, Option<String>>(16)?,
                "group_chat_prompt_template_id": r.get::<_, Option<String>>(17)?,
                "group_chat_roleplay_prompt_template_id": r.get::<_, Option<String>>(18)?,
            }))
        },
    )
}

fn usage_records(connection: &Connection) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    let mut records = rows(
        connection,
        "SELECT id, timestamp, session_id, character_id, character_name, model_id, model_name, provider_id, provider_label, operation_type, finish_reason, prompt_tokens, completion_tokens, total_tokens, memory_tokens, summary_tokens, reasoning_tokens, image_tokens, prompt_cost, completion_cost, total_cost, success, error_message, audio_tokens FROM usage_records",
        [],
        |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "timestamp": r.get::<_, i64>(1)?,
                "session_id": r.get::<_, String>(2)?,
                "character_id": r.get::<_, String>(3)?,
                "character_name": r.get::<_, String>(4)?,
                "model_id": r.get::<_, String>(5)?,
                "model_name": r.get::<_, String>(6)?,
                "provider_id": r.get::<_, String>(7)?,
                "provider_label": r.get::<_, String>(8)?,
                "operation_type": r.get::<_, Option<String>>(9)?,
                "finish_reason": r.get::<_, Option<String>>(10)?,
                "prompt_tokens": r.get::<_, Option<i64>>(11)?,
                "completion_tokens": r.get::<_, Option<i64>>(12)?,
                "total_tokens": r.get::<_, Option<i64>>(13)?,
                "memory_tokens": r.get::<_, Option<i64>>(14)?,
                "summary_tokens": r.get::<_, Option<i64>>(15)?,
                "reasoning_tokens": r.get::<_, Option<i64>>(16)?,
                "image_tokens": r.get::<_, Option<i64>>(17)?,
                "prompt_cost": r.get::<_, Option<f64>>(18)?,
                "completion_cost": r.get::<_, Option<f64>>(19)?,
                "total_cost": r.get::<_, Option<f64>>(20)?,
                "success": flag(r, 21)?,
                "error_message": r.get::<_, Option<String>>(22)?,
                "audio_tokens": r.get::<_, Option<i64>>(23)?,
            }))
        },
    )?;
    for record in &mut records {
        let id = string_field(record, "id")?;
        let metadata = rows(
            connection,
            "SELECT key, value FROM usage_metadata WHERE usage_id = ?1",
            params![id],
            |r| {
                Ok(json!({
                    "key": r.get::<_, String>(0)?,
                    "value": r.get::<_, String>(1)?,
                }))
            },
        )?;
        record["metadata"] = Value::Array(metadata);
    }
    Ok(records)
}

fn playground_generations(
    connection: &Connection,
) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    rows(
        connection,
        "SELECT id, created_at, provider_id, model_id, model_name, prompt, negative_prompt, seed, params_json, status, error, images_json FROM playground_generations ORDER BY created_at, id",
        [],
        |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "created_at": r.get::<_, i64>(1)?,
                "provider_id": r.get::<_, String>(2)?,
                "model_id": r.get::<_, String>(3)?,
                "model_name": r.get::<_, String>(4)?,
                "prompt": r.get::<_, String>(5)?,
                "negative_prompt": r.get::<_, Option<String>>(6)?,
                "seed": r.get::<_, Option<i64>>(7)?,
                "params_json": r.get::<_, String>(8)?,
                "status": r.get::<_, String>(9)?,
                "error": r.get::<_, Option<String>>(10)?,
                "images_json": r.get::<_, String>(11)?,
            }))
        },
    )
}

/// Every column is read as SQLite stored it, so a value of an unexpected type
/// reaches the planner (which records it) instead of failing the import;
/// installs from before the `message_id` column have no message links.
fn llm_generation_metrics(
    connection: &Connection,
) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    let message_column = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('llm_generation_metrics') WHERE name = 'message_id')",
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    let sql = if message_column {
        "SELECT id, created_at, model_name, summary_json, samples_json, message_id FROM llm_generation_metrics ORDER BY created_at DESC, id DESC"
    } else {
        "SELECT id, created_at, model_name, summary_json, samples_json, NULL FROM llm_generation_metrics ORDER BY created_at DESC, id DESC"
    };
    rows(connection, sql, [], |r| {
        let mut row = Map::new();
        for (index, column) in [
            "id",
            "created_at",
            "model_name",
            "summary_json",
            "samples_json",
            "message_id",
        ]
        .into_iter()
        .enumerate()
        {
            row.insert(column.to_owned(), stored_value(r.get_ref(index)?));
        }
        Ok(Value::Object(row))
    })
}

fn stored_value(value: rusqlite::types::ValueRef<'_>) -> Value {
    match value {
        rusqlite::types::ValueRef::Integer(value) => Value::from(value),
        rusqlite::types::ValueRef::Real(value) => Value::from(value),
        rusqlite::types::ValueRef::Text(value) => {
            Value::String(String::from_utf8_lossy(value).into_owned())
        }
        rusqlite::types::ValueRef::Null | rusqlite::types::ValueRef::Blob(_) => Value::Null,
    }
}

fn image_loras(connection: &Connection) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    rows(
        connection,
        "SELECT path, filename, bytes_on_disk, modified_at, sha256, keywords, keyword_source, architecture, architecture_source, created_at, updated_at FROM image_loras ORDER BY path",
        [],
        |r| {
            Ok(json!({
                "path": r.get::<_, String>(0)?,
                "filename": r.get::<_, String>(1)?,
                "bytes_on_disk": r.get::<_, i64>(2)?,
                "modified_at": r.get::<_, i64>(3)?,
                "sha256": r.get::<_, Option<String>>(4)?,
                "keywords": r.get::<_, Option<String>>(5)?,
                "keyword_source": r.get::<_, Option<String>>(6)?,
                "architecture": r.get::<_, Option<String>>(7)?,
                "architecture_source": r.get::<_, Option<String>>(8)?,
                "created_at": r.get::<_, i64>(9)?,
                "updated_at": r.get::<_, i64>(10)?,
            }))
        },
    )
}

fn lorebooks(connection: &Connection) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    let mut lorebooks = rows(
        connection,
        "SELECT id, name, avatar_path, keyword_detection_mode, created_at, updated_at FROM lorebooks",
        [],
        |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "name": r.get::<_, String>(1)?,
                "avatar_path": r.get::<_, Option<String>>(2)?,
                "keyword_detection_mode": r.get::<_, String>(3)?,
                "created_at": r.get::<_, i64>(4)?,
                "updated_at": r.get::<_, i64>(5)?,
            }))
        },
    )?;
    for lorebook in &mut lorebooks {
        let id = string_field(lorebook, "id")?;
        let entries = rows(
            connection,
            "SELECT id, title, enabled, always_active, keywords, case_sensitive, keyword_match_mode, content, priority, display_order, created_at, updated_at FROM lorebook_entries WHERE lorebook_id = ?1 ORDER BY display_order ASC",
            params![id],
            |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?,
                    "title": r.get::<_, String>(1)?,
                    "enabled": flag(r, 2)?,
                    "always_active": flag(r, 3)?,
                    "keywords": r.get::<_, String>(4)?,
                    "case_sensitive": flag(r, 5)?,
                    "keyword_match_mode": r.get::<_, String>(6)?,
                    "content": r.get::<_, String>(7)?,
                    "priority": r.get::<_, i64>(8)?,
                    "display_order": r.get::<_, i64>(9)?,
                    "created_at": r.get::<_, i64>(10)?,
                    "updated_at": r.get::<_, i64>(11)?,
                }))
            },
        )?;
        lorebook["entries"] = Value::Array(entries);
    }
    Ok(lorebooks)
}

fn creation_helper_sessions(
    connection: &Connection,
) -> Result<Vec<Value>, LegacyDatabasePreflightError> {
    rows(
        connection,
        "SELECT id, creation_goal, status, session_json, uploaded_images_json, created_at, updated_at FROM creation_helper_sessions ORDER BY updated_at DESC",
        [],
        |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "creation_goal": r.get::<_, String>(1)?,
                "status": r.get::<_, String>(2)?,
                "session_json": r.get::<_, String>(3)?,
                "uploaded_images_json": r.get::<_, String>(4)?,
                "created_at": r.get::<_, i64>(5)?,
                "updated_at": r.get::<_, i64>(6)?,
            }))
        },
    )
}

fn asr_learning(connection: &Connection) -> Result<Value, LegacyDatabasePreflightError> {
    let vocabulary_terms = rows(
        connection,
        "SELECT term, normalized_term, language, category, scope, priority, use_count, created_at, updated_at FROM asr_vocabulary_terms",
        [],
        |r| {
            Ok(json!({
                "term": r.get::<_, String>(0)?,
                "normalized_term": r.get::<_, String>(1)?,
                "language": r.get::<_, Option<String>>(2)?,
                "category": r.get::<_, Option<String>>(3)?,
                "scope": r.get::<_, String>(4)?,
                "priority": r.get::<_, i64>(5)?,
                "use_count": r.get::<_, i64>(6)?,
                "created_at": r.get::<_, String>(7)?,
                "updated_at": r.get::<_, String>(8)?,
            }))
        },
    )?;
    let corrections = rows(
        connection,
        "SELECT wrong, normalized_wrong, correct, normalized_correct, language, scope, confidence, use_count, accepted_count, rejected_count, seen_count, last_seen_at, user_approved, created_at, updated_at FROM asr_corrections",
        [],
        |r| {
            Ok(json!({
                "wrong": r.get::<_, String>(0)?,
                "normalized_wrong": r.get::<_, String>(1)?,
                "correct": r.get::<_, String>(2)?,
                "normalized_correct": r.get::<_, String>(3)?,
                "language": r.get::<_, Option<String>>(4)?,
                "scope": r.get::<_, String>(5)?,
                "confidence": r.get::<_, f64>(6)?,
                "use_count": r.get::<_, i64>(7)?,
                "accepted_count": r.get::<_, i64>(8)?,
                "rejected_count": r.get::<_, i64>(9)?,
                "seen_count": r.get::<_, i64>(10)?,
                "last_seen_at": r.get::<_, Option<String>>(11)?,
                "user_approved": r.get::<_, i64>(12)?,
                "created_at": r.get::<_, String>(13)?,
                "updated_at": r.get::<_, String>(14)?,
            }))
        },
    )?;
    let ignored_suggestions = rows(
        connection,
        "SELECT wrong, normalized_wrong, correct, normalized_correct, language, scope, ignored_count, last_ignored_at, created_at, updated_at FROM asr_ignored_suggestions",
        [],
        |r| {
            Ok(json!({
                "wrong": r.get::<_, String>(0)?,
                "normalized_wrong": r.get::<_, String>(1)?,
                "correct": r.get::<_, String>(2)?,
                "normalized_correct": r.get::<_, String>(3)?,
                "language": r.get::<_, Option<String>>(4)?,
                "scope": r.get::<_, String>(5)?,
                "ignored_count": r.get::<_, i64>(6)?,
                "last_ignored_at": r.get::<_, String>(7)?,
                "created_at": r.get::<_, String>(8)?,
                "updated_at": r.get::<_, String>(9)?,
            }))
        },
    )?;
    Ok(json!({
        "vocabularyTerms": vocabulary_terms,
        "corrections": corrections,
        "ignoredSuggestions": ignored_suggestions,
    }))
}

fn string_field(value: &Value, key: &str) -> Result<String, LegacyDatabasePreflightError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(LegacyDatabasePreflightError::InvalidSchema)
}

fn table_exists(
    connection: &Connection,
    table: &str,
) -> Result<bool, LegacyDatabasePreflightError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)
}

#[cfg(test)]
mod tests {
    use lettuce_transfer::{LegacyBackupInventory, plan_legacy_backup_compatibility};
    use lettuce_types::{ContentHash, MediaBlobId};

    use super::*;

    const SCHEMA: &str = "
        CREATE TABLE settings (id INTEGER PRIMARY KEY, default_provider_credential_id TEXT, default_model_id TEXT, app_state TEXT NOT NULL, advanced_model_settings TEXT, prompt_template_id TEXT, system_prompt TEXT, migration_version INTEGER NOT NULL, advanced_settings TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
        CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);
        CREATE TABLE provider_credentials (id TEXT PRIMARY KEY, provider_id TEXT NOT NULL, label TEXT NOT NULL, api_key_ref TEXT, api_key TEXT, base_url TEXT, default_model TEXT, headers TEXT, config TEXT);
        CREATE TABLE models (id TEXT PRIMARY KEY, name TEXT NOT NULL, provider_id TEXT NOT NULL, provider_credential_id TEXT, provider_label TEXT NOT NULL, display_name TEXT NOT NULL, created_at INTEGER NOT NULL, model_type TEXT, input_scopes TEXT, output_scopes TEXT, advanced_model_settings TEXT, prompt_template_id TEXT, system_prompt TEXT);
        CREATE TABLE audio_providers (id TEXT PRIMARY KEY, provider_type TEXT NOT NULL, label TEXT NOT NULL, api_key TEXT, project_id TEXT, location TEXT, base_url TEXT, request_path TEXT, kokoro_variant TEXT, asset_root TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
        CREATE TABLE user_voices (id TEXT PRIMARY KEY, provider_id TEXT NOT NULL, name TEXT NOT NULL, model_id TEXT NOT NULL, voice_id TEXT NOT NULL, prompt TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
        CREATE TABLE model_pricing_cache (model_id TEXT PRIMARY KEY, pricing_json TEXT, cached_at INTEGER NOT NULL);
        CREATE TABLE secrets (service TEXT NOT NULL, account TEXT NOT NULL, value TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
        CREATE TABLE prompt_templates (id TEXT PRIMARY KEY, name TEXT NOT NULL, prompt_type TEXT NOT NULL, content TEXT NOT NULL, entries TEXT NOT NULL, condense_prompt_entries INTEGER NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
        CREATE TABLE personas (id TEXT PRIMARY KEY, title TEXT NOT NULL, description TEXT NOT NULL, nickname TEXT, avatar_path TEXT, avatar_crop_x REAL, avatar_crop_y REAL, avatar_crop_scale REAL, design_description TEXT, design_reference_image_ids TEXT, lora_name TEXT, lora_strength REAL, active_lorebook_ids TEXT, is_default INTEGER NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
        CREATE TABLE characters (id TEXT PRIMARY KEY, name TEXT NOT NULL, avatar_path TEXT, avatar_crop_x REAL, avatar_crop_y REAL, avatar_crop_scale REAL, banner_crop_x REAL, banner_crop_y REAL, banner_crop_scale REAL, card_type TEXT, design_description TEXT, design_reference_image_ids TEXT, lora_name TEXT, lora_strength REAL, background_image_path TEXT, description TEXT, definition TEXT, nickname TEXT, scenario TEXT, creator_notes TEXT, creator TEXT, creator_notes_multilingual TEXT, source TEXT, tags TEXT, default_scene_id TEXT, default_model_id TEXT, mode TEXT, companion TEXT, memory_type TEXT NOT NULL, active_lorebook_ids TEXT, prompt_template_id TEXT, group_chat_prompt_template_id TEXT, group_chat_roleplay_prompt_template_id TEXT, system_prompt TEXT, voice_config TEXT, voice_autoplay INTEGER, disable_avatar_gradient INTEGER NOT NULL, avatar_gradient_source TEXT, custom_gradient_enabled INTEGER NOT NULL, custom_gradient_colors TEXT, custom_text_color TEXT, custom_text_secondary TEXT, chat_appearance TEXT, default_chat_template_id TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
        CREATE TABLE character_rules (character_id TEXT NOT NULL, idx INTEGER NOT NULL, rule TEXT NOT NULL);
        CREATE TABLE scenes (id TEXT PRIMARY KEY, character_id TEXT NOT NULL, content TEXT NOT NULL, direction TEXT, background_image_path TEXT, created_at INTEGER NOT NULL, selected_variant_id TEXT);
        CREATE TABLE scene_variants (id TEXT PRIMARY KEY, scene_id TEXT NOT NULL, content TEXT NOT NULL, direction TEXT, created_at INTEGER NOT NULL);
        CREATE TABLE chat_templates (id TEXT PRIMARY KEY, character_id TEXT NOT NULL, name TEXT NOT NULL, scene_id TEXT, prompt_template_id TEXT, lorebook_ids_override TEXT, created_at INTEGER NOT NULL);
        CREATE TABLE chat_template_messages (id TEXT PRIMARY KEY, template_id TEXT NOT NULL, idx INTEGER NOT NULL, role TEXT NOT NULL, content TEXT NOT NULL);
        CREATE TABLE companion_scheduled_notes (id TEXT PRIMARY KEY, character_id TEXT NOT NULL, label TEXT NOT NULL, content TEXT NOT NULL, available_at INTEGER NOT NULL, expires_at INTEGER, recurrence TEXT NOT NULL, recurrence_window_ms INTEGER, enabled INTEGER NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
        CREATE TABLE companion_shared_memory_state (character_id TEXT PRIMARY KEY, memories TEXT NOT NULL, memory_summary TEXT, memory_summary_token_count INTEGER NOT NULL, memory_tool_events TEXT NOT NULL, memory_status TEXT, memory_error TEXT, memory_progress_step INTEGER, soul_growth TEXT NOT NULL, relationship_states TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
        CREATE TABLE memory_embeddings (session_id TEXT NOT NULL, session_kind TEXT NOT NULL, memory_id TEXT NOT NULL, embedding BLOB NOT NULL, embedding_dim INTEGER NOT NULL, embedding_model TEXT, text TEXT NOT NULL, token_count INTEGER NOT NULL, category TEXT, importance_score REAL NOT NULL, persistence_importance REAL NOT NULL, prompt_importance REAL NOT NULL, volatility REAL NOT NULL, is_cold INTEGER NOT NULL, is_pinned INTEGER NOT NULL, access_count INTEGER NOT NULL, fact_signature TEXT, fact_polarity INTEGER, source_role TEXT, source_message_id TEXT, superseded_by TEXT, superseded_at INTEGER, supersedes_json TEXT, canonical_entities_json TEXT, observed_at INTEGER, observed_time_precision TEXT, created_at INTEGER NOT NULL, last_accessed_at INTEGER NOT NULL);
        CREATE TABLE sessions (id TEXT PRIMARY KEY, character_id TEXT NOT NULL, title TEXT NOT NULL, parent_session_id TEXT, branched_from_message_id TEXT, root_session_id TEXT, background_image_path TEXT, system_prompt TEXT, mode TEXT NOT NULL, selected_scene_id TEXT, author_note TEXT, persona_id TEXT, persona_disabled INTEGER NOT NULL, voice_autoplay INTEGER, prompt_template_id TEXT, lorebook_ids_override TEXT, temperature REAL, top_p REAL, max_output_tokens INTEGER, frequency_penalty REAL, presence_penalty REAL, top_k INTEGER, advanced_model_settings TEXT, companion_state TEXT, memories TEXT NOT NULL, memory_embeddings TEXT NOT NULL, memory_summary TEXT, memory_summary_token_count INTEGER NOT NULL, memory_tool_events TEXT NOT NULL, memory_status TEXT, memory_error TEXT, memory_progress_step INTEGER, archived INTEGER NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
        CREATE TABLE messages (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, role TEXT NOT NULL, content TEXT NOT NULL, created_at INTEGER NOT NULL, visible_in_chat INTEGER NOT NULL, scene_edited INTEGER NOT NULL, prompt_tokens INTEGER, completion_tokens INTEGER, total_tokens INTEGER, first_token_ms INTEGER, tokens_per_second REAL, mtp_stats TEXT, model_id TEXT, selected_variant_id TEXT, is_pinned INTEGER NOT NULL, memory_refs TEXT NOT NULL, used_lorebook_entries TEXT NOT NULL, attachments TEXT NOT NULL, reasoning TEXT, parent_message_id TEXT, effective_at INTEGER);
        CREATE TABLE message_variants (id TEXT PRIMARY KEY, message_id TEXT NOT NULL, content TEXT NOT NULL, created_at INTEGER NOT NULL, prompt_tokens INTEGER, completion_tokens INTEGER, total_tokens INTEGER, first_token_ms INTEGER, tokens_per_second REAL, mtp_stats TEXT, reasoning TEXT);
        CREATE TABLE group_characters (id TEXT PRIMARY KEY, name TEXT NOT NULL, character_ids TEXT NOT NULL, muted_character_ids TEXT NOT NULL, persona_id TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, archived INTEGER NOT NULL, chat_type TEXT NOT NULL, starting_scene TEXT, background_image_path TEXT, lorebook_ids TEXT NOT NULL, disable_character_lorebooks INTEGER NOT NULL, chat_appearance TEXT, speaker_selection_method TEXT NOT NULL DEFAULT 'llm', memory_type TEXT NOT NULL DEFAULT 'manual', character_model_overrides TEXT, group_chat_prompt_template_id TEXT, group_chat_roleplay_prompt_template_id TEXT);
        CREATE TABLE group_sessions (id TEXT PRIMARY KEY, group_character_id TEXT, name TEXT NOT NULL, character_ids TEXT NOT NULL, muted_character_ids TEXT NOT NULL, persona_id TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, archived INTEGER NOT NULL, chat_type TEXT NOT NULL, starting_scene TEXT, background_image_path TEXT, lorebook_ids TEXT NOT NULL, disable_character_lorebooks INTEGER NOT NULL, author_note TEXT, memories TEXT NOT NULL, memory_embeddings TEXT NOT NULL, memory_summary TEXT NOT NULL, memory_summary_token_count INTEGER NOT NULL, memory_tool_events TEXT NOT NULL, memory_status TEXT, memory_error TEXT, memory_progress_step INTEGER, speaker_selection_method TEXT NOT NULL DEFAULT 'llm', memory_type TEXT NOT NULL DEFAULT 'manual', config_overrides TEXT, parent_session_id TEXT, branched_from_message_id TEXT, root_session_id TEXT, character_model_overrides TEXT, group_chat_prompt_template_id TEXT, group_chat_roleplay_prompt_template_id TEXT);
        CREATE TABLE group_participation (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, character_id TEXT NOT NULL, speak_count INTEGER NOT NULL, last_spoke_turn INTEGER, last_spoke_at INTEGER);
        CREATE TABLE group_messages (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, role TEXT NOT NULL, content TEXT NOT NULL, speaker_character_id TEXT, turn_number INTEGER NOT NULL, created_at INTEGER NOT NULL, prompt_tokens INTEGER, completion_tokens INTEGER, total_tokens INTEGER, first_token_ms INTEGER, tokens_per_second REAL, mtp_stats TEXT, selected_variant_id TEXT, is_pinned INTEGER NOT NULL, attachments TEXT NOT NULL, used_lorebook_entries TEXT NOT NULL, memory_refs TEXT NOT NULL, reasoning TEXT, selection_reasoning TEXT, model_id TEXT, gemini_content TEXT, usage_json TEXT, parent_message_id TEXT);
        CREATE TABLE group_message_variants (id TEXT PRIMARY KEY, message_id TEXT NOT NULL, content TEXT NOT NULL, speaker_character_id TEXT, created_at INTEGER NOT NULL, prompt_tokens INTEGER, completion_tokens INTEGER, total_tokens INTEGER, first_token_ms INTEGER, tokens_per_second REAL, mtp_stats TEXT, reasoning TEXT, selection_reasoning TEXT, model_id TEXT, attachments TEXT, gemini_content TEXT, usage_json TEXT);
        CREATE TABLE usage_records (id TEXT PRIMARY KEY, timestamp INTEGER NOT NULL, session_id TEXT NOT NULL, character_id TEXT NOT NULL, character_name TEXT NOT NULL, model_id TEXT NOT NULL, model_name TEXT NOT NULL, provider_id TEXT NOT NULL, provider_label TEXT NOT NULL, operation_type TEXT, finish_reason TEXT, prompt_tokens INTEGER, completion_tokens INTEGER, total_tokens INTEGER, memory_tokens INTEGER, summary_tokens INTEGER, reasoning_tokens INTEGER, image_tokens INTEGER, prompt_cost REAL, completion_cost REAL, total_cost REAL, success INTEGER NOT NULL, error_message TEXT, audio_tokens INTEGER);
        CREATE TABLE usage_metadata (usage_id TEXT NOT NULL, key TEXT NOT NULL, value TEXT NOT NULL);
        CREATE TABLE lorebooks (id TEXT PRIMARY KEY, name TEXT NOT NULL, avatar_path TEXT, keyword_detection_mode TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
        CREATE TABLE lorebook_entries (id TEXT PRIMARY KEY, lorebook_id TEXT NOT NULL, title TEXT NOT NULL, enabled INTEGER NOT NULL, always_active INTEGER NOT NULL, keywords TEXT NOT NULL, case_sensitive INTEGER NOT NULL, keyword_match_mode TEXT NOT NULL, content TEXT NOT NULL, priority INTEGER NOT NULL, display_order INTEGER NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
        CREATE TABLE creation_helper_sessions (id TEXT PRIMARY KEY, creation_goal TEXT NOT NULL, status TEXT NOT NULL, session_json TEXT NOT NULL, uploaded_images_json TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
        CREATE TABLE asr_vocabulary_terms (id INTEGER PRIMARY KEY, term TEXT NOT NULL, normalized_term TEXT NOT NULL, language TEXT, category TEXT, scope TEXT NOT NULL, priority INTEGER NOT NULL, use_count INTEGER NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
        CREATE TABLE asr_corrections (id INTEGER PRIMARY KEY, wrong TEXT NOT NULL, normalized_wrong TEXT NOT NULL, correct TEXT NOT NULL, normalized_correct TEXT NOT NULL, language TEXT, scope TEXT NOT NULL, confidence REAL NOT NULL, use_count INTEGER NOT NULL, accepted_count INTEGER NOT NULL, rejected_count INTEGER NOT NULL, seen_count INTEGER NOT NULL, last_seen_at TEXT, user_approved INTEGER NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
        CREATE TABLE asr_ignored_suggestions (id INTEGER PRIMARY KEY, wrong TEXT NOT NULL, normalized_wrong TEXT NOT NULL, correct TEXT NOT NULL, normalized_correct TEXT NOT NULL, language TEXT, scope TEXT NOT NULL, ignored_count INTEGER NOT NULL, last_ignored_at TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
        CREATE TABLE asr_voice_examples (id INTEGER PRIMARY KEY, audio_path TEXT NOT NULL, expected_text TEXT NOT NULL, normalized_expected_text TEXT NOT NULL, whisper_output TEXT, normalized_whisper_output TEXT, language TEXT, scope TEXT NOT NULL, term_id INTEGER, correction_id INTEGER, created_at TEXT NOT NULL);
        CREATE TABLE image_loras (path TEXT PRIMARY KEY, filename TEXT NOT NULL, bytes_on_disk INTEGER NOT NULL DEFAULT 0, modified_at INTEGER NOT NULL DEFAULT 0, sha256 TEXT, keywords TEXT NOT NULL DEFAULT '[]', keyword_source TEXT NOT NULL DEFAULT 'none', architecture TEXT, architecture_source TEXT NOT NULL DEFAULT 'none', created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
        INSERT INTO image_loras (path, filename, bytes_on_disk, modified_at, keywords, keyword_source, created_at, updated_at) VALUES ('/loras/ink.safetensors', 'ink.safetensors', 64, 9, '[\"ink\"]', 'manual', 1, 2);
        CREATE TABLE llm_generation_metrics (id TEXT PRIMARY KEY, created_at INTEGER NOT NULL, model_name TEXT, summary_json TEXT NOT NULL, samples_json TEXT NOT NULL DEFAULT '[]', message_id TEXT);
        INSERT INTO llm_generation_metrics VALUES ('gen-1', 5, '/models/a.gguf', '{\"completionTokens\":3}', '[]', 'message-1'), ('gen-2', 6, NULL, 'not json', '[1]', NULL);
    ";

    fn id(value: u128) -> String {
        uuid::Uuid::from_u128(value).to_string()
    }

    fn legacy_database() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "lettuce-legacy-documents-{}.db",
            MediaBlobId::new()
        ));
        let connection = Connection::open(&path).expect("create legacy database");
        connection.execute_batch(SCHEMA).expect("legacy schema");
        let character = id(1);
        let session = id(2);
        let group = id(3);
        let group_session = id(4);
        let lorebook = id(5);
        let embedding = [0.5_f32; 64]
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        connection
            .execute(
                "INSERT INTO settings VALUES (1, NULL, NULL, 'not json', NULL, NULL, NULL, 92, NULL, 1, 1)",
                [],
            )
            .expect("settings");
        connection
            .execute("INSERT INTO meta VALUES ('schema_version', '92')", [])
            .expect("meta");
        connection
            .execute(
                "INSERT INTO lorebooks VALUES (?1, 'World', NULL, 'recent_message_window', 1, 1)",
                [&lorebook],
            )
            .expect("lorebook");
        for (entry, order) in [(id(6), 2), (id(7), 1)] {
            connection
                .execute(
                    "INSERT INTO lorebook_entries VALUES (?1, ?2, 'Entry', 1, 0, '[\"castle\"]', 0, 'literal', 'A castle', 0, ?3, 1, 1)",
                    params![entry, lorebook, order],
                )
                .expect("lorebook entry");
        }
        let provider = id(10);
        connection
            .execute(
                "INSERT INTO provider_credentials VALUES (?1, 'openai', 'OpenAI', NULL, 'sk-test', NULL, NULL, NULL, '{}')",
                [&provider],
            )
            .expect("provider");
        connection
            .execute(
                "INSERT INTO models VALUES (?1, 'gpt-4o', 'openai', ?2, 'OpenAI', 'GPT-4o', 1, 'chat', '[\"text\",\"image\"]', '[\"text\"]', NULL, NULL, NULL)",
                [id(11), provider.clone()],
            )
            .expect("model");
        connection
            .execute(
                "INSERT INTO prompt_templates VALUES (?1, 'Narrator', 'directChat', 'Stay in character', '[]', 0, 1, 1)",
                [id(12)],
            )
            .expect("prompt");
        connection
            .execute(
                "INSERT INTO personas (id, title, description, active_lorebook_ids, is_default, created_at, updated_at) VALUES (?1, 'Reader', 'Reads stories', ?2, 1, 1, 1)",
                [id(13), format!("[\"{lorebook}\"]")],
            )
            .expect("persona");
        connection
            .execute(
                "INSERT INTO characters (id, name, memory_type, voice_autoplay, disable_avatar_gradient, custom_gradient_enabled, created_at, updated_at) VALUES (?1, 'Ada', 'manual', NULL, 0, 0, 1, 1), (?2, 'Grace', 'manual', 1, 0, 0, 1, 1)",
                [character.clone(), id(9)],
            )
            .expect("character");
        for (index, rule) in [(1, "Second"), (0, "First")] {
            connection
                .execute(
                    "INSERT INTO character_rules VALUES (?1, ?2, ?3)",
                    params![character, index, rule],
                )
                .expect("rule");
        }
        connection
            .execute(
                "INSERT INTO sessions (id, character_id, title, mode, persona_disabled, memories, memory_embeddings, memory_summary_token_count, memory_tool_events, archived, created_at, updated_at) VALUES (?1, ?2, 'Chat', 'roleplay', 0, '[]', '[{\"id\":\"stale\"}]', 0, '[]', 0, 1, 1)",
                [&session, &character],
            )
            .expect("session");
        connection
            .execute(
                "INSERT INTO messages (id, session_id, role, content, created_at, visible_in_chat, scene_edited, is_pinned, memory_refs, used_lorebook_entries, attachments) VALUES (?1, ?2, 'user', 'Hello', 5, 1, 0, 0, '[]', '[]', '[]')",
                [id(8), session.clone()],
            )
            .expect("message");
        connection
            .execute(
                "INSERT INTO memory_embeddings (session_id, session_kind, memory_id, embedding, embedding_dim, embedding_model, text, token_count, importance_score, persistence_importance, prompt_importance, volatility, is_cold, is_pinned, access_count, supersedes_json, canonical_entities_json, created_at, last_accessed_at) VALUES (?1, 'session', 'memory-1', ?2, 2, 'lettuce-v4', 'Ada likes castles', 4, 1.0, 1.0, 1.0, 0.1, 0, 1, 2, 'not json', '[{\"label\":\"place\",\"surface\":\"castle\",\"canonicalKey\":\"castle\",\"canonicalName\":\"Castle\"}]', 3, 4)",
                params![session, embedding],
            )
            .expect("memory embedding");
        connection
            .execute(
                "INSERT INTO group_characters (id, name, character_ids, muted_character_ids, created_at, updated_at, archived, chat_type, lorebook_ids, disable_character_lorebooks) VALUES (?1, 'Room', ?2, '[]', 1, 1, 0, 'conversation', '[]', 0)",
                params![group, serde_json::to_string(&[&character, &id(9)]).expect("members")],
            )
            .expect("group");
        connection
            .execute(
                "INSERT INTO group_sessions (id, group_character_id, name, character_ids, muted_character_ids, created_at, updated_at, archived, chat_type, lorebook_ids, disable_character_lorebooks, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events) VALUES (?1, ?2, 'Room chat', ?3, '[]', 1, 1, 0, 'conversation', '[]', 0, '[]', '[]', '', 0, '[]')",
                params![group_session, group, serde_json::to_string(&[&character, &id(9)]).expect("members")],
            )
            .expect("group session");
        drop(connection);
        path
    }

    fn document_value(documents: &[LegacyBackupDocument], kind: LegacyBackupDocumentKind) -> Value {
        let document = documents
            .iter()
            .find(|document| document.kind == kind)
            .expect("document");
        serde_json::from_slice(&document.bytes).expect("document json")
    }

    #[test]
    fn legacy_database_reads_into_backup_documents_the_backup_planner_accepts() {
        let path = legacy_database();

        let documents = read_legacy_database_documents(&path).expect("legacy documents");

        assert_eq!(documents.len(), 24);
        let metrics = document_value(&documents, LegacyBackupDocumentKind::LlmGenerationMetrics);
        assert_eq!(metrics[0]["id"], "gen-2");
        assert_eq!(metrics[1]["message_id"], "message-1");
        let loras = document_value(&documents, LegacyBackupDocumentKind::ImageLoras);
        assert_eq!(loras[0]["keywords"], "[\"ink\"]");
        assert_eq!(loras[0]["keyword_source"], "manual");
        let settings = document_value(&documents, LegacyBackupDocumentKind::Settings);
        assert_eq!(settings["app_state"], json!("not json"));
        let lorebooks = document_value(&documents, LegacyBackupDocumentKind::Lorebooks);
        assert_eq!(lorebooks[0]["entries"][0]["display_order"], 1);
        let characters = document_value(&documents, LegacyBackupDocumentKind::Characters);
        assert_eq!(characters[0]["rules"], json!(["First", "Second"]));
        assert_eq!(characters[0]["voice_autoplay"], false);
        assert_eq!(characters[0]["card_type"], "circle");
        let sessions = document_value(&documents, LegacyBackupDocumentKind::Sessions);
        let memories: Value = serde_json::from_str(
            sessions[0]["memory_embeddings"]
                .as_str()
                .expect("canonical memories"),
        )
        .expect("memory json");
        assert_eq!(memories[0]["embedding"][63], 0.5);
        assert_eq!(memories[0]["embeddingDimensions"], 64);
        let memory_text = sessions[0]["memory_embeddings"]
            .as_str()
            .expect("memory text");
        assert!(memory_text.starts_with(
            "[{\"id\":\"memory-1\",\"text\":\"Ada likes castles\",\"embedding\":[0.5,"
        ));
        assert!(memory_text.contains("\"volatility\":0.1,\"isPinned\":true"));
        assert_eq!(memories[0]["embeddingSourceVersion"], "lettuce-v4");
        assert_eq!(memories[0]["canonicalEntities"][0]["confidence"], 0.0);
        assert!(memories[0].get("supersedes").is_none());
        let owners = document_value(&documents, LegacyBackupDocumentKind::MemoryEmbeddings);
        assert_eq!(
            owners[0]["memory_embeddings"],
            sessions[0]["memory_embeddings"]
        );
        let group_sessions = document_value(&documents, LegacyBackupDocumentKind::GroupSessions);
        assert_eq!(group_sessions[0]["memory_embeddings"], "[]");
        assert_eq!(group_sessions[0]["config_overrides"], "{\"version\":1}");
        assert_eq!(group_sessions[0]["character_model_overrides"], "{}");

        let plan = plan_legacy_backup_compatibility(LegacyBackupInventory {
            version: 1,
            created_at: 1,
            app_version: "legacy-database".into(),
            source_hash: ContentHash::parse("11".repeat(32)).expect("source hash"),
            documents,
            media: Vec::new(),
        })
        .expect("compatibility plan");
        assert_eq!(plan.coverage.present_document_count, 24);
        assert_eq!(plan.images.loras[0].keywords, vec!["ink".to_owned()]);
        assert_eq!(plan.llm_metrics.metrics.len(), 2);
        assert_eq!(plan.llm_metrics.metrics[0].summary_json, "{}");
        assert_eq!(
            plan.llm_metrics.metrics[1].message_source_id.as_deref(),
            Some("message-1")
        );
        std::fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn optional_message_columns_an_older_install_lacks_read_as_legacy_defaults() {
        let path = legacy_database();
        let connection = Connection::open(&path).expect("open legacy database");
        connection
            .execute_batch(
                "ALTER TABLE messages DROP COLUMN mtp_stats;
                 ALTER TABLE messages DROP COLUMN model_id;
                 ALTER TABLE messages DROP COLUMN first_token_ms;
                 ALTER TABLE message_variants DROP COLUMN tokens_per_second;
                 ALTER TABLE message_variants DROP COLUMN mtp_stats;
                 ALTER TABLE group_messages DROP COLUMN usage_json;
                 ALTER TABLE group_messages DROP COLUMN gemini_content;
                 ALTER TABLE group_messages DROP COLUMN model_id;
                 ALTER TABLE group_message_variants DROP COLUMN attachments;
                 ALTER TABLE group_message_variants DROP COLUMN usage_json;",
            )
            .expect("drift the schema");
        connection
            .execute(
                "INSERT INTO message_variants (id, message_id, content, created_at) VALUES ('variant-late', ?1, 'Late', 20), ('variant-early', ?1, 'Early', 10)",
                [id(8)],
            )
            .expect("variants");
        drop(connection);

        let documents = read_legacy_database_documents(&path).expect("legacy documents");

        let sessions = document_value(&documents, LegacyBackupDocumentKind::Sessions);
        let message = &sessions[0]["messages"][0];
        assert_eq!(message["mtp_stats"], Value::Null);
        assert_eq!(message["model_id"], Value::Null);
        assert_eq!(message["first_token_ms"], Value::Null);
        let variants = message["variants"]
            .as_array()
            .expect("variants")
            .iter()
            .map(|variant| variant["id"].as_str().expect("variant id"))
            .collect::<Vec<_>>();
        assert_eq!(variants, vec!["variant-early", "variant-late"]);
        assert_eq!(message["variants"][0]["tokens_per_second"], Value::Null);
        std::fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn companion_episodes_without_a_shared_memory_row_are_kept_under_a_default_state() {
        let path = legacy_database();
        let connection = Connection::open(&path).expect("open legacy database");
        connection
            .execute_batch(
                "CREATE TABLE companion_episodes (session_id TEXT PRIMARY KEY, character_id TEXT NOT NULL, persona_key TEXT NOT NULL DEFAULT '__default__', episode_index INTEGER NOT NULL, previous_session_id TEXT, started_at INTEGER NOT NULL, ended_at INTEGER, updated_at INTEGER NOT NULL);",
            )
            .expect("episodes table");
        connection
            .execute(
                "UPDATE characters SET mode = 'companion' WHERE id = ?1",
                [id(1)],
            )
            .expect("companion character");
        connection
            .execute(
                "INSERT INTO companion_episodes (session_id, character_id, episode_index, started_at, updated_at) VALUES (?1, ?2, 1, 7, 9)",
                [id(2), id(1)],
            )
            .expect("episode");
        drop(connection);

        let documents = read_legacy_database_documents(&path).expect("legacy documents");

        let states = document_value(&documents, LegacyBackupDocumentKind::CompanionSharedMemory);
        let state = states
            .as_array()
            .expect("states")
            .iter()
            .find(|state| state["character_id"] == id(1))
            .expect("default state for the episode owner");
        assert_eq!(state["memories"], "[]");
        assert_eq!(state["relationship_states"], "{}");
        assert_eq!(state["created_at"], 7);
        assert_eq!(state["updated_at"], 9);
        assert_eq!(state["episodes"][0]["session_id"], id(2));
        std::fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn metrics_of_installs_without_the_message_column_or_the_table_are_read_as_such() {
        let path = legacy_database();
        let connection = Connection::open(&path).expect("open legacy database");
        connection
            .execute_batch(
                "DROP TABLE llm_generation_metrics;
                 CREATE TABLE llm_generation_metrics (id TEXT PRIMARY KEY, created_at INTEGER NOT NULL, model_name TEXT, summary_json TEXT NOT NULL, samples_json TEXT NOT NULL DEFAULT '[]');
                 INSERT INTO llm_generation_metrics VALUES ('gen-1', 'soon', 3, X'00', '[]');",
            )
            .expect("older metrics table");
        let documents = read_legacy_database_documents(&path).expect("legacy documents");
        let metrics = document_value(&documents, LegacyBackupDocumentKind::LlmGenerationMetrics);
        assert_eq!(
            metrics,
            json!([{
                "id": "gen-1",
                "created_at": "soon",
                "model_name": "3",
                "summary_json": null,
                "samples_json": "[]",
                "message_id": null
            }])
        );
        connection
            .execute_batch("DROP TABLE llm_generation_metrics")
            .expect("drop metrics table");
        drop(connection);
        let documents = read_legacy_database_documents(&path).expect("legacy documents");
        assert!(
            documents
                .iter()
                .all(|document| document.kind != LegacyBackupDocumentKind::LlmGenerationMetrics)
        );
        std::fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn backup_chain_import_plan_matches_the_sqlite_planners_for_the_same_database() {
        let path = legacy_database();
        let documents = read_legacy_database_documents(&path).expect("legacy documents");
        let plan = plan_legacy_backup_compatibility(LegacyBackupInventory {
            version: 1,
            created_at: 1,
            app_version: "legacy-database".into(),
            source_hash: ContentHash::parse("11".repeat(32)).expect("source hash"),
            documents,
            media: Vec::new(),
        })
        .expect("compatibility plan");

        let import = plan.legacy_import_plan();

        assert_eq!(
            import.provider_models,
            crate::plan_legacy_provider_models(&path).expect("provider plan")
        );
        assert_eq!(
            import.prompts,
            crate::plan_legacy_prompts(&path).expect("prompt plan")
        );
        assert_eq!(
            import.personas,
            crate::plan_legacy_personas(&path).expect("persona plan")
        );
        assert_eq!(
            import.lorebooks,
            crate::plan_legacy_lorebooks(&path).expect("lorebook plan")
        );
        std::fs::remove_file(path).expect("remove fixture");
    }
}
