use chrono::{
    DateTime, Datelike, Duration, Local, LocalResult, Months, NaiveDate, TimeZone, Timelike,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use uuid::Uuid;

use crate::storage_manager::db::open_db;
use crate::utils::now_millis;

const MAX_NOTE_CONTENT_CHARS: usize = 1000;
const MAX_BLOCK_CHARS: usize = 4000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompanionScheduledNote {
    pub id: String,
    pub character_id: String,
    pub label: String,
    pub content: String,
    pub available_at: u64,
    pub expires_at: Option<u64>,
    pub recurrence: String,
    pub recurrence_window_ms: Option<u64>,
    pub enabled: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

fn normalize_recurrence(value: &str) -> Result<&str, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => Ok("none"),
        "daily" => Ok("daily"),
        "weekly" => Ok("weekly"),
        "monthly" => Ok("monthly"),
        "yearly" => Ok("yearly"),
        other => Err(format!("Unsupported recurrence '{}'", other)),
    }
}

fn timestamp_ms_to_local(ts_ms: u64) -> Result<DateTime<Local>, String> {
    match Local.timestamp_millis_opt(ts_ms as i64) {
        LocalResult::Single(value) => Ok(value),
        LocalResult::Ambiguous(earliest, _) => Ok(earliest),
        LocalResult::None => Err(format!("Invalid timestamp {}", ts_ms)),
    }
}

pub fn local_datetime_to_ms(dt: DateTime<Local>) -> u64 {
    dt.timestamp_millis().max(0) as u64
}

pub fn resolve_local_datetime(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    millis: u32,
) -> Result<DateTime<Local>, String> {
    let date = NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| format!("Invalid local date {year:04}-{month:02}-{day:02}"))?;
    let naive = date
        .and_hms_milli_opt(hour, minute, second, millis)
        .ok_or_else(|| "Invalid local time".to_string())?;
    match Local.from_local_datetime(&naive) {
        LocalResult::Single(value) => Ok(value),
        LocalResult::Ambiguous(earliest, _) => Ok(earliest),
        LocalResult::None => {
            let shifted = naive + Duration::hours(1);
            match Local.from_local_datetime(&shifted) {
                LocalResult::Single(value) => Ok(value),
                LocalResult::Ambiguous(earliest, _) => Ok(earliest),
                LocalResult::None => Err("Could not resolve local datetime".to_string()),
            }
        }
    }
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
    for day in (28..=31).rev() {
        if NaiveDate::from_ymd_opt(year, month, day).is_some() {
            return day;
        }
    }
    28
}

fn yearly_occurrence(base: DateTime<Local>, year: i32) -> Result<DateTime<Local>, String> {
    let month = base.month();
    let day = base.day();
    let resolved_day = if month == 2 && day == 29 && NaiveDate::from_ymd_opt(year, 2, 29).is_none()
    {
        28
    } else {
        day.min(last_day_of_month(year, month))
    };
    resolve_local_datetime(
        year,
        month,
        resolved_day,
        base.hour(),
        base.minute(),
        base.second(),
        base.timestamp_subsec_millis(),
    )
}

fn monthly_occurrence(
    base: DateTime<Local>,
    year: i32,
    month: u32,
) -> Result<DateTime<Local>, String> {
    let resolved_day = base.day().min(last_day_of_month(year, month));
    resolve_local_datetime(
        year,
        month,
        resolved_day,
        base.hour(),
        base.minute(),
        base.second(),
        base.timestamp_subsec_millis(),
    )
}

pub fn next_occurrence_after_ms(
    available_at: u64,
    recurrence: &str,
    occurrence_ms: u64,
) -> Result<Option<u64>, String> {
    if recurrence == "none" {
        return Ok(None);
    }
    let base = timestamp_ms_to_local(available_at)?;
    let occurrence = timestamp_ms_to_local(occurrence_ms)?;
    let next = match recurrence {
        "daily" => occurrence + Duration::days(1),
        "weekly" => occurrence + Duration::weeks(1),
        "monthly" => {
            let first_of_month = occurrence
                .with_day(1)
                .ok_or_else(|| "Failed to compute first day of month".to_string())?;
            let target = first_of_month
                .checked_add_months(Months::new(1))
                .ok_or_else(|| "Failed to advance month".to_string())?;
            monthly_occurrence(base, target.year(), target.month())?
        }
        "yearly" => yearly_occurrence(base, occurrence.year() + 1)?,
        _ => return Err(format!("Unsupported recurrence '{}'", recurrence)),
    };
    Ok(Some(local_datetime_to_ms(next)))
}

