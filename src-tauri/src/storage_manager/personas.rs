use rusqlite::{params, OptionalExtension};
use serde_json::{Map as JsonMap, Value as JsonValue};

use super::db::{now_ms, open_db};

pub fn personas_list_typed<T>(app: &tauri::AppHandle) -> Result<Vec<T>, String>
where
    T: serde::de::DeserializeOwned,
{
    let conn = open_db(app)?;
    let mut stmt = conn.prepare("SELECT id, title, description, nickname, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, design_description, design_reference_image_ids, COALESCE(active_lorebook_ids, '[]'), is_default, created_at, updated_at FROM personas ORDER BY created_at ASC").map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<f64>>(5)?,
                r.get::<_, Option<f64>>(6)?,
                r.get::<_, Option<f64>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, String>(10)?,
                r.get::<_, i64>(11)?,
                r.get::<_, i64>(12)?,
                r.get::<_, i64>(13)?,
            ))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut out = Vec::new();
    for row in rows {
        let (
            id,
            title,
            description,
            nickname,
            avatar_path,
            avatar_crop_x,
            avatar_crop_y,
            avatar_crop_scale,
            design_description,
            design_reference_image_ids,
            active_lorebook_ids,
            is_default,
            created_at,
            updated_at,
        ) = row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let mut obj = JsonMap::new();
        obj.insert("id".into(), JsonValue::String(id));
        obj.insert("title".into(), JsonValue::String(title));
        obj.insert("description".into(), JsonValue::String(description));
        if let Some(value) = nickname {
            obj.insert("nickname".into(), JsonValue::String(value));
        }
        if let Some(value) = avatar_path {
            obj.insert("avatarPath".into(), JsonValue::String(value));
        }
        if let (Some(x), Some(y), Some(scale)) = (avatar_crop_x, avatar_crop_y, avatar_crop_scale) {
            let mut crop = JsonMap::new();
            crop.insert("x".into(), JsonValue::from(x));
            crop.insert("y".into(), JsonValue::from(y));
            crop.insert("scale".into(), JsonValue::from(scale));
            obj.insert("avatarCrop".into(), JsonValue::Object(crop));
        }
        if let Some(value) = design_description {
            obj.insert("designDescription".into(), JsonValue::String(value));
        }
        if let Some(value) = design_reference_image_ids {
            if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&value) {
                obj.insert("designReferenceImageIds".into(), serde_json::json!(parsed));
            }
        }
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&active_lorebook_ids) {
            obj.insert("activeLorebookIds".into(), serde_json::json!(parsed));
        }
        obj.insert("isDefault".into(), JsonValue::Bool(is_default != 0));
        obj.insert("createdAt".into(), JsonValue::from(created_at));
        obj.insert("updatedAt".into(), JsonValue::from(updated_at));
        out.push(JsonValue::Object(obj));
    }

    serde_json::from_value(JsonValue::Array(out))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

pub fn persona_default_get_typed<T>(app: &tauri::AppHandle) -> Result<Option<T>, String>
where
    T: serde::de::DeserializeOwned,
{
    let personas: Vec<JsonValue> = personas_list_typed(app)?;
    personas
        .into_iter()
        .find(|persona| {
            persona
                .get("isDefault")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
        })
        .map(|persona| {
            serde_json::from_value(persona)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
        })
        .transpose()
}

