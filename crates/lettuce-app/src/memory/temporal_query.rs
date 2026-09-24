use std::sync::OnceLock;

use chrono::{
    DateTime, Datelike, Duration, Local, LocalResult, NaiveDate, NaiveDateTime, NaiveTime,
    TimeZone, Utc,
};
use lettuce_types::TimestampMillis;
use regex::Regex;

/// A half-open local-calendar window a memory query asks about, legacy
/// `TemporalRange`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TemporalRange {
    pub(crate) start: TimestampMillis,
    pub(crate) end: TimestampMillis,
}

impl TemporalRange {
    pub(crate) fn contains(self, at: TimestampMillis) -> bool {
        at >= self.start && at < self.end
    }
}

fn local_datetime(millis: i64) -> DateTime<Local> {
    match Local.timestamp_millis_opt(millis) {
        LocalResult::Single(datetime) | LocalResult::Ambiguous(datetime, _) => datetime,
        LocalResult::None => Local::now(),
    }
}

fn local_midnight(date: NaiveDate) -> DateTime<Local> {
    let naive = date
        .and_hms_opt(0, 0, 0)
        .unwrap_or_else(|| NaiveDateTime::new(date, NaiveTime::MIN));
    match Local.from_local_datetime(&naive) {
        LocalResult::Single(datetime) | LocalResult::Ambiguous(datetime, _) => datetime,
        LocalResult::None => local_datetime(Utc::now().timestamp_millis().max(0)),
    }
}

fn range_from_local_dates(start: NaiveDate, end: NaiveDate) -> TemporalRange {
    TemporalRange {
        start: TimestampMillis::new(local_midnight(start).timestamp_millis().max(0)),
        end: TimestampMillis::new(local_midnight(end).timestamp_millis().max(0)),
    }
}

fn rolling_range(now: DateTime<Local>, days: i64) -> Option<TemporalRange> {
    let start = now.checked_sub_signed(Duration::try_days(days)?)?;
    Some(TemporalRange {
        start: TimestampMillis::new(start.timestamp_millis().max(0)),
        end: TimestampMillis::new(now.timestamp_millis().max(0)),
    })
}

fn add_days(date: NaiveDate, days: i64) -> Option<NaiveDate> {
    date.checked_add_signed(Duration::try_days(days)?)
}

fn sub_days(date: NaiveDate, days: i64) -> Option<NaiveDate> {
    date.checked_sub_signed(Duration::try_days(days)?)
}

fn day_range(date: NaiveDate) -> Option<TemporalRange> {
    Some(range_from_local_dates(date, add_days(date, 1)?))
}

fn months_before(today: NaiveDate, amount: i64) -> Option<(i32, u32)> {
    let total = i64::from(today.year())
        .checked_mul(12)?
        .checked_add(i64::from(today.month0()))?
        .checked_sub(amount)?;
    Some((
        i32::try_from(total.div_euclid(12)).ok()?,
        u32::try_from(total.rem_euclid(12)).ok()? + 1,
    ))
}

fn normalized_query(query: &str) -> String {
    query
        .chars()
        .map(|ch| if ch.is_ascii_punctuation() { ' ' } else { ch })
        .collect::<String>()
        .to_ascii_lowercase()
}

fn number_range_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"\b(?P<num>\d+|one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve)\s+(?P<unit>day|days|week|weeks|month|months|year|years)\s+ago(?:\s+(?P<anchor>today|tonight))?\b",
        )
        .expect("valid ago regex")
    })
}

fn past_range_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"\b(?:past|last|previous|within the last|in the last)\s+(?P<num>\d+|one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve)\s+(?P<unit>day|days|week|weeks|month|months|year|years)\b",
        )
        .expect("valid past-range regex")
    })
}

fn weekday_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"\b(?P<qualifier>last|this)\s+(?P<weekday>monday|tuesday|wednesday|thursday|friday|saturday|sunday)\b",
        )
        .expect("valid weekday regex")
    })
}

fn weekday_ago_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"\b(?P<num>\d+|one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve)\s+(?P<weekday>monday|tuesday|wednesday|thursday|friday|saturday|sunday)s?\s+ago\b",
        )
        .expect("valid weekday ago regex")
    })
}

fn parse_count(raw: &str) -> Option<i64> {
    match raw {
        "one" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "five" => Some(5),
        "six" => Some(6),
        "seven" => Some(7),
        "eight" => Some(8),
        "nine" => Some(9),
        "ten" => Some(10),
        "eleven" => Some(11),
        "twelve" => Some(12),
        _ => raw.parse::<i64>().ok(),
    }
}

fn weekday_number(name: &str) -> Option<u32> {
    match name {
        "monday" => Some(0),
        "tuesday" => Some(1),
        "wednesday" => Some(2),
        "thursday" => Some(3),
        "friday" => Some(4),
        "saturday" => Some(5),
        "sunday" => Some(6),
        _ => None,
    }
}