pub fn most_recent_occurrence_on_or_before(
    available_at: u64,
    recurrence: &str,
    now_ms: u64,
) -> Result<u64, String> {
    if now_ms < available_at {
        return Ok(available_at);
    }
    if recurrence == "none" {
        return Ok(available_at);
    }

    let base = timestamp_ms_to_local(available_at)?;
    let now = timestamp_ms_to_local(now_ms)?;
    let occurrence = match recurrence {
        "daily" => {
            let diff_days = now
                .date_naive()
                .signed_duration_since(base.date_naive())
                .num_days();
            let days = diff_days.max(0);
            base + Duration::days(days)
        }
        "weekly" => {
            let diff_days = now
                .date_naive()
                .signed_duration_since(base.date_naive())
                .num_days();
            let weeks = (diff_days / 7).max(0);
            base + Duration::weeks(weeks)
        }
        "monthly" => {
            let month_diff =
                (now.year() - base.year()) * 12 + (now.month() as i32 - base.month() as i32);
            let mut candidate = monthly_occurrence(
                base,
                base.year() + month_diff.div_euclid(12),
                ((base.month0() as i32 + month_diff.rem_euclid(12)) as u32) + 1,
            )?;
            if candidate > now {
                let prev_month_anchor = candidate
                    .with_day(1)
                    .ok_or_else(|| "Failed to compute previous month anchor".to_string())?
                    .checked_sub_months(Months::new(1))
                    .ok_or_else(|| "Failed to step back one month".to_string())?;
                candidate =
                    monthly_occurrence(base, prev_month_anchor.year(), prev_month_anchor.month())?;
            }
            candidate
        }
        "yearly" => {
            let mut candidate = yearly_occurrence(base, now.year())?;
            if candidate > now {
                candidate = yearly_occurrence(base, now.year() - 1)?;
            }
            candidate
        }
        _ => return Err(format!("Unsupported recurrence '{}'", recurrence)),
    };
    Ok(local_datetime_to_ms(occurrence))
}

pub fn is_note_active(note: &CompanionScheduledNote, now_ms: u64) -> Result<bool, String> {
    if !note.enabled {
        return Ok(false);
    }
    if let Some(expires_at) = note.expires_at {
        if now_ms >= expires_at {
            return Ok(false);
        }
    }
    if now_ms < note.available_at {
        return Ok(false);
    }

    let recurrence = normalize_recurrence(&note.recurrence)?;
    if recurrence == "none" {
        return Ok(true);
    }

    let occurrence = most_recent_occurrence_on_or_before(note.available_at, recurrence, now_ms)?;
    if let Some(window_ms) = note.recurrence_window_ms {
        return Ok(now_ms < occurrence.saturating_add(window_ms));
    }
    Ok(true)
}

fn note_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CompanionScheduledNote> {
    Ok(CompanionScheduledNote {
        id: row.get(0)?,
        character_id: row.get(1)?,
        label: row.get(2)?,
        content: row.get(3)?,
        available_at: row.get::<_, i64>(4)?.max(0) as u64,
        expires_at: row
            .get::<_, Option<i64>>(5)?
            .map(|value| value.max(0) as u64),
        recurrence: row.get(6)?,
        recurrence_window_ms: row
            .get::<_, Option<i64>>(7)?
            .map(|value| value.max(0) as u64),
        enabled: row.get::<_, i64>(8)? != 0,
        created_at: row.get::<_, i64>(9)?.max(0) as u64,
        updated_at: row.get::<_, i64>(10)?.max(0) as u64,
    })
}

