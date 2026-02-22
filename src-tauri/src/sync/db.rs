use rusqlite::params;

use crate::storage_manager::db::DbConnection;
use crate::sync::models::{
    AudioProvider, AudioVoiceCache, Character, CharacterLorebookLink, CharacterRule, GroupMessage,
    GroupMessageVariant, GroupParticipation, GroupSession, Message, MessageVariant, Model,
    ModelPricingCache, Persona, PromptTemplate, ProviderCredential, Scene, SceneVariant, Secret,
    Session, Settings, SyncLorebook, SyncLorebookEntry, UsageMetadata, UsageRecord, UserVoice,
};
use crate::sync::protocol::{Manifest, ManifestV2, SyncLayer};

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
    match layer {
        SyncLayer::Globals => fetch_globals(conn),
        SyncLayer::Lorebooks => fetch_lorebooks(conn, ids),
        SyncLayer::Characters => fetch_characters(conn, ids),
        SyncLayer::Sessions => fetch_sessions(conn, ids),
        SyncLayer::GroupSessions => fetch_group_sessions(conn, ids),
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
    Vec<Settings>,
    Vec<Persona>,
    Vec<Model>,
    Vec<Secret>,
    Vec<ProviderCredential>,
    Vec<PromptTemplate>,
    Vec<ModelPricingCache>,
);

