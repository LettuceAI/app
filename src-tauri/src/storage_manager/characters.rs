use rusqlite::{params, OptionalExtension};
use serde_json::{Map as JsonMap, Value as JsonValue};

use super::db::{now_ms, open_db};
use crate::utils::{log_error, log_info, log_warn};

fn read_character(conn: &rusqlite::Connection, id: &str) -> Result<JsonValue, String> {
    let row: (
        String,
        Option<String>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<String>,
        i64,
        Option<String>,
        i64,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        i64,
        i64,
    ) = conn
        .query_row(
            "SELECT name, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, banner_crop_x, banner_crop_y, banner_crop_scale, card_type, design_description, design_reference_image_ids, background_image_path, description, definition, nickname, scenario, creator_notes, creator, creator_notes_multilingual, source, tags, default_scene_id, default_model_id, mode, companion, prompt_template_id, active_lorebook_ids, group_chat_prompt_template_id, group_chat_roleplay_prompt_template_id, system_prompt, voice_config, voice_autoplay, memory_type, disable_avatar_gradient, avatar_gradient_source, custom_gradient_enabled, custom_gradient_colors, custom_text_color, custom_text_secondary, chat_appearance, default_chat_template_id, created_at, updated_at FROM characters WHERE id = ?",
            params![id],
            |r| Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<f64>>(2)?,
                r.get::<_, Option<f64>>(3)?,
                r.get::<_, Option<f64>>(4)?,
                r.get::<_, Option<f64>>(5)?,
                r.get::<_, Option<f64>>(6)?,
                r.get::<_, Option<f64>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, Option<String>>(10)?,
                r.get::<_, Option<String>>(11)?,
                r.get::<_, Option<String>>(12)?,
                r.get::<_, Option<String>>(13)?,
                r.get::<_, Option<String>>(14)?,
                r.get::<_, Option<String>>(15)?,
                r.get::<_, Option<String>>(16)?,
                r.get::<_, Option<String>>(17)?,
                r.get::<_, Option<String>>(18)?,
                r.get::<_, Option<String>>(19)?,
                r.get::<_, Option<String>>(20)?,
                r.get::<_, Option<String>>(21)?,
                r.get::<_, Option<String>>(22)?,
                r.get::<_, Option<String>>(23)?,
                r.get::<_, Option<String>>(24)?,
                r.get::<_, Option<String>>(25)?,
                r.get::<_, Option<String>>(26)?,
                r.get::<_, Option<String>>(27)?,
                r.get::<_, Option<String>>(28)?,
                r.get::<_, Option<String>>(29)?,
                r.get::<_, Option<String>>(30)?,
                r.get::<_, Option<i64>>(31)?,
                r.get::<_, Option<String>>(32)?,
                r.get::<_, i64>(33)?,
                r.get::<_, Option<String>>(34)?,
                r.get::<_, i64>(35)?,
                r.get::<_, Option<String>>(36)?,
                r.get::<_, Option<String>>(37)?,
                r.get::<_, Option<String>>(38)?,
                r.get::<_, Option<String>>(39)?,
                r.get::<_, Option<String>>(40)?,
                r.get::<_, i64>(41)?,
                r.get::<_, i64>(42)?
            )),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let (
        name,
        avatar_path,
        avatar_crop_x,
        avatar_crop_y,
        avatar_crop_scale,
        banner_crop_x,
        banner_crop_y,
        banner_crop_scale,
        card_type,
        design_description,
        design_reference_image_ids,
        bg_path,
        description,
        definition,
        nickname,
        scenario,
        creator_notes,
        creator,
        creator_notes_multilingual,
        source,
        tags,
        default_scene_id,
        default_model_id,
        mode,
        companion,
        prompt_template_id,
        active_lorebook_ids,
        group_chat_prompt_template_id,
        group_chat_roleplay_prompt_template_id,
        system_prompt,
        voice_config,
        voice_autoplay,
        memory_type,
        disable_avatar_gradient,
        avatar_gradient_source,
        custom_gradient_enabled,
        custom_gradient_colors,
        custom_text_color,
        custom_text_secondary,
        chat_appearance,
        default_chat_template_id,
        created_at,
        updated_at,
    ) = row;

    // rules
    let mut rules: Vec<JsonValue> = Vec::new();
    let mut stmt = conn
        .prepare("SELECT rule FROM character_rules WHERE character_id = ? ORDER BY idx ASC")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let q = stmt
        .query_map(params![id], |r| r.get::<_, String>(0))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for it in q {
        rules.push(JsonValue::String(it.map_err(|e| {
            crate::utils::err_to_string(module_path!(), line!(), e)
        })?));
    }

    // scenes
    let mut scenes_stmt = conn.prepare("SELECT id, content, direction, background_image_path, created_at, selected_variant_id FROM scenes WHERE character_id = ? ORDER BY created_at ASC").map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let scenes_rows = scenes_stmt
        .query_map(params![id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut scenes: Vec<JsonValue> = Vec::new();
    for row in scenes_rows {
        let (
            scene_id,
            content,
            direction,
            background_image_path,
            created_at_s,
            selected_variant_id,
        ) = row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        // variants
        let mut var_stmt = conn.prepare("SELECT id, content, direction, created_at FROM scene_variants WHERE scene_id = ? ORDER BY created_at ASC").map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let var_rows = var_stmt
            .query_map(params![&scene_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let mut variants: Vec<JsonValue> = Vec::new();
        for v in var_rows {
            let (vid, vcontent, vdirection, vcreated) =
                v.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let mut variant_obj =
                serde_json::json!({"id": vid, "content": vcontent, "createdAt": vcreated});
            if let Some(dir) = vdirection {
                variant_obj["direction"] = serde_json::json!(dir);
            }
            variants.push(variant_obj);
        }
        let mut obj = JsonMap::new();
        obj.insert("id".into(), JsonValue::String(scene_id));
        obj.insert("content".into(), JsonValue::String(content));
        if let Some(dir) = direction {
            obj.insert("direction".into(), JsonValue::String(dir));
        }
        if let Some(path) = background_image_path {
            obj.insert("backgroundImagePath".into(), JsonValue::String(path));
        }
        obj.insert("createdAt".into(), JsonValue::from(created_at_s));
        if !variants.is_empty() {
            obj.insert("variants".into(), JsonValue::Array(variants));
        }
        if let Some(sel) = selected_variant_id {
            obj.insert("selectedVariantId".into(), JsonValue::String(sel));
        }
        scenes.push(JsonValue::Object(obj));
    }

    // chat templates
    let mut templates_stmt = conn.prepare("SELECT id, name, scene_id, prompt_template_id, lorebook_ids_override, created_at FROM chat_templates WHERE character_id = ? ORDER BY created_at ASC").map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let templates_rows = templates_stmt
        .query_map(params![id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut chat_templates: Vec<JsonValue> = Vec::new();
    for row in templates_rows {
        let (
            tmpl_id,
            tmpl_name,
            tmpl_scene_id,
            tmpl_prompt_template_id,
            tmpl_lorebook_ids_override,
            tmpl_created_at,
        ) = row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let mut msg_stmt = conn.prepare("SELECT id, role, content FROM chat_template_messages WHERE template_id = ? ORDER BY idx ASC").map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let msg_rows = msg_stmt
            .query_map(params![&tmpl_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let mut messages: Vec<JsonValue> = Vec::new();
        for msg in msg_rows {
            let (msg_id, role, content) =
                msg.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            messages.push(serde_json::json!({"id": msg_id, "role": role, "content": content}));
        }
        let mut tmpl_json = serde_json::json!({
            "id": tmpl_id,
            "name": tmpl_name,
            "messages": messages,
            "createdAt": tmpl_created_at,
        });
        if let Some(ref sid) = tmpl_scene_id {
            tmpl_json["sceneId"] = JsonValue::String(sid.clone());
        }
        if let Some(ref ptid) = tmpl_prompt_template_id {
            tmpl_json["promptTemplateId"] = JsonValue::String(ptid.clone());
        }
        if let Some(ref lorebook_ids_json) = tmpl_lorebook_ids_override {
            if let Ok(value) = serde_json::from_str::<JsonValue>(lorebook_ids_json) {
                tmpl_json["lorebookIdsOverride"] = value;
            }
        }
        chat_templates.push(tmpl_json);
    }

    let mut root = JsonMap::new();
    root.insert("id".into(), JsonValue::String(id.to_string()));
    root.insert("name".into(), JsonValue::String(name));
    if let Some(a) = avatar_path {
        root.insert("avatarPath".into(), JsonValue::String(a));
    }
    if let (Some(x), Some(y), Some(scale)) = (avatar_crop_x, avatar_crop_y, avatar_crop_scale) {
        let mut crop = JsonMap::new();
        crop.insert("x".into(), JsonValue::from(x));
        crop.insert("y".into(), JsonValue::from(y));
        crop.insert("scale".into(), JsonValue::from(scale));
        root.insert("avatarCrop".into(), JsonValue::Object(crop));
    }
    if let (Some(x), Some(y), Some(scale)) = (banner_crop_x, banner_crop_y, banner_crop_scale) {
        let mut crop = JsonMap::new();
        crop.insert("x".into(), JsonValue::from(x));
        crop.insert("y".into(), JsonValue::from(y));
        crop.insert("scale".into(), JsonValue::from(scale));
        root.insert("bannerCrop".into(), JsonValue::Object(crop));
    }
    if let Some(value) = card_type {
        root.insert("cardType".into(), JsonValue::String(value));
    }
    if let Some(value) = design_description {
        root.insert("designDescription".into(), JsonValue::String(value));
    }
    if let Some(value) = design_reference_image_ids {
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&value) {
            root.insert("designReferenceImageIds".into(), serde_json::json!(parsed));
        }
    }
    if let Some(b) = bg_path {
        root.insert("backgroundImagePath".into(), JsonValue::String(b));
    }
    if let Ok((lora_name, lora_strength)) = conn.query_row(
        "SELECT lora_name, lora_strength FROM characters WHERE id = ?",
        params![id],
        |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<f64>>(1)?,
            ))
        },
    ) {
        if let Some(value) = lora_name {
            root.insert("loraName".into(), JsonValue::String(value));
        }
        if let Some(value) = lora_strength {
            root.insert("loraStrength".into(), JsonValue::from(value));
        }
    }
    let resolved_definition = definition.or_else(|| description.clone());
    if let Some(def) = resolved_definition {
        root.insert("definition".into(), JsonValue::String(def));
    }
    if let Some(d) = description {
        root.insert("description".into(), JsonValue::String(d));
    }
    if let Some(value) = nickname {
        root.insert("nickname".into(), JsonValue::String(value));
    }
    if let Some(value) = scenario {
        root.insert("scenario".into(), JsonValue::String(value));
    }
    if let Some(value) = creator_notes {
        root.insert("creatorNotes".into(), JsonValue::String(value));
    }
    if let Some(value) = creator {
        root.insert("creator".into(), JsonValue::String(value));
    }
    if let Some(value) = creator_notes_multilingual {
        if let Ok(parsed) = serde_json::from_str::<JsonValue>(&value) {
            if parsed.is_object() {
                root.insert("creatorNotesMultilingual".into(), parsed);
            }
        }
    }
    if let Some(value) = source {
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&value) {
            root.insert("source".into(), serde_json::json!(parsed));
        }
    }
    if let Some(value) = tags {
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&value) {
            root.insert("tags".into(), serde_json::json!(parsed));
        }
    }
    root.insert("rules".into(), JsonValue::Array(rules));
    root.insert("scenes".into(), JsonValue::Array(scenes));
    root.insert("chatTemplates".into(), JsonValue::Array(chat_templates));
    if let Some(ds) = default_scene_id {
        root.insert("defaultSceneId".into(), JsonValue::String(ds));
    }
    if let Some(dct) = default_chat_template_id {
        root.insert("defaultChatTemplateId".into(), JsonValue::String(dct));
    }
    if let Some(dm) = default_model_id {
        root.insert("defaultModelId".into(), JsonValue::String(dm));
    }
    root.insert(
        "mode".into(),
        JsonValue::String(mode.unwrap_or_else(|| "roleplay".to_string())),
    );
    if let Some(companion_json) = companion {
        if let Ok(parsed) = serde_json::from_str::<JsonValue>(&companion_json) {
            if !parsed.is_null() {
                root.insert("companion".into(), parsed);
            }
        }
    }
    let memory_value = memory_type.unwrap_or_else(|| "manual".to_string());
    root.insert("memoryType".into(), JsonValue::String(memory_value));
    if let Some(value) = active_lorebook_ids {
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&value) {
            root.insert("activeLorebookIds".into(), serde_json::json!(parsed));
        }
    }
    if let Some(pt) = prompt_template_id {
        root.insert("promptTemplateId".into(), JsonValue::String(pt));
    }
    if let Some(pt) = group_chat_prompt_template_id {
        root.insert("groupChatPromptTemplateId".into(), JsonValue::String(pt));
    }
    if let Some(pt) = group_chat_roleplay_prompt_template_id {
        root.insert(
            "groupChatRoleplayPromptTemplateId".into(),
            JsonValue::String(pt),
        );
    }
    if let Some(sp) = system_prompt {
        root.insert("systemPrompt".into(), JsonValue::String(sp));
    }
    if let Some(vc) = voice_config {
        if let Ok(value) = serde_json::from_str::<JsonValue>(&vc) {
            if !value.is_null() {
                root.insert("voiceConfig".into(), value);
            }
        }
    }
    root.insert(
        "voiceAutoplay".into(),
        JsonValue::Bool(voice_autoplay.unwrap_or(0) != 0),
    );
    root.insert(
        "disableAvatarGradient".into(),
        JsonValue::Bool(disable_avatar_gradient != 0),
    );
    root.insert(
        "avatarGradientSource".into(),
        JsonValue::String(avatar_gradient_source.unwrap_or_else(|| "base".to_string())),
    );
    // Custom gradient fields
    root.insert(
        "customGradientEnabled".into(),
        JsonValue::Bool(custom_gradient_enabled != 0),
    );
    if let Some(colors_json) = custom_gradient_colors {
        if let Ok(colors) = serde_json::from_str::<Vec<String>>(&colors_json) {
            root.insert("customGradientColors".into(), serde_json::json!(colors));
        }
    }
    if let Some(tc) = custom_text_color {
        root.insert("customTextColor".into(), JsonValue::String(tc));
    }
    if let Some(ts) = custom_text_secondary {
        root.insert("customTextSecondary".into(), JsonValue::String(ts));
    }
    if let Some(value) = chat_appearance {
        if let Ok(parsed) = serde_json::from_str::<JsonValue>(&value) {
            if parsed.is_object() {
                root.insert("chatAppearance".into(), parsed);
            }
        }
    }
    root.insert("createdAt".into(), JsonValue::from(created_at));
    root.insert("updatedAt".into(), JsonValue::from(updated_at));
    Ok(JsonValue::Object(root))
}