fn ensure_companion_character(
    conn: &rusqlite::Connection,
    character_id: &str,
) -> Result<(), String> {
    let mode = conn
        .query_row(
            "SELECT mode FROM characters WHERE id = ?1",
            params![character_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .flatten()
        .ok_or_else(|| "Character not found".to_string())?;
    if !mode.eq_ignore_ascii_case("companion") {
        return Err("Scheduled notes are only available for companion-mode characters".to_string());
    }
    Ok(())
}

fn get_note_character_id(conn: &rusqlite::Connection, id: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT character_id FROM companion_scheduled_notes WHERE id = ?1",
        params![id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

pub fn list_notes(
    app: &AppHandle,
    character_id: &str,
) -> Result<Vec<CompanionScheduledNote>, String> {
    let conn = open_db(app)?;
    ensure_companion_character(&conn, character_id)?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, character_id, label, content, available_at, expires_at, recurrence,
                   recurrence_window_ms, enabled, created_at, updated_at
            FROM companion_scheduled_notes
            WHERE character_id = ?1
            ORDER BY available_at ASC, id ASC
            "#,
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map(params![character_id], note_from_row)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

pub fn active_notes_for_character(
    app: &AppHandle,
    character_id: &str,
    now_ms: u64,
) -> Result<Vec<CompanionScheduledNote>, String> {
    let notes = list_notes(app, character_id)?;
    let mut active = Vec::new();
    for note in notes {
        if is_note_active(&note, now_ms)? {
            active.push(note);
        }
    }
    Ok(active)
}

pub fn render_scheduled_notes_block(
    app: &AppHandle,
    character_id: &str,
    now_ms: u64,
) -> Result<Option<String>, String> {
    let active_notes = active_notes_for_character(app, character_id, now_ms)?;
    if active_notes.is_empty() {
        return Ok(None);
    }

    let mut lines = Vec::new();
    let mut total_chars = 0usize;
    for note in active_notes {
        let trimmed = note.content.trim();
        if trimmed.is_empty() {
            continue;
        }
        let capped = if trimmed.chars().count() > MAX_NOTE_CONTENT_CHARS {
            let shortened: String = trimmed.chars().take(MAX_NOTE_CONTENT_CHARS).collect();
            format!("{}...", shortened.trim_end())
        } else {
            trimmed.to_string()
        };
        let line = format!("- {}", capped);
        if total_chars + line.len() > MAX_BLOCK_CHARS {
            break;
        }
        total_chars += line.len() + 1;
        lines.push(line);
    }

    if lines.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!(
        "[Background context you currently hold in mind]\n{}",
        lines.join("\n")
    )))
}

#[tauri::command]
pub fn companion_scheduled_notes_list(
    app: AppHandle,
    character_id: String,
) -> Result<String, String> {
    serde_json::to_string(&list_notes(&app, &character_id)?)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn companion_scheduled_notes_preview_active(
    app: AppHandle,
    character_id: String,
    as_of_ms: u64,
) -> Result<String, String> {
    serde_json::to_string(&active_notes_for_character(&app, &character_id, as_of_ms)?)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn companion_scheduled_notes_upsert(
    app: AppHandle,
    note_json: String,
) -> Result<String, String> {
    let mut note: CompanionScheduledNote = serde_json::from_str(&note_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let recurrence = normalize_recurrence(&note.recurrence)?.to_string();
    let conn = open_db(&app)?;
    ensure_companion_character(&conn, &note.character_id)?;
    if let Some(expires_at) = note.expires_at {
        if expires_at <= note.available_at {
            return Err("End date must be after the start date".to_string());
        }
    }
    note.recurrence = recurrence;
    note.label = note.label.trim().to_string();
    note.content = note.content.trim().to_string();
    if note.content.is_empty() {
        return Err("Scheduled note content cannot be empty".to_string());
    }
    let now = now_millis()?;
    if note.id.trim().is_empty() || Uuid::parse_str(&note.id).is_err() {
        note.id = Uuid::new_v4().to_string();
    }
    if note.created_at == 0 {
        note.created_at = now;
    }
    note.updated_at = now;

    conn.execute(
        r#"
        INSERT INTO companion_scheduled_notes (
            id, character_id, label, content, available_at, expires_at, recurrence,
            recurrence_window_ms, enabled, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        ON CONFLICT(id) DO UPDATE SET
            character_id = excluded.character_id,
            label = excluded.label,
            content = excluded.content,
            available_at = excluded.available_at,
            expires_at = excluded.expires_at,
            recurrence = excluded.recurrence,
            recurrence_window_ms = excluded.recurrence_window_ms,
            enabled = excluded.enabled,
            updated_at = excluded.updated_at
        "#,
        params![
            note.id,
            note.character_id,
            note.label,
            note.content,
            note.available_at as i64,
            note.expires_at.map(|value| value as i64),
            note.recurrence,
            note.recurrence_window_ms.map(|value| value as i64),
            if note.enabled { 1 } else { 0 },
            note.created_at as i64,
            note.updated_at as i64
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    serde_json::to_string(&note)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn companion_scheduled_notes_delete(app: AppHandle, id: String) -> Result<(), String> {
    let conn = open_db(&app)?;
    if let Some(character_id) = get_note_character_id(&conn, &id)? {
        ensure_companion_character(&conn, &character_id)?;
    }
    conn.execute(
        "DELETE FROM companion_scheduled_notes WHERE id = ?1",
        params![id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}