pub fn fetch_globals_for_protocol(
    conn: &DbConnection,
    protocol_version: u32,
) -> Result<Vec<u8>, String> {
    let (settings, personas, models, secrets, creds, templates, pricing) = fetch_global_core(conn)?;

    if protocol_version >= 3 {
        let audio_providers = fetch_audio_providers(conn)?;
        let voice_cache = fetch_audio_voice_cache(conn)?;
        let user_voices = fetch_user_voices(conn)?;
        let payload_tuple = (
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

fn fetch_globals(conn: &DbConnection) -> Result<Vec<u8>, String> {
    let (settings, personas, models, secrets, creds, templates, pricing) = fetch_global_core(conn)?;
    let audio_providers = fetch_audio_providers(conn)?;
    let voice_cache = fetch_audio_voice_cache(conn)?;
    let user_voices = fetch_user_voices(conn)?;
    let payload_tuple = (
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
    );
    bincode::serialize(&payload_tuple)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn fetch_global_core(conn: &DbConnection) -> Result<GlobalCoreData, String> {
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
        .prepare("SELECT id, title, description, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, is_default, created_at, updated_at FROM personas")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let personas: Vec<Persona> = stmt
        .query_map([], |r| {
            Ok(Persona {
                id: r.get(0)?,
                title: r.get(1)?,
                description: r.get(2)?,
                avatar_path: r.get(3)?,
                avatar_crop_x: r.get(4)?,
                avatar_crop_y: r.get(5)?,
                avatar_crop_scale: r.get(6)?,
                is_default: r.get(7)?,
                created_at: r.get(8)?,
                updated_at: r.get(9)?,
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

    Ok((
        settings, personas, models, secrets, creds, templates, pricing,
    ))
}

fn fetch_audio_providers(conn: &DbConnection) -> Result<Vec<AudioProvider>, String> {
    let mut stmt = conn
        .prepare("SELECT id, provider_type, label, api_key, project_id, location, created_at, updated_at FROM audio_providers")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let providers = stmt
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
        .collect::<Vec<_>>();
    Ok(providers)
}

fn fetch_audio_voice_cache(conn: &DbConnection) -> Result<Vec<AudioVoiceCache>, String> {
    let mut stmt = conn
        .prepare("SELECT id, provider_id, voice_id, name, preview_url, labels, cached_at FROM audio_voice_cache")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let voices = stmt
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
        .collect::<Vec<_>>();
    Ok(voices)
}

fn fetch_user_voices(conn: &DbConnection) -> Result<Vec<UserVoice>, String> {
    let mut stmt = conn
        .prepare("SELECT id, provider_id, name, model_id, voice_id, prompt, created_at, updated_at FROM user_voices")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let voices = stmt
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
        .collect::<Vec<_>>();
    Ok(voices)
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

fn fetch_characters(conn: &DbConnection, ids: &[String]) -> Result<Vec<u8>, String> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

    // Characters
    let sql = format!("SELECT id, name, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, background_image_path, description, definition, default_scene_id, default_model_id, memory_type, prompt_template_id, system_prompt, voice_config, voice_autoplay, disable_avatar_gradient, custom_gradient_enabled, custom_gradient_colors, custom_text_color, custom_text_secondary, created_at, updated_at FROM characters WHERE id IN ({})", placeholders);
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
                description: r.get(7)?,
                definition: r.get(8)?,
                default_scene_id: r.get(9)?,
                default_model_id: r.get(10)?,
                memory_type: r.get(11)?,
                prompt_template_id: r.get(12)?,
                system_prompt: r.get(13)?,
                voice_config: r.get(14)?,
                voice_autoplay: r.get(15)?,
                disable_avatar_gradient: r.get(16)?,
                custom_gradient_enabled: r.get(17)?,
                custom_gradient_colors: r.get(18)?,
                custom_text_color: r.get(19)?,
                custom_text_secondary: r.get(20)?,
                created_at: r.get(21)?,
                updated_at: r.get(22)?,
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
    let sql_scenes = format!("SELECT id, character_id, content, created_at, selected_variant_id FROM scenes WHERE character_id IN ({})", placeholders);
    let mut stmt = conn
        .prepare(&sql_scenes)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let scenes: Vec<Scene> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(Scene {
                id: r.get(0)?,
                character_id: r.get(1)?,
                content: r.get(2)?,
                created_at: r.get(3)?,
                selected_variant_id: r.get(4)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .map(|r| r.unwrap())
        .collect();

    // Scene Variants
    let sql_vars = format!("SELECT id, scene_id, content, created_at FROM scene_variants WHERE scene_id IN (SELECT id FROM scenes WHERE character_id IN ({}))", placeholders);
    let mut stmt = conn
        .prepare(&sql_vars)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let variants: Vec<SceneVariant> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(SceneVariant {
                id: r.get(0)?,
                scene_id: r.get(1)?,
                content: r.get(2)?,
                created_at: r.get(3)?,
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

    bincode::serialize(&(chars, rules, scenes, variants, links))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn fetch_sessions(conn: &DbConnection, ids: &[String]) -> Result<Vec<u8>, String> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

    // Sessions
    let sql = format!("SELECT id, character_id, title, system_prompt, selected_scene_id, persona_id, persona_disabled, voice_autoplay, temperature, top_p, max_output_tokens, frequency_penalty, presence_penalty, top_k, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, archived, created_at, updated_at, memory_status, memory_error FROM sessions WHERE id IN ({})", placeholders);
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
                persona_id: r.get(5)?,
                persona_disabled: r.get(6)?,
                voice_autoplay: r.get(7)?,
                temperature: r.get(8)?,
                top_p: r.get(9)?,
                max_output_tokens: r.get(10)?,
                frequency_penalty: r.get(11)?,
                presence_penalty: r.get(12)?,
                top_k: r.get(13)?,
                memories: r.get(14)?,
                memory_embeddings: r.get(15)?,
                memory_summary: r.get(16)?,
                memory_summary_token_count: r.get(17)?,
                memory_tool_events: r.get(18)?,
                archived: r.get(19)?,
                created_at: r.get(20)?,
                updated_at: r.get(21)?,
                memory_status: r.get(22)?,
                memory_error: r.get(23)?,
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
    let sql_usage = format!("SELECT id, timestamp, session_id, character_id, character_name, model_id, model_name, provider_id, provider_label, operation_type, prompt_tokens, completion_tokens, total_tokens, memory_tokens, summary_tokens, reasoning_tokens, image_tokens, prompt_cost, completion_cost, total_cost, success, error_message FROM usage_records WHERE session_id IN ({})", placeholders);
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
                prompt_tokens: r.get(10)?,
                completion_tokens: r.get(11)?,
                total_tokens: r.get(12)?,
                memory_tokens: r.get(13)?,
                summary_tokens: r.get(14)?,
                reasoning_tokens: r.get(15)?,
                image_tokens: r.get(16)?,
                prompt_cost: r.get(17)?,
                completion_cost: r.get(18)?,
                total_cost: r.get(19)?,
                success: r.get(20)?,
                error_message: r.get(21)?,
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

    bincode::serialize(&(sessions, messages, variants, usages, metadata))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn fetch_group_sessions(conn: &DbConnection, ids: &[String]) -> Result<Vec<u8>, String> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

    let sql = format!("SELECT id, name, character_ids, persona_id, created_at, updated_at, archived, chat_type, starting_scene, background_image_path, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events FROM group_sessions WHERE id IN ({})", placeholders);
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let sessions: Vec<GroupSession> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(GroupSession {
                id: r.get(0)?,
                name: r.get(1)?,
                character_ids: r.get(2)?,
                persona_id: r.get(3)?,
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
                archived: r.get(6)?,
                chat_type: r.get(7)?,
                starting_scene: r.get(8)?,
                background_image_path: r.get(9)?,
                memories: r.get(10)?,
                memory_embeddings: r.get(11)?,
                memory_summary: r.get(12)?,
                memory_summary_token_count: r.get(13)?,
                memory_tool_events: r.get(14)?,
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

    let sql_msg = format!("SELECT id, session_id, role, content, speaker_character_id, turn_number, created_at, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned, attachments, reasoning, selection_reasoning, model_id FROM group_messages WHERE session_id IN ({})", placeholders);
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
                reasoning: r.get(13)?,
                selection_reasoning: r.get(14)?,
                model_id: r.get(15)?,
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

    let sql_usage = format!("SELECT id, timestamp, session_id, character_id, character_name, model_id, model_name, provider_id, provider_label, operation_type, prompt_tokens, completion_tokens, total_tokens, memory_tokens, summary_tokens, reasoning_tokens, image_tokens, prompt_cost, completion_cost, total_cost, success, error_message FROM usage_records WHERE session_id IN ({})", placeholders);
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
                prompt_tokens: r.get(10)?,
                completion_tokens: r.get(11)?,
                total_tokens: r.get(12)?,
                memory_tokens: r.get(13)?,
                summary_tokens: r.get(14)?,
                reasoning_tokens: r.get(15)?,
                image_tokens: r.get(16)?,
                prompt_cost: r.get(17)?,
                completion_cost: r.get(18)?,
                total_cost: r.get(19)?,
                success: r.get(20)?,
                error_message: r.get(21)?,
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

    bincode::serialize(&(
        sessions,
        participation,
        messages,
        variants,
        usages,
        metadata,
    ))
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
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

type GlobalsData = (
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
    ): GlobalsData = match bincode::deserialize(data) {
        Ok(payload) => payload,
        Err(_) => {
            if let Ok((settings, personas, models, secrets, creds, templates, pricing)) =
                bincode::deserialize::<LegacyGlobalsDataV1>(data)
            {
                (
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
                )
            }
        }
    };

    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Settings (ID=1)
    if let Some(s) = settings.first() {
        tx.execute(r#"INSERT OR REPLACE INTO settings (id, default_provider_credential_id, default_model_id, app_state, prompt_template_id, system_prompt, advanced_settings, migration_version, created_at, updated_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
                    params![s.id, s.default_provider_credential_id, s.default_model_id, s.app_state, s.prompt_template_id, s.system_prompt, s.advanced_settings, s.migration_version, s.created_at, s.updated_at])
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    // Personas
    for p in personas {
        tx.execute(r#"INSERT OR REPLACE INTO personas (id, title, description, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, is_default, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
                   params![p.id, p.title, p.description, p.avatar_path, p.avatar_crop_x, p.avatar_crop_y, p.avatar_crop_scale, p.is_default, p.created_at, p.updated_at]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
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

    // Audio Providers
    for p in audio_providers {
        tx.execute(
            r#"INSERT OR REPLACE INTO audio_providers (id, provider_type, label, api_key, project_id, location, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            params![
                p.id,
                p.provider_type,
                p.label,
                p.api_key,
                p.project_id,
                p.location,
                p.created_at,
                p.updated_at
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    // Audio Voice Cache
    for v in voice_cache {
        tx.execute(
            r#"INSERT OR REPLACE INTO audio_voice_cache (id, provider_id, voice_id, name, preview_url, labels, cached_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
            params![
                v.id,
                v.provider_id,
                v.voice_id,
                v.name,
                v.preview_url,
                v.labels,
                v.cached_at
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    // User Voices
    for v in user_voices {
        tx.execute(
            r#"INSERT OR REPLACE INTO user_voices (id, provider_id, name, model_id, voice_id, prompt, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            params![
                v.id,
                v.provider_id,
                v.name,
                v.model_id,
                v.voice_id,
                v.prompt,
                v.created_at,
                v.updated_at
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
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
);

type LegacyCharactersDataV1 = (
    Vec<LegacyCharacterV1>,
    Vec<CharacterRule>,
    Vec<LegacySceneV1>,
    Vec<SceneVariant>,
    Vec<CharacterLorebookLink>,
);

type LegacyCharactersDataV0 = (
    Vec<LegacyCharacterV0>,
    Vec<CharacterRule>,
    Vec<LegacySceneV1>,
    Vec<SceneVariant>,
    Vec<CharacterLorebookLink>,
);

type LegacyCharactersDataVMinus1 = (
    Vec<LegacyCharacterVMinus1>,
    Vec<CharacterRule>,
    Vec<LegacySceneV0>,
    Vec<SceneVariant>,
    Vec<CharacterLorebookLink>,
);

#[derive(serde::Deserialize)]
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

#[derive(serde::Deserialize)]
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

#[derive(serde::Deserialize)]
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

#[derive(serde::Deserialize)]
struct LegacySceneV1 {
    pub id: String,
    pub character_id: String,
    pub content: String,
    pub created_at: i64,
    pub selected_variant_id: Option<String>,
}

#[derive(serde::Deserialize)]
struct LegacySceneV0 {
    pub id: String,
    pub character_id: String,
    pub content: String,
    pub created_at: i64,
}

fn apply_characters(conn: &mut DbConnection, data: &[u8]) -> Result<(), String> {
    let (chars, rules, scenes, variants, links) = deserialize_characters(data)?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for c in chars {
        tx.execute(r#"INSERT OR REPLACE INTO characters (id, name, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, background_image_path, description, definition, default_scene_id, default_model_id, memory_type, prompt_template_id, system_prompt, voice_config, voice_autoplay, disable_avatar_gradient, custom_gradient_enabled, custom_gradient_colors, custom_text_color, custom_text_secondary, created_at, updated_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)"#,
                    params![c.id, c.name, c.avatar_path, c.avatar_crop_x, c.avatar_crop_y, c.avatar_crop_scale, c.background_image_path, c.description, c.definition, c.default_scene_id, c.default_model_id, c.memory_type, c.prompt_template_id, c.system_prompt, c.voice_config, c.voice_autoplay, c.disable_avatar_gradient, c.custom_gradient_enabled, c.custom_gradient_colors, c.custom_text_color, c.custom_text_secondary, c.created_at, c.updated_at]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
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
        tx.execute(r#"INSERT OR REPLACE INTO scenes (id, character_id, content, created_at, selected_variant_id) VALUES (?1, ?2, ?3, ?4, ?5)"#,
                    params![s.id, s.character_id, s.content, s.created_at, s.selected_variant_id]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for v in variants {
        tx.execute(r#"INSERT OR REPLACE INTO scene_variants (id, scene_id, content, created_at) VALUES (?1, ?2, ?3, ?4)"#,
                   params![v.id, v.scene_id, v.content, v.created_at]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    for l in links {
        tx.execute(r#"INSERT OR REPLACE INTO character_lorebooks (character_id, lorebook_id, enabled, display_order, created_at, updated_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
                    params![l.character_id, l.lorebook_id, l.enabled, l.display_order, l.created_at, l.updated_at]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
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
    ),
    String,
> {
    if let Ok(payload) = bincode::deserialize::<CharactersData>(data) {
        return Ok(payload);
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
                description: c.description,
                definition: c.definition,
                default_scene_id: c.default_scene_id,
                default_model_id: c.default_model_id,
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
                created_at: s.created_at,
                selected_variant_id: s.selected_variant_id,
            })
            .collect();
        return Ok((mapped_chars, rules, mapped_scenes, variants, links));
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
                description: c.description,
                definition: c.definition,
                default_scene_id: c.default_scene_id,
                default_model_id: c.default_model_id,
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
                created_at: s.created_at,
                selected_variant_id: s.selected_variant_id,
            })
            .collect();
        return Ok((mapped_chars, rules, mapped_scenes, variants, links));
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
                description: c.description,
                definition: None,
                default_scene_id: c.default_scene_id,
                default_model_id: c.default_model_id,
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
                created_at: s.created_at,
                selected_variant_id: None,
            })
            .collect();
        return Ok((mapped_chars, rules, mapped_scenes, variants, links));
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

type LegacySessionsDataV1 = (
    Vec<LegacySessionV1>,
    Vec<LegacyMessageV1>,
    Vec<LegacyMessageVariantV1>,
    Vec<UsageRecord>,
    Vec<UsageMetadata>,
);

type LegacySessionsDataV0 = (
    Vec<LegacySessionV0>,
    Vec<LegacyMessageV1>,
    Vec<LegacyMessageVariantV1>,
    Vec<UsageRecord>,
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

#[derive(serde::Deserialize)]
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

#[derive(serde::Deserialize)]
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

#[derive(serde::Deserialize)]
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

#[derive(serde::Deserialize)]
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
        tx.execute(r#"INSERT OR REPLACE INTO sessions (id, character_id, title, system_prompt, selected_scene_id, persona_id, persona_disabled, voice_autoplay, temperature, top_p, max_output_tokens, frequency_penalty, presence_penalty, top_k, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, archived, created_at, updated_at, memory_status, memory_error)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)"#,
                    params![s.id, s.character_id, s.title, s.system_prompt, s.selected_scene_id, s.persona_id, s.persona_disabled, s.voice_autoplay, s.temperature, s.top_p, s.max_output_tokens, s.frequency_penalty, s.presence_penalty, s.top_k, s.memories, s.memory_embeddings, s.memory_summary, s.memory_summary_token_count, s.memory_tool_events, s.archived, s.created_at, s.updated_at, s.memory_status, s.memory_error]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
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
        tx.execute(r#"INSERT OR REPLACE INTO usage_records (id, timestamp, session_id, character_id, character_name, model_id, model_name, provider_id, provider_label, operation_type, prompt_tokens, completion_tokens, total_tokens, memory_tokens, summary_tokens, reasoning_tokens, image_tokens, prompt_cost, completion_cost, total_cost, success, error_message)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)"#,
                    params![u.id, u.timestamp, u.session_id, u.character_id, u.character_name, u.model_id, u.model_name, u.provider_id, u.provider_label, u.operation_type, u.prompt_tokens, u.completion_tokens, u.total_tokens, u.memory_tokens, u.summary_tokens, u.reasoning_tokens, u.image_tokens, u.prompt_cost, u.completion_cost, u.total_cost, u.success, u.error_message]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
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

    let (sessions, participation, messages, variants, usages, metadata): GroupSessionsData =
        bincode::deserialize(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for s in sessions {
        tx.execute(r#"INSERT OR REPLACE INTO group_sessions (id, name, character_ids, persona_id, created_at, updated_at, archived, chat_type, starting_scene, background_image_path, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"#,
                    params![s.id, s.name, s.character_ids, s.persona_id, s.created_at, s.updated_at, s.archived, s.chat_type, s.starting_scene, s.background_image_path, s.memories, s.memory_embeddings, s.memory_summary, s.memory_summary_token_count, s.memory_tool_events]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
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
        tx.execute(r#"INSERT OR REPLACE INTO usage_records (id, timestamp, session_id, character_id, character_name, model_id, model_name, provider_id, provider_label, operation_type, prompt_tokens, completion_tokens, total_tokens, memory_tokens, summary_tokens, reasoning_tokens, image_tokens, prompt_cost, completion_cost, total_cost, success, error_message)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)"#,
                    params![u.id, u.timestamp, u.session_id, u.character_id, u.character_name, u.model_id, u.model_name, u.provider_id, u.provider_label, u.operation_type, u.prompt_tokens, u.completion_tokens, u.total_tokens, u.memory_tokens, u.summary_tokens, u.reasoning_tokens, u.image_tokens, u.prompt_cost, u.completion_cost, u.total_cost, u.success, u.error_message]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
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
            usages,
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
            usages,
            metadata,
        ));
    }

    bincode::deserialize::<SessionsData>(data)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
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
