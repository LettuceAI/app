use chrono::{
    DateTime, Datelike, Duration, Local, LocalResult, Months, NaiveDate, TimeZone, Timelike,
};
use lettuce_types::{CharacterId, TimestampMillis};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const MAX_NOTE_CONTENT_CHARS: usize = 1000;
const MAX_BLOCK_CHARS: usize = 4000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduledNoteRecurrence {
    None,
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionScheduledNote {
    pub id: Uuid,
    pub character_id: CharacterId,
    pub label: String,
    pub content: String,
    pub available_at: TimestampMillis,
    pub expires_at: Option<TimestampMillis>,
    pub recurrence: ScheduledNoteRecurrence,
    pub recurrence_window_ms: Option<u64>,
    pub enabled: bool,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompanionScheduledNoteError {
    #[error("companion scheduled note was not found")]
    NotFound,
    #[error("companion scheduled note conflicts with durable state")]
    Conflict,
    #[error("companion scheduled note is invalid")]
    Invalid,
    #[error("companion scheduled note storage failed")]
    Failure,
    #[error("companion scheduled note storage is corrupt")]
    Corrupt,
}

pub trait CompanionScheduledNoteRepository: Send + Sync {
    fn list_scheduled_notes(
        &self,
        character_id: CharacterId,
    ) -> Result<Vec<CompanionScheduledNote>, CompanionScheduledNoteError>;

    fn upsert_scheduled_note(
        &self,
        note: CompanionScheduledNote,
    ) -> Result<CompanionScheduledNote, CompanionScheduledNoteError>;

    fn delete_scheduled_note(&self, id: Uuid) -> Result<(), CompanionScheduledNoteError>;
}

impl CompanionScheduledNote {
    pub fn normalize(mut self) -> Result<Self, CompanionScheduledNoteError> {
        self.label = self.label.trim().to_owned();
        self.content = self.content.trim().to_owned();
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), CompanionScheduledNoteError> {
        if self.content.is_empty()
            || self.available_at.get() < 0
            || self.created_at.get() < 0
            || self.updated_at.get() < 0
            || self
                .expires_at
                .is_some_and(|expires| expires <= self.available_at || expires.get() < 0)
        {
            return Err(CompanionScheduledNoteError::Invalid);
        }
        Ok(())
    }
}

pub fn active_scheduled_notes(
    notes: Vec<CompanionScheduledNote>,
    now: TimestampMillis,
) -> Result<Vec<CompanionScheduledNote>, CompanionScheduledNoteError> {
    if now.get() < 0 {
        return Err(CompanionScheduledNoteError::Invalid);
    }
    let mut active = Vec::new();
    for note in notes {
        note.validate()?;
        if is_scheduled_note_active(&note, now)? {
            active.push(note);
        }
    }
    active.sort_by_key(|note| (note.available_at, note.id));
    Ok(active)
}

pub fn is_scheduled_note_active(
    note: &CompanionScheduledNote,
    now: TimestampMillis,
) -> Result<bool, CompanionScheduledNoteError> {
    note.validate()?;
    if !note.enabled
        || note.expires_at.is_some_and(|expires| now >= expires)
        || now < note.available_at
    {
        return Ok(false);
    }
    if note.recurrence == ScheduledNoteRecurrence::None {
        return Ok(true);
    }
    let occurrence =
        most_recent_scheduled_note_occurrence(note.available_at, note.recurrence, now)?;
    if let Some(window_ms) = note.recurrence_window_ms {
        return Ok(now.get() < occurrence.get().saturating_add_unsigned(window_ms));
    }
    Ok(true)
}

pub fn render_scheduled_notes_block(notes: &[CompanionScheduledNote]) -> Option<String> {
    let mut lines = Vec::new();
    let mut total_chars = 0usize;
    for note in notes {
        let trimmed = note.content.trim();
        if trimmed.is_empty() {
            continue;
        }
        let capped = if trimmed.chars().count() > MAX_NOTE_CONTENT_CHARS {
            let shortened: String = trimmed.chars().take(MAX_NOTE_CONTENT_CHARS).collect();
            format!("{}...", shortened.trim_end())
        } else {
            trimmed.to_owned()
        };
        let line = format!("- {capped}");
        if total_chars + line.len() > MAX_BLOCK_CHARS {
            break;
        }
        total_chars += line.len() + 1;
        lines.push(line);
    }
    (!lines.is_empty()).then(|| {
        format!(
            "[Background context you currently hold in mind]\n{}",
            lines.join("\n")
        )
    })
}

fn timestamp_ms_to_local(
    timestamp: TimestampMillis,
) -> Result<DateTime<Local>, CompanionScheduledNoteError> {
    match Local.timestamp_millis_opt(timestamp.get()) {
        LocalResult::Single(value) => Ok(value),
        LocalResult::Ambiguous(earliest, _) => Ok(earliest),
        LocalResult::None => Err(CompanionScheduledNoteError::Invalid),
    }
}

fn local_datetime_to_ms(datetime: DateTime<Local>) -> TimestampMillis {
    TimestampMillis::new(datetime.timestamp_millis().max(0))
}

fn resolve_local_datetime(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    millis: u32,
) -> Result<DateTime<Local>, CompanionScheduledNoteError> {
    let date =
        NaiveDate::from_ymd_opt(year, month, day).ok_or(CompanionScheduledNoteError::Invalid)?;
    let naive = date
        .and_hms_milli_opt(hour, minute, second, millis)
        .ok_or(CompanionScheduledNoteError::Invalid)?;
    match Local.from_local_datetime(&naive) {
        LocalResult::Single(value) => Ok(value),
        LocalResult::Ambiguous(earliest, _) => Ok(earliest),
        LocalResult::None => match Local.from_local_datetime(&(naive + Duration::hours(1))) {
            LocalResult::Single(value) => Ok(value),
            LocalResult::Ambiguous(earliest, _) => Ok(earliest),
            LocalResult::None => Err(CompanionScheduledNoteError::Invalid),
        },
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

fn yearly_occurrence(
    base: DateTime<Local>,
    year: i32,
) -> Result<DateTime<Local>, CompanionScheduledNoteError> {
    let month = base.month();
    let day = if month == 2 && base.day() == 29 && NaiveDate::from_ymd_opt(year, 2, 29).is_none() {
        28
    } else {
        base.day().min(last_day_of_month(year, month))
    };
    resolve_local_datetime(
        year,
        month,
        day,
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
) -> Result<DateTime<Local>, CompanionScheduledNoteError> {
    resolve_local_datetime(
        year,
        month,
        base.day().min(last_day_of_month(year, month)),
        base.hour(),
        base.minute(),
        base.second(),
        base.timestamp_subsec_millis(),
    )
}

pub fn next_scheduled_note_occurrence(
    available_at: TimestampMillis,
    recurrence: ScheduledNoteRecurrence,
    occurrence: TimestampMillis,
) -> Result<Option<TimestampMillis>, CompanionScheduledNoteError> {
    if recurrence == ScheduledNoteRecurrence::None {
        return Ok(None);
    }
    let base = timestamp_ms_to_local(available_at)?;
    let occurrence = timestamp_ms_to_local(occurrence)?;
    let next = match recurrence {
        ScheduledNoteRecurrence::None => return Ok(None),
        ScheduledNoteRecurrence::Daily => occurrence + Duration::days(1),
        ScheduledNoteRecurrence::Weekly => occurrence + Duration::weeks(1),
        ScheduledNoteRecurrence::Monthly => {
            let target = occurrence
                .with_day(1)
                .ok_or(CompanionScheduledNoteError::Invalid)?
                .checked_add_months(Months::new(1))
                .ok_or(CompanionScheduledNoteError::Invalid)?;
            monthly_occurrence(base, target.year(), target.month())?
        }
        ScheduledNoteRecurrence::Yearly => yearly_occurrence(base, occurrence.year() + 1)?,
    };
    Ok(Some(local_datetime_to_ms(next)))
}

pub fn most_recent_scheduled_note_occurrence(
    available_at: TimestampMillis,
    recurrence: ScheduledNoteRecurrence,
    now: TimestampMillis,
) -> Result<TimestampMillis, CompanionScheduledNoteError> {
    if now < available_at || recurrence == ScheduledNoteRecurrence::None {
        return Ok(available_at);
    }
    let base = timestamp_ms_to_local(available_at)?;
    let now = timestamp_ms_to_local(now)?;
    let occurrence = match recurrence {
        ScheduledNoteRecurrence::None => base,
        ScheduledNoteRecurrence::Daily => {
            let days = now
                .date_naive()
                .signed_duration_since(base.date_naive())
                .num_days()
                .max(0);
            base + Duration::days(days)
        }
        ScheduledNoteRecurrence::Weekly => {
            let days = now
                .date_naive()
                .signed_duration_since(base.date_naive())
                .num_days();
            base + Duration::weeks((days / 7).max(0))
        }
        ScheduledNoteRecurrence::Monthly => {
            let month_diff =
                (now.year() - base.year()) * 12 + (now.month() as i32 - base.month() as i32);
            let mut candidate = monthly_occurrence(
                base,
                base.year() + month_diff.div_euclid(12),
                ((base.month0() as i32 + month_diff.rem_euclid(12)) as u32) + 1,
            )?;
            if candidate > now {
                let previous = candidate
                    .with_day(1)
                    .ok_or(CompanionScheduledNoteError::Invalid)?
                    .checked_sub_months(Months::new(1))
                    .ok_or(CompanionScheduledNoteError::Invalid)?;
                candidate = monthly_occurrence(base, previous.year(), previous.month())?;
            }
            candidate
        }
        ScheduledNoteRecurrence::Yearly => {
            let mut candidate = yearly_occurrence(base, now.year())?;
            if candidate > now {
                candidate = yearly_occurrence(base, now.year() - 1)?;
            }
            candidate
        }
    };
    Ok(local_datetime_to_ms(occurrence))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(available_at: i64) -> CompanionScheduledNote {
        CompanionScheduledNote {
            id: Uuid::new_v4(),
            character_id: CharacterId::new(),
            label: " reminder ".into(),
            content: " remember this ".into(),
            available_at: TimestampMillis::new(available_at),
            expires_at: None,
            recurrence: ScheduledNoteRecurrence::None,
            recurrence_window_ms: None,
            enabled: true,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        }
    }

    #[test]
    fn activation_and_rendering_copy_legacy_boundaries() {
        let mut one_time = note(1_000);
        assert!(!is_scheduled_note_active(&one_time, TimestampMillis::new(999)).expect("before"));
        assert!(is_scheduled_note_active(&one_time, TimestampMillis::new(1_000)).expect("start"));
        one_time.expires_at = Some(TimestampMillis::new(2_000));
        assert!(!is_scheduled_note_active(&one_time, TimestampMillis::new(2_000)).expect("expiry"));

        let normalized = one_time.normalize().expect("normalize note");
        assert_eq!(normalized.label, "reminder");
        assert_eq!(normalized.content, "remember this");
        assert_eq!(
            render_scheduled_notes_block(&[normalized]),
            Some("[Background context you currently hold in mind]\n- remember this".into())
        );
    }

    #[test]
    fn daily_and_weekly_windows_close_at_the_exact_end() {
        let base = Local
            .with_ymd_and_hms(2026, 1, 5, 12, 0, 0)
            .single()
            .expect("local date")
            .timestamp_millis();
        for (recurrence, offset) in [
            (ScheduledNoteRecurrence::Daily, Duration::days(2)),
            (ScheduledNoteRecurrence::Weekly, Duration::weeks(2)),
        ] {
            let mut scheduled = note(base);
            scheduled.recurrence = recurrence;
            scheduled.recurrence_window_ms = Some(60_000);
            let occurrence = base + offset.num_milliseconds();
            assert!(
                is_scheduled_note_active(&scheduled, TimestampMillis::new(occurrence + 59_999))
                    .expect("inside recurrence window")
            );
            assert!(
                !is_scheduled_note_active(&scheduled, TimestampMillis::new(occurrence + 60_000))
                    .expect("at recurrence window end")
            );
        }
    }

    #[test]
    fn calendar_recurrence_matches_legacy_month_year_and_next_week() {
        let millis = |year, month, day, hour| {
            TimestampMillis::new(
                Local
                    .with_ymd_and_hms(year, month, day, hour, 0, 0)
                    .single()
                    .expect("local date")
                    .timestamp_millis(),
            )
        };
        assert_eq!(
            most_recent_scheduled_note_occurrence(
                millis(2026, 1, 31, 8),
                ScheduledNoteRecurrence::Monthly,
                millis(2026, 2, 28, 12),
            )
            .expect("monthly occurrence"),
            millis(2026, 2, 28, 8)
        );
        assert_eq!(
            most_recent_scheduled_note_occurrence(
                millis(2024, 2, 29, 8),
                ScheduledNoteRecurrence::Yearly,
                millis(2025, 2, 28, 12),
            )
            .expect("yearly occurrence"),
            millis(2025, 2, 28, 8)
        );
        assert_eq!(
            next_scheduled_note_occurrence(
                millis(2026, 5, 1, 9),
                ScheduledNoteRecurrence::Weekly,
                millis(2026, 5, 1, 9),
            )
            .expect("next weekly occurrence"),
            Some(millis(2026, 5, 8, 9))
        );
    }
}
