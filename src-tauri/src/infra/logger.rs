use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Manager};

#[cfg(target_os = "android")]
use std::path::Path;
#[cfg(target_os = "android")]
use tauri_plugin_android_fs::{
    convert_dir_path_to_string, convert_file_path_to_string, convert_string_to_dir_path, AndroidFs,
    AndroidFsExt, PersistableAccessMode, PublicGeneralPurposeDir, PublicStorage,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub component: String,
    pub function: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogPage {
    pub total: usize,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogSearchResult {
    pub matches: Vec<usize>,
    pub total: usize,
}

pub struct LogManager {
    file: Mutex<Option<File>>,
    log_dir: PathBuf,
}

#[cfg(target_os = "android")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AndroidLogExportDirConfig {
    dir_path_json: String,
    display_name: String,
}

static GLOBAL_APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

pub fn set_global_app_handle(app_handle: AppHandle) {
    let _ = GLOBAL_APP_HANDLE.set(app_handle);
}

pub fn get_global_app_handle() -> Option<AppHandle> {
    GLOBAL_APP_HANDLE.get().cloned()
}

impl LogManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let log_dir = app_handle.path().app_log_dir().map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to get log directory: {}", e),
            )
        })?;

        fs::create_dir_all(&log_dir)?;

        Ok(Self {
            file: Mutex::new(None),
            log_dir,
        })
    }

    fn get_current_log_file_path(&self) -> PathBuf {
        let now = chrono::Local::now();
        let filename = format!("app-{}.log", now.format("%Y-%m-%d"));
        self.log_dir.join(filename)
    }

    pub fn write_log(&self, entry: LogEntry) -> Result<(), String> {
        let log_path = self.get_current_log_file_path();
        let mut file_lock = self.file.lock().map_err(|e| {
            crate::utils::err_msg(module_path!(), line!(), format!("Lock error: {}", e))
        })?;

        // Check if we need to rotate to a new file (date changed)
        let needs_new_file = file_lock.is_none() || {
            if let Some(ref f) = *file_lock {
                // Check if current file still matches today's date
                !log_path.exists() || f.metadata().is_err()
            } else {
                true
            }
        };

        if needs_new_file {
            let new_file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("Failed to open log file: {}", e),
                    )
                })?;
            *file_lock = Some(new_file);
        }

        if let Some(ref mut file) = *file_lock {
            let mut log_line = format!("[{}] {} {}", entry.timestamp, entry.level, entry.component);
            if let Some(ref f) = entry.function {
                log_line.push_str(" at=");
                log_line.push_str(f);
            }
            log_line.push_str(" | ");
            log_line.push_str(&entry.message);
            log_line.push('\n');

            file.write_all(log_line.as_bytes()).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to write log: {}", e),
                )
            })?;

            file.flush().map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to flush log: {}", e),
                )
            })?;
        }

        Ok(())
    }

    pub fn list_log_files(&self) -> Result<Vec<String>, String> {
        let entries = fs::read_dir(&self.log_dir).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to read log directory: {}", e),
            )
        })?;

        let mut log_files: Vec<String> = entries
            .filter_map(|entry| {
                entry.ok().and_then(|e| {
                    let path = e.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("log") {
                        path.file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                })
            })
            .collect();

        log_files.sort_by(|a, b| b.cmp(a)); // Most recent first
        Ok(log_files)
    }

    pub fn read_log_file(&self, filename: &str) -> Result<String, String> {
        let path = self.log_dir.join(filename);

        if !path.exists() || !path.is_file() {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Log file not found",
            ));
        }

        fs::read_to_string(path).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to read log file: {}", e),
            )
        })
    }

    /// Read a log file returning total line count and a specific page of lines.
    /// `offset` is the starting line (0-based), `limit` is the max lines to return.
    pub fn read_log_file_page(
        &self,
        filename: &str,
        offset: usize,
        limit: usize,
    ) -> Result<LogPage, String> {
        let path = self.log_dir.join(filename);

        if !path.exists() || !path.is_file() {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Log file not found",
            ));
        }

        let file = File::open(&path).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to open log file: {}", e),
            )
        })?;
        let reader = BufReader::new(file);
        let mut total: usize = 0;
        let mut lines: Vec<String> = Vec::with_capacity(limit.min(2000));

        for line_result in reader.lines() {
            let line = line_result.map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to read line: {}", e),
                )
            })?;
            if total >= offset && lines.len() < limit {
                lines.push(line);
            }
            total += 1;
        }

        Ok(LogPage { total, lines })
    }

    /// Search a log file for lines containing the query (case-insensitive).
    /// Returns the 0-based line indices of all matching lines.
    pub fn search_log_file(&self, filename: &str, query: &str) -> Result<LogSearchResult, String> {
        let path = self.log_dir.join(filename);

        if !path.exists() || !path.is_file() {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Log file not found",
            ));
        }

        let file = File::open(&path).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to open log file: {}", e),
            )
        })?;
        let reader = BufReader::new(file);
        let query_lower = query.to_lowercase();
        let mut matches = Vec::new();
        let mut total: usize = 0;

        for line_result in reader.lines() {
            let line = line_result.map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to read line: {}", e),
                )
            })?;
            if line.to_lowercase().contains(&query_lower) {
                matches.push(total);
            }
            total += 1;
        }

        Ok(LogSearchResult { matches, total })
    }

    /// Find log lines relevant to a given reference line.
    pub fn find_relevant_lines(
        &self,
        filename: &str,
        reference_line: &str,
    ) -> Result<LogSearchResult, String> {
        let path = self.log_dir.join(filename);
        if !path.exists() || !path.is_file() {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Log file not found",
            ));
        }

        let ref_component = reference_line
            .split_whitespace()
            .nth(2)
            .unwrap_or("")
            .to_string();

        let uuid_re = regex::Regex::new(
            r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
        )
        .unwrap();
        let ref_uuids: Vec<String> = uuid_re
            .find_iter(reference_line)
            .map(|m| m.as_str().to_lowercase())
            .collect();

        let ref_message_keywords: Vec<String> = reference_line
            .split('|')
            .nth(1)
            .unwrap_or("")
            .split_whitespace()
            .filter(|w| w.len() >= 5) // only significant words
            .map(|w| w.to_lowercase())
            .collect();

        let parse_epoch = |line: &str| -> Option<i64> {
            let ts = line.get(1..20)?;
            chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|dt| dt.and_utc().timestamp())
        };
        let ref_epoch = parse_epoch(reference_line);

        let file = File::open(&path).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to open log file: {}", e),
            )
        })?;
        let reader = BufReader::new(file);
        let mut matches = Vec::new();
        let mut total: usize = 0;

        for line_result in reader.lines() {
            let line = line_result.map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to read line: {}", e),
                )
            })?;

            let mut matched = false;

            if !ref_uuids.is_empty() {
                let line_lower = line.to_lowercase();
                if ref_uuids.iter().any(|uuid| line_lower.contains(uuid)) {
                    matched = true;
                }
            }

            if !matched && !ref_component.is_empty() {
                let line_component = line.split_whitespace().nth(2).unwrap_or("");
                if line_component == ref_component {
                    if let (Some(ref_ts), Some(line_ts)) = (ref_epoch, parse_epoch(&line)) {
                        if (line_ts - ref_ts).abs() <= 30 {
                            matched = true;
                        }
                    }

                    if !matched && !ref_message_keywords.is_empty() {
                        if let Some(line_msg) = line.split('|').nth(1) {
                            let line_msg_lower = line_msg.to_lowercase();
                            let shared = ref_message_keywords
                                .iter()
                                .filter(|kw| line_msg_lower.contains(kw.as_str()))
                                .count();
                            if shared >= 2
                                || (ref_message_keywords.len() > 0
                                    && shared * 100 / ref_message_keywords.len() > 40)
                            {
                                matched = true;
                            }
                        }
                    }
                }
            }

            if matched {
                matches.push(total);
            }
            total += 1;
        }

        Ok(LogSearchResult { matches, total })
    }

    pub fn delete_log_file(&self, filename: &str) -> Result<(), String> {
        let path = self.log_dir.join(filename);

        if !path.exists() || !path.is_file() {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Log file not found",
            ));
        }

        fs::remove_file(path).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to delete log file: {}", e),
            )
        })
    }

    pub fn clear_all_logs(&self) -> Result<(), String> {
        let entries = fs::read_dir(&self.log_dir).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to read log directory: {}", e),
            )
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension() == Some("log".as_ref()) {
                fs::remove_file(path).map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("Failed to delete log file: {}", e),
                    )
                })?;
            }
        }

        Ok(())
    }

    pub fn get_log_dir_path(&self) -> String {
        self.log_dir.to_string_lossy().to_string()
    }
}

