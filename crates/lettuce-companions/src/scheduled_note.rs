use chrono::{DateTime, Datelike, Duration, Local, LocalResult, NaiveDate, TimeZone, Timelike};
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
    #[error("the character of the scheduled note was not found")]
    CharacterNotFound,
    #[error("scheduled notes are only available for companion-mode characters")]
    NotCompanion,
    #[error("companion scheduled note conflicts with durable state")]
    Conflict,
    #[error("companion scheduled note is invalid: {reason}")]
    Invalid { reason: &'static str },
    #[error("companion scheduled note storage failed: {reason}")]
    Failure { reason: String },
    #[error("companion scheduled note storage is corrupt: {reason}")]
    Corrupt { reason: String },
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
        if self.content.is_empty() {
            return Err(CompanionScheduledNoteError::Invalid {
                reason: "note content is empty",
            });
        }
        if self.created_at.get() < 0 {
            return Err(CompanionScheduledNoteError::Invalid {
                reason: "creation time is negative",
            });
        }
        if self.updated_at.get() < 0 {
            return Err(CompanionScheduledNoteError::Invalid {
                reason: "update time is negative",
            });
        }
        if self
            .expires_at
            .is_some_and(|expires| expires <= self.available_at)
        {
            return Err(CompanionScheduledNoteError::Invalid {
                reason: "expiry is not after the available time",
            });
        }
        Ok(())
    }
}

