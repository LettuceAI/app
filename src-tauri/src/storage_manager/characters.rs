use rusqlite::{params, OptionalExtension};
use serde_json::{Map as JsonMap, Value as JsonValue};

use super::db::{now_ms, open_db};
use crate::utils::{log_error, log_info};

fn read_character(conn: &rusqlite::Connection, id: &str) -> Result<JsonValue, String> {
    let (
        name,
        avatar_path,
        avatar_crop_x,
        avatar_crop_y,
        avatar_crop_scale,
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
        fallback_model_id,
        prompt_template_id,
        system_prompt,
        voice_config,
        voice_autoplay,
        memory_type,
        disable_avatar_gradient,
        custom_gradient_enabled,
        custom_gradient_colors,
        custom_text_color,
        custom_text_secondary,
        chat_appearance,
        created_at,
        updated_at,
    ): (
        String,
        Option<String>,
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
        Option<i64>,
        Option<String>,
        i64,
        i64,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        i64,
        i64,
    ) = conn
        .query_row(
            "SELECT name, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, background_image_path, description, definition, nickname, scenario, creator_notes, creator, creator_notes_multilingual, source, tags, default_scene_id, default_model_id, fallback_model_id, prompt_template_id, system_prompt, voice_config, voice_autoplay, memory_type, disable_avatar_gradient, custom_gradient_enabled, custom_gradient_colors, custom_text_color, custom_text_secondary, chat_appearance, created_at, updated_at FROM characters WHERE id = ?",
            params![id],
            |r| Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?, r.get(12)?, r.get(13)?, r.get(14)?, r.get(15)?, r.get(16)?, r.get(17)?, r.get(18)?, r.get(19)?, r.get(20)?, r.get(21)?, r.get(22)?, r.get::<_, i64>(23)?, r.get::<_, i64>(24)?, r.get(25)?, r.get(26)?, r.get(27)?, r.get(28)?, r.get(29)?, r.get(30)?
            )),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // rules
    let mut rules: Vec<JsonValue> = Vec::new();
    let mut stmt = conn
        .prepare("SELECT rule FROM character_rules WHERE character_id = ? ORDER BY idx ASC")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let q = stmt
        .query_map(params![id], |r| Ok(r.get::<_, String>(0)?))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for it in q {
        rules.push(JsonValue::String(it.map_err(|e| {
            crate::utils::err_to_string(module_path!(), line!(), e)
        })?));
    }

    // scenes
    let mut scenes_stmt = conn.prepare("SELECT id, content, direction, created_at, selected_variant_id FROM scenes WHERE character_id = ? ORDER BY created_at ASC").map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let scenes_rows = scenes_stmt
        .query_map(params![id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut scenes: Vec<JsonValue> = Vec::new();
    for row in scenes_rows {
        let (scene_id, content, direction, created_at_s, selected_variant_id) =
            row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
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
        obj.insert("createdAt".into(), JsonValue::from(created_at_s));
        if !variants.is_empty() {
            obj.insert("variants".into(), JsonValue::Array(variants));
        }
        if let Some(sel) = selected_variant_id {
            obj.insert("selectedVariantId".into(), JsonValue::String(sel));
        }
        scenes.push(JsonValue::Object(obj));
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
    if let Some(b) = bg_path {
        root.insert("backgroundImagePath".into(), JsonValue::String(b));
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
    if let Some(ds) = default_scene_id {
        root.insert("defaultSceneId".into(), JsonValue::String(ds));
    }
    if let Some(dm) = default_model_id {
        root.insert("defaultModelId".into(), JsonValue::String(dm));
    }
    if let Some(fm) = fallback_model_id {
        root.insert("fallbackModelId".into(), JsonValue::String(fm));
    }
    let memory_value = memory_type.unwrap_or_else(|| "manual".to_string());
    root.insert("memoryType".into(), JsonValue::String(memory_value));
    if let Some(pt) = prompt_template_id {
        root.insert("promptTemplateId".into(), JsonValue::String(pt));
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
    let rows = stmt
        .query_map([], |r| Ok(r.get::<_, String>(0)?))
        .map_err(|e| {
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
                log_error(
                    &app,
                    "characters_list",
                    format!("Failed to read character {}: {}", id, e),
                );
                return Err(e);
            }
        }
    }
    log_info(
        &app,
        "characters_list",
        format!("Found {} characters", out.len()),
    );
    Ok(serde_json::to_string(&out)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?)
}

#[tauri::command]
pub fn character_upsert(app: tauri::AppHandle, character_json: String) -> Result<String, String> {
    let mut conn = open_db(&app)?;
    let c: JsonValue = serde_json::from_str(&character_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let id = c
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    log_info(
        &app,
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
    let fallback_model_id = c
        .get("fallbackModelId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let prompt_template_id = c
        .get("promptTemplateId")
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
    let existing_created: Option<i64> = tx
        .query_row(
            "SELECT created_at FROM characters WHERE id = ?",
            params![&id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let created_at = existing_created.unwrap_or(now);

    tx.execute(
        r#"INSERT INTO characters (id, name, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, background_image_path, description, definition, nickname, scenario, creator_notes, creator, creator_notes_multilingual, source, tags, default_scene_id, default_model_id, fallback_model_id, prompt_template_id, system_prompt, voice_config, voice_autoplay, memory_type, disable_avatar_gradient, custom_gradient_enabled, custom_gradient_colors, custom_text_color, custom_text_secondary, chat_appearance, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
              name=excluded.name,
              avatar_path=excluded.avatar_path,
              avatar_crop_x=excluded.avatar_crop_x,
              avatar_crop_y=excluded.avatar_crop_y,
              avatar_crop_scale=excluded.avatar_crop_scale,
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
              fallback_model_id=excluded.fallback_model_id,
              prompt_template_id=excluded.prompt_template_id,
              system_prompt=excluded.system_prompt,
              voice_config=excluded.voice_config,
              voice_autoplay=excluded.voice_autoplay,
              memory_type=excluded.memory_type,
              disable_avatar_gradient=excluded.disable_avatar_gradient,
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
            fallback_model_id,
            prompt_template_id,
            system_prompt,
            voice_config,
            voice_autoplay,
            memory_type,
            disable_avatar_gradient,
            custom_gradient_enabled,
            custom_gradient_colors,
            custom_text_color,
            custom_text_secondary,
            chat_appearance,
            created_at,
            now
        ],
    ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

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
            .query_map(params![&id], |r| Ok(r.get::<_, String>(0)?))
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
            tx.execute("INSERT INTO scenes (id, character_id, content, direction, created_at, selected_variant_id) VALUES (?, ?, ?, ?, ?, ?)", params![&sid, &id, content, direction, created_at_s, selected_variant_id]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
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
            &app,
            "character_upsert",
            format!("Failed to update default scene: {}", e),
        );
        e.to_string()
    })?;
    tx.commit().map_err(|e| {
        log_error(
            &app,
            "character_upsert",
            format!("Failed to commit transaction: {}", e),
        );
        e.to_string()
    })?;

    log_info(
        &app,
        "character_upsert",
        format!("Successfully upserted character {}", id),
    );

    let conn2 = open_db(&app)?;
    read_character(&conn2, &id).and_then(|v| {
        serde_json::to_string(&v)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
    })
}

#[tauri::command]
pub fn character_delete(app: tauri::AppHandle, id: String) -> Result<(), String> {
    log_info(
        &app,
        "character_delete",
        format!("Deleting character {}", id),
    );
    let conn = open_db(&app)?;
    conn.execute("DELETE FROM characters WHERE id = ?", params![id])
        .map_err(|e| {
            log_error(
                &app,
                "character_delete",
                format!("Failed to delete character {}: {}", id, e),
            );
            e.to_string()
        })?;
    log_info(
        &app,
        "character_delete",
        format!("Successfully deleted character {}", id),
    );
    Ok(())
}