fn resolve_relative_weekday(
    today: NaiveDate,
    current_weekday_num: u32,
    qualifier: &str,
    weekday_name: &str,
) -> Option<NaiveDate> {
    let target = i64::from(weekday_number(weekday_name)?);
    let current = i64::from(current_weekday_num);
    match qualifier {
        "this" => add_days(today, target - current),
        "last" => {
            let backward = (current - target).rem_euclid(7);
            sub_days(today, if backward == 0 { 7 } else { backward })
        }
        _ => None,
    }
}

fn nth_prior_weekday(
    today: NaiveDate,
    current_weekday_num: u32,
    weekday_name: &str,
    count: i64,
) -> Option<NaiveDate> {
    let target = i64::from(weekday_number(weekday_name)?);
    let current = i64::from(current_weekday_num);
    let backward = (current - target).rem_euclid(7);
    let first = if backward == 0 { 7 } else { backward };
    sub_days(
        today,
        (count - 1).max(0).checked_mul(7)?.checked_add(first)?,
    )
}

fn days_in_month(year: i32, month: u32) -> Option<u32> {
    let (next_year, next_month) = if month == 12 {
        (year.checked_add(1)?, 1)
    } else {
        (year, month + 1)
    };
    Some(sub_days(NaiveDate::from_ymd_opt(next_year, next_month, 1)?, 1)?.day())
}