#[cfg(target_os = "android")]
fn android_log_export_config_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let monitor_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to get app data dir: {}", e),
            )
        })?
        .join("monitor");

    fs::create_dir_all(&monitor_dir).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to create monitor directory: {}", e),
        )
    })?;

    Ok(monitor_dir.join("log-export-dir.json"))
}

#[cfg(target_os = "android")]
fn read_android_log_export_dir_config(
    app_handle: &AppHandle,
) -> Result<Option<AndroidLogExportDirConfig>, String> {
    let path = android_log_export_config_path(app_handle)?;
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to read Android log export config: {}", e),
        )
    })?;

    serde_json::from_str(&content).map(Some).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse Android log export config: {}", e),
        )
    })
}

#[cfg(target_os = "android")]
fn write_android_log_export_dir_config(
    app_handle: &AppHandle,
    config: &AndroidLogExportDirConfig,
) -> Result<(), String> {
    let path = android_log_export_config_path(app_handle)?;
    let content = serde_json::to_string_pretty(config).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize Android log export config: {}", e),
        )
    })?;

    fs::write(path, content).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to write Android log export config: {}", e),
        )
    })
}

#[tauri::command]
pub async fn log_to_file(
    app_handle: AppHandle,
    timestamp: String,
    level: String,
    component: String,
    function: Option<String>,
    message: String,
) -> Result<(), String> {
    let logger = app_handle.state::<LogManager>();

    let entry = LogEntry {
        timestamp,
        level,
        component,
        function,
        message,
    };

    logger.write_log(entry)
}

