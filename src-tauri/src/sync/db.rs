use rusqlite::params;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tauri::Manager;

use crate::storage_manager::db::DbConnection;
use crate::sync::models::{
    AudioProvider, Character, CharacterLorebookLink, CharacterRule, ChatTemplate,
    ChatTemplateMessage, GroupMessage, GroupMessageVariant, GroupParticipation, GroupSession,
    Message, MessageVariant, MetaEntry, Model, Persona, PromptTemplate, ProviderCredential, Scene,
    SceneVariant, Secret, Session, Settings, SyncLorebook, SyncLorebookEntry, UsageMetadata,
    UsageRecord, UserVoice,
};
use crate::sync::protocol::{ChangeOp, ChangeRecord, CursorSet, DomainCursor, SyncDomain};
use crate::utils::{log_error_global, log_info_global};

pub const CHANGE_SCHEMA_VERSION: u16 = 1;
pub const LOCAL_SYNC_STATE_VERSION: u16 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct EntityKey {
    domain: SyncDomain,
    entity_type: String,
    entity_id: String,
}

#[derive(Debug, Clone)]
struct CurrentEntityRecord {
    key: EntityKey,
    payload_schema: u16,
    payload_hash: String,
    payload: Vec<u8>,
}

#[derive(Debug, Clone)]
struct EntityHeadRecord {
    payload_hash: String,
    _payload_schema: u16,
    payload: Vec<u8>,
    deleted: bool,
    _last_change_id: i64,
    source_device_id: String,
    source_created_at: i64,
    source_change_id: i64,
}