/// Legacy `detect_temporal_query_range`: the first calendar phrase of a memory
/// query, resolved against the reference time in the local timezone.
pub(crate) fn detect_temporal_query_range(
    query: &str,
    reference: TimestampMillis,
) -> Option<TemporalRange> {
    let normalized = normalized_query(query);
    let now = local_datetime(reference.get());
    let today = now.date_naive();
    let start_of_week = || sub_days(today, i64::from(today.weekday().num_days_from_monday()));

    if normalized.contains("yesterday") {
        return Some(range_from_local_dates(sub_days(today, 1)?, today));
    }
    if normalized.contains("today") || normalized.contains("tonight") {
        return day_range(today);
    }
    if normalized.contains("last week") {
        let start_of_week = start_of_week()?;
        return Some(range_from_local_dates(
            sub_days(start_of_week, 7)?,
            start_of_week,
        ));
    }
    if normalized.contains("this week") || normalized.contains("earlier this week") {
        let start_of_week = start_of_week()?;
        return Some(range_from_local_dates(
            start_of_week,
            add_days(start_of_week, 7)?,
        ));
    }
    if normalized.contains("last month") {
        let start_of_this_month = today.with_day(1)?;
        let start_of_last_month = sub_days(start_of_this_month, 1)?.with_day(1)?;
        return Some(range_from_local_dates(
            start_of_last_month,
            start_of_this_month,
        ));
    }
    if normalized.contains("this month") || normalized.contains("earlier this month") {
        let start_of_this_month = today.with_day(1)?;
        let start_of_next_month = if start_of_this_month.month() == 12 {
            NaiveDate::from_ymd_opt(start_of_this_month.year() + 1, 1, 1)?
        } else {
            NaiveDate::from_ymd_opt(
                start_of_this_month.year(),
                start_of_this_month.month() + 1,
                1,
            )?
        };
        return Some(range_from_local_dates(
            start_of_this_month,
            start_of_next_month,
        ));
    }
    if normalized.contains("last year") {
        return Some(range_from_local_dates(
            NaiveDate::from_ymd_opt(today.year() - 1, 1, 1)?,
            NaiveDate::from_ymd_opt(today.year(), 1, 1)?,
        ));
    }
    if normalized.contains("this year") || normalized.contains("earlier this year") {
        return Some(range_from_local_dates(
            NaiveDate::from_ymd_opt(today.year(), 1, 1)?,
            NaiveDate::from_ymd_opt(today.year() + 1, 1, 1)?,
        ));
    }

    if let Some(captures) = weekday_regex().captures(&normalized) {
        let qualifier = captures.name("qualifier")?.as_str();
        let weekday = captures.name("weekday")?.as_str();
        return day_range(resolve_relative_weekday(
            today,
            today.weekday().num_days_from_monday(),
            qualifier,
            weekday,
        )?);
    }

    if let Some(captures) = weekday_ago_regex().captures(&normalized) {
        let amount = parse_count(captures.name("num")?.as_str())?;
        let weekday = captures.name("weekday")?.as_str();
        return day_range(nth_prior_weekday(
            today,
            today.weekday().num_days_from_monday(),
            weekday,
            amount,
        )?);
    }

    if let Some(captures) = number_range_regex().captures(&normalized) {
        let amount = parse_count(captures.name("num")?.as_str())?;
        let unit = captures.name("unit")?.as_str();
        let anchored = captures.name("anchor").is_some();
        return match unit {
            "day" | "days" if anchored => day_range(sub_days(today, amount)?),
            "day" | "days" => rolling_range(now, amount),
            "week" | "weeks" if anchored => day_range(sub_days(today, amount.checked_mul(7)?)?),
            "week" | "weeks" => {
                let start = sub_days(start_of_week()?, amount.checked_mul(7)?)?;
                Some(range_from_local_dates(start, add_days(start, 7)?))
            }
            "month" | "months" if anchored => {
                let (year, month) = months_before(today, amount)?;
                let day = today.day().min(days_in_month(year, month)?);
                day_range(NaiveDate::from_ymd_opt(year, month, day)?)
            }
            "month" | "months" => {
                let (year, month) = months_before(today, amount)?;
                let start = NaiveDate::from_ymd_opt(year, month, 1)?;
                let end = if month == 12 {
                    NaiveDate::from_ymd_opt(year.checked_add(1)?, 1, 1)?
                } else {
                    NaiveDate::from_ymd_opt(year, month + 1, 1)?
                };
                Some(range_from_local_dates(start, end))
            }
            "year" | "years" if anchored => {
                let target_year = today.year().checked_sub(i32::try_from(amount).ok()?)?;
                let day = today.day().min(days_in_month(target_year, today.month())?);
                day_range(NaiveDate::from_ymd_opt(target_year, today.month(), day)?)
            }
            "year" | "years" => {
                let target_year = today.year().checked_sub(i32::try_from(amount).ok()?)?;
                Some(range_from_local_dates(
                    NaiveDate::from_ymd_opt(target_year, 1, 1)?,
                    NaiveDate::from_ymd_opt(target_year.checked_add(1)?, 1, 1)?,
                ))
            }
            _ => None,
        };
    }

    if let Some(captures) = past_range_regex().captures(&normalized) {
        let amount = parse_count(captures.name("num")?.as_str())?;
        let unit = captures.name("unit")?.as_str();
        return match unit {
            "day" | "days" => rolling_range(now, amount),
            "week" | "weeks" => rolling_range(now, amount.checked_mul(7)?),
            "month" | "months" => rolling_range(now, amount.checked_mul(30)?),
            "year" | "years" => rolling_range(now, amount.checked_mul(365)?),
            _ => None,
        };
    }

    None
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone};
    use lettuce_types::TimestampMillis;

    use super::detect_temporal_query_range;

    fn local_ms(year: i32, month: u32, day: u32, hour: u32) -> TimestampMillis {
        TimestampMillis::new(
            Local
                .with_ymd_and_hms(year, month, day, hour, 0, 0)
                .earliest()
                .expect("valid local datetime")
                .timestamp_millis(),
        )
    }

    #[test]
    fn calendar_phrases_resolve_to_nonempty_ranges_like_legacy() {
        let reference = local_ms(2026, 5, 10, 12);
        for query in [
            "what place did we go to last week",
            "where did we eat 2 days ago",
            "what did we do after coffee last saturday",
            "what did we do two fridays ago",
        ] {
            let range = detect_temporal_query_range(query, reference).expect(query);
            assert!(range.start < range.end, "{query}");
        }
        let anchored = detect_temporal_query_range("what did we do 5 week ago today", reference)
            .expect("anchored range");
        assert!(anchored.end.get() - anchored.start.get() <= 86_400_000);
        assert!(detect_temporal_query_range("tell me a story", reference).is_none());
    }

    #[test]
    fn overflowing_amounts_yield_no_range_instead_of_panicking() {
        let reference = local_ms(2026, 5, 10, 12);
        for query in [
            "what did we do 200000000000 days ago",
            "past 100000000 days",
            "past 99999999999999999 years",
            "99999999999999999 months ago",
            "9999999999 fridays ago",
            "300000 years ago",
            "4000000 months ago",
            "999999999999999999 weeks ago",
        ] {
            assert!(
                detect_temporal_query_range(query, reference).is_none(),
                "{query}"
            );
        }
        let month = detect_temporal_query_range("what did we do 14 months ago", reference)
            .expect("month range");
        assert_eq!(month.start, local_ms(2025, 3, 1, 0));
        assert_eq!(month.end, local_ms(2025, 4, 1, 0));
    }

    #[test]
    fn yesterday_covers_exactly_the_previous_local_day() {
        let reference = local_ms(2026, 5, 10, 12);
        let range =
            detect_temporal_query_range("What happened yesterday?", reference).expect("yesterday");
        assert_eq!(range.start, local_ms(2026, 5, 9, 0));
        assert_eq!(range.end, local_ms(2026, 5, 10, 0));
        assert!(range.contains(local_ms(2026, 5, 9, 23)));
        assert!(!range.contains(local_ms(2026, 5, 10, 0)));
    }
}
