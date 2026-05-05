use rusqlite::{params, OptionalExtension};
use serde_json::Value as JsonValue;

use super::db::{now_ms, open_db};
use super::legacy::{
    characters_path, personas_path, read_encrypted_file, sessions_dir, sessions_index_path,
    settings_path,
};
use crate::utils::log_info;

pub fn run_legacy_import(app: &tauri::AppHandle) -> Result<(), String> {
    let mut conn = open_db(app)?;

    // Check meta flag
    let imported: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'legacy_imported'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    if imported.is_some() {
        return Ok(());
    }

    let mut imported_any = false;

    // Settings: trigger lazy import by calling the typed settings loader
    if settings_path(app)?.exists() {
        if let Ok(Some(_)) = super::settings::read_settings_typed::<JsonValue>(app) {
            imported_any = true;
        }
    }

    // Characters
    let chars_file = characters_path(app)?;
    if chars_file.exists() {
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM characters", [], |r| r.get(0))
            .unwrap_or(0);
        if count == 0 {
            if let Some(json) = read_encrypted_file(&chars_file)? {
                import_characters(&mut conn, &json)?;
                imported_any = true;
            }
        }
    }

    // Personas
    let pers_file = personas_path(app)?;
    if pers_file.exists() {
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM personas", [], |r| r.get(0))
            .unwrap_or(0);
        if count == 0 {
            if let Some(json) = read_encrypted_file(&pers_file)? {
                import_personas(&mut conn, &json)?;
                imported_any = true;
            }
        }
    }

    // Sessions
    let dir = sessions_dir(app)?;
    if dir.exists() {
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap_or(0);
        if count == 0 {
            import_sessions(app, &mut conn)?;
            imported_any = true;
        }
    }

    if imported_any {
        let ts = now_ms().to_string();
        conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('legacy_imported', ?)",
            params![ts],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        log_info(app, "import", "Imported legacy storage into SQLite");
    }
    Ok(())
}