pub fn persona_upsert_typed<T, R>(app: &tauri::AppHandle, persona: &T) -> Result<R, String>
where
    T: serde::Serialize,
    R: serde::de::DeserializeOwned,
{
    let value = serde_json::to_value(persona)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    serde_json::from_value(upsert_persona_value(app, &value)?)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

pub fn default_persona_id(app: tauri::AppHandle) -> Option<String> {
    persona_default_get_typed::<JsonValue>(&app)
        .ok()
        .flatten()
        .and_then(|persona| {
            persona
                .get("id")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
        })
}

#[tauri::command]
pub fn personas_list(app: tauri::AppHandle) -> Result<String, String> {
    let conn = open_db(&app)?;
    let mut stmt = conn.prepare("SELECT id, title, description, nickname, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, design_description, design_reference_image_ids, COALESCE(active_lorebook_ids, '[]'), is_default, created_at, updated_at FROM personas ORDER BY created_at ASC").map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<f64>>(5)?,
                r.get::<_, Option<f64>>(6)?,
                r.get::<_, Option<f64>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, String>(10)?,
                r.get::<_, i64>(11)?,
                r.get::<_, i64>(12)?,
                r.get::<_, i64>(13)?,
            ))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut out = Vec::new();
    for row in rows {
        let (
            id,
            title,
            description,
            nickname,
            avatar_path,
            avatar_crop_x,
            avatar_crop_y,
            avatar_crop_scale,
            design_description,
            design_reference_image_ids,
            active_lorebook_ids,
            is_default,
            created_at,
            updated_at,
        ) = row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let mut obj = JsonMap::new();
        obj.insert("id".into(), JsonValue::String(id));
        obj.insert("title".into(), JsonValue::String(title.to_string()));
        obj.insert(
            "description".into(),
            JsonValue::String(description.to_string()),
        );
        if let Some(n) = nickname {
            obj.insert("nickname".into(), JsonValue::String(n));
        }
        if let Some(a) = avatar_path {
            obj.insert("avatarPath".into(), JsonValue::String(a));
        }
        if let (Some(x), Some(y), Some(scale)) = (avatar_crop_x, avatar_crop_y, avatar_crop_scale) {
            let mut crop = JsonMap::new();
            crop.insert("x".into(), JsonValue::from(x));
            crop.insert("y".into(), JsonValue::from(y));
            crop.insert("scale".into(), JsonValue::from(scale));
            obj.insert("avatarCrop".into(), JsonValue::Object(crop));
        }
        if let Some(value) = design_description {
            obj.insert("designDescription".into(), JsonValue::String(value));
        }
        if let Some(value) = design_reference_image_ids {
            if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&value) {
                obj.insert("designReferenceImageIds".into(), serde_json::json!(parsed));
            }
        }
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&active_lorebook_ids) {
            obj.insert("activeLorebookIds".into(), serde_json::json!(parsed));
        }
        obj.insert("isDefault".into(), JsonValue::Bool(is_default != 0));
        obj.insert("createdAt".into(), JsonValue::from(created_at));
        obj.insert("updatedAt".into(), JsonValue::from(updated_at));
        out.push(JsonValue::Object(obj));
    }
    serde_json::to_string(&out).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn persona_upsert(app: tauri::AppHandle, persona_json: String) -> Result<String, String> {
    let persona = serde_json::from_str::<JsonValue>(&persona_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let result = upsert_persona_value(&app, &persona)?;
    serde_json::to_string(&result)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn upsert_persona_value(app: &tauri::AppHandle, p: &JsonValue) -> Result<JsonValue, String> {
    let mut conn = open_db(app)?;
    let id = p
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let title = p
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "title is required".to_string())?;
    let description = p
        .get("description")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "description is required".to_string())?;
    let nickname = p
        .get("nickname")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let avatar_path = p
        .get("avatarPath")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let avatar_crop = p.get("avatarCrop").and_then(|v| v.as_object());
    let avatar_crop_x = avatar_crop.and_then(|crop| crop.get("x").and_then(|v| v.as_f64()));
    let avatar_crop_y = avatar_crop.and_then(|crop| crop.get("y").and_then(|v| v.as_f64()));
    let avatar_crop_scale = avatar_crop.and_then(|crop| crop.get("scale").and_then(|v| v.as_f64()));
    let design_description = p
        .get("designDescription")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let design_reference_image_ids: Option<String> = p
        .get("designReferenceImageIds")
        .and_then(|v| v.as_array())
        .and_then(|arr| serde_json::to_string(arr).ok());
    let active_lorebook_ids: Option<String> = p
        .get("activeLorebookIds")
        .and_then(|v| v.as_array())
        .and_then(|arr| serde_json::to_string(arr).ok());
    let is_default = p
        .get("isDefault")
        .and_then(|v| v.as_bool())
        .unwrap_or(false) as i64;
    let now = now_ms() as i64;

    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let existing_created: Option<i64> = tx
        .query_row(
            "SELECT created_at FROM personas WHERE id = ?",
            params![&id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let created_at = existing_created.unwrap_or(now);

    tx.execute(
        r#"INSERT INTO personas (id, title, description, nickname, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, design_description, design_reference_image_ids, active_lorebook_ids, is_default, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
              title=excluded.title,
              description=excluded.description,
              nickname=excluded.nickname,
              avatar_path=excluded.avatar_path,
              avatar_crop_x=excluded.avatar_crop_x,
              avatar_crop_y=excluded.avatar_crop_y,
              avatar_crop_scale=excluded.avatar_crop_scale,
              design_description=excluded.design_description,
              design_reference_image_ids=excluded.design_reference_image_ids,
              active_lorebook_ids=excluded.active_lorebook_ids,
              is_default=excluded.is_default,
              updated_at=excluded.updated_at"#,
        params![
            &id,
            title,
            description,
            nickname,
            avatar_path,
            avatar_crop_x,
            avatar_crop_y,
            avatar_crop_scale,
            design_description,
            design_reference_image_ids,
            active_lorebook_ids,
            is_default,
            created_at,
            now
        ],
    ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if is_default != 0 {
        tx.execute(
            "UPDATE personas SET is_default = 0 WHERE id <> ?",
            params![&id],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut obj = JsonMap::new();
    obj.insert("id".into(), JsonValue::String(id));
    obj.insert("title".into(), JsonValue::String(title.to_string()));
    obj.insert(
        "description".into(),
        JsonValue::String(description.to_string()),
    );
    if let Some(n) = nickname {
        obj.insert("nickname".into(), JsonValue::String(n));
    }
    if let Some(a) = avatar_path {
        obj.insert("avatarPath".into(), JsonValue::String(a));
    }
    if let (Some(x), Some(y), Some(scale)) = (avatar_crop_x, avatar_crop_y, avatar_crop_scale) {
        let mut crop = JsonMap::new();
        crop.insert("x".into(), JsonValue::from(x));
        crop.insert("y".into(), JsonValue::from(y));
        crop.insert("scale".into(), JsonValue::from(scale));
        obj.insert("avatarCrop".into(), JsonValue::Object(crop));
    }
    if let Some(value) = design_description {
        obj.insert("designDescription".into(), JsonValue::String(value));
    }
    if let Some(value) = design_reference_image_ids {
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&value) {
            obj.insert("designReferenceImageIds".into(), serde_json::json!(parsed));
        }
    }
    if let Some(value) = active_lorebook_ids {
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&value) {
            obj.insert("activeLorebookIds".into(), serde_json::json!(parsed));
        }
    }
    obj.insert("isDefault".into(), JsonValue::Bool(is_default != 0));
    obj.insert("createdAt".into(), JsonValue::from(created_at));
    obj.insert("updatedAt".into(), JsonValue::from(now));
    Ok(JsonValue::Object(obj))
}

#[tauri::command]
pub fn persona_delete(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let conn = open_db(&app)?;
    conn.execute("DELETE FROM personas WHERE id = ?", params![id])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn persona_default_get(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let conn = open_db(&app)?;
    let row = conn.query_row("SELECT id, title, description, nickname, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, design_description, design_reference_image_ids, COALESCE(active_lorebook_ids, '[]'), is_default, created_at, updated_at FROM personas WHERE is_default = 1 LIMIT 1", [], |r| Ok((
        r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, Option<String>>(4)?, r.get::<_, Option<f64>>(5)?, r.get::<_, Option<f64>>(6)?, r.get::<_, Option<f64>>(7)?, r.get::<_, Option<String>>(8)?, r.get::<_, Option<String>>(9)?, r.get::<_, String>(10)?, r.get::<_, i64>(11)?, r.get::<_, i64>(12)?, r.get::<_, i64>(13)?
    ))).optional().map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    if let Some((
        id,
        title,
        description,
        nickname,
        avatar_path,
        avatar_crop_x,
        avatar_crop_y,
        avatar_crop_scale,
        design_description,
        design_reference_image_ids,
        active_lorebook_ids,
        is_default,
        created_at,
        updated_at,
    )) = row
    {
        let mut obj = JsonMap::new();
        obj.insert("id".into(), JsonValue::String(id));
        obj.insert("title".into(), JsonValue::String(title.to_string()));
        obj.insert(
            "description".into(),
            JsonValue::String(description.to_string()),
        );
        if let Some(n) = nickname {
            obj.insert("nickname".into(), JsonValue::String(n));
        }
        if let Some(a) = avatar_path {
            obj.insert("avatarPath".into(), JsonValue::String(a));
        }
        if let (Some(x), Some(y), Some(scale)) = (avatar_crop_x, avatar_crop_y, avatar_crop_scale) {
            let mut crop = JsonMap::new();
            crop.insert("x".into(), JsonValue::from(x));
            crop.insert("y".into(), JsonValue::from(y));
            crop.insert("scale".into(), JsonValue::from(scale));
            obj.insert("avatarCrop".into(), JsonValue::Object(crop));
        }
        if let Some(value) = design_description {
            obj.insert("designDescription".into(), JsonValue::String(value));
        }
        if let Some(value) = design_reference_image_ids {
            if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&value) {
                obj.insert("designReferenceImageIds".into(), serde_json::json!(parsed));
            }
        }
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&active_lorebook_ids) {
            obj.insert("activeLorebookIds".into(), serde_json::json!(parsed));
        }
        obj.insert("isDefault".into(), JsonValue::Bool(is_default != 0));
        obj.insert("createdAt".into(), JsonValue::from(created_at));
        obj.insert("updatedAt".into(), JsonValue::from(updated_at));
        Ok(Some(
            serde_json::to_string(&JsonValue::Object(obj))
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
        ))
    } else {
        Ok(None)
    }
}