pub fn active_scheduled_notes(
    notes: Vec<CompanionScheduledNote>,
    now: TimestampMillis,
) -> Result<Vec<CompanionScheduledNote>, CompanionScheduledNoteError> {
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

/// Note lines kept under the block's character budget. `line` renders one
/// note and `truncated` ends a note cut at its content cap; the application
/// supplies both from the prompt catalog.
pub fn scheduled_note_lines(
    notes: &[CompanionScheduledNote],
    truncated: &str,
    line: &dyn Fn(&str) -> String,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut total_chars = 0usize;
    for note in notes {
        let trimmed = note.content.trim();
        if trimmed.is_empty() {
            continue;
        }
        let capped = if trimmed.chars().count() > MAX_NOTE_CONTENT_CHARS {
            let shortened: String = trimmed.chars().take(MAX_NOTE_CONTENT_CHARS).collect();
            format!("{}{truncated}", shortened.trim_end())
        } else {
            trimmed.to_owned()
        };
        let line = line(&capped);
        if total_chars + line.len() > MAX_BLOCK_CHARS {
            break;
        }
        total_chars += line.len() + 1;
        lines.push(line);
    }
    lines
}

fn timestamp_ms_to_local(
    timestamp: TimestampMillis,
) -> Result<DateTime<Local>, CompanionScheduledNoteError> {
    match Local.timestamp_millis_opt(timestamp.get()) {
        LocalResult::Single(value) => Ok(value),
        LocalResult::Ambiguous(earliest, _) => Ok(earliest),
        LocalResult::None => Err(CompanionScheduledNoteError::Invalid {
            reason: "timestamp does not exist in the local time zone",
        }),
    }
}

fn local_datetime_to_ms(datetime: DateTime<Local>) -> TimestampMillis {
    TimestampMillis::new(datetime.timestamp_millis())
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
        NaiveDate::from_ymd_opt(year, month, day).ok_or(CompanionScheduledNoteError::Invalid {
            reason: "calendar date does not exist",
        })?;
    let naive = date.and_hms_milli_opt(hour, minute, second, millis).ok_or(
        CompanionScheduledNoteError::Invalid {
            reason: "time of day does not exist",
        },
    )?;
    match Local.from_local_datetime(&naive) {
        LocalResult::Single(value) => Ok(value),
        LocalResult::Ambiguous(earliest, _) => Ok(earliest),
        LocalResult::None => match Local.from_local_datetime(&(naive + Duration::hours(1))) {
            LocalResult::Single(value) => Ok(value),
            LocalResult::Ambiguous(earliest, _) => Ok(earliest),
            LocalResult::None => Err(CompanionScheduledNoteError::Invalid {
                reason: "local time does not exist after the daylight saving shift",
            }),
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

fn shift_month(
    year: i32,
    month: u32,
    delta: i32,
) -> Result<(i32, u32), CompanionScheduledNoteError> {
    let total = year
        .checked_mul(12)
        .and_then(|months| months.checked_add(month as i32 - 1 + delta))
        .ok_or(CompanionScheduledNoteError::Invalid {
            reason: "month shift overflows",
        })?;
    Ok((total.div_euclid(12), total.rem_euclid(12) as u32 + 1))
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
            let (year, month) = shift_month(occurrence.year(), occurrence.month(), 1)?;
            monthly_occurrence(base, year, month)?
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
            let candidate = base + Duration::days(days);
            if candidate > now {
                candidate - Duration::days(1)
            } else {
                candidate
            }
        }
        ScheduledNoteRecurrence::Weekly => {
            let days = now
                .date_naive()
                .signed_duration_since(base.date_naive())
                .num_days();
            let candidate = base + Duration::weeks((days / 7).max(0));
            if candidate > now {
                candidate - Duration::weeks(1)
            } else {
                candidate
            }
        }
        ScheduledNoteRecurrence::Monthly => {
            let mut candidate = monthly_occurrence(base, now.year(), now.month())?;
            if candidate > now {
                let (year, month) = shift_month(now.year(), now.month(), -1)?;
                candidate = monthly_occurrence(base, year, month)?;
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
        let line = |content: &str| format!("- {content}");
        assert_eq!(
            scheduled_note_lines(std::slice::from_ref(&normalized), "...", &line),
            vec!["- remember this".to_owned()]
        );
        let mut long = normalized.clone();
        long.content = format!("{} tail", "a".repeat(999));
        let mut blank = normalized.clone();
        blank.content = "   ".into();
        assert_eq!(
            scheduled_note_lines(&[blank, long], "...", &line),
            vec![format!("- {}...", "a".repeat(999))]
        );
        let mut filler = normalized;
        filler.content = "b".repeat(1000);
        let lines = scheduled_note_lines(
            &[
                filler.clone(),
                filler.clone(),
                filler.clone(),
                filler.clone(),
            ],
            "...",
            &line,
        );
        assert_eq!(lines.len(), 3);
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

    fn local(year: i32, month: u32, day: u32, hour: u32) -> TimestampMillis {
        TimestampMillis::new(
            Local
                .with_ymd_and_hms(year, month, day, hour, 0, 0)
                .earliest()
                .expect("local date")
                .timestamp_millis(),
        )
    }

    #[test]
    fn pre_1970_notes_validate_and_keep_their_exact_instants() {
        let mut scheduled = note(local(1890, 3, 14, 10).get());
        assert!(scheduled.available_at.get() < 0);
        scheduled.expires_at = Some(local(1890, 6, 1, 10));
        let normalized = scheduled.clone().normalize().expect("1890 note");
        assert_eq!(normalized.available_at, scheduled.available_at);
        assert_eq!(normalized.expires_at, scheduled.expires_at);
        assert!(is_scheduled_note_active(&normalized, local(1890, 4, 1, 12)).expect("inside"));
        assert!(!is_scheduled_note_active(&normalized, local(1890, 6, 1, 10)).expect("expired"));
        assert!(!is_scheduled_note_active(&normalized, local(1890, 3, 14, 9)).expect("before"));
        assert_eq!(
            active_scheduled_notes(vec![normalized], local(1890, 4, 1, 12))
                .expect("active notes")
                .len(),
            1
        );
        for expires in [scheduled.available_at, local(1890, 3, 1, 10)] {
            let mut backwards = scheduled.clone();
            backwards.expires_at = Some(expires);
            assert!(matches!(
                backwards.validate(),
                Err(CompanionScheduledNoteError::Invalid { .. })
            ));
        }
    }

    #[test]
    fn recurrence_works_before_and_across_the_epoch() {
        for (now_year, now_month, now_day) in [(1890, 12, 31), (1975, 12, 31)] {
            let now = local(now_year, now_month, now_day, 12);
            assert_eq!(
                most_recent_scheduled_note_occurrence(
                    local(1889, 12, 31, 9),
                    ScheduledNoteRecurrence::Yearly,
                    now,
                )
                .expect("yearly occurrence"),
                local(now_year, 12, 31, 9)
            );
            assert_eq!(
                most_recent_scheduled_note_occurrence(
                    local(1889, 1, 31, 9),
                    ScheduledNoteRecurrence::Monthly,
                    local(now_year, 2, 28, 12),
                )
                .expect("monthly occurrence"),
                local(now_year, 2, 28, 9)
            );
        }
        const DAY: i64 = 86_400_000;
        for (base, now, recurrence, expected_days) in [
            (
                local(1889, 12, 30, 9),
                local(1890, 1, 5, 12),
                ScheduledNoteRecurrence::Daily,
                6,
            ),
            (
                local(1889, 12, 30, 9),
                local(1890, 1, 15, 12),
                ScheduledNoteRecurrence::Weekly,
                14,
            ),
            (
                local(1969, 12, 25, 9),
                local(1970, 1, 2, 12),
                ScheduledNoteRecurrence::Daily,
                8,
            ),
            (
                local(1969, 12, 25, 9),
                local(1970, 1, 2, 12),
                ScheduledNoteRecurrence::Weekly,
                7,
            ),
        ] {
            assert_eq!(
                most_recent_scheduled_note_occurrence(base, recurrence, now)
                    .expect("daily or weekly occurrence"),
                TimestampMillis::new(base.get() + expected_days * DAY)
            );
        }
        assert_eq!(
            most_recent_scheduled_note_occurrence(
                local(1969, 11, 30, 9),
                ScheduledNoteRecurrence::Monthly,
                local(1970, 2, 28, 12),
            )
            .expect("monthly occurrence across the epoch"),
            local(1970, 2, 28, 9)
        );
        assert_eq!(
            most_recent_scheduled_note_occurrence(
                local(2025, 11, 30, 9),
                ScheduledNoteRecurrence::Monthly,
                local(2026, 2, 28, 12),
            )
            .expect("monthly occurrence across a year end"),
            local(2026, 2, 28, 9)
        );
        let mut yearly = note(local(1889, 12, 31, 9).get());
        yearly.recurrence = ScheduledNoteRecurrence::Yearly;
        yearly.recurrence_window_ms = Some(86_400_000);
        assert!(is_scheduled_note_active(&yearly, local(1890, 12, 31, 12)).expect("anniversary"));
        assert!(!is_scheduled_note_active(&yearly, local(1890, 12, 30, 12)).expect("day before"));
        assert!(is_scheduled_note_active(&yearly, local(1975, 12, 31, 12)).expect("after 1970"));
        assert_eq!(
            next_scheduled_note_occurrence(
                yearly.available_at,
                ScheduledNoteRecurrence::Yearly,
                local(1969, 12, 31, 9),
            )
            .expect("next yearly occurrence"),
            Some(local(1970, 12, 31, 9))
        );
    }

    #[test]
    fn daily_and_weekly_occurrences_never_lie_after_now() {
        const DAY: i64 = 86_400_000;
        for (base, now, recurrence, expected_days) in [
            (
                local(2026, 1, 5, 9),
                local(2026, 1, 7, 8),
                ScheduledNoteRecurrence::Daily,
                1,
            ),
            (
                local(2026, 1, 5, 9),
                local(2026, 1, 19, 8),
                ScheduledNoteRecurrence::Weekly,
                7,
            ),
            (
                local(1889, 12, 30, 9),
                local(1890, 1, 1, 8),
                ScheduledNoteRecurrence::Daily,
                1,
            ),
            (
                local(1889, 12, 30, 9),
                local(1890, 1, 13, 8),
                ScheduledNoteRecurrence::Weekly,
                7,
            ),
        ] {
            let occurrence = most_recent_scheduled_note_occurrence(base, recurrence, now)
                .expect("daily or weekly occurrence");
            assert!(occurrence <= now);
            assert_eq!(
                occurrence,
                TimestampMillis::new(base.get() + expected_days * DAY)
            );
            let mut scheduled = note(base.get());
            scheduled.recurrence = recurrence;
            scheduled.recurrence_window_ms = Some(60_000);
            assert!(!is_scheduled_note_active(&scheduled, now).expect("before the window"));
        }
    }

    #[test]
    fn monthly_step_back_crosses_a_year_end() {
        assert_eq!(
            most_recent_scheduled_note_occurrence(
                local(2026, 1, 31, 9),
                ScheduledNoteRecurrence::Monthly,
                local(2026, 3, 1, 8),
            )
            .expect("monthly step back"),
            local(2026, 2, 28, 9)
        );
        assert_eq!(
            most_recent_scheduled_note_occurrence(
                local(2025, 12, 15, 9),
                ScheduledNoteRecurrence::Monthly,
                local(2026, 1, 10, 8),
            )
            .expect("monthly step back into December"),
            local(2025, 12, 15, 9)
        );
        assert_eq!(
            next_scheduled_note_occurrence(
                local(1889, 12, 15, 9),
                ScheduledNoteRecurrence::Monthly,
                local(1889, 12, 15, 9),
            )
            .expect("next monthly occurrence"),
            Some(local(1890, 1, 15, 9))
        );
        assert_eq!(shift_month(1890, 1, -1), Ok((1889, 12)));
        assert_eq!(shift_month(1969, 12, 1), Ok((1970, 1)));
    }
}