#[tauri::command]
pub fn legacy_backup_and_remove(app: tauri::AppHandle) -> Result<String, String> {
    use std::fs;
    use std::path::PathBuf;

    let conn = super::db::open_db(&app)?;
    // only allow after import
    let imported: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'legacy_imported'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    if imported.is_none() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Legacy import not completed; refusing to remove files",
        ));
    }

    let root = super::legacy::storage_root(&app)?;
    let ts = super::db::now_ms();
    let backup_dir = root.join(format!("backup_legacy_{}", ts));
    fs::create_dir_all(&backup_dir)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut moved: Vec<String> = Vec::new();
    let mut skip_missing = |p: PathBuf, _name: &str| -> Result<(), String> {
        if p.exists() {
            let target = backup_dir.join(p.file_name().unwrap_or_default());
            fs::rename(&p, &target)
                .or_else(|_| {
                    // fallback to copy if rename across devices fails
                    fs::copy(&p, &target)
                        .map(|_| ())
                        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
                })
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            moved.push(format!(
                "{} -> {}",
                p.to_string_lossy(),
                target.to_string_lossy()
            ));
        }
        Ok(())
    };

    // Files
    skip_missing(settings_path(&app)?, "settings.bin")?;
    skip_missing(characters_path(&app)?, "characters.bin")?;
    skip_missing(personas_path(&app)?, "personas.bin")?;
    skip_missing(sessions_index_path(&app)?, "sessions/index.bin")?;

    // Sessions directory
    let sdir = sessions_dir(&app)?;
    if sdir.exists() {
        let target = backup_dir.join("sessions");
        fs::rename(&sdir, &target)
            .or_else(|_| {
                // copy recursively
                fn copy_dir(src: &PathBuf, dst: &PathBuf) -> Result<(), String> {
                    fs::create_dir_all(dst)
                        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                    for entry in fs::read_dir(src)
                        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
                    {
                        let entry = entry
                            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                        let path = entry.path();
                        let dest = dst.join(entry.file_name());
                        if path.is_dir() {
                            copy_dir(&path, &dest)?;
                        } else {
                            fs::copy(&path, &dest).map_err(|e| {
                                crate::utils::err_to_string(module_path!(), line!(), e)
                            })?;
                        }
                    }
                    Ok(())
                }
                copy_dir(&sdir, &target)
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        moved.push(format!(
            "{} -> {}",
            sdir.to_string_lossy(),
            target.to_string_lossy()
        ));
    }

    Ok(format!(
        "Backed up legacy files to {}\n{}",
        backup_dir.to_string_lossy(),
        moved.join("\n")
    ))
}

fn import_characters(conn: &mut rusqlite::Connection, json: &str) -> Result<(), String> {
    let data: JsonValue = serde_json::from_str(json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let arr = data.as_array().cloned().unwrap_or_default();
    let now = now_ms() as i64;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for c in arr {
        let id = c
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("Unnamed");
        let avatar_path = c
            .get("avatarPath")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let avatar_crop = c.get("avatarCrop").and_then(|v| v.as_object());
        let avatar_crop_x = avatar_crop.and_then(|crop| crop.get("x").and_then(|v| v.as_f64()));
        let avatar_crop_y = avatar_crop.and_then(|crop| crop.get("y").and_then(|v| v.as_f64()));
        let avatar_crop_scale =
            avatar_crop.and_then(|crop| crop.get("scale").and_then(|v| v.as_f64()));
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
        let default_scene_id = c
            .get("defaultSceneId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let default_model_id = c
            .get("defaultModelId")
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
        let disable_avatar_gradient = c
            .get("disableAvatarGradient")
            .and_then(|v| v.as_bool())
            .unwrap_or(false) as i64;
        let created_at = c.get("createdAt").and_then(|v| v.as_i64()).unwrap_or(now);
        let updated_at = c.get("updatedAt").and_then(|v| v.as_i64()).unwrap_or(now);

        tx.execute(
            r#"INSERT OR REPLACE INTO characters (id, name, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, background_image_path, description, definition, default_scene_id, default_model_id, prompt_template_id, system_prompt, disable_avatar_gradient, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
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
                default_scene_id,
                default_model_id,
                prompt_template_id,
                system_prompt,
                disable_avatar_gradient,
                created_at,
                updated_at
            ],
        ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        // Rules
        if let Some(rules) = c.get("rules").and_then(|v| v.as_array()) {
            for (idx, r) in rules.iter().enumerate() {
                if let Some(text) = r.as_str() {
                    tx.execute(
                        "INSERT INTO character_rules (character_id, idx, rule) VALUES (?, ?, ?)",
                        params![&id, idx as i64, text],
                    )
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                }
            }
        }

        // Scenes
        if let Some(scenes) = c.get("scenes").and_then(|v| v.as_array()) {
            for s in scenes {
                if s.is_string() {
                    let sid = uuid::Uuid::new_v4().to_string();
                    let content = s.as_str().unwrap_or("");
                    tx.execute("INSERT INTO scenes (id, character_id, content, direction, background_image_path, created_at, selected_variant_id) VALUES (?, ?, ?, ?, ?, ?, NULL)", params![&sid, &id, content, None::<String>, None::<String>, now]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                } else if let Some(obj) = s.as_object() {
                    let sid = obj
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                    let content = obj.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    let screated = obj.get("createdAt").and_then(|v| v.as_i64()).unwrap_or(now);
                    let sel = obj
                        .get("selectedVariantId")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let direction = obj.get("direction").and_then(|v| v.as_str());
                    let background_image_path =
                        obj.get("backgroundImagePath").and_then(|v| v.as_str());
                    tx.execute("INSERT INTO scenes (id, character_id, content, direction, background_image_path, created_at, selected_variant_id) VALUES (?, ?, ?, ?, ?, ?, ?)", params![&sid, &id, content, direction, background_image_path, screated, sel]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                    if let Some(vars) = obj.get("variants").and_then(|v| v.as_array()) {
                        for v in vars {
                            let vid = v
                                .get("id")
                                .and_then(|x| x.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                            let vcontent = v.get("content").and_then(|x| x.as_str()).unwrap_or("");
                            let vdirection = v.get("direction").and_then(|x| x.as_str());
                            let vcreated =
                                v.get("createdAt").and_then(|x| x.as_i64()).unwrap_or(now);
                            tx.execute("INSERT INTO scene_variants (id, scene_id, content, direction, created_at) VALUES (?, ?, ?, ?, ?)", params![vid, &sid, vcontent, vdirection, vcreated]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                        }
                    }
                }
            }
        }
    }
    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn import_personas(conn: &mut rusqlite::Connection, json: &str) -> Result<(), String> {
    let data: JsonValue = serde_json::from_str(json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let arr = data.as_array().cloned().unwrap_or_default();
    let now = now_ms() as i64;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for p in arr {
        let id = p
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let title = p
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled");
        let description = p.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let avatar_path = p
            .get("avatarPath")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let avatar_crop = p.get("avatarCrop").and_then(|v| v.as_object());
        let avatar_crop_x = avatar_crop.and_then(|crop| crop.get("x").and_then(|v| v.as_f64()));
        let avatar_crop_y = avatar_crop.and_then(|crop| crop.get("y").and_then(|v| v.as_f64()));
        let avatar_crop_scale =
            avatar_crop.and_then(|crop| crop.get("scale").and_then(|v| v.as_f64()));
        let active_lorebook_ids = p
            .get("activeLorebookIds")
            .and_then(|v| v.as_array())
            .and_then(|arr| serde_json::to_string(arr).ok())
            .unwrap_or_else(|| "[]".to_string());
        let is_default = p
            .get("isDefault")
            .and_then(|v| v.as_bool())
            .unwrap_or(false) as i64;
        let created_at = p.get("createdAt").and_then(|v| v.as_i64()).unwrap_or(now);
        let updated_at = p.get("updatedAt").and_then(|v| v.as_i64()).unwrap_or(now);
        tx.execute(
            r#"INSERT OR REPLACE INTO personas (id, title, description, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, active_lorebook_ids, is_default, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            params![
                &id,
                title,
                description,
                avatar_path,
                avatar_crop_x,
                avatar_crop_y,
                avatar_crop_scale,
                active_lorebook_ids,
                is_default,
                created_at,
                updated_at
            ],
        ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn import_sessions(app: &tauri::AppHandle, conn: &mut rusqlite::Connection) -> Result<(), String> {
    use std::fs;
    let dir = sessions_dir(app)?;
    let index_path = sessions_index_path(app)?;
    let mut ids: Vec<String> = Vec::new();
    if index_path.exists() {
        if let Some(json) = read_encrypted_file(&index_path)? {
            if let Ok(v) = serde_json::from_str::<JsonValue>(&json) {
                if let Some(arr) = v.as_array() {
                    for s in arr {
                        if let Some(id) = s.as_str() {
                            ids.push(id.to_string());
                        }
                    }
                }
            }
        }
    }
    if ids.is_empty() {
        // discover by scanning
        for entry in fs::read_dir(&dir)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        {
            let entry =
                entry.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "bin" {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        ids.push(stem.to_string());
                    }
                }
            }
        }
    }

    let now = now_ms() as i64;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for id in ids {
        let path = dir.join(format!("{}.bin", &id));
        if !path.exists() {
            continue;
        }
        if let Some(json) = read_encrypted_file(&path)? {
            let s: JsonValue = serde_json::from_str(&json)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let character_id = s.get("characterId").and_then(|v| v.as_str()).unwrap_or("");
            let title = s.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let system_prompt = s
                .get("systemPrompt")
                .and_then(|v| v.as_str())
                .map(|x| x.to_string());
            let selected_scene_id = s
                .get("selectedSceneId")
                .and_then(|v| v.as_str())
                .map(|x| x.to_string());
            let persona_id = s
                .get("personaId")
                .and_then(|v| v.as_str())
                .map(|x| x.to_string());
            let persona_disabled = s
                .get("personaDisabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false) as i64;
            let archived = s.get("archived").and_then(|v| v.as_bool()).unwrap_or(false) as i64;
            let created_at = s.get("createdAt").and_then(|v| v.as_i64()).unwrap_or(now);
            let updated_at = s.get("updatedAt").and_then(|v| v.as_i64()).unwrap_or(now);
            let adv = s.get("advancedModelSettings");
            let temperature = adv
                .and_then(|v| v.get("temperature"))
                .and_then(|v| v.as_f64());
            let top_p = adv.and_then(|v| v.get("topP")).and_then(|v| v.as_f64());
            let max_output_tokens = adv
                .and_then(|v| v.get("maxOutputTokens"))
                .and_then(|v| v.as_i64());
            let frequency_penalty = adv
                .and_then(|v| v.get("frequencyPenalty"))
                .and_then(|v| v.as_f64());
            let presence_penalty = adv
                .and_then(|v| v.get("presencePenalty"))
                .and_then(|v| v.as_f64());
            let top_k = adv.and_then(|v| v.get("topK")).and_then(|v| v.as_i64());

            tx.execute(
                r#"INSERT OR REPLACE INTO sessions (id, character_id, title, system_prompt, selected_scene_id, persona_id, persona_disabled, temperature, top_p, max_output_tokens, frequency_penalty, presence_penalty, top_k, archived, created_at, updated_at)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
                params![&id, character_id, title, system_prompt, selected_scene_id, persona_id, persona_disabled, temperature, top_p, max_output_tokens, frequency_penalty, presence_penalty, top_k, archived, created_at, updated_at],
            ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            if let Some(msgs) = s.get("messages").and_then(|v| v.as_array()) {
                for m in msgs {
                    let mid = m
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                    let role = m.get("role").and_then(|v| v.as_str()).unwrap_or("user");
                    let content = m.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    let mcreated = m
                        .get("createdAt")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(created_at);
                    let is_pinned =
                        m.get("isPinned").and_then(|v| v.as_bool()).unwrap_or(false) as i64;
                    let usage = m.get("usage");
                    let pt = usage
                        .and_then(|u| u.get("promptTokens"))
                        .and_then(|v| v.as_i64());
                    let ct = usage
                        .and_then(|u| u.get("completionTokens"))
                        .and_then(|v| v.as_i64());
                    let tt = usage
                        .and_then(|u| u.get("totalTokens"))
                        .and_then(|v| v.as_i64());
                    let selected_variant_id = m
                        .get("selectedVariantId")
                        .and_then(|v| v.as_str())
                        .map(|x| x.to_string());
                    tx.execute(
                        "INSERT INTO messages (id, session_id, role, content, created_at, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        params![&mid, &id, role, content, mcreated, pt, ct, tt, selected_variant_id, is_pinned],
                    ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

                    if let Some(vars) = m.get("variants").and_then(|v| v.as_array()) {
                        for v in vars {
                            let vid = v
                                .get("id")
                                .and_then(|x| x.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                            let vcontent = v.get("content").and_then(|x| x.as_str()).unwrap_or("");
                            let vcreated = v
                                .get("createdAt")
                                .and_then(|x| x.as_i64())
                                .unwrap_or(mcreated);
                            let u = v.get("usage");
                            let vp = u
                                .and_then(|u| u.get("promptTokens"))
                                .and_then(|v| v.as_i64());
                            let vc = u
                                .and_then(|u| u.get("completionTokens"))
                                .and_then(|v| v.as_i64());
                            let vt = u
                                .and_then(|u| u.get("totalTokens"))
                                .and_then(|v| v.as_i64());
                            tx.execute("INSERT INTO message_variants (id, message_id, content, created_at, prompt_tokens, completion_tokens, total_tokens) VALUES (?, ?, ?, ?, ?, ?, ?)", params![vid, &mid, vcontent, vcreated, vp, vc, vt]).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                        }
                    }
                }
            }
        }
    }
    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}
