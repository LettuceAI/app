use rusqlite::params;

use crate::storage_manager::db::DbConnection;
use crate::sync::models::{
    AudioProvider, AudioVoiceCache, Character, CharacterLorebookLink, CharacterRule, ChatTemplate,
    ChatTemplateMessage, CreationHelperSession, GroupCharacter, GroupMessage, GroupMessageVariant,
    GroupParticipation, GroupSession, Message, MessageVariant, MetaEntry, Model, ModelPricingCache,
    Persona, PromptTemplate, ProviderCredential, Scene, SceneVariant, Secret, Session, Settings,
    SyncLorebook, SyncLorebookEntry, UsageMetadata, UsageRecord, UserVoice,
};
use crate::sync::protocol::{
    DomainManifest, Manifest, ManifestV2, SyncDomain, SyncLayer, SyncManifest,
};

#[derive(serde::Serialize, serde::Deserialize)]
struct CoreSnapshot {
    meta: Vec<MetaEntry>,
    settings: Vec<Settings>,
    personas: Vec<Persona>,
    models: Vec<Model>,
    secrets: Vec<Secret>,
    provider_credentials: Vec<ProviderCredential>,
    prompt_templates: Vec<PromptTemplate>,
    model_pricing_cache: Vec<ModelPricingCache>,
    creation_helper_sessions: Vec<CreationHelperSession>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct TtsSnapshot {
    audio_providers: Vec<AudioProvider>,
    audio_voice_cache: Vec<AudioVoiceCache>,
    user_voices: Vec<UserVoice>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LorebooksSnapshot {
    lorebooks: Vec<SyncLorebook>,
    entries: Vec<SyncLorebookEntry>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CharactersSnapshot {
    characters: Vec<Character>,
    rules: Vec<CharacterRule>,
    scenes: Vec<Scene>,
    scene_variants: Vec<SceneVariant>,
    character_lorebooks: Vec<CharacterLorebookLink>,
    chat_templates: Vec<ChatTemplate>,
    chat_template_messages: Vec<ChatTemplateMessage>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct SyncGroupConfigRecord {
    id: String,
    name: String,
    character_ids: String,
    muted_character_ids: String,
    persona_id: Option<String>,
    created_at: i64,
    updated_at: i64,
    archived: i64,
    chat_type: String,
    starting_scene: Option<String>,
    background_image_path: Option<String>,
    lorebook_ids: String,
    disable_character_lorebooks: i64,
    speaker_selection_method: String,
    memory_type: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct GroupsSnapshot {
    group_characters: Vec<SyncGroupConfigRecord>,
    group_sessions: Vec<SyncGroupSessionRecord>,
    group_participation: Vec<GroupParticipation>,
    group_messages: Vec<GroupMessage>,
    group_message_variants: Vec<GroupMessageVariant>,
    usage_records: Vec<UsageRecord>,
    usage_metadata: Vec<UsageMetadata>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct SyncGroupSessionRecord {
    id: String,
    group_character_id: Option<String>,
    name: String,
    character_ids: String,
    muted_character_ids: String,
    persona_id: Option<String>,
    created_at: i64,
    updated_at: i64,
    archived: i64,
    chat_type: String,
    starting_scene: Option<String>,
    background_image_path: Option<String>,
    lorebook_ids: String,
    disable_character_lorebooks: i64,
    memories: String,
    memory_embeddings: String,
    memory_summary: String,
    memory_summary_token_count: i64,
    memory_tool_events: String,
    memory_status: Option<String>,
    memory_error: Option<String>,
    speaker_selection_method: String,
    memory_type: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ConversationsSnapshot {
    sessions: Vec<Session>,
    messages: Vec<Message>,
    message_variants: Vec<MessageVariant>,
    usage_records: Vec<UsageRecord>,
    usage_metadata: Vec<UsageMetadata>,
}

pub fn build_sync_manifest(conn: &DbConnection) -> Result<SyncManifest, String> {
    let domains = [
        SyncDomain::Core,
        SyncDomain::Tts,
        SyncDomain::Lorebooks,
        SyncDomain::Characters,
        SyncDomain::Groups,
        SyncDomain::Conversations,
    ];

    let mut manifest = SyncManifest::default();
    for domain in domains {
        let payload = fetch_domain_snapshot(conn, domain)?;
        manifest.domains.push(DomainManifest {
            domain,
            fingerprint: blake3::hash(&payload).to_hex().to_string(),
        });
    }

    Ok(manifest)
}

pub fn fetch_domain_snapshot(conn: &DbConnection, domain: SyncDomain) -> Result<Vec<u8>, String> {
    match domain {
        SyncDomain::Core => serialize_core_snapshot(conn),
        SyncDomain::Tts => serialize_tts_snapshot(conn),
        SyncDomain::Lorebooks => serialize_lorebooks_snapshot(conn),
        SyncDomain::Characters => serialize_characters_snapshot(conn),
        SyncDomain::Groups => serialize_groups_snapshot(conn),
        SyncDomain::Conversations => serialize_conversations_snapshot(conn),
    }
}

pub fn apply_domain_snapshot(
    conn: &mut DbConnection,
    domain: SyncDomain,
    payload: &[u8],
) -> Result<(), String> {
    match domain {
        SyncDomain::Core => apply_core_snapshot(conn, payload),
        SyncDomain::Tts => apply_tts_snapshot(conn, payload),
        SyncDomain::Lorebooks => apply_lorebooks_snapshot(conn, payload),
        SyncDomain::Characters => apply_characters_snapshot(conn, payload),
        SyncDomain::Groups => apply_groups_snapshot(conn, payload),
        SyncDomain::Conversations => apply_conversations_snapshot(conn, payload),
    }
}

fn serialize_core_snapshot(conn: &DbConnection) -> Result<Vec<u8>, String> {
    let (
        meta,
        settings,
        personas,
        models,
        secrets,
        provider_credentials,
        prompt_templates,
        model_pricing_cache,
        _audio_providers,
        _audio_voice_cache,
        _user_voices,
        creation_helper_sessions,
        _groups,
    ) = fetch_global_core(conn)?;

    bincode::serialize(&CoreSnapshot {
        meta,
        settings,
        personas,
        models,
        secrets,
        provider_credentials,
        prompt_templates,
        model_pricing_cache,
        creation_helper_sessions,
    })
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn serialize_tts_snapshot(conn: &DbConnection) -> Result<Vec<u8>, String> {
    let (
        _meta,
        _settings,
        _personas,
        _models,
        _secrets,
        _provider_credentials,
        _prompt_templates,
        _model_pricing_cache,
        audio_providers,
        audio_voice_cache,
        user_voices,
        _creation_helper_sessions,
        _groups,
    ) = fetch_global_core(conn)?;

    bincode::serialize(&TtsSnapshot {
        audio_providers,
        audio_voice_cache,
        user_voices,
    })
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn serialize_lorebooks_snapshot(conn: &DbConnection) -> Result<Vec<u8>, String> {
    let ids = collect_text_ids(conn, "SELECT id FROM lorebooks")?;
    let payload = fetch_lorebooks(conn, &ids)?;
    let (lorebooks, entries): (Vec<SyncLorebook>, Vec<SyncLorebookEntry>) =
        bincode::deserialize(&payload)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    bincode::serialize(&LorebooksSnapshot { lorebooks, entries })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn serialize_characters_snapshot(conn: &DbConnection) -> Result<Vec<u8>, String> {
    let ids = collect_text_ids(conn, "SELECT id FROM characters")?;
    let (
        characters,
        rules,
        scenes,
        scene_variants,
        character_lorebooks,
        chat_templates,
        chat_template_messages,
    ) = fetch_characters_data(conn, &ids)?;
    bincode::serialize(&CharactersSnapshot {
        characters,
        rules,
        scenes,
        scene_variants,
        character_lorebooks,
        chat_templates,
        chat_template_messages,
    })
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn serialize_groups_snapshot(conn: &DbConnection) -> Result<Vec<u8>, String> {
    let group_characters = fetch_group_configs(conn)?;
    let group_session_ids = collect_text_ids(conn, "SELECT id FROM group_sessions")?;
    let (
        group_sessions,
        group_participation,
        group_messages,
        group_message_variants,
        usage_records,
        usage_metadata,
    ) = fetch_group_sessions_full(conn, &group_session_ids)?;
    bincode::serialize(&GroupsSnapshot {
        group_characters,
        group_sessions,
        group_participation,
        group_messages,
        group_message_variants,
        usage_records,
        usage_metadata,
    })
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn serialize_conversations_snapshot(conn: &DbConnection) -> Result<Vec<u8>, String> {
    let session_ids = collect_text_ids(conn, "SELECT id FROM sessions")?;
    let (sessions, messages, message_variants, session_usage_records, session_usage_metadata) =
        fetch_sessions_data(conn, &session_ids)?;

    bincode::serialize(&ConversationsSnapshot {
        sessions,
        messages,
        message_variants,
        usage_records: session_usage_records,
        usage_metadata: session_usage_metadata,
    })
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn collect_text_ids(conn: &DbConnection, sql: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn fetch_group_configs(conn: &DbConnection) -> Result<Vec<SyncGroupConfigRecord>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived, chat_type, starting_scene, background_image_path, COALESCE(lorebook_ids, '[]'), COALESCE(disable_character_lorebooks, 0), COALESCE(speaker_selection_method, 'llm'), COALESCE(memory_type, 'manual') FROM group_characters")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(SyncGroupConfigRecord {
                id: r.get(0)?,
                name: r.get(1)?,
                character_ids: r.get(2)?,
                muted_character_ids: r.get(3)?,
                persona_id: r.get(4)?,
                created_at: r.get(5)?,
                updated_at: r.get(6)?,
                archived: r.get(7)?,
                chat_type: r.get(8)?,
                starting_scene: r.get(9)?,
                background_image_path: r.get(10)?,
                lorebook_ids: r.get(11)?,
                disable_character_lorebooks: r.get(12)?,
                speaker_selection_method: r.get(13)?,
                memory_type: r.get(14)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn fetch_group_sessions_full(
    conn: &DbConnection,
    ids: &[String],
) -> Result<
    (
        Vec<SyncGroupSessionRecord>,
        Vec<GroupParticipation>,
        Vec<GroupMessage>,
        Vec<GroupMessageVariant>,
        Vec<UsageRecord>,
        Vec<UsageMetadata>,
    ),
    String,
> {
    if ids.is_empty() {
        return Ok((
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ));
    }

    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!("SELECT id, group_character_id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived, chat_type, starting_scene, background_image_path, COALESCE(lorebook_ids, '[]'), COALESCE(disable_character_lorebooks, 0), memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, memory_status, memory_error, COALESCE(speaker_selection_method, 'llm'), COALESCE(memory_type, 'manual') FROM group_sessions WHERE id IN ({})", placeholders);
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let sessions: Vec<SyncGroupSessionRecord> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(SyncGroupSessionRecord {
                id: r.get(0)?,
                group_character_id: r.get(1)?,
                name: r.get(2)?,
                character_ids: r.get(3)?,
                muted_character_ids: r.get(4)?,
                persona_id: r.get(5)?,
                created_at: r.get(6)?,
                updated_at: r.get(7)?,
                archived: r.get(8)?,
                chat_type: r.get(9)?,
                starting_scene: r.get(10)?,
                background_image_path: r.get(11)?,
                lorebook_ids: r.get(12)?,
                disable_character_lorebooks: r.get(13)?,
                memories: r.get(14)?,
                memory_embeddings: r.get(15)?,
                memory_summary: r.get(16)?,
                memory_summary_token_count: r.get(17)?,
                memory_tool_events: r.get(18)?,
                memory_status: r.get(19)?,
                memory_error: r.get(20)?,
                speaker_selection_method: r.get(21)?,
                memory_type: r.get(22)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    let (_legacy_sessions, participation, messages, variants, usages, metadata) =
        fetch_group_sessions_data(conn, ids)?;

    Ok((
        sessions,
        participation,
        messages,
        variants,
        usages,
        metadata,
    ))
}

fn delete_missing_rows(
    tx: &rusqlite::Transaction<'_>,
    table: &str,
    key_column: &str,
    ids: &[String],
) -> Result<(), String> {
    let sql = if ids.is_empty() {
        format!("DELETE FROM {}", table)
    } else {
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        format!(
            "DELETE FROM {} WHERE {} NOT IN ({})",
            table, key_column, placeholders
        )
    };
    tx.execute(&sql, rusqlite::params_from_iter(ids.iter()))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

fn apply_core_snapshot(conn: &mut DbConnection, payload: &[u8]) -> Result<(), String> {
    let snapshot: CoreSnapshot = bincode::deserialize(payload)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    tx.execute("DELETE FROM meta", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for item in snapshot.meta {
        tx.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
            params![item.key, item.value],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    tx.execute("DELETE FROM settings", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    if let Some(settings) = snapshot.settings.first() {
        tx.execute(
            r#"INSERT OR REPLACE INTO settings (id, default_provider_credential_id, default_model_id, app_state, prompt_template_id, system_prompt, advanced_settings, migration_version, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
            params![
                settings.id,
                settings.default_provider_credential_id,
                settings.default_model_id,
                settings.app_state,
                settings.prompt_template_id,
                settings.system_prompt,
                settings.advanced_settings,
                settings.migration_version,
                settings.created_at,
                settings.updated_at
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    let persona_ids = snapshot
        .personas
        .iter()
        .map(|persona| persona.id.clone())
        .collect::<Vec<_>>();
    for persona in snapshot.personas {
        tx.execute(
            r#"INSERT OR REPLACE INTO personas (id, title, description, nickname, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, is_default, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
            params![
                persona.id,
                persona.title,
                persona.description,
                persona.nickname,
                persona.avatar_path,
                persona.avatar_crop_x,
                persona.avatar_crop_y,
                persona.avatar_crop_scale,
                persona.is_default,
                persona.created_at,
                persona.updated_at
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    delete_missing_rows(&tx, "personas", "id", &persona_ids)?;

    for table in [
        "models",
        "secrets",
        "provider_credentials",
        "prompt_templates",
        "model_pricing_cache",
        "creation_helper_sessions",
    ] {
        tx.execute(&format!("DELETE FROM {}", table), [])
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for model in snapshot.models {
        tx.execute(
            r#"INSERT OR REPLACE INTO models (id, name, provider_id, provider_credential_id, provider_label, display_name, created_at, model_type, input_scopes, output_scopes, advanced_model_settings, prompt_template_id, system_prompt)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#,
            params![
                model.id,
                model.name,
                model.provider_id,
                model.provider_credential_id,
                model.provider_label,
                model.display_name,
                model.created_at,
                model.model_type,
                model.input_scopes,
                model.output_scopes,
                model.advanced_model_settings,
                model.prompt_template_id,
                model.system_prompt
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for secret in snapshot.secrets {
        tx.execute(
            "INSERT OR REPLACE INTO secrets (service, account, value, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![secret.service, secret.account, secret.value, secret.created_at, secret.updated_at],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for credential in snapshot.provider_credentials {
        tx.execute(
            r#"INSERT OR REPLACE INTO provider_credentials (id, provider_id, label, api_key_ref, api_key, base_url, default_model, headers, config)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
            params![
                credential.id,
                credential.provider_id,
                credential.label,
                credential.api_key_ref,
                credential.api_key,
                credential.base_url,
                credential.default_model,
                credential.headers,
                credential.config
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for template in snapshot.prompt_templates {
        tx.execute(
            r#"INSERT OR REPLACE INTO prompt_templates (id, name, scope, target_ids, content, entries, condense_prompt_entries, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
            params![
                template.id,
                template.name,
                template.scope,
                template.target_ids,
                template.content,
                template.entries,
                template.condense_prompt_entries,
                template.created_at,
                template.updated_at
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for pricing in snapshot.model_pricing_cache {
        tx.execute(
            "INSERT OR REPLACE INTO model_pricing_cache (model_id, pricing_json, cached_at) VALUES (?1, ?2, ?3)",
            params![pricing.model_id, pricing.pricing_json, pricing.cached_at],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for helper in snapshot.creation_helper_sessions {
        tx.execute(
            r#"INSERT OR REPLACE INTO creation_helper_sessions (id, creation_goal, status, session_json, uploaded_images_json, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
            params![
                helper.id,
                helper.creation_goal,
                helper.status,
                helper.session_json,
                helper.uploaded_images_json,
                helper.created_at,
                helper.updated_at
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn apply_tts_snapshot(conn: &mut DbConnection, payload: &[u8]) -> Result<(), String> {
    let snapshot: TtsSnapshot = bincode::deserialize(payload)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    tx.execute("DELETE FROM audio_voice_cache", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    tx.execute("DELETE FROM user_voices", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let provider_ids = snapshot
        .audio_providers
        .iter()
        .map(|provider| provider.id.clone())
        .collect::<Vec<_>>();
    for provider in snapshot.audio_providers {
        tx.execute(
            r#"INSERT OR REPLACE INTO audio_providers (id, provider_type, label, api_key, project_id, location, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            params![
                provider.id,
                provider.provider_type,
                provider.label,
                provider.api_key,
                provider.project_id,
                provider.location,
                provider.created_at,
                provider.updated_at
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    delete_missing_rows(&tx, "audio_providers", "id", &provider_ids)?;

    for voice in snapshot.audio_voice_cache {
        tx.execute(
            r#"INSERT OR REPLACE INTO audio_voice_cache (id, provider_id, voice_id, name, preview_url, labels, cached_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
            params![
                voice.id,
                voice.provider_id,
                voice.voice_id,
                voice.name,
                voice.preview_url,
                voice.labels,
                voice.cached_at
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for voice in snapshot.user_voices {
        tx.execute(
            r#"INSERT OR REPLACE INTO user_voices (id, provider_id, name, model_id, voice_id, prompt, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            params![
                voice.id,
                voice.provider_id,
                voice.name,
                voice.model_id,
                voice.voice_id,
                voice.prompt,
                voice.created_at,
                voice.updated_at
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn apply_lorebooks_snapshot(conn: &mut DbConnection, payload: &[u8]) -> Result<(), String> {
    let snapshot: LorebooksSnapshot = bincode::deserialize(payload)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let lorebook_ids = snapshot
        .lorebooks
        .iter()
        .map(|lorebook| lorebook.id.clone())
        .collect::<Vec<_>>();
    for lorebook in snapshot.lorebooks {
        tx.execute(
            "INSERT OR REPLACE INTO lorebooks (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![lorebook.id, lorebook.name, lorebook.created_at, lorebook.updated_at],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    delete_missing_rows(&tx, "lorebooks", "id", &lorebook_ids)?;

    tx.execute("DELETE FROM lorebook_entries", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for entry in snapshot.entries {
        tx.execute(
            r#"INSERT OR REPLACE INTO lorebook_entries (id, lorebook_id, title, enabled, always_active, keywords, case_sensitive, content, priority, display_order, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"#,
            params![
                entry.id,
                entry.lorebook_id,
                entry.title,
                entry.enabled,
                entry.always_active,
                entry.keywords,
                entry.case_sensitive,
                entry.content,
                entry.priority,
                entry.display_order,
                entry.created_at,
                entry.updated_at
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn apply_characters_snapshot(conn: &mut DbConnection, payload: &[u8]) -> Result<(), String> {
    let snapshot: CharactersSnapshot = bincode::deserialize(payload)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let character_ids = snapshot
        .characters
        .iter()
        .map(|character| character.id.clone())
        .collect::<Vec<_>>();
    for character in snapshot.characters {
        tx.execute(
            r#"INSERT OR REPLACE INTO characters (id, name, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, background_image_path, definition, description, nickname, scenario, creator_notes, creator, creator_notes_multilingual, source, tags, default_scene_id, default_model_id, fallback_model_id, memory_type, prompt_template_id, system_prompt, voice_config, voice_autoplay, disable_avatar_gradient, custom_gradient_enabled, custom_gradient_colors, custom_text_color, custom_text_secondary, chat_appearance, default_chat_template_id, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33)"#,
            params![
                character.id,
                character.name,
                character.avatar_path,
                character.avatar_crop_x,
                character.avatar_crop_y,
                character.avatar_crop_scale,
                character.background_image_path,
                character.definition,
                character.description,
                character.nickname,
                character.scenario,
                character.creator_notes,
                character.creator,
                character.creator_notes_multilingual,
                character.source,
                character.tags,
                character.default_scene_id,
                character.default_model_id,
                character.fallback_model_id,
                character.memory_type,
                character.prompt_template_id,
                character.system_prompt,
                character.voice_config,
                character.voice_autoplay,
                character.disable_avatar_gradient,
                character.custom_gradient_enabled,
                character.custom_gradient_colors,
                character.custom_text_color,
                character.custom_text_secondary,
                character.chat_appearance,
                character.default_chat_template_id,
                character.created_at,
                character.updated_at
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    delete_missing_rows(&tx, "characters", "id", &character_ids)?;

    for table in [
        "chat_template_messages",
        "chat_templates",
        "scene_variants",
        "scenes",
        "character_rules",
        "character_lorebooks",
    ] {
        tx.execute(&format!("DELETE FROM {}", table), [])
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for rule in snapshot.rules {
        tx.execute(
            "INSERT INTO character_rules (character_id, idx, rule) VALUES (?1, ?2, ?3)",
            params![rule.character_id, rule.idx, rule.rule],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for scene in snapshot.scenes {
        tx.execute(
            "INSERT OR REPLACE INTO scenes (id, character_id, content, direction, created_at, selected_variant_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![scene.id, scene.character_id, scene.content, scene.direction, scene.created_at, scene.selected_variant_id],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for variant in snapshot.scene_variants {
        tx.execute(
            "INSERT OR REPLACE INTO scene_variants (id, scene_id, content, direction, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![variant.id, variant.scene_id, variant.content, variant.direction, variant.created_at],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for link in snapshot.character_lorebooks {
        tx.execute(
            "INSERT OR REPLACE INTO character_lorebooks (character_id, lorebook_id, enabled, display_order, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![link.character_id, link.lorebook_id, link.enabled, link.display_order, link.created_at, link.updated_at],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for template in snapshot.chat_templates {
        tx.execute(
            "INSERT OR REPLACE INTO chat_templates (id, character_id, name, scene_id, prompt_template_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![template.id, template.character_id, template.name, template.scene_id, template.prompt_template_id, template.created_at],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for message in snapshot.chat_template_messages {
        tx.execute(
            "INSERT OR REPLACE INTO chat_template_messages (id, template_id, idx, role, content) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![message.id, message.template_id, message.idx, message.role, message.content],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn apply_groups_snapshot(conn: &mut DbConnection, payload: &[u8]) -> Result<(), String> {
    let snapshot: GroupsSnapshot = bincode::deserialize(payload)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let existing_group_session_ids = {
        let mut stmt = tx
            .prepare("SELECT id FROM group_sessions")
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    };

    if !existing_group_session_ids.is_empty() {
        let placeholders = existing_group_session_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let delete_usage_metadata_sql = format!(
            "DELETE FROM usage_metadata WHERE usage_id IN (SELECT id FROM usage_records WHERE session_id IN ({}))",
            placeholders
        );
        tx.execute(
            &delete_usage_metadata_sql,
            rusqlite::params_from_iter(existing_group_session_ids.iter()),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let delete_usage_sql = format!(
            "DELETE FROM usage_records WHERE session_id IN ({})",
            placeholders
        );
        tx.execute(
            &delete_usage_sql,
            rusqlite::params_from_iter(existing_group_session_ids.iter()),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for table in [
        "group_message_variants",
        "group_messages",
        "group_participation",
        "group_sessions",
    ] {
        tx.execute(&format!("DELETE FROM {}", table), [])
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    let group_ids = snapshot
        .group_characters
        .iter()
        .map(|group| group.id.clone())
        .collect::<Vec<_>>();
    for group in snapshot.group_characters {
        tx.execute(
            r#"INSERT OR REPLACE INTO group_characters (id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived, chat_type, starting_scene, background_image_path, lorebook_ids, disable_character_lorebooks, speaker_selection_method, memory_type)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"#,
            params![
                group.id,
                group.name,
                group.character_ids,
                group.muted_character_ids,
                group.persona_id,
                group.created_at,
                group.updated_at,
                group.archived,
                group.chat_type,
                group.starting_scene,
                group.background_image_path,
                group.lorebook_ids,
                group.disable_character_lorebooks,
                group.speaker_selection_method,
                group.memory_type
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    delete_missing_rows(&tx, "group_characters", "id", &group_ids)?;

    for session in snapshot.group_sessions {
        tx.execute(
            r#"INSERT OR REPLACE INTO group_sessions (id, group_character_id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived, chat_type, starting_scene, background_image_path, lorebook_ids, disable_character_lorebooks, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, memory_status, memory_error, speaker_selection_method, memory_type)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)"#,
            params![
                session.id,
                session.group_character_id,
                session.name,
                session.character_ids,
                session.muted_character_ids,
                session.persona_id,
                session.created_at,
                session.updated_at,
                session.archived,
                session.chat_type,
                session.starting_scene,
                session.background_image_path,
                session.lorebook_ids,
                session.disable_character_lorebooks,
                session.memories,
                session.memory_embeddings,
                session.memory_summary,
                session.memory_summary_token_count,
                session.memory_tool_events,
                session.memory_status,
                session.memory_error,
                session.speaker_selection_method,
                session.memory_type
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for participation in snapshot.group_participation {
        tx.execute(
            "INSERT OR REPLACE INTO group_participation (id, session_id, character_id, speak_count, last_spoke_turn, last_spoke_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![participation.id, participation.session_id, participation.character_id, participation.speak_count, participation.last_spoke_turn, participation.last_spoke_at],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for message in snapshot.group_messages {
        tx.execute(
            r#"INSERT OR REPLACE INTO group_messages (id, session_id, role, content, speaker_character_id, turn_number, created_at, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned, attachments, used_lorebook_entries, reasoning, selection_reasoning, model_id)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)"#,
            params![
                message.id,
                message.session_id,
                message.role,
                message.content,
                message.speaker_character_id,
                message.turn_number,
                message.created_at,
                message.prompt_tokens,
                message.completion_tokens,
                message.total_tokens,
                message.selected_variant_id,
                message.is_pinned,
                message.attachments,
                message.used_lorebook_entries,
                message.reasoning,
                message.selection_reasoning,
                message.model_id
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for variant in snapshot.group_message_variants {
        tx.execute(
            "INSERT OR REPLACE INTO group_message_variants (id, message_id, content, speaker_character_id, created_at, prompt_tokens, completion_tokens, total_tokens, reasoning, selection_reasoning, model_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![variant.id, variant.message_id, variant.content, variant.speaker_character_id, variant.created_at, variant.prompt_tokens, variant.completion_tokens, variant.total_tokens, variant.reasoning, variant.selection_reasoning, variant.model_id],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for usage in snapshot.usage_records {
        tx.execute(
            r#"INSERT OR REPLACE INTO usage_records (id, timestamp, session_id, character_id, character_name, model_id, model_name, provider_id, provider_label, operation_type, finish_reason, prompt_tokens, completion_tokens, total_tokens, memory_tokens, summary_tokens, reasoning_tokens, image_tokens, prompt_cost, completion_cost, total_cost, success, error_message)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)"#,
            params![
                usage.id,
                usage.timestamp,
                usage.session_id,
                usage.character_id,
                usage.character_name,
                usage.model_id,
                usage.model_name,
                usage.provider_id,
                usage.provider_label,
                usage.operation_type,
                usage.finish_reason,
                usage.prompt_tokens,
                usage.completion_tokens,
                usage.total_tokens,
                usage.memory_tokens,
                usage.summary_tokens,
                usage.reasoning_tokens,
                usage.image_tokens,
                usage.prompt_cost,
                usage.completion_cost,
                usage.total_cost,
                usage.success,
                usage.error_message
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for metadata in snapshot.usage_metadata {
        tx.execute(
            "INSERT OR REPLACE INTO usage_metadata (usage_id, key, value) VALUES (?1, ?2, ?3)",
            params![metadata.usage_id, metadata.key, metadata.value],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn apply_conversations_snapshot(conn: &mut DbConnection, payload: &[u8]) -> Result<(), String> {
    let snapshot: ConversationsSnapshot = bincode::deserialize(payload)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let existing_session_ids = {
        let mut stmt = tx
            .prepare("SELECT id FROM sessions")
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    };

    if !existing_session_ids.is_empty() {
        let placeholders = existing_session_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let delete_usage_metadata_sql = format!(
            "DELETE FROM usage_metadata WHERE usage_id IN (SELECT id FROM usage_records WHERE session_id IN ({}))",
            placeholders
        );
        tx.execute(
            &delete_usage_metadata_sql,
            rusqlite::params_from_iter(existing_session_ids.iter()),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let delete_usage_sql = format!(
            "DELETE FROM usage_records WHERE session_id IN ({})",
            placeholders
        );
        tx.execute(
            &delete_usage_sql,
            rusqlite::params_from_iter(existing_session_ids.iter()),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for table in ["message_variants", "messages", "sessions"] {
        tx.execute(&format!("DELETE FROM {}", table), [])
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for session in snapshot.sessions {
        tx.execute(
            r#"INSERT OR REPLACE INTO sessions (id, character_id, title, system_prompt, selected_scene_id, prompt_template_id, persona_id, persona_disabled, voice_autoplay, temperature, top_p, max_output_tokens, frequency_penalty, presence_penalty, top_k, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, archived, created_at, updated_at, memory_status, memory_error)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)"#,
            params![
                session.id,
                session.character_id,
                session.title,
                session.system_prompt,
                session.selected_scene_id,
                session.prompt_template_id,
                session.persona_id,
                session.persona_disabled,
                session.voice_autoplay,
                session.temperature,
                session.top_p,
                session.max_output_tokens,
                session.frequency_penalty,
                session.presence_penalty,
                session.top_k,
                session.memories,
                session.memory_embeddings,
                session.memory_summary,
                session.memory_summary_token_count,
                session.memory_tool_events,
                session.archived,
                session.created_at,
                session.updated_at,
                session.memory_status,
                session.memory_error
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for message in snapshot.messages {
        tx.execute(
            r#"INSERT OR REPLACE INTO messages (id, session_id, role, content, created_at, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned, memory_refs, used_lorebook_entries, attachments, reasoning)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"#,
            params![
                message.id,
                message.session_id,
                message.role,
                message.content,
                message.created_at,
                message.prompt_tokens,
                message.completion_tokens,
                message.total_tokens,
                message.selected_variant_id,
                message.is_pinned,
                message.memory_refs,
                message.used_lorebook_entries,
                message.attachments,
                message.reasoning
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for variant in snapshot.message_variants {
        tx.execute(
            "INSERT OR REPLACE INTO message_variants (id, message_id, content, created_at, prompt_tokens, completion_tokens, total_tokens, reasoning) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![variant.id, variant.message_id, variant.content, variant.created_at, variant.prompt_tokens, variant.completion_tokens, variant.total_tokens, variant.reasoning],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for usage in snapshot.usage_records {
        tx.execute(
            r#"INSERT OR REPLACE INTO usage_records (id, timestamp, session_id, character_id, character_name, model_id, model_name, provider_id, provider_label, operation_type, finish_reason, prompt_tokens, completion_tokens, total_tokens, memory_tokens, summary_tokens, reasoning_tokens, image_tokens, prompt_cost, completion_cost, total_cost, success, error_message)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)"#,
            params![
                usage.id,
                usage.timestamp,
                usage.session_id,
                usage.character_id,
                usage.character_name,
                usage.model_id,
                usage.model_name,
                usage.provider_id,
                usage.provider_label,
                usage.operation_type,
                usage.finish_reason,
                usage.prompt_tokens,
                usage.completion_tokens,
                usage.total_tokens,
                usage.memory_tokens,
                usage.summary_tokens,
                usage.reasoning_tokens,
                usage.image_tokens,
                usage.prompt_cost,
                usage.completion_cost,
                usage.total_cost,
                usage.success,
                usage.error_message
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for metadata in snapshot.usage_metadata {
        tx.execute(
            "INSERT OR REPLACE INTO usage_metadata (usage_id, key, value) VALUES (?1, ?2, ?3)",
            params![metadata.usage_id, metadata.key, metadata.value],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

pub fn get_local_manifest(conn: &DbConnection) -> Result<Manifest, String> {
    let mut manifest = Manifest::default();

    // 1. Lorebooks
    let mut stmt = conn
        .prepare("SELECT id, updated_at FROM lorebooks")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for row in rows {
        let (id, updated): (String, i64) =
            row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        manifest.lorebooks.insert(id, updated);
    }

    // 2. Characters
    let mut stmt = conn
        .prepare("SELECT id, updated_at FROM characters")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for row in rows {
        let (id, updated): (String, i64) =
            row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        manifest.characters.insert(id, updated);
    }

    // 3. Sessions
    let mut stmt = conn
        .prepare("SELECT id, updated_at FROM sessions")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for row in rows {
        let (id, updated): (String, i64) =
            row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        manifest.sessions.insert(id, updated);
    }

    Ok(manifest)
}

pub fn get_local_manifest_v2(conn: &DbConnection) -> Result<ManifestV2, String> {
    let mut manifest = ManifestV2::default();

    let mut stmt = conn
        .prepare("SELECT id, updated_at FROM lorebooks")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for row in rows {
        let (id, updated): (String, i64) =
            row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        manifest.lorebooks.insert(id, updated);
    }

    let mut stmt = conn
        .prepare("SELECT id, updated_at FROM characters")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for row in rows {
        let (id, updated): (String, i64) =
            row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        manifest.characters.insert(id, updated);
    }

    let mut stmt = conn
        .prepare("SELECT id, updated_at FROM sessions")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for row in rows {
        let (id, updated): (String, i64) =
            row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        manifest.sessions.insert(id, updated);
    }

    let mut stmt = conn
        .prepare("SELECT id, updated_at FROM group_sessions")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for row in rows {
        let (id, updated): (String, i64) =
            row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        manifest.group_sessions.insert(id, updated);
    }

    Ok(manifest)
}

pub fn fetch_layer_data(
    conn: &DbConnection,
    layer: SyncLayer,
    ids: &[String],
) -> Result<Vec<u8>, String> {
    fetch_layer_data_for_protocol(conn, layer, ids, 5)
}

pub fn fetch_layer_data_for_protocol(
    conn: &DbConnection,
    layer: SyncLayer,
    ids: &[String],
    protocol_version: u32,
) -> Result<Vec<u8>, String> {
    match layer {
        SyncLayer::Globals => fetch_globals_for_protocol(conn, protocol_version),
        SyncLayer::Lorebooks => fetch_lorebooks(conn, ids),
        SyncLayer::Characters => fetch_characters_for_protocol(conn, ids, protocol_version),
        SyncLayer::Sessions => fetch_sessions_for_protocol(conn, ids, protocol_version),
        SyncLayer::GroupSessions => fetch_group_sessions_for_protocol(conn, ids, protocol_version),
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LegacyPromptTemplate {
    pub id: String,
    pub name: String,
    pub scope: String,
    pub target_ids: String,
    pub content: String,
    pub created_at: i64,
    pub updated_at: i64,
}

type GlobalCoreData = (
    Vec<MetaEntry>,
    Vec<Settings>,
    Vec<Persona>,
    Vec<Model>,
    Vec<Secret>,
    Vec<ProviderCredential>,
    Vec<PromptTemplate>,
    Vec<ModelPricingCache>,
    Vec<AudioProvider>,
    Vec<AudioVoiceCache>,
    Vec<UserVoice>,
    Vec<CreationHelperSession>,
    Vec<GroupCharacter>,
);

type GlobalsDataV5 = (
    Vec<MetaEntry>,
    Vec<Settings>,
    Vec<Persona>,
    Vec<Model>,
    Vec<Secret>,
    Vec<ProviderCredential>,
    Vec<PromptTemplate>,
    Vec<ModelPricingCache>,
    Vec<AudioProvider>,
    Vec<AudioVoiceCache>,
    Vec<UserVoice>,
    Vec<CreationHelperSession>,
    Vec<GroupCharacter>,
);

type GlobalsDataV4 = (
    Vec<MetaEntry>,
    Vec<Settings>,
    Vec<Persona>,
    Vec<Model>,
    Vec<Secret>,
    Vec<ProviderCredential>,
    Vec<PromptTemplate>,
    Vec<ModelPricingCache>,
    Vec<CreationHelperSession>,
    Vec<GroupCharacter>,
);

type LegacyGlobalsDataV3 = (
    Vec<Settings>,
    Vec<Persona>,
    Vec<Model>,
    Vec<Secret>,
    Vec<ProviderCredential>,
    Vec<PromptTemplate>,
    Vec<ModelPricingCache>,
    Vec<AudioProvider>,
    Vec<AudioVoiceCache>,
    Vec<UserVoice>,
);

pub fn fetch_globals_for_protocol(
    conn: &DbConnection,
    protocol_version: u32,
) -> Result<Vec<u8>, String> {
    let (
        meta,
        settings,
        personas,
        models,
        secrets,
        creds,
        templates,
        pricing,
        audio_providers,
        voice_cache,
        user_voices,
        helper_sessions,
        groups,
    ) = fetch_global_core(conn)?;

    if protocol_version >= 5 {
        let payload_tuple = (
            meta,
            settings,
            personas,
            models,
            secrets,
            creds,
            templates,
            pricing,
            audio_providers,
            voice_cache,
            user_voices,
            helper_sessions,
            groups,
        );
        return bincode::serialize(&payload_tuple)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e));
    }

    if protocol_version >= 4 {
        let payload_tuple = (
            meta,
            settings,
            personas,
            models,
            secrets,
            creds,
            templates,
            pricing,
            helper_sessions,
            groups,
        );
        return bincode::serialize(&payload_tuple)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e));
    }

    if protocol_version >= 3 {
        let payload_tuple = (
            settings,
            personas,
            models,
            secrets,
            creds,
            templates,
            pricing,
            Vec::<AudioProvider>::new(),
            Vec::<AudioVoiceCache>::new(),
            Vec::<UserVoice>::new(),
        );
        return bincode::serialize(&payload_tuple)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e));
    }

    if protocol_version >= 2 {
        let payload_tuple = (
            settings, personas, models, secrets, creds, templates, pricing,
        );
        return bincode::serialize(&payload_tuple)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e));
    }

    let legacy_templates = templates
        .into_iter()
        .map(|template| LegacyPromptTemplate {
            id: template.id,
            name: template.name,
            scope: template.scope,
            target_ids: template.target_ids,
            content: template.content,
            created_at: template.created_at,
            updated_at: template.updated_at,
        })
        .collect::<Vec<_>>();
    let legacy_payload = (
        settings,
        personas,
        models,
        secrets,
        creds,
        legacy_templates,
        pricing,
    );
    bincode::serialize(&legacy_payload)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn fetch_global_core(conn: &DbConnection) -> Result<GlobalCoreData, String> {
    // Meta
    let mut stmt = conn
        .prepare("SELECT key, value FROM meta")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let meta: Vec<MetaEntry> = stmt
        .query_map([], |r| {
            Ok(MetaEntry {
                key: r.get(0)?,
                value: r.get(1)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    // Settings
    let mut stmt = conn.prepare("SELECT id, default_provider_credential_id, default_model_id, app_state, prompt_template_id, system_prompt, advanced_settings, migration_version, created_at, updated_at FROM settings").map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let settings_iter = stmt
        .query_map([], |r| {
            Ok(Settings {
                id: r.get(0)?,
                default_provider_credential_id: r.get(1)?,
                default_model_id: r.get(2)?,
                app_state: r.get(3)?,
                prompt_template_id: r.get(4)?,
                system_prompt: r.get(5)?,
                advanced_settings: r.get(6)?,
                migration_version: r.get(7)?,
                created_at: r.get(8)?,
                updated_at: r.get(9)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let settings: Vec<Settings> = settings_iter.map(|r| r.unwrap()).collect(); // Expect safe unwrap if query OK

    // Personas
    let mut stmt = conn
        .prepare("SELECT id, title, description, nickname, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, is_default, created_at, updated_at FROM personas")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let personas: Vec<Persona> = stmt
        .query_map([], |r| {
            Ok(Persona {
                id: r.get(0)?,
                title: r.get(1)?,
                description: r.get(2)?,
                nickname: r.get(3)?,
                avatar_path: r.get(4)?,
                avatar_crop_x: r.get(5)?,
                avatar_crop_y: r.get(6)?,
                avatar_crop_scale: r.get(7)?,
                is_default: r.get(8)?,
                created_at: r.get(9)?,
                updated_at: r.get(10)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    // Models
    let mut stmt = conn.prepare("SELECT id, name, provider_id, provider_credential_id, provider_label, display_name, created_at, model_type, input_scopes, output_scopes, advanced_model_settings, prompt_template_id, system_prompt FROM models").map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let models: Vec<Model> = stmt
        .query_map([], |r| {
            Ok(Model {
                id: r.get(0)?,
                name: r.get(1)?,
                provider_id: r.get(2)?,
                provider_credential_id: r.get(3)?,
                provider_label: r.get(4)?,
                display_name: r.get(5)?,
                created_at: r.get(6)?,
                model_type: r.get(7)?,
                input_scopes: r.get(8)?,
                output_scopes: r.get(9)?,
                advanced_model_settings: r.get(10)?,
                prompt_template_id: r.get(11)?,
                system_prompt: r.get(12)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    // Secrets
    let mut stmt = conn
        .prepare("SELECT service, account, value, created_at, updated_at FROM secrets")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let secrets: Vec<Secret> = stmt
        .query_map([], |r| {
            Ok(Secret {
                service: r.get(0)?,
                account: r.get(1)?,
                value: r.get(2)?,
                created_at: r.get(3)?,
                updated_at: r.get(4)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    // Provider Creds
    let mut stmt = conn.prepare("SELECT id, provider_id, label, api_key_ref, api_key, base_url, default_model, headers, config FROM provider_credentials").map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let creds: Vec<ProviderCredential> = stmt
        .query_map([], |r| {
            Ok(ProviderCredential {
                id: r.get(0)?,
                provider_id: r.get(1)?,
                label: r.get(2)?,
                api_key_ref: r.get(3)?,
                api_key: r.get(4)?,
                base_url: r.get(5)?,
                default_model: r.get(6)?,
                headers: r.get(7)?,
                config: r.get(8)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    // Prompt Templates
    let mut stmt = conn.prepare("SELECT id, name, scope, target_ids, content, entries, condense_prompt_entries, created_at, updated_at FROM prompt_templates").map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let templates: Vec<PromptTemplate> = stmt
        .query_map([], |r| {
            Ok(PromptTemplate {
                id: r.get(0)?,
                name: r.get(1)?,
                scope: r.get(2)?,
                target_ids: r.get(3)?,
                content: r.get(4)?,
                entries: r.get(5)?,
                condense_prompt_entries: r.get(6)?,
                created_at: r.get(7)?,
                updated_at: r.get(8)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    // Model Pricing
    let mut stmt = conn
        .prepare("SELECT model_id, pricing_json, cached_at FROM model_pricing_cache")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let pricing: Vec<ModelPricingCache> = stmt
        .query_map([], |r| {
            Ok(ModelPricingCache {
                model_id: r.get(0)?,
                pricing_json: r.get(1)?,
                cached_at: r.get(2)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    let mut stmt = conn
        .prepare("SELECT id, provider_type, label, api_key, project_id, location, created_at, updated_at FROM audio_providers")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let audio_providers: Vec<AudioProvider> = stmt
        .query_map([], |r| {
            Ok(AudioProvider {
                id: r.get(0)?,
                provider_type: r.get(1)?,
                label: r.get(2)?,
                api_key: r.get(3)?,
                project_id: r.get(4)?,
                location: r.get(5)?,
                created_at: r.get(6)?,
                updated_at: r.get(7)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    let mut stmt = conn
        .prepare("SELECT id, provider_id, voice_id, name, preview_url, labels, cached_at FROM audio_voice_cache")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let voice_cache: Vec<AudioVoiceCache> = stmt
        .query_map([], |r| {
            Ok(AudioVoiceCache {
                id: r.get(0)?,
                provider_id: r.get(1)?,
                voice_id: r.get(2)?,
                name: r.get(3)?,
                preview_url: r.get(4)?,
                labels: r.get(5)?,
                cached_at: r.get(6)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    let mut stmt = conn
        .prepare("SELECT id, provider_id, name, model_id, voice_id, prompt, created_at, updated_at FROM user_voices")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let user_voices: Vec<UserVoice> = stmt
        .query_map([], |r| {
            Ok(UserVoice {
                id: r.get(0)?,
                provider_id: r.get(1)?,
                name: r.get(2)?,
                model_id: r.get(3)?,
                voice_id: r.get(4)?,
                prompt: r.get(5)?,
                created_at: r.get(6)?,
                updated_at: r.get(7)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    let mut stmt = conn
        .prepare("SELECT id, creation_goal, status, session_json, uploaded_images_json, created_at, updated_at FROM creation_helper_sessions")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let helper_sessions: Vec<CreationHelperSession> = stmt
        .query_map([], |r| {
            Ok(CreationHelperSession {
                id: r.get(0)?,
                creation_goal: r.get(1)?,
                status: r.get(2)?,
                session_json: r.get(3)?,
                uploaded_images_json: r.get(4)?,
                created_at: r.get(5)?,
                updated_at: r.get(6)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    let mut stmt = conn
        .prepare("SELECT id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived, chat_type, starting_scene, background_image_path, COALESCE(speaker_selection_method, 'llm'), COALESCE(memory_type, 'manual') FROM group_characters")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let groups: Vec<GroupCharacter> = stmt
        .query_map([], |r| {
            Ok(GroupCharacter {
                id: r.get(0)?,
                name: r.get(1)?,
                character_ids: r.get(2)?,
                muted_character_ids: r.get(3)?,
                persona_id: r.get(4)?,
                created_at: r.get(5)?,
                updated_at: r.get(6)?,
                archived: r.get(7)?,
                chat_type: r.get(8)?,
                starting_scene: r.get(9)?,
                background_image_path: r.get(10)?,
                speaker_selection_method: r.get(11)?,
                memory_type: r.get(12)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    Ok((
        meta,
        settings,
        personas,
        models,
        secrets,
        creds,
        templates,
        pricing,
        audio_providers,
        voice_cache,
        user_voices,
        helper_sessions,
        groups,
    ))
}

fn fetch_lorebooks(conn: &DbConnection, ids: &[String]) -> Result<Vec<u8>, String> {
    if ids.is_empty() {
        return Ok(vec![]);
    }

    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql_lb = format!(
        "SELECT id, name, created_at, updated_at FROM lorebooks WHERE id IN ({})",
        placeholders
    );

    let mut stmt = conn
        .prepare(&sql_lb)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let lorebooks: Vec<SyncLorebook> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(SyncLorebook {
                id: r.get(0)?,
                name: r.get(1)?,
                created_at: r.get(2)?,
                updated_at: r.get(3)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    // Entries for these lorebooks
    let sql_ent = format!("SELECT id, lorebook_id, title, enabled, always_active, keywords, case_sensitive, content, priority, display_order, created_at, updated_at FROM lorebook_entries WHERE lorebook_id IN ({})", placeholders);
    let mut stmt = conn
        .prepare(&sql_ent)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let entries: Vec<SyncLorebookEntry> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(SyncLorebookEntry {
                id: r.get(0)?,
                lorebook_id: r.get(1)?,
                title: r.get(2)?,
                enabled: r.get(3)?,
                always_active: r.get(4)?,
                keywords: r.get(5)?,
                case_sensitive: r.get(6)?,
                content: r.get(7)?,
                priority: r.get(8)?,
                display_order: r.get(9)?,
                created_at: r.get(10)?,
                updated_at: r.get(11)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    bincode::serialize(&(lorebooks, entries))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn fetch_characters_for_protocol(
    conn: &DbConnection,
    ids: &[String],
    protocol_version: u32,
) -> Result<Vec<u8>, String> {
    let (chars, rules, scenes, variants, links, templates, template_messages) =
        fetch_characters_data(conn, ids)?;
    if protocol_version >= 4 {
        return bincode::serialize(&(
            chars,
            rules,
            scenes,
            variants,
            links,
            templates,
            template_messages,
        ))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e));
    }

    let legacy_chars = chars
        .into_iter()
        .map(|c| LegacyCharacterV2 {
            id: c.id,
            name: c.name,
            avatar_path: c.avatar_path,
            avatar_crop_x: c.avatar_crop_x,
            avatar_crop_y: c.avatar_crop_y,
            avatar_crop_scale: c.avatar_crop_scale,
            background_image_path: c.background_image_path,
            definition: c.definition,
            description: c.description,
            default_scene_id: c.default_scene_id,
            default_model_id: c.default_model_id,
            memory_type: c.memory_type,
            prompt_template_id: c.prompt_template_id,
            system_prompt: c.system_prompt,
            voice_config: c.voice_config,
            voice_autoplay: c.voice_autoplay,
            disable_avatar_gradient: c.disable_avatar_gradient,
            custom_gradient_enabled: c.custom_gradient_enabled,
            custom_gradient_colors: c.custom_gradient_colors,
            custom_text_color: c.custom_text_color,
            custom_text_secondary: c.custom_text_secondary,
            created_at: c.created_at,
            updated_at: c.updated_at,
        })
        .collect::<Vec<_>>();
    let legacy_scenes = scenes
        .into_iter()
        .map(|s| LegacySceneV2 {
            id: s.id,
            character_id: s.character_id,
            content: s.content,
            created_at: s.created_at,
            selected_variant_id: s.selected_variant_id,
        })
        .collect::<Vec<_>>();
    let legacy_variants = variants
        .into_iter()
        .map(|v| LegacySceneVariantV2 {
            id: v.id,
            scene_id: v.scene_id,
            content: v.content,
            created_at: v.created_at,
        })
        .collect::<Vec<_>>();
    bincode::serialize(&(legacy_chars, rules, legacy_scenes, legacy_variants, links))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn fetch_characters_data(
    conn: &DbConnection,
    ids: &[String],
) -> Result<
    (
        Vec<Character>,
        Vec<CharacterRule>,
        Vec<Scene>,
        Vec<SceneVariant>,
        Vec<CharacterLorebookLink>,
        Vec<ChatTemplate>,
        Vec<ChatTemplateMessage>,
    ),
    String,
> {
    if ids.is_empty() {
        return Ok((
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ));
    }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

    // Characters
    let sql = format!("SELECT id, name, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, background_image_path, definition, description, nickname, scenario, creator_notes, creator, creator_notes_multilingual, source, tags, default_scene_id, default_model_id, fallback_model_id, memory_type, prompt_template_id, system_prompt, voice_config, voice_autoplay, disable_avatar_gradient, custom_gradient_enabled, custom_gradient_colors, custom_text_color, custom_text_secondary, chat_appearance, default_chat_template_id, created_at, updated_at FROM characters WHERE id IN ({})", placeholders);
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let chars: Vec<Character> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(Character {
                id: r.get(0)?,
                name: r.get(1)?,
                avatar_path: r.get(2)?,
                avatar_crop_x: r.get(3)?,
                avatar_crop_y: r.get(4)?,
                avatar_crop_scale: r.get(5)?,
                background_image_path: r.get(6)?,
                definition: r.get(7)?,
                description: r.get(8)?,
                nickname: r.get(9)?,
                scenario: r.get(10)?,
                creator_notes: r.get(11)?,
                creator: r.get(12)?,
                creator_notes_multilingual: r.get(13)?,
                source: r.get(14)?,
                tags: r.get(15)?,
                default_scene_id: r.get(16)?,
                default_model_id: r.get(17)?,
                fallback_model_id: r.get(18)?,
                memory_type: r.get(19)?,
                prompt_template_id: r.get(20)?,
                system_prompt: r.get(21)?,
                voice_config: r.get(22)?,
                voice_autoplay: r.get(23)?,
                disable_avatar_gradient: r.get(24)?,
                custom_gradient_enabled: r.get(25)?,
                custom_gradient_colors: r.get(26)?,
                custom_text_color: r.get(27)?,
                custom_text_secondary: r.get(28)?,
                chat_appearance: r.get(29)?,
                default_chat_template_id: r.get(30)?,
                created_at: r.get(31)?,
                updated_at: r.get(32)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    // Rules
    let sql_rules = format!(
        "SELECT character_id, idx, rule FROM character_rules WHERE character_id IN ({})",
        placeholders
    );
    let mut stmt = conn
        .prepare(&sql_rules)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rules: Vec<CharacterRule> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(CharacterRule {
                id: None,
                character_id: r.get(0)?,
                idx: r.get(1)?,
                rule: r.get(2)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    // Scenes
    let sql_scenes = format!("SELECT id, character_id, content, direction, created_at, selected_variant_id FROM scenes WHERE character_id IN ({})", placeholders);
    let mut stmt = conn
        .prepare(&sql_scenes)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let scenes: Vec<Scene> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(Scene {
                id: r.get(0)?,
                character_id: r.get(1)?,
                content: r.get(2)?,
                direction: r.get(3)?,
                created_at: r.get(4)?,
                selected_variant_id: r.get(5)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    // Scene Variants
    let sql_vars = format!("SELECT id, scene_id, content, direction, created_at FROM scene_variants WHERE scene_id IN (SELECT id FROM scenes WHERE character_id IN ({}))", placeholders);
    let mut stmt = conn
        .prepare(&sql_vars)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let variants: Vec<SceneVariant> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(SceneVariant {
                id: r.get(0)?,
                scene_id: r.get(1)?,
                content: r.get(2)?,
                direction: r.get(3)?,
                created_at: r.get(4)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    // Character Lorebook Links
    let sql_links = format!("SELECT character_id, lorebook_id, enabled, display_order, created_at, updated_at FROM character_lorebooks WHERE character_id IN ({})", placeholders);
    let mut stmt = conn
        .prepare(&sql_links)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let links: Vec<CharacterLorebookLink> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(CharacterLorebookLink {
                character_id: r.get(0)?,
                lorebook_id: r.get(1)?,
                enabled: r.get(2)?,
                display_order: r.get(3)?,
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    let sql_templates = format!(
        "SELECT id, character_id, name, scene_id, prompt_template_id, created_at FROM chat_templates WHERE character_id IN ({})",
        placeholders
    );
    let mut stmt = conn
        .prepare(&sql_templates)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let templates: Vec<ChatTemplate> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(ChatTemplate {
                id: r.get(0)?,
                character_id: r.get(1)?,
                name: r.get(2)?,
                scene_id: r.get(3)?,
                prompt_template_id: r.get(4)?,
                created_at: r.get(5)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    let sql_template_msgs = format!(
        "SELECT id, template_id, idx, role, content FROM chat_template_messages WHERE template_id IN (SELECT id FROM chat_templates WHERE character_id IN ({}))",
        placeholders
    );
    let mut stmt = conn
        .prepare(&sql_template_msgs)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let template_messages: Vec<ChatTemplateMessage> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(ChatTemplateMessage {
                id: r.get(0)?,
                template_id: r.get(1)?,
                idx: r.get(2)?,
                role: r.get(3)?,
                content: r.get(4)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    Ok((
        chars,
        rules,
        scenes,
        variants,
        links,
        templates,
        template_messages,
    ))
}

fn fetch_sessions_for_protocol(
    conn: &DbConnection,
    ids: &[String],
    protocol_version: u32,
) -> Result<Vec<u8>, String> {
    let (sessions, messages, variants, usages, metadata) = fetch_sessions_data(conn, ids)?;
    if protocol_version >= 4 {
        return bincode::serialize(&(sessions, messages, variants, usages, metadata))
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e));
    }

    let legacy_sessions = sessions
        .into_iter()
        .map(|s| LegacySessionV2 {
            id: s.id,
            character_id: s.character_id,
            title: s.title,
            system_prompt: s.system_prompt,
            selected_scene_id: s.selected_scene_id,
            persona_id: s.persona_id,
            persona_disabled: s.persona_disabled,
            voice_autoplay: s.voice_autoplay,
            temperature: s.temperature,
            top_p: s.top_p,
            max_output_tokens: s.max_output_tokens,
            frequency_penalty: s.frequency_penalty,
            presence_penalty: s.presence_penalty,
            top_k: s.top_k,
            memories: s.memories,
            memory_embeddings: s.memory_embeddings,
            memory_summary: s.memory_summary,
            memory_summary_token_count: s.memory_summary_token_count,
            memory_tool_events: s.memory_tool_events,
            archived: s.archived,
            created_at: s.created_at,
            updated_at: s.updated_at,
            memory_status: s.memory_status,
            memory_error: s.memory_error,
        })
        .collect::<Vec<_>>();
    let legacy_usages = usages
        .into_iter()
        .map(|u| LegacyUsageRecordV2 {
            id: u.id,
            timestamp: u.timestamp,
            session_id: u.session_id,
            character_id: u.character_id,
            character_name: u.character_name,
            model_id: u.model_id,
            model_name: u.model_name,
            provider_id: u.provider_id,
            provider_label: u.provider_label,
            operation_type: u.operation_type,
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
            memory_tokens: u.memory_tokens,
            summary_tokens: u.summary_tokens,
            reasoning_tokens: u.reasoning_tokens,
            image_tokens: u.image_tokens,
            prompt_cost: u.prompt_cost,
            completion_cost: u.completion_cost,
            total_cost: u.total_cost,
            success: u.success,
            error_message: u.error_message,
        })
        .collect::<Vec<_>>();
    bincode::serialize(&(legacy_sessions, messages, variants, legacy_usages, metadata))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn fetch_sessions_data(
    conn: &DbConnection,
    ids: &[String],
) -> Result<
    (
        Vec<Session>,
        Vec<Message>,
        Vec<MessageVariant>,
        Vec<UsageRecord>,
        Vec<UsageMetadata>,
    ),
    String,
> {
    if ids.is_empty() {
        return Ok((Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()));
    }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

    // Sessions
    let sql = format!("SELECT id, character_id, title, system_prompt, selected_scene_id, prompt_template_id, persona_id, persona_disabled, voice_autoplay, temperature, top_p, max_output_tokens, frequency_penalty, presence_penalty, top_k, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, archived, created_at, updated_at, memory_status, memory_error FROM sessions WHERE id IN ({})", placeholders);
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let sessions: Vec<Session> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(Session {
                id: r.get(0)?,
                character_id: r.get(1)?,
                title: r.get(2)?,
                system_prompt: r.get(3)?,
                selected_scene_id: r.get(4)?,
                prompt_template_id: r.get(5)?,
                persona_id: r.get(6)?,
                persona_disabled: r.get(7)?,
                voice_autoplay: r.get(8)?,
                temperature: r.get(9)?,
                top_p: r.get(10)?,
                max_output_tokens: r.get(11)?,
                frequency_penalty: r.get(12)?,
                presence_penalty: r.get(13)?,
                top_k: r.get(14)?,
                memories: r.get(15)?,
                memory_embeddings: r.get(16)?,
                memory_summary: r.get(17)?,
                memory_summary_token_count: r.get(18)?,
                memory_tool_events: r.get(19)?,
                archived: r.get(20)?,
                created_at: r.get(21)?,
                updated_at: r.get(22)?,
                memory_status: r.get(23)?,
                memory_error: r.get(24)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    // Messages
    let sql_msg = format!("SELECT id, session_id, role, content, created_at, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned, memory_refs, used_lorebook_entries, attachments, reasoning FROM messages WHERE session_id IN ({})", placeholders);
    let mut stmt = conn
        .prepare(&sql_msg)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let messages: Vec<Message> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(Message {
                id: r.get(0)?,
                session_id: r.get(1)?,
                role: r.get(2)?,
                content: r.get(3)?,
                created_at: r.get(4)?,
                prompt_tokens: r.get(5)?,
                completion_tokens: r.get(6)?,
                total_tokens: r.get(7)?,
                selected_variant_id: r.get(8)?,
                is_pinned: r.get(9)?,
                memory_refs: r.get(10)?,
                used_lorebook_entries: r.get(11)?,
                attachments: r.get(12)?,
                reasoning: r.get(13)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    // Message Variants
    let sql_var = format!("SELECT id, message_id, content, created_at, prompt_tokens, completion_tokens, total_tokens, reasoning FROM message_variants WHERE message_id IN (SELECT id FROM messages WHERE session_id IN ({}))", placeholders);
    let mut stmt = conn
        .prepare(&sql_var)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let variants: Vec<MessageVariant> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(MessageVariant {
                id: r.get(0)?,
                message_id: r.get(1)?,
                content: r.get(2)?,
                created_at: r.get(3)?,
                prompt_tokens: r.get(4)?,
                completion_tokens: r.get(5)?,
                total_tokens: r.get(6)?,
                reasoning: r.get(7)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    // Usage Records
    let sql_usage = format!("SELECT id, timestamp, session_id, character_id, character_name, model_id, model_name, provider_id, provider_label, operation_type, finish_reason, prompt_tokens, completion_tokens, total_tokens, memory_tokens, summary_tokens, reasoning_tokens, image_tokens, prompt_cost, completion_cost, total_cost, success, error_message FROM usage_records WHERE session_id IN ({})", placeholders);
    let mut stmt = conn
        .prepare(&sql_usage)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let usages: Vec<UsageRecord> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(UsageRecord {
                id: r.get(0)?,
                timestamp: r.get(1)?,
                session_id: r.get(2)?,
                character_id: r.get(3)?,
                character_name: r.get(4)?,
                model_id: r.get(5)?,
                model_name: r.get(6)?,
                provider_id: r.get(7)?,
                provider_label: r.get(8)?,
                operation_type: r.get(9)?,
                finish_reason: r.get(10)?,
                prompt_tokens: r.get(11)?,
                completion_tokens: r.get(12)?,
                total_tokens: r.get(13)?,
                memory_tokens: r.get(14)?,
                summary_tokens: r.get(15)?,
                reasoning_tokens: r.get(16)?,
                image_tokens: r.get(17)?,
                prompt_cost: r.get(18)?,
                completion_cost: r.get(19)?,
                total_cost: r.get(20)?,
                success: r.get(21)?,
                error_message: r.get(22)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    // Usage Metadata
    let sql_meta = format!("SELECT usage_id, key, value FROM usage_metadata WHERE usage_id IN (SELECT id FROM usage_records WHERE session_id IN ({}))", placeholders);
    let mut stmt = conn
        .prepare(&sql_meta)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let metadata: Vec<UsageMetadata> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(UsageMetadata {
                usage_id: r.get(0)?,
                key: r.get(1)?,
                value: r.get(2)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    Ok((sessions, messages, variants, usages, metadata))
}

fn fetch_group_sessions_for_protocol(
    conn: &DbConnection,
    ids: &[String],
    protocol_version: u32,
) -> Result<Vec<u8>, String> {
    let (sessions, participation, messages, variants, usages, metadata) =
        fetch_group_sessions_data(conn, ids)?;
    if protocol_version >= 4 {
        return bincode::serialize(&(
            sessions,
            participation,
            messages,
            variants,
            usages,
            metadata,
        ))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e));
    }

    let legacy_sessions = sessions
        .into_iter()
        .map(|s| LegacyGroupSessionV3 {
            id: s.id,
            name: s.name,
            character_ids: s.character_ids,
            muted_character_ids: s.muted_character_ids,
            persona_id: s.persona_id,
            created_at: s.created_at,
            updated_at: s.updated_at,
            archived: s.archived,
            chat_type: s.chat_type,
            starting_scene: s.starting_scene,
            background_image_path: s.background_image_path,
            memories: s.memories,
            memory_embeddings: s.memory_embeddings,
            memory_summary: s.memory_summary,
            memory_summary_token_count: s.memory_summary_token_count,
            memory_tool_events: s.memory_tool_events,
        })
        .collect::<Vec<_>>();
    let legacy_usages = usages
        .into_iter()
        .map(|u| LegacyUsageRecordV2 {
            id: u.id,
            timestamp: u.timestamp,
            session_id: u.session_id,
            character_id: u.character_id,
            character_name: u.character_name,
            model_id: u.model_id,
            model_name: u.model_name,
            provider_id: u.provider_id,
            provider_label: u.provider_label,
            operation_type: u.operation_type,
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
            memory_tokens: u.memory_tokens,
            summary_tokens: u.summary_tokens,
            reasoning_tokens: u.reasoning_tokens,
            image_tokens: u.image_tokens,
            prompt_cost: u.prompt_cost,
            completion_cost: u.completion_cost,
            total_cost: u.total_cost,
            success: u.success,
            error_message: u.error_message,
        })
        .collect::<Vec<_>>();
    bincode::serialize(&(
        legacy_sessions,
        participation,
        messages,
        variants,
        legacy_usages,
        metadata,
    ))
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn fetch_group_sessions_data(
    conn: &DbConnection,
    ids: &[String],
) -> Result<
    (
        Vec<GroupSession>,
        Vec<GroupParticipation>,
        Vec<GroupMessage>,
        Vec<GroupMessageVariant>,
        Vec<UsageRecord>,
        Vec<UsageMetadata>,
    ),
    String,
> {
    if ids.is_empty() {
        return Ok((
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ));
    }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

    let sql = format!("SELECT id, group_character_id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived, chat_type, starting_scene, background_image_path, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, COALESCE(speaker_selection_method, 'llm'), COALESCE(memory_type, 'manual') FROM group_sessions WHERE id IN ({})", placeholders);
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let sessions: Vec<GroupSession> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(GroupSession {
                id: r.get(0)?,
                group_character_id: r.get(1)?,
                name: r.get(2)?,
                character_ids: r.get(3)?,
                muted_character_ids: r.get(4)?,
                persona_id: r.get(5)?,
                created_at: r.get(6)?,
                updated_at: r.get(7)?,
                archived: r.get(8)?,
                chat_type: r.get(9)?,
                starting_scene: r.get(10)?,
                background_image_path: r.get(11)?,
                memories: r.get(12)?,
                memory_embeddings: r.get(13)?,
                memory_summary: r.get(14)?,
                memory_summary_token_count: r.get(15)?,
                memory_tool_events: r.get(16)?,
                speaker_selection_method: r.get(17)?,
                memory_type: r.get(18)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    let sql_part = format!("SELECT id, session_id, character_id, speak_count, last_spoke_turn, last_spoke_at FROM group_participation WHERE session_id IN ({})", placeholders);
    let mut stmt = conn
        .prepare(&sql_part)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let participation: Vec<GroupParticipation> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(GroupParticipation {
                id: r.get(0)?,
                session_id: r.get(1)?,
                character_id: r.get(2)?,
                speak_count: r.get(3)?,
                last_spoke_turn: r.get(4)?,
                last_spoke_at: r.get(5)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    let sql_msg = format!("SELECT id, session_id, role, content, speaker_character_id, turn_number, created_at, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned, attachments, used_lorebook_entries, reasoning, selection_reasoning, model_id FROM group_messages WHERE session_id IN ({})", placeholders);
    let mut stmt = conn
        .prepare(&sql_msg)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let messages: Vec<GroupMessage> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(GroupMessage {
                id: r.get(0)?,
                session_id: r.get(1)?,
                role: r.get(2)?,
                content: r.get(3)?,
                speaker_character_id: r.get(4)?,
                turn_number: r.get(5)?,
                created_at: r.get(6)?,
                prompt_tokens: r.get(7)?,
                completion_tokens: r.get(8)?,
                total_tokens: r.get(9)?,
                selected_variant_id: r.get(10)?,
                is_pinned: r.get(11)?,
                attachments: r.get(12)?,
                used_lorebook_entries: r.get(13)?,
                reasoning: r.get(14)?,
                selection_reasoning: r.get(15)?,
                model_id: r.get(16)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    let sql_var = format!("SELECT id, message_id, content, speaker_character_id, created_at, prompt_tokens, completion_tokens, total_tokens, reasoning, selection_reasoning, model_id FROM group_message_variants WHERE message_id IN (SELECT id FROM group_messages WHERE session_id IN ({}))", placeholders);
    let mut stmt = conn
        .prepare(&sql_var)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let variants: Vec<GroupMessageVariant> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(GroupMessageVariant {
                id: r.get(0)?,
                message_id: r.get(1)?,
                content: r.get(2)?,
                speaker_character_id: r.get(3)?,
                created_at: r.get(4)?,
                prompt_tokens: r.get(5)?,
                completion_tokens: r.get(6)?,
                total_tokens: r.get(7)?,
                reasoning: r.get(8)?,
                selection_reasoning: r.get(9)?,
                model_id: r.get(10)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    let sql_usage = format!("SELECT id, timestamp, session_id, character_id, character_name, model_id, model_name, provider_id, provider_label, operation_type, finish_reason, prompt_tokens, completion_tokens, total_tokens, memory_tokens, summary_tokens, reasoning_tokens, image_tokens, prompt_cost, completion_cost, total_cost, success, error_message FROM usage_records WHERE session_id IN ({})", placeholders);
    let mut stmt = conn
        .prepare(&sql_usage)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let usages: Vec<UsageRecord> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(UsageRecord {
                id: r.get(0)?,
                timestamp: r.get(1)?,
                session_id: r.get(2)?,
                character_id: r.get(3)?,
                character_name: r.get(4)?,
                model_id: r.get(5)?,
                model_name: r.get(6)?,
                provider_id: r.get(7)?,
                provider_label: r.get(8)?,
                operation_type: r.get(9)?,
                finish_reason: r.get(10)?,
                prompt_tokens: r.get(11)?,
                completion_tokens: r.get(12)?,
                total_tokens: r.get(13)?,
                memory_tokens: r.get(14)?,
                summary_tokens: r.get(15)?,
                reasoning_tokens: r.get(16)?,
                image_tokens: r.get(17)?,
                prompt_cost: r.get(18)?,
                completion_cost: r.get(19)?,
                total_cost: r.get(20)?,
                success: r.get(21)?,
                error_message: r.get(22)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    let sql_meta = format!("SELECT usage_id, key, value FROM usage_metadata WHERE usage_id IN (SELECT id FROM usage_records WHERE session_id IN ({}))", placeholders);
    let mut stmt = conn
        .prepare(&sql_meta)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let metadata: Vec<UsageMetadata> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(UsageMetadata {
                usage_id: r.get(0)?,
                key: r.get(1)?,
                value: r.get(2)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    Ok((
        sessions,
        participation,
        messages,
        variants,
        usages,
        metadata,
    ))
}

pub fn apply_layer_data(
    conn: &mut DbConnection,
    layer: SyncLayer,
    data: &[u8],
) -> Result<(), String> {
    match layer {
        SyncLayer::Globals => apply_globals(conn, data),
        SyncLayer::Lorebooks => apply_lorebooks(conn, data),
        SyncLayer::Characters => apply_characters(conn, data),
        SyncLayer::Sessions => apply_sessions(conn, data),
        SyncLayer::GroupSessions => apply_group_sessions(conn, data),
    }
}

type LegacyGlobalsDataV1 = (
    Vec<Settings>,
    Vec<Persona>,
    Vec<Model>,
    Vec<Secret>,
    Vec<ProviderCredential>,
    Vec<PromptTemplate>,
    Vec<ModelPricingCache>,
);

type LegacyGlobalsDataV0 = (
    Vec<Settings>,
    Vec<Persona>,
    Vec<Model>,
    Vec<Secret>,
    Vec<ProviderCredential>,
    Vec<LegacyPromptTemplate>,
    Vec<ModelPricingCache>,
);

fn apply_globals(conn: &mut DbConnection, data: &[u8]) -> Result<(), String> {
    let (
        meta,
        settings,
        personas,
        models,
        secrets,
        creds,
        templates,
        pricing,
        audio_providers,
        voice_cache,
        user_voices,
        helper_sessions,
        groups,
    ): GlobalsDataV5 = match bincode::deserialize(data) {
        Ok(payload) => payload,
        Err(_) => {
            if let Ok((
                meta,
                settings,
                personas,
                models,
                secrets,
                creds,
                templates,
                pricing,
                helper_sessions,
                groups,
            )) = bincode::deserialize::<GlobalsDataV4>(data)
            {
                (
                    meta,
                    settings,
                    personas,
                    models,
                    secrets,
                    creds,
                    templates,
                    pricing,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    helper_sessions,
                    groups,
                )
            } else if let Ok((
                settings,
                personas,
                models,
                secrets,
                creds,
                templates,
                pricing,
                audio_providers,
                voice_cache,
                user_voices,
            )) = bincode::deserialize::<LegacyGlobalsDataV3>(data)
            {
                (
                    Vec::new(),
                    settings,
                    personas,
                    models,
                    secrets,
                    creds,
                    templates,
                    pricing,
                    audio_providers,
                    voice_cache,
                    user_voices,
                    Vec::new(),
                    Vec::new(),
                )
            } else if let Ok((settings, personas, models, secrets, creds, templates, pricing)) =
                bincode::deserialize::<LegacyGlobalsDataV1>(data)
            {
                (
                    Vec::new(),
                    settings,
                    personas,
                    models,
                    secrets,
                    creds,
                    templates,
                    pricing,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )
            } else {
                let legacy: LegacyGlobalsDataV0 = bincode::deserialize(data)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                let (settings, personas, models, secrets, creds, legacy_templates, pricing) =
                    legacy;
                let templates = legacy_templates
                    .into_iter()
                    .map(|template| PromptTemplate {
                        id: template.id,
                        name: template.name,
                        scope: template.scope,
                        target_ids: template.target_ids,
                        content: template.content,
                        entries: "[]".to_string(),
                        condense_prompt_entries: 0,
                        created_at: template.created_at,
                        updated_at: template.updated_at,
                    })
                    .collect();
                (
                    Vec::new(),
                    settings,
                    personas,
                    models,
                    secrets,
                    creds,
                    templates,
                    pricing,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )
            }
        }
    };

    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for item in meta {
        tx.execute(
            r#"INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)"#,
            params![item.key, item.value],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    // Settings (ID=1)
    if let Some(s) = settings.first() {
        tx.execute(r#"INSERT OR REPLACE INTO settings (id, default_provider_credential_id, default_model_id, app_state, prompt_template_id, system_prompt, advanced_settings, migration_version, created_at, updated_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
                    params![s.id, s.default_provider_credential_id, s.default_model_id, s.app_state, s.prompt_template_id, s.system_prompt, s.advanced_settings, s.migration_version, s.created_at, s.updated_at])
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    // Personas
    for p in personas {
        tx.execute(r#"INSERT OR REPLACE INTO personas (id, title, description, nickname, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, is_default, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
                    params![p.id, p.title, p.description, p.nickname, p.avatar_path, p.avatar_crop_x, p.avatar_crop_y, p.avatar_crop_scale, p.is_default, p.created_at, p.updated_at]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    // Models
    for m in models {
        tx.execute(r#"INSERT OR REPLACE INTO models (id, name, provider_id, provider_credential_id, provider_label, display_name, created_at, model_type, input_scopes, output_scopes, advanced_model_settings, prompt_template_id, system_prompt)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#,
                     params![m.id, m.name, m.provider_id, m.provider_credential_id, m.provider_label, m.display_name, m.created_at, m.model_type, m.input_scopes, m.output_scopes, m.advanced_model_settings, m.prompt_template_id, m.system_prompt]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    // Secrets
    for s in secrets {
        tx.execute(r#"INSERT OR REPLACE INTO secrets (service, account, value, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)"#,
                   params![s.service, s.account, s.value, s.created_at, s.updated_at]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    // Provider Credentials
    for c in creds {
        tx.execute(r#"INSERT OR REPLACE INTO provider_credentials (id, provider_id, label, api_key_ref, api_key, base_url, default_model, headers, config)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
                    params![c.id, c.provider_id, c.label, c.api_key_ref, c.api_key, c.base_url, c.default_model, c.headers, c.config]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    // Prompt Templates
    for t in templates {
        tx.execute(r#"INSERT OR REPLACE INTO prompt_templates (id, name, scope, target_ids, content, entries, condense_prompt_entries, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
                   params![t.id, t.name, t.scope, t.target_ids, t.content, t.entries, t.condense_prompt_entries, t.created_at, t.updated_at]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    // Pricing
    for p in pricing {
        tx.execute(r#"INSERT OR REPLACE INTO model_pricing_cache (model_id, pricing_json, cached_at) VALUES (?1, ?2, ?3)"#,
                   params![p.model_id, p.pricing_json, p.cached_at]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for provider in audio_providers {
        tx.execute(
            r#"INSERT OR REPLACE INTO audio_providers (id, provider_type, label, api_key, project_id, location, created_at, updated_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            params![
                provider.id,
                provider.provider_type,
                provider.label,
                provider.api_key,
                provider.project_id,
                provider.location,
                provider.created_at,
                provider.updated_at
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for voice in voice_cache {
        tx.execute(
            r#"INSERT OR REPLACE INTO audio_voice_cache (id, provider_id, voice_id, name, preview_url, labels, cached_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
            params![
                voice.id,
                voice.provider_id,
                voice.voice_id,
                voice.name,
                voice.preview_url,
                voice.labels,
                voice.cached_at
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for voice in user_voices {
        tx.execute(
            r#"INSERT OR REPLACE INTO user_voices (id, provider_id, name, model_id, voice_id, prompt, created_at, updated_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            params![
                voice.id,
                voice.provider_id,
                voice.name,
                voice.model_id,
                voice.voice_id,
                voice.prompt,
                voice.created_at,
                voice.updated_at
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for session in helper_sessions {
        tx.execute(r#"INSERT OR REPLACE INTO creation_helper_sessions (id, creation_goal, status, session_json, uploaded_images_json, created_at, updated_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
                    params![session.id, session.creation_goal, session.status, session.session_json, session.uploaded_images_json, session.created_at, session.updated_at]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for group in groups {
        tx.execute(r#"INSERT OR REPLACE INTO group_characters (id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived, chat_type, starting_scene, background_image_path, speaker_selection_method, memory_type)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#,
                    params![group.id, group.name, group.character_ids, group.muted_character_ids, group.persona_id, group.created_at, group.updated_at, group.archived, group.chat_type, group.starting_scene, group.background_image_path, group.speaker_selection_method, group.memory_type]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

fn apply_lorebooks(conn: &mut DbConnection, data: &[u8]) -> Result<(), String> {
    let (lorebooks, entries): (Vec<SyncLorebook>, Vec<SyncLorebookEntry>) =
        bincode::deserialize(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for l in lorebooks {
        tx.execute(r#"INSERT OR REPLACE INTO lorebooks (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)"#,
                   params![l.id, l.name, l.created_at, l.updated_at]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for e in entries {
        tx.execute(r#"INSERT OR REPLACE INTO lorebook_entries (id, lorebook_id, title, enabled, always_active, keywords, case_sensitive, content, priority, display_order, created_at, updated_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"#,
                    params![e.id, e.lorebook_id, e.title, e.enabled, e.always_active, e.keywords, e.case_sensitive, e.content, e.priority, e.display_order, e.created_at, e.updated_at]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

type CharactersData = (
    Vec<Character>,
    Vec<CharacterRule>,
    Vec<Scene>,
    Vec<SceneVariant>,
    Vec<CharacterLorebookLink>,
    Vec<ChatTemplate>,
    Vec<ChatTemplateMessage>,
);

type LegacyCharactersDataV2 = (
    Vec<LegacyCharacterV2>,
    Vec<CharacterRule>,
    Vec<LegacySceneV2>,
    Vec<LegacySceneVariantV2>,
    Vec<CharacterLorebookLink>,
);

type LegacyCharactersDataV1 = (
    Vec<LegacyCharacterV1>,
    Vec<CharacterRule>,
    Vec<LegacySceneV1>,
    Vec<LegacySceneVariantV2>,
    Vec<CharacterLorebookLink>,
);

type LegacyCharactersDataV0 = (
    Vec<LegacyCharacterV0>,
    Vec<CharacterRule>,
    Vec<LegacySceneV1>,
    Vec<LegacySceneVariantV2>,
    Vec<CharacterLorebookLink>,
);

type LegacyCharactersDataVMinus1 = (
    Vec<LegacyCharacterVMinus1>,
    Vec<CharacterRule>,
    Vec<LegacySceneV0>,
    Vec<LegacySceneVariantV2>,
    Vec<CharacterLorebookLink>,
);

#[derive(serde::Serialize, serde::Deserialize)]
struct LegacyCharacterV2 {
    pub id: String,
    pub name: String,
    pub avatar_path: Option<String>,
    pub avatar_crop_x: Option<f64>,
    pub avatar_crop_y: Option<f64>,
    pub avatar_crop_scale: Option<f64>,
    pub background_image_path: Option<String>,
    pub definition: Option<String>,
    pub description: Option<String>,
    pub default_scene_id: Option<String>,
    pub default_model_id: Option<String>,
    pub memory_type: String,
    pub prompt_template_id: Option<String>,
    pub system_prompt: Option<String>,
    pub voice_config: Option<String>,
    pub voice_autoplay: i64,
    pub disable_avatar_gradient: i64,
    pub custom_gradient_enabled: Option<i64>,
    pub custom_gradient_colors: Option<String>,
    pub custom_text_color: Option<String>,
    pub custom_text_secondary: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LegacyCharacterV1 {
    pub id: String,
    pub name: String,
    pub avatar_path: Option<String>,
    pub avatar_crop_x: Option<f64>,
    pub avatar_crop_y: Option<f64>,
    pub avatar_crop_scale: Option<f64>,
    pub background_image_path: Option<String>,
    pub description: Option<String>,
    pub definition: Option<String>,
    pub default_scene_id: Option<String>,
    pub default_model_id: Option<String>,
    pub memory_type: String,
    pub prompt_template_id: Option<String>,
    pub system_prompt: Option<String>,
    pub voice_config: Option<String>,
    pub voice_autoplay: i64,
    pub disable_avatar_gradient: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LegacyCharacterV0 {
    pub id: String,
    pub name: String,
    pub avatar_path: Option<String>,
    pub avatar_crop_x: Option<f64>,
    pub avatar_crop_y: Option<f64>,
    pub avatar_crop_scale: Option<f64>,
    pub background_image_path: Option<String>,
    pub description: Option<String>,
    pub definition: Option<String>,
    pub default_scene_id: Option<String>,
    pub default_model_id: Option<String>,
    pub memory_type: String,
    pub prompt_template_id: Option<String>,
    pub system_prompt: Option<String>,
    pub voice_config: Option<String>,
    pub disable_avatar_gradient: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LegacyCharacterVMinus1 {
    pub id: String,
    pub name: String,
    pub avatar_path: Option<String>,
    pub background_image_path: Option<String>,
    pub description: Option<String>,
    pub default_scene_id: Option<String>,
    pub default_model_id: Option<String>,
    pub prompt_template_id: Option<String>,
    pub system_prompt: Option<String>,
    pub voice_config: Option<String>,
    pub disable_avatar_gradient: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LegacySceneV2 {
    pub id: String,
    pub character_id: String,
    pub content: String,
    pub created_at: i64,
    pub selected_variant_id: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LegacySceneVariantV2 {
    pub id: String,
    pub scene_id: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LegacySceneV1 {
    pub id: String,
    pub character_id: String,
    pub content: String,
    pub created_at: i64,
    pub selected_variant_id: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LegacySceneV0 {
    pub id: String,
    pub character_id: String,
    pub content: String,
    pub created_at: i64,
}

fn apply_characters(conn: &mut DbConnection, data: &[u8]) -> Result<(), String> {
    let (chars, rules, scenes, variants, links, templates, template_messages) =
        deserialize_characters(data)?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for c in chars {
        tx.execute(r#"INSERT OR REPLACE INTO characters (id, name, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, background_image_path, definition, description, nickname, scenario, creator_notes, creator, creator_notes_multilingual, source, tags, default_scene_id, default_model_id, fallback_model_id, memory_type, prompt_template_id, system_prompt, voice_config, voice_autoplay, disable_avatar_gradient, custom_gradient_enabled, custom_gradient_colors, custom_text_color, custom_text_secondary, chat_appearance, default_chat_template_id, created_at, updated_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33)"#,
                    params![c.id, c.name, c.avatar_path, c.avatar_crop_x, c.avatar_crop_y, c.avatar_crop_scale, c.background_image_path, c.definition, c.description, c.nickname, c.scenario, c.creator_notes, c.creator, c.creator_notes_multilingual, c.source, c.tags, c.default_scene_id, c.default_model_id, c.fallback_model_id, c.memory_type, c.prompt_template_id, c.system_prompt, c.voice_config, c.voice_autoplay, c.disable_avatar_gradient, c.custom_gradient_enabled, c.custom_gradient_colors, c.custom_text_color, c.custom_text_secondary, c.chat_appearance, c.default_chat_template_id, c.created_at, c.updated_at]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for r in &rules {
        tx.execute(
            "DELETE FROM character_rules WHERE character_id = ?1",
            params![r.character_id],
        )
        .ok();
    }
    for r in rules {
        tx.execute(
            "INSERT INTO character_rules (character_id, idx, rule) VALUES (?1, ?2, ?3)",
            params![r.character_id, r.idx, r.rule],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for s in scenes {
        tx.execute(r#"INSERT OR REPLACE INTO scenes (id, character_id, content, direction, created_at, selected_variant_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
                    params![s.id, s.character_id, s.content, s.direction, s.created_at, s.selected_variant_id]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for v in variants {
        tx.execute(r#"INSERT OR REPLACE INTO scene_variants (id, scene_id, content, direction, created_at) VALUES (?1, ?2, ?3, ?4, ?5)"#,
                   params![v.id, v.scene_id, v.content, v.direction, v.created_at]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for l in links {
        tx.execute(r#"INSERT OR REPLACE INTO character_lorebooks (character_id, lorebook_id, enabled, display_order, created_at, updated_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
                    params![l.character_id, l.lorebook_id, l.enabled, l.display_order, l.created_at, l.updated_at]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for t in templates {
        tx.execute(r#"INSERT OR REPLACE INTO chat_templates (id, character_id, name, scene_id, prompt_template_id, created_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
                    params![t.id, t.character_id, t.name, t.scene_id, t.prompt_template_id, t.created_at]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for m in template_messages {
        tx.execute(
            r#"INSERT OR REPLACE INTO chat_template_messages (id, template_id, idx, role, content)
                    VALUES (?1, ?2, ?3, ?4, ?5)"#,
            params![m.id, m.template_id, m.idx, m.role, m.content],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

fn deserialize_characters(
    data: &[u8],
) -> Result<
    (
        Vec<Character>,
        Vec<CharacterRule>,
        Vec<Scene>,
        Vec<SceneVariant>,
        Vec<CharacterLorebookLink>,
        Vec<ChatTemplate>,
        Vec<ChatTemplateMessage>,
    ),
    String,
> {
    if let Ok(payload) = bincode::deserialize::<CharactersData>(data) {
        return Ok(payload);
    }

    if let Ok((chars, rules, scenes, variants, links)) =
        bincode::deserialize::<LegacyCharactersDataV2>(data)
    {
        let mapped_chars = chars
            .into_iter()
            .map(|c| Character {
                id: c.id,
                name: c.name,
                avatar_path: c.avatar_path,
                avatar_crop_x: c.avatar_crop_x,
                avatar_crop_y: c.avatar_crop_y,
                avatar_crop_scale: c.avatar_crop_scale,
                background_image_path: c.background_image_path,
                definition: c.definition,
                description: c.description,
                nickname: None,
                scenario: None,
                creator_notes: None,
                creator: None,
                creator_notes_multilingual: None,
                source: None,
                tags: None,
                default_scene_id: c.default_scene_id,
                default_model_id: c.default_model_id,
                fallback_model_id: None,
                memory_type: c.memory_type,
                prompt_template_id: c.prompt_template_id,
                system_prompt: c.system_prompt,
                voice_config: c.voice_config,
                voice_autoplay: c.voice_autoplay,
                disable_avatar_gradient: c.disable_avatar_gradient,
                custom_gradient_enabled: c.custom_gradient_enabled,
                custom_gradient_colors: c.custom_gradient_colors,
                custom_text_color: c.custom_text_color,
                custom_text_secondary: c.custom_text_secondary,
                chat_appearance: None,
                default_chat_template_id: None,
                created_at: c.created_at,
                updated_at: c.updated_at,
            })
            .collect();
        let mapped_scenes = scenes
            .into_iter()
            .map(|s| Scene {
                id: s.id,
                character_id: s.character_id,
                content: s.content,
                direction: None,
                created_at: s.created_at,
                selected_variant_id: s.selected_variant_id,
            })
            .collect();
        let mapped_variants = variants
            .into_iter()
            .map(|v| SceneVariant {
                id: v.id,
                scene_id: v.scene_id,
                content: v.content,
                direction: None,
                created_at: v.created_at,
            })
            .collect();
        return Ok((
            mapped_chars,
            rules,
            mapped_scenes,
            mapped_variants,
            links,
            Vec::new(),
            Vec::new(),
        ));
    }

    if let Ok((chars, rules, scenes, variants, links)) =
        bincode::deserialize::<LegacyCharactersDataV1>(data)
    {
        let mapped_chars = chars
            .into_iter()
            .map(|c| Character {
                id: c.id,
                name: c.name,
                avatar_path: c.avatar_path,
                avatar_crop_x: c.avatar_crop_x,
                avatar_crop_y: c.avatar_crop_y,
                avatar_crop_scale: c.avatar_crop_scale,
                background_image_path: c.background_image_path,
                definition: c.definition,
                description: c.description,
                nickname: None,
                scenario: None,
                creator_notes: None,
                creator: None,
                creator_notes_multilingual: None,
                source: None,
                tags: None,
                default_scene_id: c.default_scene_id,
                default_model_id: c.default_model_id,
                fallback_model_id: None,
                memory_type: c.memory_type,
                prompt_template_id: c.prompt_template_id,
                system_prompt: c.system_prompt,
                voice_config: c.voice_config,
                voice_autoplay: c.voice_autoplay,
                disable_avatar_gradient: c.disable_avatar_gradient,
                custom_gradient_enabled: None,
                custom_gradient_colors: None,
                custom_text_color: None,
                custom_text_secondary: None,
                chat_appearance: None,
                default_chat_template_id: None,
                created_at: c.created_at,
                updated_at: c.updated_at,
            })
            .collect();
        let mapped_scenes = scenes
            .into_iter()
            .map(|s| Scene {
                id: s.id,
                character_id: s.character_id,
                content: s.content,
                direction: None,
                created_at: s.created_at,
                selected_variant_id: s.selected_variant_id,
            })
            .collect();
        let mapped_variants = variants
            .into_iter()
            .map(|v| SceneVariant {
                id: v.id,
                scene_id: v.scene_id,
                content: v.content,
                direction: None,
                created_at: v.created_at,
            })
            .collect();
        return Ok((
            mapped_chars,
            rules,
            mapped_scenes,
            mapped_variants,
            links,
            Vec::new(),
            Vec::new(),
        ));
    }

    if let Ok((chars, rules, scenes, variants, links)) =
        bincode::deserialize::<LegacyCharactersDataV0>(data)
    {
        let mapped_chars = chars
            .into_iter()
            .map(|c| Character {
                id: c.id,
                name: c.name,
                avatar_path: c.avatar_path,
                avatar_crop_x: c.avatar_crop_x,
                avatar_crop_y: c.avatar_crop_y,
                avatar_crop_scale: c.avatar_crop_scale,
                background_image_path: c.background_image_path,
                definition: c.definition,
                description: c.description,
                nickname: None,
                scenario: None,
                creator_notes: None,
                creator: None,
                creator_notes_multilingual: None,
                source: None,
                tags: None,
                default_scene_id: c.default_scene_id,
                default_model_id: c.default_model_id,
                fallback_model_id: None,
                memory_type: c.memory_type,
                prompt_template_id: c.prompt_template_id,
                system_prompt: c.system_prompt,
                voice_config: c.voice_config,
                voice_autoplay: 0,
                disable_avatar_gradient: c.disable_avatar_gradient,
                custom_gradient_enabled: None,
                custom_gradient_colors: None,
                custom_text_color: None,
                custom_text_secondary: None,
                chat_appearance: None,
                default_chat_template_id: None,
                created_at: c.created_at,
                updated_at: c.updated_at,
            })
            .collect();
        let mapped_scenes = scenes
            .into_iter()
            .map(|s| Scene {
                id: s.id,
                character_id: s.character_id,
                content: s.content,
                direction: None,
                created_at: s.created_at,
                selected_variant_id: s.selected_variant_id,
            })
            .collect();
        let mapped_variants = variants
            .into_iter()
            .map(|v| SceneVariant {
                id: v.id,
                scene_id: v.scene_id,
                content: v.content,
                direction: None,
                created_at: v.created_at,
            })
            .collect();
        return Ok((
            mapped_chars,
            rules,
            mapped_scenes,
            mapped_variants,
            links,
            Vec::new(),
            Vec::new(),
        ));
    }

    if let Ok((chars, rules, scenes, variants, links)) =
        bincode::deserialize::<LegacyCharactersDataVMinus1>(data)
    {
        let mapped_chars = chars
            .into_iter()
            .map(|c| Character {
                id: c.id,
                name: c.name,
                avatar_path: c.avatar_path,
                avatar_crop_x: None,
                avatar_crop_y: None,
                avatar_crop_scale: None,
                background_image_path: c.background_image_path,
                definition: None,
                description: c.description,
                nickname: None,
                scenario: None,
                creator_notes: None,
                creator: None,
                creator_notes_multilingual: None,
                source: None,
                tags: None,
                default_scene_id: c.default_scene_id,
                default_model_id: c.default_model_id,
                fallback_model_id: None,
                memory_type: "manual".to_string(),
                prompt_template_id: c.prompt_template_id,
                system_prompt: c.system_prompt,
                voice_config: c.voice_config,
                voice_autoplay: 0,
                disable_avatar_gradient: c.disable_avatar_gradient,
                custom_gradient_enabled: None,
                custom_gradient_colors: None,
                custom_text_color: None,
                custom_text_secondary: None,
                chat_appearance: None,
                default_chat_template_id: None,
                created_at: c.created_at,
                updated_at: c.updated_at,
            })
            .collect();
        let mapped_scenes = scenes
            .into_iter()
            .map(|s| Scene {
                id: s.id,
                character_id: s.character_id,
                content: s.content,
                direction: None,
                created_at: s.created_at,
                selected_variant_id: None,
            })
            .collect();
        let mapped_variants = variants
            .into_iter()
            .map(|v| SceneVariant {
                id: v.id,
                scene_id: v.scene_id,
                content: v.content,
                direction: None,
                created_at: v.created_at,
            })
            .collect();
        return Ok((
            mapped_chars,
            rules,
            mapped_scenes,
            mapped_variants,
            links,
            Vec::new(),
            Vec::new(),
        ));
    }

    bincode::deserialize::<CharactersData>(data)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

type SessionsData = (
    Vec<Session>,
    Vec<Message>,
    Vec<MessageVariant>,
    Vec<UsageRecord>,
    Vec<UsageMetadata>,
);

type LegacySessionsDataV2 = (
    Vec<LegacySessionV2>,
    Vec<Message>,
    Vec<MessageVariant>,
    Vec<LegacyUsageRecordV2>,
    Vec<UsageMetadata>,
);

type LegacySessionsDataV1 = (
    Vec<LegacySessionV1>,
    Vec<LegacyMessageV1>,
    Vec<LegacyMessageVariantV1>,
    Vec<LegacyUsageRecordV2>,
    Vec<UsageMetadata>,
);

type LegacySessionsDataV0 = (
    Vec<LegacySessionV0>,
    Vec<LegacyMessageV1>,
    Vec<LegacyMessageVariantV1>,
    Vec<LegacyUsageRecordV2>,
    Vec<UsageMetadata>,
);

type GroupSessionsData = (
    Vec<GroupSession>,
    Vec<GroupParticipation>,
    Vec<GroupMessage>,
    Vec<GroupMessageVariant>,
    Vec<UsageRecord>,
    Vec<UsageMetadata>,
);

type LegacyGroupSessionsDataV3 = (
    Vec<LegacyGroupSessionV3>,
    Vec<GroupParticipation>,
    Vec<GroupMessage>,
    Vec<GroupMessageVariant>,
    Vec<LegacyUsageRecordV2>,
    Vec<UsageMetadata>,
);

#[derive(serde::Serialize, serde::Deserialize)]
struct LegacySessionV2 {
    pub id: String,
    pub character_id: String,
    pub title: String,
    pub system_prompt: Option<String>,
    pub selected_scene_id: Option<String>,
    pub persona_id: Option<String>,
    pub persona_disabled: Option<i64>,
    pub voice_autoplay: Option<i64>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub max_output_tokens: Option<i64>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub top_k: Option<i64>,
    pub memories: String,
    pub memory_embeddings: String,
    pub memory_summary: Option<String>,
    pub memory_summary_token_count: i64,
    pub memory_tool_events: String,
    pub archived: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub memory_status: Option<String>,
    pub memory_error: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LegacySessionV1 {
    pub id: String,
    pub character_id: String,
    pub title: String,
    pub system_prompt: Option<String>,
    pub selected_scene_id: Option<String>,
    pub persona_id: Option<String>,
    pub persona_disabled: Option<i64>,
    pub voice_autoplay: Option<i64>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub max_output_tokens: Option<i64>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub top_k: Option<i64>,
    pub memories: String,
    pub memory_embeddings: String,
    pub memory_summary: Option<String>,
    pub memory_summary_token_count: i64,
    pub memory_tool_events: String,
    pub archived: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LegacySessionV0 {
    pub id: String,
    pub character_id: String,
    pub title: String,
    pub system_prompt: Option<String>,
    pub selected_scene_id: Option<String>,
    pub persona_id: Option<String>,
    pub persona_disabled: Option<i64>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub max_output_tokens: Option<i64>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub top_k: Option<i64>,
    pub memories: String,
    pub memory_embeddings: String,
    pub memory_summary: Option<String>,
    pub memory_summary_token_count: i64,
    pub memory_tool_events: String,
    pub archived: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LegacyUsageRecordV2 {
    pub id: String,
    pub timestamp: i64,
    pub session_id: String,
    pub character_id: String,
    pub character_name: String,
    pub model_id: String,
    pub model_name: String,
    pub provider_id: String,
    pub provider_label: String,
    pub operation_type: Option<String>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub memory_tokens: Option<i64>,
    pub summary_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub image_tokens: Option<i64>,
    pub prompt_cost: Option<f64>,
    pub completion_cost: Option<f64>,
    pub total_cost: Option<f64>,
    pub success: i64,
    pub error_message: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LegacyGroupSessionV3 {
    pub id: String,
    pub name: String,
    pub character_ids: String,
    pub muted_character_ids: String,
    pub persona_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived: i64,
    pub chat_type: String,
    pub starting_scene: Option<String>,
    pub background_image_path: Option<String>,
    pub memories: String,
    pub memory_embeddings: String,
    pub memory_summary: String,
    pub memory_summary_token_count: i64,
    pub memory_tool_events: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LegacyMessageV1 {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: i64,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub selected_variant_id: Option<String>,
    pub is_pinned: i64,
    pub memory_refs: String,
    pub attachments: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LegacyMessageVariantV1 {
    pub id: String,
    pub message_id: String,
    pub content: String,
    pub created_at: i64,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
}

fn apply_sessions(conn: &mut DbConnection, data: &[u8]) -> Result<(), String> {
    let (sessions, messages, variants, usages, metadata) = deserialize_sessions(data)?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for s in sessions {
        tx.execute(r#"INSERT OR REPLACE INTO sessions (id, character_id, title, system_prompt, selected_scene_id, prompt_template_id, persona_id, persona_disabled, voice_autoplay, temperature, top_p, max_output_tokens, frequency_penalty, presence_penalty, top_k, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, archived, created_at, updated_at, memory_status, memory_error)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)"#,
                    params![s.id, s.character_id, s.title, s.system_prompt, s.selected_scene_id, s.prompt_template_id, s.persona_id, s.persona_disabled, s.voice_autoplay, s.temperature, s.top_p, s.max_output_tokens, s.frequency_penalty, s.presence_penalty, s.top_k, s.memories, s.memory_embeddings, s.memory_summary, s.memory_summary_token_count, s.memory_tool_events, s.archived, s.created_at, s.updated_at, s.memory_status, s.memory_error]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for m in messages {
        let used_lorebook_entries = if m.used_lorebook_entries.is_empty() {
            "[]".to_string()
        } else {
            m.used_lorebook_entries.clone()
        };
        tx.execute(r#"INSERT OR REPLACE INTO messages (id, session_id, role, content, created_at, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned, memory_refs, used_lorebook_entries, attachments, reasoning)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"#,
                    params![m.id, m.session_id, m.role, m.content, m.created_at, m.prompt_tokens, m.completion_tokens, m.total_tokens, m.selected_variant_id, m.is_pinned, m.memory_refs, used_lorebook_entries, m.attachments, m.reasoning]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for v in variants {
        tx.execute(r#"INSERT OR REPLACE INTO message_variants (id, message_id, content, created_at, prompt_tokens, completion_tokens, total_tokens, reasoning)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
                    params![v.id, v.message_id, v.content, v.created_at, v.prompt_tokens, v.completion_tokens, v.total_tokens, v.reasoning]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for u in usages {
        tx.execute(r#"INSERT OR REPLACE INTO usage_records (id, timestamp, session_id, character_id, character_name, model_id, model_name, provider_id, provider_label, operation_type, finish_reason, prompt_tokens, completion_tokens, total_tokens, memory_tokens, summary_tokens, reasoning_tokens, image_tokens, prompt_cost, completion_cost, total_cost, success, error_message)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)"#,
                    params![u.id, u.timestamp, u.session_id, u.character_id, u.character_name, u.model_id, u.model_name, u.provider_id, u.provider_label, u.operation_type, u.finish_reason, u.prompt_tokens, u.completion_tokens, u.total_tokens, u.memory_tokens, u.summary_tokens, u.reasoning_tokens, u.image_tokens, u.prompt_cost, u.completion_cost, u.total_cost, u.success, u.error_message]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for md in metadata {
        tx.execute(
            r#"INSERT OR REPLACE INTO usage_metadata (usage_id, key, value) VALUES (?1, ?2, ?3)"#,
            params![md.usage_id, md.key, md.value],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

fn apply_group_sessions(conn: &mut DbConnection, data: &[u8]) -> Result<(), String> {
    if data.is_empty() {
        return Ok(());
    }

    let (sessions, participation, messages, variants, usages, metadata) =
        deserialize_group_sessions(data)?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for s in sessions {
        tx.execute(r#"INSERT OR REPLACE INTO group_sessions (id, group_character_id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived, chat_type, starting_scene, background_image_path, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, speaker_selection_method, memory_type)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)"#,
                    params![s.id, s.group_character_id, s.name, s.character_ids, s.muted_character_ids, s.persona_id, s.created_at, s.updated_at, s.archived, s.chat_type, s.starting_scene, s.background_image_path, s.memories, s.memory_embeddings, s.memory_summary, s.memory_summary_token_count, s.memory_tool_events, s.speaker_selection_method, s.memory_type]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for p in participation {
        tx.execute(r#"INSERT OR REPLACE INTO group_participation (id, session_id, character_id, speak_count, last_spoke_turn, last_spoke_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
                    params![p.id, p.session_id, p.character_id, p.speak_count, p.last_spoke_turn, p.last_spoke_at]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for m in messages {
        tx.execute(r#"INSERT OR REPLACE INTO group_messages (id, session_id, role, content, speaker_character_id, turn_number, created_at, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned, attachments, reasoning, selection_reasoning, model_id)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)"#,
                    params![m.id, m.session_id, m.role, m.content, m.speaker_character_id, m.turn_number, m.created_at, m.prompt_tokens, m.completion_tokens, m.total_tokens, m.selected_variant_id, m.is_pinned, m.attachments, m.reasoning, m.selection_reasoning, m.model_id]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for v in variants {
        tx.execute(r#"INSERT OR REPLACE INTO group_message_variants (id, message_id, content, speaker_character_id, created_at, prompt_tokens, completion_tokens, total_tokens, reasoning, selection_reasoning, model_id)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
                    params![v.id, v.message_id, v.content, v.speaker_character_id, v.created_at, v.prompt_tokens, v.completion_tokens, v.total_tokens, v.reasoning, v.selection_reasoning, v.model_id]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for u in usages {
        tx.execute(r#"INSERT OR REPLACE INTO usage_records (id, timestamp, session_id, character_id, character_name, model_id, model_name, provider_id, provider_label, operation_type, finish_reason, prompt_tokens, completion_tokens, total_tokens, memory_tokens, summary_tokens, reasoning_tokens, image_tokens, prompt_cost, completion_cost, total_cost, success, error_message)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)"#,
                    params![u.id, u.timestamp, u.session_id, u.character_id, u.character_name, u.model_id, u.model_name, u.provider_id, u.provider_label, u.operation_type, u.finish_reason, u.prompt_tokens, u.completion_tokens, u.total_tokens, u.memory_tokens, u.summary_tokens, u.reasoning_tokens, u.image_tokens, u.prompt_cost, u.completion_cost, u.total_cost, u.success, u.error_message]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for md in metadata {
        tx.execute(
            r#"INSERT OR REPLACE INTO usage_metadata (usage_id, key, value) VALUES (?1, ?2, ?3)"#,
            params![md.usage_id, md.key, md.value],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

fn deserialize_sessions(
    data: &[u8],
) -> Result<
    (
        Vec<Session>,
        Vec<Message>,
        Vec<MessageVariant>,
        Vec<UsageRecord>,
        Vec<UsageMetadata>,
    ),
    String,
> {
    if let Ok(payload) = bincode::deserialize::<SessionsData>(data) {
        return Ok(payload);
    }

    if let Ok((sessions, messages, variants, usages, metadata)) =
        bincode::deserialize::<LegacySessionsDataV2>(data)
    {
        let mapped_sessions = sessions
            .into_iter()
            .map(|s| Session {
                id: s.id,
                character_id: s.character_id,
                title: s.title,
                system_prompt: s.system_prompt,
                selected_scene_id: s.selected_scene_id,
                prompt_template_id: None,
                persona_id: s.persona_id,
                persona_disabled: s.persona_disabled,
                voice_autoplay: s.voice_autoplay,
                temperature: s.temperature,
                top_p: s.top_p,
                max_output_tokens: s.max_output_tokens,
                frequency_penalty: s.frequency_penalty,
                presence_penalty: s.presence_penalty,
                top_k: s.top_k,
                memories: s.memories,
                memory_embeddings: s.memory_embeddings,
                memory_summary: s.memory_summary,
                memory_summary_token_count: s.memory_summary_token_count,
                memory_tool_events: s.memory_tool_events,
                archived: s.archived,
                created_at: s.created_at,
                updated_at: s.updated_at,
                memory_status: s.memory_status,
                memory_error: s.memory_error,
            })
            .collect();
        let mapped_usages = usages.into_iter().map(map_legacy_usage).collect();
        return Ok((mapped_sessions, messages, variants, mapped_usages, metadata));
    }

    if let Ok((sessions, messages, variants, usages, metadata)) =
        bincode::deserialize::<LegacySessionsDataV1>(data)
    {
        let mapped_sessions = sessions
            .into_iter()
            .map(|s| Session {
                id: s.id,
                character_id: s.character_id,
                title: s.title,
                system_prompt: s.system_prompt,
                selected_scene_id: s.selected_scene_id,
                prompt_template_id: None,
                persona_id: s.persona_id,
                persona_disabled: s.persona_disabled,
                voice_autoplay: s.voice_autoplay,
                temperature: s.temperature,
                top_p: s.top_p,
                max_output_tokens: s.max_output_tokens,
                frequency_penalty: s.frequency_penalty,
                presence_penalty: s.presence_penalty,
                top_k: s.top_k,
                memories: s.memories,
                memory_embeddings: s.memory_embeddings,
                memory_summary: s.memory_summary,
                memory_summary_token_count: s.memory_summary_token_count,
                memory_tool_events: s.memory_tool_events,
                archived: s.archived,
                created_at: s.created_at,
                updated_at: s.updated_at,
                memory_status: None,
                memory_error: None,
            })
            .collect();
        let mapped_usages = usages.into_iter().map(map_legacy_usage).collect();
        let mapped_messages = messages
            .into_iter()
            .map(|m| Message {
                id: m.id,
                session_id: m.session_id,
                role: m.role,
                content: m.content,
                created_at: m.created_at,
                prompt_tokens: m.prompt_tokens,
                completion_tokens: m.completion_tokens,
                total_tokens: m.total_tokens,
                selected_variant_id: m.selected_variant_id,
                is_pinned: m.is_pinned,
                memory_refs: m.memory_refs,
                used_lorebook_entries: "[]".to_string(),
                attachments: m.attachments,
                reasoning: None,
            })
            .collect();
        let mapped_variants = variants
            .into_iter()
            .map(|v| MessageVariant {
                id: v.id,
                message_id: v.message_id,
                content: v.content,
                created_at: v.created_at,
                prompt_tokens: v.prompt_tokens,
                completion_tokens: v.completion_tokens,
                total_tokens: v.total_tokens,
                reasoning: None,
            })
            .collect();
        return Ok((
            mapped_sessions,
            mapped_messages,
            mapped_variants,
            mapped_usages,
            metadata,
        ));
    }

    if let Ok((sessions, messages, variants, usages, metadata)) =
        bincode::deserialize::<LegacySessionsDataV0>(data)
    {
        let mapped_sessions = sessions
            .into_iter()
            .map(|s| Session {
                id: s.id,
                character_id: s.character_id,
                title: s.title,
                system_prompt: s.system_prompt,
                selected_scene_id: s.selected_scene_id,
                prompt_template_id: None,
                persona_id: s.persona_id,
                persona_disabled: s.persona_disabled,
                voice_autoplay: None,
                temperature: s.temperature,
                top_p: s.top_p,
                max_output_tokens: s.max_output_tokens,
                frequency_penalty: s.frequency_penalty,
                presence_penalty: s.presence_penalty,
                top_k: s.top_k,
                memories: s.memories,
                memory_embeddings: s.memory_embeddings,
                memory_summary: s.memory_summary,
                memory_summary_token_count: s.memory_summary_token_count,
                memory_tool_events: s.memory_tool_events,
                archived: s.archived,
                created_at: s.created_at,
                updated_at: s.updated_at,
                memory_status: None,
                memory_error: None,
            })
            .collect();
        let mapped_usages = usages.into_iter().map(map_legacy_usage).collect();
        let mapped_messages = messages
            .into_iter()
            .map(|m| Message {
                id: m.id,
                session_id: m.session_id,
                role: m.role,
                content: m.content,
                created_at: m.created_at,
                prompt_tokens: m.prompt_tokens,
                completion_tokens: m.completion_tokens,
                total_tokens: m.total_tokens,
                selected_variant_id: m.selected_variant_id,
                is_pinned: m.is_pinned,
                memory_refs: m.memory_refs,
                used_lorebook_entries: "[]".to_string(),
                attachments: m.attachments,
                reasoning: None,
            })
            .collect();
        let mapped_variants = variants
            .into_iter()
            .map(|v| MessageVariant {
                id: v.id,
                message_id: v.message_id,
                content: v.content,
                created_at: v.created_at,
                prompt_tokens: v.prompt_tokens,
                completion_tokens: v.completion_tokens,
                total_tokens: v.total_tokens,
                reasoning: None,
            })
            .collect();
        return Ok((
            mapped_sessions,
            mapped_messages,
            mapped_variants,
            mapped_usages,
            metadata,
        ));
    }

    bincode::deserialize::<SessionsData>(data)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn deserialize_group_sessions(
    data: &[u8],
) -> Result<
    (
        Vec<GroupSession>,
        Vec<GroupParticipation>,
        Vec<GroupMessage>,
        Vec<GroupMessageVariant>,
        Vec<UsageRecord>,
        Vec<UsageMetadata>,
    ),
    String,
> {
    if let Ok(payload) = bincode::deserialize::<GroupSessionsData>(data) {
        return Ok(payload);
    }

    if let Ok((sessions, participation, messages, variants, usages, metadata)) =
        bincode::deserialize::<LegacyGroupSessionsDataV3>(data)
    {
        let mapped_sessions = sessions
            .into_iter()
            .map(|s| GroupSession {
                id: s.id,
                group_character_id: None,
                name: s.name,
                character_ids: s.character_ids,
                muted_character_ids: s.muted_character_ids,
                persona_id: s.persona_id,
                created_at: s.created_at,
                updated_at: s.updated_at,
                archived: s.archived,
                chat_type: s.chat_type,
                starting_scene: s.starting_scene,
                background_image_path: s.background_image_path,
                memories: s.memories,
                memory_embeddings: s.memory_embeddings,
                memory_summary: s.memory_summary,
                memory_summary_token_count: s.memory_summary_token_count,
                memory_tool_events: s.memory_tool_events,
                speaker_selection_method: "llm".to_string(),
                memory_type: "manual".to_string(),
            })
            .collect();
        let mapped_usages = usages.into_iter().map(map_legacy_usage).collect();
        return Ok((
            mapped_sessions,
            participation,
            messages,
            variants,
            mapped_usages,
            metadata,
        ));
    }

    bincode::deserialize::<GroupSessionsData>(data)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn map_legacy_usage(usage: LegacyUsageRecordV2) -> UsageRecord {
    UsageRecord {
        id: usage.id,
        timestamp: usage.timestamp,
        session_id: usage.session_id,
        character_id: usage.character_id,
        character_name: usage.character_name,
        model_id: usage.model_id,
        model_name: usage.model_name,
        provider_id: usage.provider_id,
        provider_label: usage.provider_label,
        operation_type: usage.operation_type,
        finish_reason: None,
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        memory_tokens: usage.memory_tokens,
        summary_tokens: usage.summary_tokens,
        reasoning_tokens: usage.reasoning_tokens,
        image_tokens: usage.image_tokens,
        prompt_cost: usage.prompt_cost,
        completion_cost: usage.completion_cost,
        total_cost: usage.total_cost,
        success: usage.success,
        error_message: usage.error_message,
    }
}

pub struct FileMeta {
    pub path: String,
}

pub fn scan_for_missing_files(conn: &DbConnection, app_handle: &tauri::AppHandle) -> Vec<String> {
    let mut missing = Vec::new();
    let storage_root = crate::storage_manager::legacy::storage_root(app_handle).unwrap_or_default();

    let mut check = |path: Option<String>| {
        if let Some(p) = path {
            if !p.starts_with("http") {
                let full_path = storage_root.join(&p);
                if !full_path.exists() {
                    missing.push(p);
                }
            }
        }
    };

    let mut stmt = conn
        .prepare("SELECT avatar_path, background_image_path FROM characters")
        .unwrap();
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    for r in rows {
        let (a, b): (Option<String>, Option<String>) = r.unwrap();
        check(a);
        check(b);
    }

    let mut stmt = conn.prepare("SELECT avatar_path FROM personas").unwrap();
    let rows = stmt.query_map([], |r| r.get(0)).unwrap();
    for r in rows {
        let a: Option<String> = r.unwrap();
        check(a);
    }

    #[derive(serde::Deserialize)]
    struct AttachmentStub {
        path: String,
    }

    let mut stmt = conn
        .prepare("SELECT attachments FROM messages WHERE attachments != '[]'")
        .unwrap();
    let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
    for r in rows {
        let json = r.unwrap();
        if let Ok(atts) = serde_json::from_str::<Vec<AttachmentStub>>(&json) {
            for att in atts {
                check(Some(att.path));
            }
        }
    }

    missing.sort();
    missing.dedup();
    missing
}