#[tauri::command]
pub async fn list_log_files(app_handle: AppHandle) -> Result<Vec<String>, String> {
    let logger = app_handle.state::<LogManager>();
    logger.list_log_files()
}

#[tauri::command]
pub async fn read_log_file(app_handle: AppHandle, filename: String) -> Result<String, String> {
    let logger = app_handle.state::<LogManager>();
    logger.read_log_file(&filename)
}

#[tauri::command]
pub async fn read_log_file_page(
    app_handle: AppHandle,
    filename: String,
    offset: usize,
    limit: usize,
) -> Result<LogPage, String> {
    let logger = app_handle.state::<LogManager>();
    logger.read_log_file_page(&filename, offset, limit)
}

#[tauri::command]
pub async fn find_relevant_lines(
    app_handle: AppHandle,
    filename: String,
    reference_line: String,
) -> Result<LogSearchResult, String> {
    let logger = app_handle.state::<LogManager>();
    logger.find_relevant_lines(&filename, &reference_line)
}

#[tauri::command]
pub async fn search_log_file(
    app_handle: AppHandle,
    filename: String,
    query: String,
) -> Result<LogSearchResult, String> {
    let logger = app_handle.state::<LogManager>();
    logger.search_log_file(&filename, &query)
}

#[tauri::command]
pub async fn delete_log_file(app_handle: AppHandle, filename: String) -> Result<(), String> {
    let logger = app_handle.state::<LogManager>();
    logger.delete_log_file(&filename)
}

#[tauri::command]
pub async fn clear_all_logs(app_handle: AppHandle) -> Result<(), String> {
    let logger = app_handle.state::<LogManager>();
    logger.clear_all_logs()
}

#[tauri::command]
pub async fn get_log_dir_path(app_handle: AppHandle) -> Result<String, String> {
    let logger = app_handle.state::<LogManager>();
    Ok(logger.get_log_dir_path())
}