#[derive(Debug, Clone)]
struct ChangeOrigin<'a> {
    source_device_id: &'a str,
    source_created_at: i64,
    source_change_id: i64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CoreSnapshot {
    meta: Vec<MetaEntry>,
    settings: Vec<Settings>,
    personas: Vec<Persona>,
    models: Vec<Model>,
    secrets: Vec<Secret>,
    provider_credentials: Vec<ProviderCredential>,
    prompt_templates: Vec<PromptTemplate>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct TtsSnapshot {
    audio_providers: Vec<AudioProvider>,
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
    memory_progress_step: Option<i64>,
    speaker_selection_method: String,
    memory_type: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SessionsSnapshot {
    sessions: Vec<Session>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct MessagesSnapshot {
    messages: Vec<Message>,
    message_variants: Vec<MessageVariant>,
    usage_records: Vec<UsageRecord>,
    usage_metadata: Vec<UsageMetadata>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct AssetRecord {
    pub path: String,
    pub content_hash: String,
    pub size_bytes: u64,
}

pub fn get_or_create_local_device_id(conn: &DbConnection) -> Result<String, String> {
    if let Ok(device_id) = conn.query_row(
        "SELECT value FROM sync_local_state WHERE key = 'device_id'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        return Ok(device_id);
    }

    let device_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT OR REPLACE INTO sync_local_state (key, value) VALUES ('device_id', ?1)",
        params![device_id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(device_id)
}

pub fn rebuild_change_log(app: &tauri::AppHandle, conn: &mut DbConnection) -> Result<(), String> {
    let local_device_id = get_or_create_local_device_id(conn)?;
    let current_records = collect_current_entity_records(app, conn)?;
    let current_keys = current_records
        .iter()
        .map(|record| record.key.clone())
        .collect::<HashSet<_>>();
    let heads = load_entity_heads(conn)?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for record in current_records {
        let head = heads.get(&record.key);
        if head.is_some_and(|head| !head.deleted && head.payload_hash == record.payload_hash) {
            continue;
        }

        append_local_change(
            &tx,
            &record.key,
            ChangeOp::Upsert,
            record.payload_schema,
            &record.payload_hash,
            &record.payload,
            &local_device_id,
        )?;
    }

    for (key, head) in heads {
        if !head.deleted && !current_keys.contains(&key) {
            append_local_change(
                &tx,
                &key,
                ChangeOp::Delete,
                CHANGE_SCHEMA_VERSION,
                "",
                &[],
                &local_device_id,
            )?;
        }
    }

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

pub fn load_peer_cursors(conn: &DbConnection, peer_device_id: &str) -> Result<CursorSet, String> {
    let domains = [
        SyncDomain::Core,
        SyncDomain::Tts,
        SyncDomain::Lorebooks,
        SyncDomain::Characters,
        SyncDomain::Groups,
        SyncDomain::Sessions,
        SyncDomain::Messages,
        SyncDomain::Assets,
    ];
    let mut cursors = Vec::with_capacity(domains.len());

    for domain in domains {
        let last_change_id = conn
            .query_row(
                "SELECT last_change_id FROM sync_peer_cursors WHERE peer_device_id = ?1 AND domain = ?2",
                params![peer_device_id, sync_domain_name(domain)],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0);
        cursors.push(DomainCursor {
            domain,
            last_change_id,
        });
    }

    Ok(CursorSet { cursors })
}

pub fn fetch_changes_since(
    conn: &DbConnection,
    domain: SyncDomain,
    after_change_id: i64,
) -> Result<Vec<ChangeRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, source_device_id, source_created_at, source_change_id, entity_type, entity_id, op, payload_schema, payload_hash, payload
             FROM sync_changes
             WHERE domain = ?1 AND id > ?2
             ORDER BY id ASC",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map(params![sync_domain_name(domain), after_change_id], |row| {
            Ok(ChangeRecord {
                change_id: row.get(0)?,
                source_device_id: row.get(1)?,
                source_created_at: row.get(2)?,
                source_change_id: row.get(3)?,
                entity_type: row.get(4)?,
                entity_id: row.get(5)?,
                op: parse_change_op(&row.get::<_, String>(6)?),
                payload_schema: row.get(7)?,
                payload_hash: row.get(8)?,
                payload: row.get(9)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

pub fn record_peer_cursor(
    conn: &DbConnection,
    peer_device_id: &str,
    domain: SyncDomain,
    last_change_id: i64,
) -> Result<(), String> {
    conn.execute(
        r#"INSERT INTO sync_peer_cursors (peer_device_id, domain, last_change_id)
           VALUES (?1, ?2, ?3)
           ON CONFLICT(peer_device_id, domain)
           DO UPDATE SET last_change_id = excluded.last_change_id"#,
        params![peer_device_id, sync_domain_name(domain), last_change_id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

pub fn apply_change_batch(
    conn: &mut DbConnection,
    domain: SyncDomain,
    changes: &[ChangeRecord],
) -> Result<(), String> {
    if changes.is_empty() {
        return Ok(());
    }

    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for change in changes {
        let key = EntityKey {
            domain,
            entity_type: change.entity_type.clone(),
            entity_id: change.entity_id.clone(),
        };
        append_remote_change(&tx, &key, change)?;
    }

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    materialize_domain_heads(conn, domain)
}

fn sync_domain_name(domain: SyncDomain) -> &'static str {
    match domain {
        SyncDomain::Core => "core",
        SyncDomain::Tts => "tts",
        SyncDomain::Lorebooks => "lorebooks",
        SyncDomain::Characters => "characters",
        SyncDomain::Groups => "groups",
        SyncDomain::Sessions => "sessions",
        SyncDomain::Messages => "messages",
        SyncDomain::Assets => "assets",
    }
}

fn parse_sync_domain(value: &str) -> Result<SyncDomain, String> {
    match value {
        "core" => Ok(SyncDomain::Core),
        "tts" => Ok(SyncDomain::Tts),
        "lorebooks" => Ok(SyncDomain::Lorebooks),
        "characters" => Ok(SyncDomain::Characters),
        "groups" => Ok(SyncDomain::Groups),
        "sessions" => Ok(SyncDomain::Sessions),
        "messages" => Ok(SyncDomain::Messages),
        "assets" => Ok(SyncDomain::Assets),
        _ => Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Unknown sync domain: {}", value),
        )),
    }
}

fn change_op_name(op: ChangeOp) -> &'static str {
    match op {
        ChangeOp::Upsert => "upsert",
        ChangeOp::Delete => "delete",
    }
}

fn parse_change_op(value: &str) -> ChangeOp {
    match value {
        "delete" => ChangeOp::Delete,
        _ => ChangeOp::Upsert,
    }
}

fn build_entity_record<T: serde::Serialize>(
    domain: SyncDomain,
    entity_type: &str,
    entity_id: String,
    value: &T,
) -> Result<CurrentEntityRecord, String> {
    let payload = bincode::serialize(value)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let payload_hash = blake3::hash(&payload).to_hex().to_string();
    Ok(CurrentEntityRecord {
        key: EntityKey {
            domain,
            entity_type: entity_type.to_string(),
            entity_id,
        },
        payload_schema: CHANGE_SCHEMA_VERSION,
        payload_hash,
        payload,
    })
}

fn push_entity_record<T: serde::Serialize>(
    records: &mut Vec<CurrentEntityRecord>,
    domain: SyncDomain,
    entity_type: &str,
    entity_id: String,
    value: &T,
) -> Result<(), String> {
    records.push(build_entity_record(domain, entity_type, entity_id, value)?);
    Ok(())
}

fn collect_current_entity_records(
    app: &tauri::AppHandle,
    conn: &DbConnection,
) -> Result<Vec<CurrentEntityRecord>, String> {
    let mut records = Vec::new();

    let (
        meta,
        settings,
        personas,
        models,
        secrets,
        provider_credentials,
        prompt_templates,
        audio_providers,
        user_voices,
    ) = fetch_global_core(conn)?;

    for item in &meta {
        push_entity_record(
            &mut records,
            SyncDomain::Core,
            "meta",
            item.key.clone(),
            item,
        )?;
    }
    for item in &settings {
        push_entity_record(
            &mut records,
            SyncDomain::Core,
            "settings",
            item.id.to_string(),
            item,
        )?;
    }
    for item in &personas {
        push_entity_record(
            &mut records,
            SyncDomain::Core,
            "persona",
            item.id.clone(),
            item,
        )?;
    }
    for item in &models {
        push_entity_record(
            &mut records,
            SyncDomain::Core,
            "model",
            item.id.clone(),
            item,
        )?;
    }
    for item in &secrets {
        push_entity_record(
            &mut records,
            SyncDomain::Core,
            "secret",
            format!("{}::{}", item.service, item.account),
            item,
        )?;
    }
    for item in &provider_credentials {
        push_entity_record(
            &mut records,
            SyncDomain::Core,
            "provider_credential",
            item.id.clone(),
            item,
        )?;
    }
    for item in &prompt_templates {
        push_entity_record(
            &mut records,
            SyncDomain::Core,
            "prompt_template",
            item.id.clone(),
            item,
        )?;
    }
    for item in &audio_providers {
        push_entity_record(
            &mut records,
            SyncDomain::Tts,
            "audio_provider",
            item.id.clone(),
            item,
        )?;
    }
    for item in &user_voices {
        push_entity_record(
            &mut records,
            SyncDomain::Tts,
            "user_voice",
            item.id.clone(),
            item,
        )?;
    }

    let lorebook_ids = collect_text_ids(conn, "SELECT id FROM lorebooks")?;
    let lorebook_payload = fetch_lorebooks(conn, &lorebook_ids)?;
    let (lorebooks, lorebook_entries): (Vec<SyncLorebook>, Vec<SyncLorebookEntry>) =
        bincode::deserialize(&lorebook_payload)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for item in &lorebooks {
        push_entity_record(
            &mut records,
            SyncDomain::Lorebooks,
            "lorebook",
            item.id.clone(),
            item,
        )?;
    }
    for item in &lorebook_entries {
        push_entity_record(
            &mut records,
            SyncDomain::Lorebooks,
            "lorebook_entry",
            item.id.clone(),
            item,
        )?;
    }

    let character_ids = collect_text_ids(conn, "SELECT id FROM characters")?;
    let (
        characters,
        character_rules,
        scenes,
        scene_variants,
        character_lorebooks,
        chat_templates,
        chat_template_messages,
    ) = fetch_characters_data(conn, &character_ids)?;
    for item in &characters {
        push_entity_record(
            &mut records,
            SyncDomain::Characters,
            "character",
            item.id.clone(),
            item,
        )?;
    }
    for item in &character_rules {
        push_entity_record(
            &mut records,
            SyncDomain::Characters,
            "character_rule",
            format!("{}:{}", item.character_id, item.idx),
            item,
        )?;
    }
    for item in &scenes {
        push_entity_record(
            &mut records,
            SyncDomain::Characters,
            "scene",
            item.id.clone(),
            item,
        )?;
    }
    for item in &scene_variants {
        push_entity_record(
            &mut records,
            SyncDomain::Characters,
            "scene_variant",
            item.id.clone(),
            item,
        )?;
    }
    for item in &character_lorebooks {
        push_entity_record(
            &mut records,
            SyncDomain::Characters,
            "character_lorebook",
            format!("{}:{}", item.character_id, item.lorebook_id),
            item,
        )?;
    }
    for item in &chat_templates {
        push_entity_record(
            &mut records,
            SyncDomain::Characters,
            "chat_template",
            item.id.clone(),
            item,
        )?;
    }
    for item in &chat_template_messages {
        push_entity_record(
            &mut records,
            SyncDomain::Characters,
            "chat_template_message",
            item.id.clone(),
            item,
        )?;
    }

    let group_characters = fetch_group_configs(conn)?;
    let group_session_ids = collect_text_ids(conn, "SELECT id FROM group_sessions")?;
    let (
        group_sessions,
        group_participation,
        group_messages,
        group_message_variants,
        group_usage_records,
        group_usage_metadata,
    ) = fetch_group_sessions_full(conn, &group_session_ids)?;
    for item in &group_characters {
        push_entity_record(
            &mut records,
            SyncDomain::Groups,
            "group_character",
            item.id.clone(),
            item,
        )?;
    }
    for item in &group_sessions {
        push_entity_record(
            &mut records,
            SyncDomain::Groups,
            "group_session",
            item.id.clone(),
            item,
        )?;
    }
    for item in &group_participation {
        push_entity_record(
            &mut records,
            SyncDomain::Groups,
            "group_participation",
            item.id.clone(),
            item,
        )?;
    }
    for item in &group_messages {
        push_entity_record(
            &mut records,
            SyncDomain::Groups,
            "group_message",
            item.id.clone(),
            item,
        )?;
    }
    for item in &group_message_variants {
        push_entity_record(
            &mut records,
            SyncDomain::Groups,
            "group_message_variant",
            item.id.clone(),
            item,
        )?;
    }
    for item in &group_usage_records {
        push_entity_record(
            &mut records,
            SyncDomain::Groups,
            "group_usage_record",
            item.id.clone(),
            item,
        )?;
    }
    for item in &group_usage_metadata {
        push_entity_record(
            &mut records,
            SyncDomain::Groups,
            "group_usage_metadata",
            format!("{}:{}", item.usage_id, item.key),
            item,
        )?;
    }

    let session_ids = collect_text_ids(conn, "SELECT id FROM sessions")?;
    let (sessions, messages, message_variants, usage_records, usage_metadata) =
        fetch_sessions_data(conn, &session_ids)?;
    for item in &sessions {
        push_entity_record(
            &mut records,
            SyncDomain::Sessions,
            "session",
            item.id.clone(),
            item,
        )?;
    }
    for item in &messages {
        push_entity_record(
            &mut records,
            SyncDomain::Messages,
            "message",
            item.id.clone(),
            item,
        )?;
    }
    for item in &message_variants {
        push_entity_record(
            &mut records,
            SyncDomain::Messages,
            "message_variant",
            item.id.clone(),
            item,
        )?;
    }
    for item in &usage_records {
        push_entity_record(
            &mut records,
            SyncDomain::Messages,
            "usage_record",
            item.id.clone(),
            item,
        )?;
    }
    for item in &usage_metadata {
        push_entity_record(
            &mut records,
            SyncDomain::Messages,
            "usage_metadata",
            format!("{}:{}", item.usage_id, item.key),
            item,
        )?;
    }

    for item in collect_asset_records(app, conn)? {
        push_entity_record(
            &mut records,
            SyncDomain::Assets,
            "asset",
            item.path.clone(),
            &item,
        )?;
    }

    Ok(records)
}

fn collect_asset_records(
    app: &tauri::AppHandle,
    conn: &DbConnection,
) -> Result<Vec<AssetRecord>, String> {
    #[derive(serde::Deserialize)]
    struct AttachmentStub {
        path: String,
    }

    let mut records = Vec::new();
    let mut seen = HashSet::new();
    let root = crate::storage_manager::legacy::storage_root(app)?;
    let avatars_dir = root.join("avatars");
    let images_dir = root.join("images");

    let persona_ids = collect_text_ids(conn, "SELECT id FROM personas")?;
    for id in persona_ids {
        let raw = avatars_dir.join(&id);
        let prefixed = avatars_dir.join(format!("persona-{}", id));
        let dir = if raw.exists() { raw } else { prefixed };
        if dir.exists() {
            collect_dir_assets(app, &dir, &mut seen, &mut records, |path, name| {
                let dir_name = path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default();
                format!("avatars/{}/{}", dir_name, name)
            })?;
        }
    }
    let persona_design_reference_sets = collect_optional_text_values(
        conn,
        "SELECT design_reference_image_ids FROM personas WHERE design_reference_image_ids IS NOT NULL AND design_reference_image_ids != ''",
    )?;
    for refs in persona_design_reference_sets {
        add_image_assets_from_json_array(&images_dir, &refs, app, &mut seen, &mut records)?;
    }
    let lorebook_avatar_paths = collect_optional_text_values(
        conn,
        "SELECT avatar_path FROM lorebooks WHERE avatar_path IS NOT NULL AND avatar_path != ''",
    )?;
    for avatar_path in lorebook_avatar_paths {
        add_image_reference_asset(&images_dir, &avatar_path, app, &mut seen, &mut records)?;
    }

    let character_ids = collect_text_ids(conn, "SELECT id FROM characters")?;
    for id in &character_ids {
        let raw = avatars_dir.join(id);
        let prefixed = avatars_dir.join(format!("character-{}", id));
        let dir = if raw.exists() { raw } else { prefixed };
        if dir.exists() {
            collect_dir_assets(app, &dir, &mut seen, &mut records, |path, name| {
                let dir_name = path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default();
                format!("avatars/{}/{}", dir_name, name)
            })?;
        }

        let bg_path: Option<String> = conn
            .query_row(
                "SELECT background_image_path FROM characters WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap_or(None);
        if let Some(bg_id) = bg_path {
            add_image_reference_asset(&images_dir, &bg_id, app, &mut seen, &mut records)?;
        }
        let design_refs: Option<String> = conn
            .query_row(
                "SELECT design_reference_image_ids FROM characters WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap_or(None);
        if let Some(refs) = design_refs {
            add_image_assets_from_json_array(&images_dir, &refs, app, &mut seen, &mut records)?;
        }
    }

    let group_background_ids = collect_text_ids(
        conn,
        "SELECT background_image_path FROM group_characters WHERE background_image_path IS NOT NULL AND background_image_path != ''",
    )?;
    for bg_id in group_background_ids {
        add_image_reference_asset(&images_dir, &bg_id, app, &mut seen, &mut records)?;
    }

    let group_session_background_ids = collect_text_ids(
        conn,
        "SELECT background_image_path FROM group_sessions WHERE background_image_path IS NOT NULL AND background_image_path != ''",
    )?;
    for bg_id in group_session_background_ids {
        add_image_reference_asset(&images_dir, &bg_id, app, &mut seen, &mut records)?;
    }

    let session_background_ids = collect_text_ids(
        conn,
        "SELECT background_image_path FROM sessions WHERE background_image_path IS NOT NULL AND background_image_path != ''",
    )?;
    for bg_id in session_background_ids {
        add_image_reference_asset(&images_dir, &bg_id, app, &mut seen, &mut records)?;
    }

    let mut stmt = conn
        .prepare("SELECT id, character_id FROM sessions")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for row in rows {
        let (session_id, character_id) =
            row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let raw_base = root.join("sessions").join(&character_id);
        let prefixed_name = format!("character-{}", character_id);
        let (base_dir, dir_name) = if raw_base.exists() {
            (raw_base, character_id.clone())
        } else {
            (root.join("sessions").join(&prefixed_name), prefixed_name)
        };
        let session_dir = base_dir.join(&session_id);
        if session_dir.exists() {
            collect_dir_assets(app, &session_dir, &mut seen, &mut records, |_, name| {
                format!("sessions/{}/{}/{}", dir_name, session_id, name)
            })?;
        }
    }

    for table in ["messages", "group_messages"] {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT attachments FROM {} WHERE attachments != '[]'",
                table
            ))
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        for row in rows {
            let json = row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            if let Ok(atts) = serde_json::from_str::<Vec<AttachmentStub>>(&json) {
                for att in atts {
                    add_file_asset(app, &att.path, &mut seen, &mut records)?;
                }
            }
        }
    }

    if let Ok(app_data_dir) = app.path().app_data_dir() {
        let generated_dir = app_data_dir.join("generated_images");
        if generated_dir.exists() {
            collect_recursive_assets(
                app,
                &generated_dir,
                "generated_images",
                &mut seen,
                &mut records,
            )?;
        }
    }

    Ok(records)
}

fn collect_dir_assets<F: Fn(&Path, &str) -> String>(
    app: &tauri::AppHandle,
    dir: &Path,
    seen: &mut HashSet<String>,
    records: &mut Vec<AssetRecord>,
    path_builder: F,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = path.file_name().and_then(|value| value.to_str()) {
                add_file_asset(app, &path_builder(dir, name), seen, records)?;
            }
        }
    }
    Ok(())
}

fn collect_recursive_assets(
    app: &tauri::AppHandle,
    dir: &Path,
    prefix: &str,
    seen: &mut HashSet<String>,
    records: &mut Vec<AssetRecord>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let path = entry.path();
        let name = match path.file_name().and_then(|value| value.to_str()) {
            Some(value) => value,
            None => continue,
        };
        let rel = format!("{}/{}", prefix, name);
        if path.is_dir() {
            collect_recursive_assets(app, &path, &rel, seen, records)?;
        } else if path.is_file() {
            add_file_asset(app, &rel, seen, records)?;
        }
    }
    Ok(())
}

fn add_image_asset(
    images_dir: &Path,
    bg_id: &str,
    app: &tauri::AppHandle,
    seen: &mut HashSet<String>,
    records: &mut Vec<AssetRecord>,
) -> Result<(), String> {
    if bg_id.is_empty() || bg_id.starts_with("data:") || bg_id.starts_with("http") {
        return Ok(());
    }

    for ext in ["webp", "png", "jpg", "jpeg", "gif"] {
        let filename = format!("{}.{}", bg_id, ext);
        let file_path = images_dir.join(&filename);
        if file_path.exists() {
            add_file_asset(app, &format!("images/{}", filename), seen, records)?;
            break;
        }
    }

    Ok(())
}

fn add_image_reference_asset(
    images_dir: &Path,
    reference: &str,
    app: &tauri::AppHandle,
    seen: &mut HashSet<String>,
    records: &mut Vec<AssetRecord>,
) -> Result<(), String> {
    if reference.contains('/') {
        return add_file_asset(app, reference, seen, records);
    }

    add_image_asset(images_dir, reference, app, seen, records)
}

fn add_image_assets_from_json_array(
    images_dir: &Path,
    raw_ids: &str,
    app: &tauri::AppHandle,
    seen: &mut HashSet<String>,
    records: &mut Vec<AssetRecord>,
) -> Result<(), String> {
    let ids = serde_json::from_str::<Vec<String>>(raw_ids)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for id in ids {
        add_image_reference_asset(images_dir, &id, app, seen, records)?;
    }
    Ok(())
}

fn add_file_asset(
    app: &tauri::AppHandle,
    relative_path: &str,
    seen: &mut HashSet<String>,
    records: &mut Vec<AssetRecord>,
) -> Result<(), String> {
    if relative_path.starts_with("http")
        || relative_path.starts_with("data:")
        || relative_path.contains("..")
        || relative_path.starts_with('/')
        || relative_path.contains('\\')
        || !seen.insert(relative_path.to_string())
    {
        return Ok(());
    }

    let absolute_path = if relative_path.starts_with("generated_images/") {
        app.path()
            .app_data_dir()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .join(relative_path)
    } else {
        crate::storage_manager::legacy::storage_root(app)?.join(relative_path)
    };

    if !absolute_path.is_file() {
        return Ok(());
    }

    let content = std::fs::read(&absolute_path)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    records.push(AssetRecord {
        path: relative_path.to_string(),
        content_hash: blake3::hash(&content).to_hex().to_string(),
        size_bytes: content.len() as u64,
    });
    Ok(())
}

fn load_entity_heads(conn: &DbConnection) -> Result<HashMap<EntityKey, EntityHeadRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT domain, entity_type, entity_id, payload_hash, payload_schema, payload, deleted, last_change_id, source_device_id, source_created_at, source_change_id
             FROM sync_entity_heads",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| {
            let domain_name: String = row.get(0)?;
            Ok((
                domain_name,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, u16>(4)?,
                row.get::<_, Vec<u8>>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, i64>(10)?,
            ))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut heads = HashMap::new();
    for row in rows {
        let (
            domain_name,
            entity_type,
            entity_id,
            payload_hash,
            payload_schema,
            payload,
            deleted,
            last_change_id,
            source_device_id,
            source_created_at,
            source_change_id,
        ) = row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        heads.insert(
            EntityKey {
                domain: parse_sync_domain(&domain_name)?,
                entity_type,
                entity_id,
            },
            EntityHeadRecord {
                payload_hash,
                _payload_schema: payload_schema,
                payload,
                deleted: deleted != 0,
                _last_change_id: last_change_id,
                source_device_id,
                source_created_at,
                source_change_id,
            },
        );
    }

    Ok(heads)
}

fn append_local_change(
    tx: &rusqlite::Transaction<'_>,
    key: &EntityKey,
    op: ChangeOp,
    payload_schema: u16,
    payload_hash: &str,
    payload: &[u8],
    local_device_id: &str,
) -> Result<i64, String> {
    let origin = ChangeOrigin {
        source_device_id: local_device_id,
        source_created_at: crate::utils::now_millis().unwrap_or(0) as i64,
        source_change_id: 0,
    };
    insert_change(
        tx,
        key,
        op,
        payload_schema,
        payload_hash,
        payload,
        &origin,
        true,
    )
}

fn append_remote_change(
    tx: &rusqlite::Transaction<'_>,
    key: &EntityKey,
    change: &ChangeRecord,
) -> Result<Option<i64>, String> {
    let current_head = load_head(tx, key)?;
    if let Some(head) = &current_head {
        match compare_change_origin(head, change) {
            std::cmp::Ordering::Greater => return Ok(None),
            std::cmp::Ordering::Equal => {
                let same_deleted = head.deleted == (change.op == ChangeOp::Delete);
                let same_payload = head.payload_hash == change.payload_hash;
                if same_deleted && same_payload {
                    return Ok(None);
                }
                return Err(crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!(
                        "Conflicting duplicate change origin for {:?}/{}",
                        key.domain, key.entity_id
                    ),
                ));
            }
            std::cmp::Ordering::Less => {}
        }
    }

    let origin = ChangeOrigin {
        source_device_id: &change.source_device_id,
        source_created_at: change.source_created_at,
        source_change_id: change.source_change_id,
    };
    insert_change(
        tx,
        key,
        change.op,
        change.payload_schema,
        &change.payload_hash,
        &change.payload,
        &origin,
        false,
    )
    .map(Some)
}

fn insert_change(
    tx: &rusqlite::Transaction<'_>,
    key: &EntityKey,
    op: ChangeOp,
    payload_schema: u16,
    payload_hash: &str,
    payload: &[u8],
    origin: &ChangeOrigin<'_>,
    assign_local_source_change_id: bool,
) -> Result<i64, String> {
    let created_at = crate::utils::now_millis().unwrap_or(0) as i64;
    tx.execute(
        r#"INSERT INTO sync_changes (domain, entity_type, entity_id, op, payload_schema, payload_hash, payload, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
        params![
            sync_domain_name(key.domain),
            key.entity_type,
            key.entity_id,
            change_op_name(op),
            payload_schema,
            payload_hash,
            payload,
            created_at
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let change_id = tx.last_insert_rowid();
    let source_change_id = if assign_local_source_change_id {
        tx.execute(
            "UPDATE sync_changes SET source_device_id = ?1, source_created_at = ?2, source_change_id = ?3 WHERE id = ?4",
            params![origin.source_device_id, origin.source_created_at, change_id, change_id],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        change_id
    } else {
        tx.execute(
            "UPDATE sync_changes SET source_device_id = ?1, source_created_at = ?2, source_change_id = ?3 WHERE id = ?4",
            params![
                origin.source_device_id,
                origin.source_created_at,
                origin.source_change_id,
                change_id
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        origin.source_change_id
    };

    tx.execute(
        r#"INSERT INTO sync_entity_heads (domain, entity_type, entity_id, payload_hash, payload_schema, payload, deleted, last_change_id, source_device_id, source_created_at, source_change_id)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
           ON CONFLICT(domain, entity_type, entity_id)
           DO UPDATE SET
             payload_hash = excluded.payload_hash,
             payload_schema = excluded.payload_schema,
             payload = excluded.payload,
             deleted = excluded.deleted,
             last_change_id = excluded.last_change_id,
             source_device_id = excluded.source_device_id,
             source_created_at = excluded.source_created_at,
             source_change_id = excluded.source_change_id"#,
        params![
            sync_domain_name(key.domain),
            key.entity_type,
            key.entity_id,
            payload_hash,
            payload_schema,
            payload,
            if op == ChangeOp::Delete { 1 } else { 0 },
            change_id,
            origin.source_device_id,
            origin.source_created_at,
            source_change_id
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(change_id)
}

fn load_head(
    tx: &rusqlite::Transaction<'_>,
    key: &EntityKey,
) -> Result<Option<EntityHeadRecord>, String> {
    let result = tx.query_row(
        "SELECT payload_hash, payload_schema, payload, deleted, last_change_id, source_device_id, source_created_at, source_change_id
         FROM sync_entity_heads
         WHERE domain = ?1 AND entity_type = ?2 AND entity_id = ?3",
        params![sync_domain_name(key.domain), key.entity_type, key.entity_id],
        |row| {
            Ok(EntityHeadRecord {
                payload_hash: row.get(0)?,
                _payload_schema: row.get(1)?,
                payload: row.get(2)?,
                deleted: row.get::<_, i64>(3)? != 0,
                _last_change_id: row.get(4)?,
                source_device_id: row.get(5)?,
                source_created_at: row.get(6)?,
                source_change_id: row.get(7)?,
            })
        },
    );
    match result {
        Ok(head) => Ok(Some(head)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(crate::utils::err_to_string(module_path!(), line!(), e)),
    }
}

fn compare_change_origin(head: &EntityHeadRecord, change: &ChangeRecord) -> std::cmp::Ordering {
    (
        change.source_created_at,
        change.source_device_id.as_str(),
        change.source_change_id,
    )
        .cmp(&(
            head.source_created_at,
            head.source_device_id.as_str(),
            head.source_change_id,
        ))
}

fn materialize_domain_heads(conn: &mut DbConnection, domain: SyncDomain) -> Result<(), String> {
    let heads = load_entity_heads(conn)?;
    let domain_heads = heads
        .into_iter()
        .filter_map(|(key, head)| {
            if key.domain == domain && !head.deleted {
                Some((key, head))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    match domain {
        SyncDomain::Core => {
            let mut snapshot = CoreSnapshot {
                meta: Vec::new(),
                settings: Vec::new(),
                personas: Vec::new(),
                models: Vec::new(),
                secrets: Vec::new(),
                provider_credentials: Vec::new(),
                prompt_templates: Vec::new(),
            };
            for (key, head) in domain_heads {
                match key.entity_type.as_str() {
                    "meta" => snapshot.meta.push(deserialize_head(&key, &head)?),
                    "settings" => snapshot.settings.push(deserialize_head(&key, &head)?),
                    "persona" => snapshot.personas.push(deserialize_head(&key, &head)?),
                    "model" => snapshot.models.push(deserialize_head(&key, &head)?),
                    "secret" => snapshot.secrets.push(deserialize_head(&key, &head)?),
                    "provider_credential" => snapshot
                        .provider_credentials
                        .push(deserialize_head(&key, &head)?),
                    "prompt_template" => snapshot
                        .prompt_templates
                        .push(deserialize_head(&key, &head)?),
                    _ => {}
                }
            }
            let payload = bincode::serialize(&snapshot)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            apply_core_snapshot(conn, &payload)
        }
        SyncDomain::Tts => {
            let mut snapshot = TtsSnapshot {
                audio_providers: Vec::new(),
                user_voices: Vec::new(),
            };
            for (key, head) in domain_heads {
                match key.entity_type.as_str() {
                    "audio_provider" => snapshot
                        .audio_providers
                        .push(deserialize_head(&key, &head)?),
                    "user_voice" => snapshot.user_voices.push(deserialize_head(&key, &head)?),
                    _ => {}
                }
            }
            let payload = bincode::serialize(&snapshot)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            apply_tts_snapshot(conn, &payload)
        }
        SyncDomain::Lorebooks => {
            let mut snapshot = LorebooksSnapshot {
                lorebooks: Vec::new(),
                entries: Vec::new(),
            };
            for (key, head) in domain_heads {
                match key.entity_type.as_str() {
                    "lorebook" => snapshot.lorebooks.push(deserialize_head(&key, &head)?),
                    "lorebook_entry" => snapshot.entries.push(deserialize_head(&key, &head)?),
                    _ => {}
                }
            }
            let payload = bincode::serialize(&snapshot)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            apply_lorebooks_snapshot(conn, &payload)
        }
        SyncDomain::Characters => {
            let mut snapshot = CharactersSnapshot {
                characters: Vec::new(),
                rules: Vec::new(),
                scenes: Vec::new(),
                scene_variants: Vec::new(),
                character_lorebooks: Vec::new(),
                chat_templates: Vec::new(),
                chat_template_messages: Vec::new(),
            };
            for (key, head) in domain_heads {
                match key.entity_type.as_str() {
                    "character" => snapshot.characters.push(deserialize_head(&key, &head)?),
                    "character_rule" => snapshot.rules.push(deserialize_head(&key, &head)?),
                    "scene" => snapshot.scenes.push(deserialize_head(&key, &head)?),
                    "scene_variant" => snapshot.scene_variants.push(deserialize_head(&key, &head)?),
                    "character_lorebook" => snapshot
                        .character_lorebooks
                        .push(deserialize_head(&key, &head)?),
                    "chat_template" => snapshot.chat_templates.push(deserialize_head(&key, &head)?),
                    "chat_template_message" => snapshot
                        .chat_template_messages
                        .push(deserialize_head(&key, &head)?),
                    _ => {}
                }
            }
            let payload = bincode::serialize(&snapshot)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            apply_characters_snapshot(conn, &payload)
        }
        SyncDomain::Groups => {
            let mut snapshot = GroupsSnapshot {
                group_characters: Vec::new(),
                group_sessions: Vec::new(),
                group_participation: Vec::new(),
                group_messages: Vec::new(),
                group_message_variants: Vec::new(),
                usage_records: Vec::new(),
                usage_metadata: Vec::new(),
            };
            for (key, head) in domain_heads {
                match key.entity_type.as_str() {
                    "group_character" => snapshot
                        .group_characters
                        .push(deserialize_head(&key, &head)?),
                    "group_session" => snapshot.group_sessions.push(deserialize_head(&key, &head)?),
                    "group_participation" => snapshot
                        .group_participation
                        .push(deserialize_head(&key, &head)?),
                    "group_message" => snapshot.group_messages.push(deserialize_head(&key, &head)?),
                    "group_message_variant" => snapshot
                        .group_message_variants
                        .push(deserialize_head(&key, &head)?),
                    "group_usage_record" => {
                        snapshot.usage_records.push(deserialize_head(&key, &head)?)
                    }
                    "group_usage_metadata" => {
                        snapshot.usage_metadata.push(deserialize_head(&key, &head)?)
                    }
                    _ => {}
                }
            }
            let payload = bincode::serialize(&snapshot)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            apply_groups_snapshot(conn, &payload)
        }
        SyncDomain::Sessions => {
            let mut snapshot = SessionsSnapshot {
                sessions: Vec::new(),
            };
            for (key, head) in domain_heads {
                if key.entity_type == "session" {
                    snapshot.sessions.push(deserialize_head(&key, &head)?);
                }
            }
            let payload = bincode::serialize(&snapshot)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            apply_sessions_snapshot(conn, &payload)
        }
        SyncDomain::Messages => {
            let mut snapshot = MessagesSnapshot {
                messages: Vec::new(),
                message_variants: Vec::new(),
                usage_records: Vec::new(),
                usage_metadata: Vec::new(),
            };
            for (key, head) in domain_heads {
                match key.entity_type.as_str() {
                    "message" => snapshot.messages.push(deserialize_head(&key, &head)?),
                    "message_variant" => snapshot
                        .message_variants
                        .push(deserialize_head(&key, &head)?),
                    "usage_record" => snapshot.usage_records.push(deserialize_head(&key, &head)?),
                    "usage_metadata" => {
                        snapshot.usage_metadata.push(deserialize_head(&key, &head)?)
                    }
                    _ => {}
                }
            }
            let payload = bincode::serialize(&snapshot)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            apply_messages_snapshot(conn, &payload)
        }
        SyncDomain::Assets => Ok(()),
    }
}

fn payload_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 3);
    for (idx, byte) in bytes.iter().enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

fn deserialize_head<T: serde::de::DeserializeOwned + serde::Serialize>(
    key: &EntityKey,
    head: &EntityHeadRecord,
) -> Result<T, String> {
    match bincode::deserialize(&head.payload) {
        Ok(value) => {
            let pretty = serde_json::to_string_pretty(&value)
                .unwrap_or_else(|err| format!("<failed to render json: {}>", err));
            log_info_global(
                "sync_payload",
                format!(
                    "deserialized domain={:?} entity_type={} entity_id={} source_device_id={} source_change_id={} payload_bytes={}\n{}",
                    key.domain,
                    key.entity_type,
                    key.entity_id,
                    head.source_device_id,
                    head.source_change_id,
                    head.payload.len(),
                    pretty
                ),
            );
            Ok(value)
        }
        Err(err) => {
            log_error_global(
                "sync_payload",
                format!(
                    "failed to deserialize domain={:?} entity_type={} entity_id={} source_device_id={} source_change_id={} payload_bytes={} payload_hex={}",
                    key.domain,
                    key.entity_type,
                    key.entity_id,
                    head.source_device_id,
                    head.source_change_id,
                    head.payload.len(),
                    payload_hex(&head.payload)
                ),
            );
            Err(crate::utils::err_to_string(module_path!(), line!(), err))
        }
    }
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

fn collect_optional_text_values(conn: &DbConnection, sql: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, Option<String>>(0))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let values = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    Ok(values)
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
    let sql = format!("SELECT id, group_character_id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived, chat_type, starting_scene, background_image_path, COALESCE(lorebook_ids, '[]'), COALESCE(disable_character_lorebooks, 0), memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, memory_status, memory_error, memory_progress_step, COALESCE(speaker_selection_method, 'llm'), COALESCE(memory_type, 'manual') FROM group_sessions WHERE id IN ({})", placeholders);
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
                memory_progress_step: r.get(21)?,
                speaker_selection_method: r.get(22)?,
                memory_type: r.get(23)?,
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
            r#"INSERT OR REPLACE INTO settings (id, default_provider_credential_id, default_model_id, app_state, advanced_model_settings, prompt_template_id, system_prompt, advanced_settings, migration_version, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
            params![
                settings.id,
                settings.default_provider_credential_id,
                settings.default_model_id,
                settings.app_state,
                settings.advanced_model_settings,
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
            r#"INSERT OR REPLACE INTO personas (id, title, description, nickname, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, design_description, design_reference_image_ids, is_default, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#,
            params![
                persona.id,
                persona.title,
                persona.description,
                persona.nickname,
                persona.avatar_path,
                persona.avatar_crop_x,
                persona.avatar_crop_y,
                persona.avatar_crop_scale,
                persona.design_description,
                persona.design_reference_image_ids,
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
            r#"INSERT OR REPLACE INTO prompt_templates (id, name, prompt_type, content, entries, condense_prompt_entries, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            params![
                template.id,
                template.name,
                template.prompt_type,
                template.content,
                template.entries,
                template.condense_prompt_entries,
                template.created_at,
                template.updated_at
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

    tx.execute("DELETE FROM user_voices", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let provider_ids = snapshot
        .audio_providers
        .iter()
        .map(|provider| provider.id.clone())
        .collect::<Vec<_>>();
    for provider in snapshot.audio_providers {
        tx.execute(
            r#"INSERT OR REPLACE INTO audio_providers (id, provider_type, label, api_key, project_id, location, base_url, request_path, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
            params![
                provider.id,
                provider.provider_type,
                provider.label,
                provider.api_key,
                provider.project_id,
                provider.location,
                provider.base_url,
                provider.request_path,
                provider.created_at,
                provider.updated_at
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    delete_missing_rows(&tx, "audio_providers", "id", &provider_ids)?;

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
            "INSERT OR REPLACE INTO lorebooks (id, name, avatar_path, keyword_detection_mode, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                lorebook.id,
                lorebook.name,
                lorebook.avatar_path,
                lorebook.keyword_detection_mode,
                lorebook.created_at,
                lorebook.updated_at
            ],
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
            r#"INSERT OR REPLACE INTO characters (id, name, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, design_description, design_reference_image_ids, background_image_path, definition, description, nickname, scenario, creator_notes, creator, creator_notes_multilingual, source, tags, default_scene_id, default_model_id, fallback_model_id, memory_type, prompt_template_id, group_chat_prompt_template_id, group_chat_roleplay_prompt_template_id, system_prompt, voice_config, voice_autoplay, disable_avatar_gradient, custom_gradient_enabled, custom_gradient_colors, custom_text_color, custom_text_secondary, chat_appearance, default_chat_template_id, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37)"#,
            params![
                character.id,
                character.name,
                character.avatar_path,
                character.avatar_crop_x,
                character.avatar_crop_y,
                character.avatar_crop_scale,
                character.design_description,
                character.design_reference_image_ids,
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
                character.group_chat_prompt_template_id,
                character.group_chat_roleplay_prompt_template_id,
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
            r#"INSERT OR REPLACE INTO group_sessions (id, group_character_id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived, chat_type, starting_scene, background_image_path, lorebook_ids, disable_character_lorebooks, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, memory_status, memory_error, memory_progress_step, speaker_selection_method, memory_type)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)"#,
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
                session.memory_progress_step,
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

fn apply_sessions_snapshot(conn: &mut DbConnection, payload: &[u8]) -> Result<(), String> {
    let snapshot: SessionsSnapshot = bincode::deserialize(payload)
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

    let incoming_session_ids = snapshot
        .sessions
        .iter()
        .map(|session| session.id.clone())
        .collect::<Vec<_>>();
    let removed_session_ids = existing_session_ids
        .into_iter()
        .filter(|id| !incoming_session_ids.contains(id))
        .collect::<Vec<_>>();

    if !removed_session_ids.is_empty() {
        let placeholders = removed_session_ids
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
            rusqlite::params_from_iter(removed_session_ids.iter()),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let delete_usage_sql = format!(
            "DELETE FROM usage_records WHERE session_id IN ({})",
            placeholders
        );
        tx.execute(
            &delete_usage_sql,
            rusqlite::params_from_iter(removed_session_ids.iter()),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for session in snapshot.sessions {
        tx.execute(
            r#"INSERT OR REPLACE INTO sessions (id, character_id, title, background_image_path, system_prompt, selected_scene_id, prompt_template_id, persona_id, persona_disabled, voice_autoplay, temperature, top_p, max_output_tokens, frequency_penalty, presence_penalty, top_k, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, archived, created_at, updated_at, memory_status, memory_error, memory_progress_step)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27)"#,
            params![
                session.id,
                session.character_id,
                session.title,
                session.background_image_path,
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
                session.memory_error,
                session.memory_progress_step
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    delete_missing_rows(&tx, "sessions", "id", &incoming_session_ids)?;

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn apply_messages_snapshot(conn: &mut DbConnection, payload: &[u8]) -> Result<(), String> {
    let snapshot: MessagesSnapshot = bincode::deserialize(payload)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let session_ids = {
        let mut stmt = tx
            .prepare("SELECT id FROM sessions")
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    };

    if !session_ids.is_empty() {
        let placeholders = session_ids
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
            rusqlite::params_from_iter(session_ids.iter()),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let delete_usage_sql = format!(
            "DELETE FROM usage_records WHERE session_id IN ({})",
            placeholders
        );
        tx.execute(
            &delete_usage_sql,
            rusqlite::params_from_iter(session_ids.iter()),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let delete_variants_sql = format!(
            "DELETE FROM message_variants WHERE message_id IN (SELECT id FROM messages WHERE session_id IN ({}))",
            placeholders
        );
        tx.execute(
            &delete_variants_sql,
            rusqlite::params_from_iter(session_ids.iter()),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let delete_messages_sql = format!(
            "DELETE FROM messages WHERE session_id IN ({})",
            placeholders
        );
        tx.execute(
            &delete_messages_sql,
            rusqlite::params_from_iter(session_ids.iter()),
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

type GlobalCoreData = (
    Vec<MetaEntry>,
    Vec<Settings>,
    Vec<Persona>,
    Vec<Model>,
    Vec<Secret>,
    Vec<ProviderCredential>,
    Vec<PromptTemplate>,
    Vec<AudioProvider>,
    Vec<UserVoice>,
);

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
    let mut stmt = conn.prepare("SELECT id, default_provider_credential_id, default_model_id, app_state, advanced_model_settings, prompt_template_id, system_prompt, advanced_settings, migration_version, created_at, updated_at FROM settings").map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let settings_iter = stmt
        .query_map([], |r| {
            Ok(Settings {
                id: r.get(0)?,
                default_provider_credential_id: r.get(1)?,
                default_model_id: r.get(2)?,
                app_state: r.get(3)?,
                advanced_model_settings: r.get(4)?,
                prompt_template_id: r.get(5)?,
                system_prompt: r.get(6)?,
                advanced_settings: r.get(7)?,
                migration_version: r.get(8)?,
                created_at: r.get(9)?,
                updated_at: r.get(10)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let settings: Vec<Settings> = settings_iter.map(|r| r.unwrap()).collect(); // Expect safe unwrap if query OK

    // Personas
    let mut stmt = conn
        .prepare("SELECT id, title, description, nickname, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, design_description, design_reference_image_ids, is_default, created_at, updated_at FROM personas")
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
                design_description: r.get(8)?,
                design_reference_image_ids: r.get(9)?,
                is_default: r.get(10)?,
                created_at: r.get(11)?,
                updated_at: r.get(12)?,
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
    let mut stmt = conn.prepare("SELECT id, name, prompt_type, content, entries, condense_prompt_entries, created_at, updated_at FROM prompt_templates").map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let templates: Vec<PromptTemplate> = stmt
        .query_map([], |r| {
            Ok(PromptTemplate {
                id: r.get(0)?,
                name: r.get(1)?,
                prompt_type: r.get(2)?,
                content: r.get(3)?,
                entries: r.get(4)?,
                condense_prompt_entries: r.get(5)?,
                created_at: r.get(6)?,
                updated_at: r.get(7)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    let mut stmt = conn
        .prepare("SELECT id, provider_type, label, api_key, project_id, location, base_url, request_path, created_at, updated_at FROM audio_providers")
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
                base_url: r.get(6)?,
                request_path: r.get(7)?,
                created_at: r.get(8)?,
                updated_at: r.get(9)?,
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

    Ok((
        meta,
        settings,
        personas,
        models,
        secrets,
        creds,
        templates,
        audio_providers,
        user_voices,
    ))
}

fn fetch_lorebooks(conn: &DbConnection, ids: &[String]) -> Result<Vec<u8>, String> {
    if ids.is_empty() {
        return bincode::serialize(&(Vec::<SyncLorebook>::new(), Vec::<SyncLorebookEntry>::new()))
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e));
    }

    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql_lb = format!(
        "SELECT id, name, avatar_path, keyword_detection_mode, created_at, updated_at FROM lorebooks WHERE id IN ({})",
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
                avatar_path: r.get(2)?,
                keyword_detection_mode: r.get(3)?,
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
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
    let sql = format!("SELECT id, name, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, design_description, design_reference_image_ids, background_image_path, definition, description, nickname, scenario, creator_notes, creator, creator_notes_multilingual, source, tags, default_scene_id, default_model_id, fallback_model_id, memory_type, prompt_template_id, group_chat_prompt_template_id, group_chat_roleplay_prompt_template_id, system_prompt, voice_config, voice_autoplay, disable_avatar_gradient, custom_gradient_enabled, custom_gradient_colors, custom_text_color, custom_text_secondary, chat_appearance, default_chat_template_id, created_at, updated_at FROM characters WHERE id IN ({})", placeholders);
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
                design_description: r.get(6)?,
                design_reference_image_ids: r.get(7)?,
                background_image_path: r.get(8)?,
                definition: r.get(9)?,
                description: r.get(10)?,
                nickname: r.get(11)?,
                scenario: r.get(12)?,
                creator_notes: r.get(13)?,
                creator: r.get(14)?,
                creator_notes_multilingual: r.get(15)?,
                source: r.get(16)?,
                tags: r.get(17)?,
                default_scene_id: r.get(18)?,
                default_model_id: r.get(19)?,
                fallback_model_id: r.get(20)?,
                memory_type: r.get(21)?,
                prompt_template_id: r.get(22)?,
                group_chat_prompt_template_id: r.get(23)?,
                group_chat_roleplay_prompt_template_id: r.get(24)?,
                system_prompt: r.get(25)?,
                voice_config: r.get(26)?,
                voice_autoplay: r.get(27)?,
                disable_avatar_gradient: r.get(28)?,
                custom_gradient_enabled: r.get(29)?,
                custom_gradient_colors: r.get(30)?,
                custom_text_color: r.get(31)?,
                custom_text_secondary: r.get(32)?,
                chat_appearance: r.get(33)?,
                default_chat_template_id: r.get(34)?,
                created_at: r.get(35)?,
                updated_at: r.get(36)?,
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
    let sql = format!("SELECT id, character_id, title, background_image_path, system_prompt, selected_scene_id, prompt_template_id, persona_id, persona_disabled, voice_autoplay, temperature, top_p, max_output_tokens, frequency_penalty, presence_penalty, top_k, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, archived, created_at, updated_at, memory_status, memory_error, memory_progress_step FROM sessions WHERE id IN ({})", placeholders);
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let sessions: Vec<Session> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(Session {
                id: r.get(0)?,
                character_id: r.get(1)?,
                title: r.get(2)?,
                background_image_path: r.get(3)?,
                system_prompt: r.get(4)?,
                selected_scene_id: r.get(5)?,
                prompt_template_id: r.get(6)?,
                persona_id: r.get(7)?,
                persona_disabled: r.get(8)?,
                voice_autoplay: r.get(9)?,
                temperature: r.get(10)?,
                top_p: r.get(11)?,
                max_output_tokens: r.get(12)?,
                frequency_penalty: r.get(13)?,
                presence_penalty: r.get(14)?,
                top_k: r.get(15)?,
                memories: r.get(16)?,
                memory_embeddings: r.get(17)?,
                memory_summary: r.get(18)?,
                memory_summary_token_count: r.get(19)?,
                memory_tool_events: r.get(20)?,
                archived: r.get(21)?,
                created_at: r.get(22)?,
                updated_at: r.get(23)?,
                memory_status: r.get(24)?,
                memory_error: r.get(25)?,
                memory_progress_step: r.get(26)?,
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

    let sql = format!("SELECT id, group_character_id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived, chat_type, starting_scene, background_image_path, COALESCE(lorebook_ids, '[]'), COALESCE(disable_character_lorebooks, 0), memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, memory_status, memory_error, memory_progress_step, COALESCE(speaker_selection_method, 'llm'), COALESCE(memory_type, 'manual') FROM group_sessions WHERE id IN ({})", placeholders);
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
                lorebook_ids: r.get(12)?,
                disable_character_lorebooks: r.get(13)?,
                memories: r.get(14)?,
                memory_embeddings: r.get(15)?,
                memory_summary: r.get(16)?,
                memory_summary_token_count: r.get(17)?,
                memory_tool_events: r.get(18)?,
                memory_status: r.get(19)?,
                memory_error: r.get(20)?,
                memory_progress_step: r.get(21)?,
                speaker_selection_method: r.get(22)?,
                memory_type: r.get(23)?,
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