pub fn characters_list_typed<T>(app: &tauri::AppHandle) -> Result<Vec<T>, String>
where
    T: serde::de::DeserializeOwned,
{
    let conn = open_db(app)?;
    let mut stmt = conn
        .prepare("SELECT id FROM characters ORDER BY created_at ASC")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut out = Vec::new();
    for row in rows {
        let id = row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let value = match read_character(&conn, &id) {
            Ok(value) => value,
            Err(error) => {
                log_warn(
                    app,
                    "characters_list_typed",
                    format!("Skipping unreadable character {}: {}", id, error),
                );
                continue;
            }
        };
        match serde_json::from_value::<T>(value) {
            Ok(character) => out.push(character),
            Err(error) => {
                log_warn(
                    app,
                    "characters_list_typed",
                    format!("Skipping unparseable character {}: {}", id, error),
                );
            }
        }
    }
    Ok(out)
}

pub fn character_upsert_typed<T, R>(app: &tauri::AppHandle, character: &T) -> Result<R, String>
where
    T: serde::Serialize,
    R: serde::de::DeserializeOwned,
{
    let value = serde_json::to_value(character)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    serde_json::from_value(upsert_character_value(app, &value)?)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn characters_list(app: tauri::AppHandle) -> Result<String, String> {
    log_info(&app, "characters_list", "Listing all characters");
    let conn = open_db(&app)?;
    let mut stmt = conn
        .prepare("SELECT id FROM characters ORDER BY created_at ASC")
        .map_err(|e| {
            log_error(
                &app,
                "characters_list",
                format!("Failed to prepare statement: {}", e),
            );
            e.to_string()
        })?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(|e| {
        log_error(
            &app,
            "characters_list",
            format!("Failed to query map: {}", e),
        );
        e.to_string()
    })?;
    let mut out = Vec::new();
    for id in rows {
        let id = id.map_err(|e| {
            log_error(&app, "characters_list", format!("Failed to get id: {}", e));
            e.to_string()
        })?;
        match read_character(&conn, &id) {
            Ok(char_data) => out.push(char_data),
            Err(e) => {
                log_warn(
                    &app,
                    "characters_list",
                    format!("Skipping unreadable character {}: {}", id, e),
                );
            }
        }
    }
    log_info(
        &app,
        "characters_list",
        format!("Found {} characters", out.len()),
    );
    serde_json::to_string(&out).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn character_get(app: tauri::AppHandle, id: String) -> Result<String, String> {
    let conn = open_db(&app)?;
    let character = read_character(&conn, &id)?;
    serde_json::to_string(&character)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn character_upsert(app: tauri::AppHandle, character_json: String) -> Result<String, String> {
    let character = serde_json::from_str::<JsonValue>(&character_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let result = upsert_character_value(&app, &character)?;
    serde_json::to_string(&result)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn character_update_chat_appearance(
    app: tauri::AppHandle,
    id: String,
    chat_appearance_json: Option<String>,
) -> Result<String, String> {
    let conn = open_db(&app)?;
    let now = now_ms() as i64;

    conn.execute(
        "UPDATE characters SET chat_appearance = ?, updated_at = ? WHERE id = ?",
        params![chat_appearance_json, now, id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let refreshed = read_character(&conn, &id)?;
    serde_json::to_string(&refreshed)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn upsert_character_value(app: &tauri::AppHandle, c: &JsonValue) -> Result<JsonValue, String> {
    let mut conn = open_db(app)?;
    let id = c
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    log_info(
        app,
        "character_upsert",
        format!("Upserting character {}", id),
    );

    let name = c
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "name is required".to_string())?;
    let avatar_path = c
        .get("avatarPath")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let avatar_crop = c.get("avatarCrop").and_then(|v| v.as_object());
    let avatar_crop_x = avatar_crop.and_then(|crop| crop.get("x").and_then(|v| v.as_f64()));
    let avatar_crop_y = avatar_crop.and_then(|crop| crop.get("y").and_then(|v| v.as_f64()));
    let avatar_crop_scale = avatar_crop.and_then(|crop| crop.get("scale").and_then(|v| v.as_f64()));
    let banner_crop = c.get("bannerCrop").and_then(|v| v.as_object());
    let banner_crop_x = banner_crop.and_then(|crop| crop.get("x").and_then(|v| v.as_f64()));
    let banner_crop_y = banner_crop.and_then(|crop| crop.get("y").and_then(|v| v.as_f64()));
    let banner_crop_scale = banner_crop.and_then(|crop| crop.get("scale").and_then(|v| v.as_f64()));
    let card_type = match c.get("cardType").and_then(|v| v.as_str()) {
        Some("banner") => "banner".to_string(),
        _ => "circle".to_string(),
    };
    let design_description = c
        .get("designDescription")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let design_reference_image_ids: Option<String> = c
        .get("designReferenceImageIds")
        .and_then(|v| v.as_array())
        .and_then(|arr| serde_json::to_string(arr).ok());
    let bg_path = c
        .get("backgroundImagePath")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let description = c
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let definition = c
        .get("definition")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| description.clone());
    let nickname = c
        .get("nickname")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let scenario = c
        .get("scenario")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let creator_notes = c
        .get("creatorNotes")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let creator = c
        .get("creator")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let creator_notes_multilingual: Option<String> =
        c.get("creatorNotesMultilingual").and_then(|v| {
            if v.is_null() {
                None
            } else {
                serde_json::to_string(v).ok()
            }
        });
    let source: Option<String> = c
        .get("source")
        .and_then(|v| v.as_array())
        .and_then(|arr| serde_json::to_string(arr).ok())
        .or_else(|| Some("[\"lettuceai\"]".to_string()));
    let tags: Option<String> = c
        .get("tags")
        .and_then(|v| v.as_array())
        .and_then(|arr| serde_json::to_string(arr).ok());
    let default_model_id = c
        .get("defaultModelId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let mode = match c.get("mode").and_then(|v| v.as_str()) {
        Some("companion") => "companion".to_string(),
        _ => "roleplay".to_string(),
    };
    let companion: Option<String> = c.get("companion").and_then(|v| {
        if v.is_null() {
            None
        } else {
            serde_json::to_string(v).ok()
        }
    });
    let prompt_template_id = c
        .get("promptTemplateId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let active_lorebook_ids_json =
        c.get("activeLorebookIds")
            .and_then(|v| v.as_array())
            .map(|values| {
                serde_json::to_string(
                    &values
                        .iter()
                        .filter_map(|value| value.as_str().map(|id| id.to_string()))
                        .collect::<Vec<_>>(),
                )
                .unwrap_or_else(|_| "[]".to_string())
            });
    let group_chat_prompt_template_id = c
        .get("groupChatPromptTemplateId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let group_chat_roleplay_prompt_template_id = c
        .get("groupChatRoleplayPromptTemplateId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let system_prompt = c
        .get("systemPrompt")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let memory_type = match c.get("memoryType").and_then(|v| v.as_str()) {
        Some("dynamic") => "dynamic".to_string(),
        _ => "manual".to_string(),
    };
    let voice_config: Option<String> = c.get("voiceConfig").and_then(|v| {
        if v.is_null() {
            None
        } else {
            serde_json::to_string(v).ok()
        }
    });
    let voice_autoplay = c
        .get("voiceAutoplay")
        .and_then(|v| v.as_bool())
        .unwrap_or(false) as i64;
    let disable_avatar_gradient = c
        .get("disableAvatarGradient")
        .and_then(|v| v.as_bool())
        .unwrap_or(false) as i64;
    let avatar_gradient_source = match c.get("avatarGradientSource").and_then(|v| v.as_str()) {
        Some("round") => "round".to_string(),
        _ => "base".to_string(),
    };
    // Custom gradient fields
    let custom_gradient_enabled = c
        .get("customGradientEnabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false) as i64;
    let custom_gradient_colors: Option<String> = c
        .get("customGradientColors")
        .and_then(|v| v.as_array())
        .map(|arr| serde_json::to_string(arr).unwrap_or_else(|_| "[]".to_string()));
    let custom_text_color = c
        .get("customTextColor")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let custom_text_secondary = c
        .get("customTextSecondary")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let chat_appearance: Option<String> = c.get("chatAppearance").and_then(|v| {
        if v.is_null() {
            None
        } else {
            serde_json::to_string(v).ok()
        }
    });
    let now = now_ms() as i64;

    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let existing_character: Option<(i64, Option<String>)> = tx
        .query_row(
            "SELECT created_at, active_lorebook_ids FROM characters WHERE id = ?",
            params![&id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let created_at = existing_character
        .as_ref()
        .map(|(created_at, _)| *created_at)
        .unwrap_or(now);
    let active_lorebook_ids = active_lorebook_ids_json
        .or_else(|| {
            existing_character
                .as_ref()
                .and_then(|(_, active_lorebook_ids)| active_lorebook_ids.clone())
        })
        .unwrap_or_else(|| "[]".to_string());

    tx.execute(
        r#"INSERT INTO characters (id, name, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, banner_crop_x, banner_crop_y, banner_crop_scale, card_type, design_description, design_reference_image_ids, background_image_path, description, definition, nickname, scenario, creator_notes, creator, creator_notes_multilingual, source, tags, default_scene_id, default_model_id, mode, companion, prompt_template_id, active_lorebook_ids, group_chat_prompt_template_id, group_chat_roleplay_prompt_template_id, system_prompt, voice_config, voice_autoplay, memory_type, disable_avatar_gradient, avatar_gradient_source, custom_gradient_enabled, custom_gradient_colors, custom_text_color, custom_text_secondary, chat_appearance, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
              name=excluded.name,
              avatar_path=excluded.avatar_path,
              avatar_crop_x=excluded.avatar_crop_x,
              avatar_crop_y=excluded.avatar_crop_y,
              avatar_crop_scale=excluded.avatar_crop_scale,
              banner_crop_x=excluded.banner_crop_x,
              banner_crop_y=excluded.banner_crop_y,
              banner_crop_scale=excluded.banner_crop_scale,
              card_type=excluded.card_type,
              design_description=excluded.design_description,
              design_reference_image_ids=excluded.design_reference_image_ids,
              background_image_path=excluded.background_image_path,
              description=excluded.description,
              definition=excluded.definition,
              nickname=excluded.nickname,
              scenario=excluded.scenario,
              creator_notes=excluded.creator_notes,
              creator=excluded.creator,
              creator_notes_multilingual=excluded.creator_notes_multilingual,
              source=excluded.source,
              tags=excluded.tags,
              default_model_id=excluded.default_model_id,
              mode=excluded.mode,
              companion=excluded.companion,
              prompt_template_id=excluded.prompt_template_id,
              active_lorebook_ids=excluded.active_lorebook_ids,
              group_chat_prompt_template_id=excluded.group_chat_prompt_template_id,
              group_chat_roleplay_prompt_template_id=excluded.group_chat_roleplay_prompt_template_id,
              system_prompt=excluded.system_prompt,
              voice_config=excluded.voice_config,
              voice_autoplay=excluded.voice_autoplay,
              memory_type=excluded.memory_type,
              disable_avatar_gradient=excluded.disable_avatar_gradient,
              avatar_gradient_source=excluded.avatar_gradient_source,
              custom_gradient_enabled=excluded.custom_gradient_enabled,
              custom_gradient_colors=excluded.custom_gradient_colors,
              custom_text_color=excluded.custom_text_color,
              custom_text_secondary=excluded.custom_text_secondary,
              chat_appearance=excluded.chat_appearance,
              updated_at=excluded.updated_at"#,
        params![
            id,
            name,
            avatar_path,
            avatar_crop_x,
            avatar_crop_y,
            avatar_crop_scale,
            banner_crop_x,
            banner_crop_y,
            banner_crop_scale,
            card_type,
            design_description,
            design_reference_image_ids,
            bg_path,
            description,
            definition,
            nickname,
            scenario,
            creator_notes,
            creator,
            creator_notes_multilingual,
            source,
            tags,
            default_model_id,
            mode,
            companion,
            prompt_template_id,
            active_lorebook_ids,
            group_chat_prompt_template_id,
            group_chat_roleplay_prompt_template_id,
            system_prompt,
            voice_config,
            voice_autoplay,
            memory_type,
            disable_avatar_gradient,
            avatar_gradient_source,
            custom_gradient_enabled,
            custom_gradient_colors,
            custom_text_color,
            custom_text_secondary,
            chat_appearance,
            created_at,
            now
        ],
    ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    tx.execute(
        "UPDATE characters SET lora_name = ?, lora_strength = ? WHERE id = ?",
        params![
            c.get("loraName").and_then(|v| v.as_str()).map(str::to_string),
            c.get("loraStrength").and_then(|v| v.as_f64()),
            &id
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Replace rules
    tx.execute(
        "DELETE FROM character_rules WHERE character_id = ?",
        params![&id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    if let Some(rules) = c.get("rules").and_then(|v| v.as_array()) {
        for (idx, rule) in rules.iter().enumerate() {
            if let Some(text) = rule.as_str() {
                tx.execute(
                    "INSERT INTO character_rules (character_id, idx, rule) VALUES (?, ?, ?)",
                    params![&id, idx as i64, text],
                )
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            }
        }
    }

    // Delete existing scenes (cascade variants)
    let scene_ids: Vec<String> = {
        let mut s = tx
            .prepare("SELECT id FROM scenes WHERE character_id = ?")
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let rows = s
            .query_map(params![&id], |r| r.get::<_, String>(0))
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let mut v = Vec::new();
        for it in rows {
            v.push(it.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?);
        }
        v
    };
    for sid in scene_ids {
        tx.execute("DELETE FROM scenes WHERE id = ?", params![sid])
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    // Insert scenes
    let mut new_default_scene_id: Option<String> = c
        .get("defaultSceneId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    if let Some(scenes) = c.get("scenes").and_then(|v| v.as_array()) {
        for (i, s) in scenes.iter().enumerate() {
            let sid = s
                .get("id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let content = s.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let created_at_s = s.get("createdAt").and_then(|v| v.as_i64()).unwrap_or(now);
            let selected_variant_id = s
                .get("selectedVariantId")
                .and_then(|v| v.as_str())
                .map(|x| x.to_string());
            let direction = s.get("direction").and_then(|v| v.as_str());
            let background_image_path = s.get("backgroundImagePath").and_then(|v| v.as_str());
            tx.execute("INSERT INTO scenes (id, character_id, content, direction, background_image_path, created_at, selected_variant_id) VALUES (?, ?, ?, ?, ?, ?, ?)", params![&sid, &id, content, direction, background_image_path, created_at_s, selected_variant_id]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            if i == 0 && new_default_scene_id.is_none() {
                new_default_scene_id = Some(sid.clone());
            }
            if let Some(vars) = s.get("variants").and_then(|v| v.as_array()) {
                for v in vars {
                    let vid = v
                        .get("id")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                    let vcontent = v.get("content").and_then(|x| x.as_str()).unwrap_or("");
                    let vdirection = v.get("direction").and_then(|x| x.as_str());
                    let vcreated = v.get("createdAt").and_then(|x| x.as_i64()).unwrap_or(now);
                    tx.execute("INSERT INTO scene_variants (id, scene_id, content, direction, created_at) VALUES (?, ?, ?, ?, ?)", params![vid, &sid, vcontent, vdirection, vcreated]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                }
            }
        }
    }
    tx.execute(
        "UPDATE characters SET default_scene_id = ? WHERE id = ?",
        params![new_default_scene_id, &id],
    )
    .map_err(|e| {
        log_error(
            app,
            "character_upsert",
            format!("Failed to update default scene: {}", e),
        );
        e.to_string()
    })?;

    // Delete existing chat templates (cascade deletes messages)
    tx.execute(
        "DELETE FROM chat_templates WHERE character_id = ?",
        params![&id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Insert chat templates
    if let Some(templates) = c.get("chatTemplates").and_then(|v| v.as_array()) {
        for t in templates {
            let tid = t
                .get("id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let tname = t.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let tscene_id: Option<&str> = t.get("sceneId").and_then(|v| v.as_str());
            let tprompt_template_id: Option<&str> =
                t.get("promptTemplateId").and_then(|v| v.as_str());
            let tlorebook_ids_override = t
                .get("lorebookIdsOverride")
                .filter(|v| v.is_array())
                .map(|v| v.to_string());
            let tcreated = t.get("createdAt").and_then(|v| v.as_i64()).unwrap_or(now);
            tx.execute(
                "INSERT INTO chat_templates (id, character_id, name, scene_id, prompt_template_id, lorebook_ids_override, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![&tid, &id, tname, tscene_id, tprompt_template_id, tlorebook_ids_override, tcreated],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            if let Some(msgs) = t.get("messages").and_then(|v| v.as_array()) {
                for (idx, m) in msgs.iter().enumerate() {
                    let mid = m
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                    let role = m
                        .get("role")
                        .and_then(|v| v.as_str())
                        .unwrap_or("assistant");
                    let content = m.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    tx.execute(
                        "INSERT INTO chat_template_messages (id, template_id, idx, role, content) VALUES (?, ?, ?, ?, ?)",
                        params![&mid, &tid, idx as i64, role, content],
                    )
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                }
            }
        }
    }

    let default_chat_template_id = c
        .get("defaultChatTemplateId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    tx.execute(
        "UPDATE characters SET default_chat_template_id = ? WHERE id = ?",
        params![default_chat_template_id, &id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    tx.commit().map_err(|e| {
        log_error(
            app,
            "character_upsert",
            format!("Failed to commit transaction: {}", e),
        );
        e.to_string()
    })?;

    log_info(
        app,
        "character_upsert",
        format!("Successfully upserted character {}", id),
    );

    let conn2 = open_db(app)?;
    read_character(&conn2, &id)
}

#[tauri::command]
pub fn character_delete(app: tauri::AppHandle, id: String) -> Result<(), String> {
    log_info(
        &app,
        "character_delete",
        format!("Deleting character {}", id),
    );
    let mut conn = open_db(&app)?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    tx.execute(
        "DELETE FROM memory_embeddings
         WHERE session_kind = 'session'
           AND session_id IN (SELECT id FROM sessions WHERE character_id = ?1)",
        params![id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    crate::storage_manager::memory_embeddings::delete_all_for_session(
        &tx,
        &id,
        crate::storage_manager::memory_embeddings::SessionKind::CompanionShared,
    )?;

    tx.execute("DELETE FROM characters WHERE id = ?", params![id])
        .map_err(|e| {
            log_error(
                &app,
                "character_delete",
                format!("Failed to delete character {}: {}", id, e),
            );
            e.to_string()
        })?;
    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    log_info(
        &app,
        "character_delete",
        format!("Successfully deleted character {}", id),
    );
    Ok(())
}

fn clone_copy_rows(
    tx: &rusqlite::Transaction,
    table: &str,
    overrides: &[(&str, rusqlite::types::Value)],
    where_cols: &[(&str, rusqlite::types::Value)],
    exclude: &[&str],
) -> Result<(), String> {
    use rusqlite::types::Value;

    let columns: Vec<String> = {
        let mut stmt = tx
            .prepare(&format!("PRAGMA table_info(\"{}\")", table))
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    };

    let mut binds: Vec<Value> = Vec::new();
    let mut insert_cols: Vec<String> = Vec::new();
    let mut select_exprs: Vec<String> = Vec::new();
    for col in &columns {
        if exclude.iter().any(|e| e.eq_ignore_ascii_case(col)) {
            continue;
        }
        insert_cols.push(format!("\"{}\"", col));
        if let Some((_, value)) = overrides.iter().find(|(name, _)| name.eq_ignore_ascii_case(col)) {
            binds.push(value.clone());
            select_exprs.push(format!("?{}", binds.len()));
        } else {
            select_exprs.push(format!("\"{}\"", col));
        }
    }

    let mut where_parts: Vec<String> = Vec::new();
    for (name, value) in where_cols {
        binds.push(value.clone());
        where_parts.push(format!("\"{}\" = ?{}", name, binds.len()));
    }
    let where_sql = if where_parts.is_empty() {
        "1=1".to_string()
    } else {
        where_parts.join(" AND ")
    };

    let sql = format!(
        "INSERT INTO \"{table}\" ({cols}) SELECT {exprs} FROM \"{table}\" WHERE {where_sql}",
        table = table,
        cols = insert_cols.join(", "),
        exprs = select_exprs.join(", "),
        where_sql = where_sql,
    );
    tx.execute(&sql, rusqlite::params_from_iter(binds))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

fn clone_collect_ids(
    tx: &rusqlite::Transaction,
    sql: &str,
    param: &str,
) -> Result<Vec<String>, String> {
    let mut stmt = tx
        .prepare(sql)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map(params![param], |row| row.get::<_, String>(0))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn character_clone_deep(app: tauri::AppHandle, id: String) -> Result<String, String> {
    use rusqlite::types::Value;
    use std::collections::HashMap;

    let mut conn = open_db(&app)?;
    let now = now_ms() as i64;
    let new_char_id = uuid::Uuid::new_v4().to_string();

    let orig_name: String = conn
        .query_row("SELECT name FROM characters WHERE id = ?", params![&id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .ok_or_else(|| crate::utils::err_msg(module_path!(), line!(), "Character not found"))?;
    let clone_name = format!("{} (Clone)", orig_name);

    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // 1. character row (fresh id, name, timestamps)
    clone_copy_rows(
        &tx,
        "characters",
        &[
            ("id", Value::Text(new_char_id.clone())),
            ("name", Value::Text(clone_name)),
            ("created_at", Value::Integer(now)),
            ("updated_at", Value::Integer(now)),
        ],
        &[("id", Value::Text(id.clone()))],
        &[],
    )?;

    // 2. character rules (autoincrement id left to regenerate)
    clone_copy_rows(
        &tx,
        "character_rules",
        &[("character_id", Value::Text(new_char_id.clone()))],
        &[("character_id", Value::Text(id.clone()))],
        &["id"],
    )?;

    // 3. lorebook associations (shared lorebooks, new char link)
    clone_copy_rows(
        &tx,
        "character_lorebooks",
        &[("character_id", Value::Text(new_char_id.clone()))],
        &[("character_id", Value::Text(id.clone()))],
        &[],
    )?;

    // 4. scenes
    let mut scene_map: HashMap<String, String> = HashMap::new();
    for old in clone_collect_ids(&tx, "SELECT id FROM scenes WHERE character_id = ?", &id)? {
        let new = uuid::Uuid::new_v4().to_string();
        clone_copy_rows(
            &tx,
            "scenes",
            &[
                ("id", Value::Text(new.clone())),
                ("character_id", Value::Text(new_char_id.clone())),
            ],
            &[("id", Value::Text(old.clone()))],
            &[],
        )?;
        scene_map.insert(old, new);
    }

    // 5. scene variants
    let mut scene_variant_map: HashMap<String, String> = HashMap::new();
    for (old_scene, new_scene) in &scene_map {
        for ov in
            clone_collect_ids(&tx, "SELECT id FROM scene_variants WHERE scene_id = ?", old_scene)?
        {
            let nv = uuid::Uuid::new_v4().to_string();
            clone_copy_rows(
                &tx,
                "scene_variants",
                &[
                    ("id", Value::Text(nv.clone())),
                    ("scene_id", Value::Text(new_scene.clone())),
                ],
                &[("id", Value::Text(ov.clone()))],
                &[],
            )?;
            scene_variant_map.insert(ov, nv);
        }
    }

    // 6. remap scenes.selected_variant_id
    for (old_scene, new_scene) in &scene_map {
        let selected: Option<String> = tx
            .query_row(
                "SELECT selected_variant_id FROM scenes WHERE id = ?",
                params![old_scene],
                |r| r.get(0),
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if let Some(osv) = selected {
            if let Some(nsv) = scene_variant_map.get(&osv) {
                tx.execute(
                    "UPDATE scenes SET selected_variant_id = ? WHERE id = ?",
                    params![nsv, new_scene],
                )
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            }
        }
    }

    // 7. chat templates (remap scene_id)
    let mut template_map: HashMap<String, String> = HashMap::new();
    for old in clone_collect_ids(&tx, "SELECT id FROM chat_templates WHERE character_id = ?", &id)? {
        let new = uuid::Uuid::new_v4().to_string();
        let old_scene: Option<String> = tx
            .query_row(
                "SELECT scene_id FROM chat_templates WHERE id = ?",
                params![&old],
                |r| r.get(0),
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let mut overrides = vec![
            ("id", Value::Text(new.clone())),
            ("character_id", Value::Text(new_char_id.clone())),
        ];
        if let Some(os) = old_scene.as_ref().and_then(|s| scene_map.get(s)) {
            overrides.push(("scene_id", Value::Text(os.clone())));
        }
        clone_copy_rows(
            &tx,
            "chat_templates",
            &overrides,
            &[("id", Value::Text(old.clone()))],
            &[],
        )?;
        template_map.insert(old, new);
    }

    // 8. chat template messages
    for (old_t, new_t) in &template_map {
        for om in clone_collect_ids(
            &tx,
            "SELECT id FROM chat_template_messages WHERE template_id = ?",
            old_t,
        )? {
            let nm = uuid::Uuid::new_v4().to_string();
            clone_copy_rows(
                &tx,
                "chat_template_messages",
                &[
                    ("id", Value::Text(nm)),
                    ("template_id", Value::Text(new_t.clone())),
                ],
                &[("id", Value::Text(om))],
                &[],
            )?;
        }
    }

    // 9. remap characters.default_scene_id / default_chat_template_id
    let (def_scene, def_template): (Option<String>, Option<String>) = tx
        .query_row(
            "SELECT default_scene_id, default_chat_template_id FROM characters WHERE id = ?",
            params![&new_char_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    if let Some(ns) = def_scene.as_ref().and_then(|s| scene_map.get(s)) {
        tx.execute(
            "UPDATE characters SET default_scene_id = ? WHERE id = ?",
            params![ns, &new_char_id],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    if let Some(nt) = def_template.as_ref().and_then(|t| template_map.get(t)) {
        tx.execute(
            "UPDATE characters SET default_chat_template_id = ? WHERE id = ?",
            params![nt, &new_char_id],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    // 10. companion scheduled notes
    for on in clone_collect_ids(
        &tx,
        "SELECT id FROM companion_scheduled_notes WHERE character_id = ?",
        &id,
    )? {
        let nn = uuid::Uuid::new_v4().to_string();
        clone_copy_rows(
            &tx,
            "companion_scheduled_notes",
            &[
                ("id", Value::Text(nn)),
                ("character_id", Value::Text(new_char_id.clone())),
            ],
            &[("id", Value::Text(on))],
            &[],
        )?;
    }

    // 11. companion shared memory state (PK = character_id)
    clone_copy_rows(
        &tx,
        "companion_shared_memory_state",
        &[("character_id", Value::Text(new_char_id.clone()))],
        &[("character_id", Value::Text(id.clone()))],
        &[],
    )?;

    // 12. shared-memory embeddings (session_id == character_id)
    clone_copy_rows(
        &tx,
        "memory_embeddings",
        &[("session_id", Value::Text(new_char_id.clone()))],
        &[
            ("session_id", Value::Text(id.clone())),
            ("session_kind", Value::Text("companion_shared".to_string())),
        ],
        &[],
    )?;

    // 13. sessions (remap selected_scene_id)
    let mut session_map: HashMap<String, String> = HashMap::new();
    for old in clone_collect_ids(&tx, "SELECT id FROM sessions WHERE character_id = ?", &id)? {
        let new = uuid::Uuid::new_v4().to_string();
        let old_scene: Option<String> = tx
            .query_row(
                "SELECT selected_scene_id FROM sessions WHERE id = ?",
                params![&old],
                |r| r.get(0),
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let mut overrides = vec![
            ("id", Value::Text(new.clone())),
            ("character_id", Value::Text(new_char_id.clone())),
        ];
        if let Some(ns) = old_scene.as_ref().and_then(|s| scene_map.get(s)) {
            overrides.push(("selected_scene_id", Value::Text(ns.clone())));
        }
        clone_copy_rows(
            &tx,
            "sessions",
            &overrides,
            &[("id", Value::Text(old.clone()))],
            &[],
        )?;
        session_map.insert(old, new);
    }

    // 14. per-session: messages, variants, turn effects, embeddings
    for (old_s, new_s) in &session_map {
        let mut msg_map: HashMap<String, String> = HashMap::new();
        for om in clone_collect_ids(&tx, "SELECT id FROM messages WHERE session_id = ?", old_s)? {
            let nm = uuid::Uuid::new_v4().to_string();
            clone_copy_rows(
                &tx,
                "messages",
                &[
                    ("id", Value::Text(nm.clone())),
                    ("session_id", Value::Text(new_s.clone())),
                ],
                &[("id", Value::Text(om.clone()))],
                &[],
            )?;
            msg_map.insert(om, nm);
        }

        let mut variant_map: HashMap<String, String> = HashMap::new();
        for (om, nm) in &msg_map {
            for ov in
                clone_collect_ids(&tx, "SELECT id FROM message_variants WHERE message_id = ?", om)?
            {
                let nv = uuid::Uuid::new_v4().to_string();
                clone_copy_rows(
                    &tx,
                    "message_variants",
                    &[
                        ("id", Value::Text(nv.clone())),
                        ("message_id", Value::Text(nm.clone())),
                    ],
                    &[("id", Value::Text(ov.clone()))],
                    &[],
                )?;
                variant_map.insert(ov, nv);
            }
        }

        for (om, nm) in &msg_map {
            let selected: Option<String> = tx
                .query_row(
                    "SELECT selected_variant_id FROM messages WHERE id = ?",
                    params![om],
                    |r| r.get(0),
                )
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            if let Some(nv) = selected.as_ref().and_then(|v| variant_map.get(v)) {
                tx.execute(
                    "UPDATE messages SET selected_variant_id = ? WHERE id = ?",
                    params![nv, nm],
                )
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            }
        }

        for oe in clone_collect_ids(
            &tx,
            "SELECT id FROM companion_turn_effects WHERE session_id = ?",
            old_s,
        )? {
            let (old_user, old_assistant): (Option<String>, String) = tx
                .query_row(
                    "SELECT user_message_id, assistant_message_id FROM companion_turn_effects WHERE id = ?",
                    params![&oe],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let Some(new_assistant) = msg_map.get(&old_assistant) else {
                continue;
            };
            let ne = uuid::Uuid::new_v4().to_string();
            let mut overrides = vec![
                ("id", Value::Text(ne)),
                ("session_id", Value::Text(new_s.clone())),
                ("assistant_message_id", Value::Text(new_assistant.clone())),
            ];
            match old_user.as_ref().and_then(|u| msg_map.get(u)) {
                Some(nu) => overrides.push(("user_message_id", Value::Text(nu.clone()))),
                None => overrides.push(("user_message_id", Value::Null)),
            }
            clone_copy_rows(
                &tx,
                "companion_turn_effects",
                &overrides,
                &[("id", Value::Text(oe))],
                &[],
            )?;
        }

        clone_copy_rows(
            &tx,
            "memory_embeddings",
            &[("session_id", Value::Text(new_s.clone()))],
            &[
                ("session_id", Value::Text(old_s.clone())),
                ("session_kind", Value::Text("session".to_string())),
            ],
            &[],
        )?;
    }

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    log_info(
        &app,
        "character_clone_deep",
        format!("Cloned character {} -> {}", id, new_char_id),
    );

    let conn2 = open_db(&app)?;
    let json = read_character(&conn2, &new_char_id)?;
    serde_json::to_string(&json).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[cfg(test)]
mod read_character_positional_tests {
    use super::*;
    use rusqlite::Connection;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE characters (
                id TEXT PRIMARY KEY, name TEXT, avatar_path TEXT,
                avatar_crop_x REAL, avatar_crop_y REAL, avatar_crop_scale REAL,
                banner_crop_x REAL, banner_crop_y REAL, banner_crop_scale REAL,
                card_type TEXT, design_description TEXT, design_reference_image_ids TEXT,
                background_image_path TEXT, description TEXT, definition TEXT, nickname TEXT,
                scenario TEXT, creator_notes TEXT, creator TEXT, creator_notes_multilingual TEXT,
                source TEXT, tags TEXT, default_scene_id TEXT, default_model_id TEXT,
                mode TEXT, companion TEXT, prompt_template_id TEXT, active_lorebook_ids TEXT,
                group_chat_prompt_template_id TEXT, group_chat_roleplay_prompt_template_id TEXT,
                system_prompt TEXT, voice_config TEXT, voice_autoplay INTEGER,
                memory_type TEXT, disable_avatar_gradient INTEGER, avatar_gradient_source TEXT,
                custom_gradient_enabled INTEGER, custom_gradient_colors TEXT, custom_text_color TEXT,
                custom_text_secondary TEXT, chat_appearance TEXT, default_chat_template_id TEXT,
                lora_name TEXT, lora_strength REAL, created_at INTEGER, updated_at INTEGER
            );
            CREATE TABLE character_rules (character_id TEXT, idx INTEGER, rule TEXT);
            CREATE TABLE scenes (id TEXT, character_id TEXT, content TEXT, direction TEXT, background_image_path TEXT, created_at INTEGER, selected_variant_id TEXT);
            CREATE TABLE scene_variants (id TEXT, scene_id TEXT, content TEXT, direction TEXT, created_at INTEGER);
            CREATE TABLE chat_templates (id TEXT, character_id TEXT, name TEXT, scene_id TEXT, prompt_template_id TEXT, lorebook_ids_override TEXT, created_at INTEGER);
            CREATE TABLE chat_template_messages (id TEXT, template_id TEXT, role TEXT, content TEXT, idx INTEGER);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn maps_columns_after_fallback_column_removal() {
        let conn = setup();
        conn.execute(
            "INSERT INTO characters (id, name, default_scene_id, default_model_id, mode, system_prompt, memory_type, voice_autoplay, disable_avatar_gradient, avatar_gradient_source, custom_gradient_enabled, default_chat_template_id, created_at, updated_at)
             VALUES ('c1', 'Aria', 'scene-9', 'model-xyz', 'companion', 'be nice', 'dynamic', 1, 0, 'base', 0, 'tmpl-7', 111, 222)",
            [],
        )
        .unwrap();

        let json = read_character(&conn, "c1").unwrap();
        assert_eq!(json["id"], "c1");
        assert_eq!(json["name"], "Aria");
        assert_eq!(json["defaultSceneId"], "scene-9");
        assert_eq!(json["defaultModelId"], "model-xyz");
        assert_eq!(json["mode"], "companion");
        assert_eq!(json["systemPrompt"], "be nice");
        assert_eq!(json["memoryType"], "dynamic");
        assert_eq!(json["voiceAutoplay"], true);
        assert_eq!(json["disableAvatarGradient"], false);
        assert_eq!(json["avatarGradientSource"], "base");
        assert_eq!(json["customGradientEnabled"], false);
        assert_eq!(json["defaultChatTemplateId"], "tmpl-7");
        assert_eq!(json["createdAt"], 111);
        assert_eq!(json["updatedAt"], 222);
        assert!(json.get("fallbackModelId").is_none());
    }
}