#[tauri::command]
pub async fn get_log_export_dir(app_handle: AppHandle) -> Result<Option<String>, String> {
    #[cfg(target_os = "android")]
    {
        return Ok(
            read_android_log_export_dir_config(&app_handle)?.map(|config| config.display_name)
        );
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = app_handle;
        Ok(None)
    }
}

#[tauri::command]
pub async fn pick_log_export_dir(app_handle: AppHandle) -> Result<Option<String>, String> {
    #[cfg(target_os = "android")]
    {
        let api = app_handle.android_fs();
        let Some(dir_path) = api.show_open_dir_dialog().map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to open directory picker: {}", e),
            )
        })?
        else {
            return Ok(None);
        };

        api.grant_persistable_dir_access(&dir_path, PersistableAccessMode::ReadAndWrite)
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to persist directory access: {}", e),
                )
            })?;

        let display_name = api
            .get_dir_name(&dir_path)
            .unwrap_or_else(|_| "Selected folder".to_string());
        let dir_path_json = convert_dir_path_to_string(&dir_path).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to serialize selected directory: {}", e),
            )
        })?;

        write_android_log_export_dir_config(
            &app_handle,
            &AndroidLogExportDirConfig {
                dir_path_json,
                display_name: display_name.clone(),
            },
        )?;

        return Ok(Some(display_name));
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = app_handle;
        Ok(None)
    }
}

#[tauri::command]
pub async fn clear_log_export_dir(app_handle: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let path = android_log_export_config_path(&app_handle)?;
        if path.exists() {
            fs::remove_file(path).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to remove Android log export config: {}", e),
                )
            })?;
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = app_handle;
    }

    Ok(())
}

#[tauri::command]
pub async fn save_log_to_downloads(
    app_handle: AppHandle,
    filename: String,
) -> Result<String, String> {
    let logger = app_handle.state::<LogManager>();
    let content = logger.read_log_file(&filename)?;

    #[cfg(target_os = "android")]
    {
        let safe_filename = Path::new(&filename)
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .ok_or_else(|| crate::utils::err_msg(module_path!(), line!(), "Invalid log filename"))?
            .to_string();

        let api = app_handle.android_fs();
        if let Some(config) = read_android_log_export_dir_config(&app_handle)? {
            let dir_path = convert_string_to_dir_path(&config.dir_path_json).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to deserialize selected log directory: {}", e),
                )
            })?;

            let saved_path = api
                .new_file_with_contents(
                    &dir_path,
                    &safe_filename,
                    Some("text/plain"),
                    content.as_bytes(),
                )
                .map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!(
                            "Failed to save Android log file to selected directory: {}",
                            e
                        ),
                    )
                })?;

            let saved_name = api.get_file_name(&saved_path).unwrap_or(safe_filename);
            let saved_uri = convert_file_path_to_string(&saved_path);

            return Ok(format!(
                "{}/{}\n{}",
                config.display_name, saved_name, saved_uri
            ));
        }

        let saved_path = api
            .public_storage()
            .write(
                PublicGeneralPurposeDir::Download,
                format!("lettuceai/logs/{safe_filename}"),
                Some("text/plain"),
                content.as_bytes(),
            )
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to save Android log file: {}", e),
                )
            })?;

        let saved_name = api.get_file_name(&saved_path).unwrap_or(safe_filename);
        let saved_uri = convert_file_path_to_string(&saved_path);

        Ok(format!(
            "Downloads/lettuceai/logs/{saved_name}\n{saved_uri}"
        ))
    }

    #[cfg(not(target_os = "android"))]
    {
        let download_dir = app_handle.path().download_dir().map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to get downloads directory: {}", e),
            )
        })?;

        if !download_dir.exists() {
            std::fs::create_dir_all(&download_dir).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to create downloads directory: {}", e),
                )
            })?;
        }

        let file_path = download_dir.join(&filename);

        std::fs::write(&file_path, content.as_bytes()).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to write file: {}", e),
            )
        })?;

        let path_str = file_path
            .to_str()
            .ok_or_else(|| "Invalid path".to_string())?
            .to_string();

        Ok(path_str)
    }
}
