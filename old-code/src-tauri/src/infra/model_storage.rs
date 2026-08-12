use std::path::{Path, PathBuf};

use tauri::AppHandle;

pub fn read_custom_dir(app: &AppHandle, key: &str) -> Option<PathBuf> {
    let settings_json = crate::storage_manager::settings::internal_read_settings(app).ok()??;
    let value: serde_json::Value = serde_json::from_str(&settings_json).ok()?;
    let dir = value
        .get("advancedSettings")?
        .get(key)?
        .as_str()?
        .trim()
        .to_string();
    if dir.is_empty() {
        None
    } else {
        Some(PathBuf::from(dir))
    }
}

pub fn persist_custom_dir(app: &AppHandle, key: &str, dir: Option<&str>) -> Result<(), String> {
    let settings_json = crate::storage_manager::settings::internal_read_settings(app)?;
    let mut advanced = settings_json
        .as_deref()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .and_then(|value| value.get("advancedSettings").cloned())
        .filter(|value| value.is_object())
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    let map = advanced.as_object_mut().ok_or_else(|| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            "advancedSettings is not an object".to_string(),
        )
    })?;
    match dir {
        Some(value) if !value.trim().is_empty() => {
            map.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
        _ => {
            map.remove(key);
        }
    }
    let advanced_json = serde_json::to_string(&advanced)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    crate::storage_manager::settings::settings_set_advanced(app.clone(), advanced_json)
}

pub fn count_models_in_dir(dir: &Path) -> u32 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut count = 0u32;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() || path.extension().map(|ext| !ext.is_empty()).unwrap_or(false) {
            count += 1;
        }
    }
    count
}

pub fn paths_equal(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

pub fn rewrite_path_prefix(path: &str, old_prefix: &str, new_prefix: &str) -> String {
    let sep = std::path::MAIN_SEPARATOR;
    let old_trim = old_prefix.trim_end_matches(|c| c == '/' || c == '\\');
    let new_trim = new_prefix.trim_end_matches(|c| c == '/' || c == '\\');
    if path == old_trim {
        return new_trim.to_string();
    }
    let with_sep = format!("{}{}", old_trim, sep);
    if let Some(rest) = path.strip_prefix(&with_sep) {
        return format!("{}{}{}", new_trim, sep, rest);
    }
    path.to_string()
}

fn remove_path(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

fn copy_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    if src.is_dir() {
        std::fs::create_dir_all(dest)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let entries = std::fs::read_dir(src)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        for entry in entries {
            let entry =
                entry.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            copy_recursive(&entry.path(), &dest.join(entry.file_name()))?;
        }
    } else {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
        std::fs::copy(src, dest)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if let Ok(file) = std::fs::File::open(dest) {
            file.sync_all()
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }
    Ok(())
}

pub fn migrate_models_dir<F>(from: &Path, to: &Path, rewire: F) -> Result<(u32, u32), String>
where
    F: FnOnce(&str, &str) -> Result<u32, String>,
{
    if !from.exists() {
        return Ok((0, 0));
    }
    std::fs::create_dir_all(to).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to create destination folder: {}", e),
        )
    })?;

    let mut sources: Vec<(std::ffi::OsString, PathBuf)> = Vec::new();
    let entries = std::fs::read_dir(from).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to read models folder: {}", e),
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        sources.push((entry.file_name(), entry.path()));
    }

    for (name, _) in &sources {
        if to.join(name).exists() {
            return Err(format!(
                "Destination already contains \"{}\". Pick an empty folder.",
                name.to_string_lossy()
            ));
        }
    }

    let mut created: Vec<PathBuf> = Vec::new();
    for (name, src) in &sources {
        let dest = to.join(name);
        if let Err(err) = copy_recursive(src, &dest) {
            for path in &created {
                let _ = remove_path(path);
            }
            let _ = remove_path(&dest);
            return Err(err);
        }
        created.push(dest);
    }

    if let Ok(dir) = std::fs::File::open(to) {
        let _ = dir.sync_all();
    }

    let old_prefix = from.to_string_lossy().to_string();
    let new_prefix = to.to_string_lossy().to_string();
    let rewired = match rewire(&old_prefix, &new_prefix) {
        Ok(count) => count,
        Err(err) => {
            for path in &created {
                let _ = remove_path(path);
            }
            return Err(err);
        }
    };

    for (_, src) in &sources {
        let _ = remove_path(src);
    }

    Ok((created.len() as u32, rewired))
}
