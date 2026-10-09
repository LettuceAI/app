use std::collections::BTreeMap;

use crate::{UsageReportError, UsageReportRow, UsageReportStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UsageReportFilters {
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub provider_kind: Option<String>,
    pub model: Option<String>,
    pub character: Option<String>,
    pub operation: Option<String>,
    pub status: Option<UsageReportStatus>,
}

impl UsageReportFilters {
    pub fn validate(&self) -> Result<(), UsageReportError> {
        if matches!((self.start, self.end), (Some(start), Some(end)) if start > end) {
            return Err(UsageReportError::InvalidRange);
        }
        Ok(())
    }

    pub fn matches(&self, row: &UsageReportRow) -> bool {
        let matches = |filter: &Option<String>, value: &Option<String>| {
            filter
                .as_ref()
                .is_none_or(|filter| value.as_ref() == Some(filter))
        };
        self.start.is_none_or(|start| row.timestamp >= start)
            && self.end.is_none_or(|end| row.timestamp <= end)
            && matches(&self.provider_kind, &row.provider_kind)
            && matches(&self.model, &row.model_id)
            && matches(&self.character, &row.character_id)
            && matches(&self.operation, &row.operation_type)
            && self.status.is_none_or(|status| row.status == status)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageReportSort {
    #[default]
    NewestFirst,
    OldestFirst,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageReportPage {
    pub items: Vec<UsageReportRow>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u32,
    filters: UsageReportFilters,
    sort: UsageReportSort,
    timestamp: i64,
    id: String,
}

pub fn usage_report_page(
    rows: &[UsageReportRow],
    filters: &UsageReportFilters,
    sort: UsageReportSort,
    cursor: Option<&str>,
    limit: u32,
) -> Result<UsageReportPage, UsageReportError> {
    filters.validate()?;
    let cursor = cursor
        .map(|bytes| {
            serde_json::from_str::<Cursor>(bytes).map_err(|_| UsageReportError::InvalidCursor)
        })
        .transpose()?;
    if cursor.as_ref().is_some_and(|cursor| {
        cursor.version != 1 || &cursor.filters != filters || cursor.sort != sort
    }) {
        return Err(UsageReportError::InvalidCursor);
    }
    let limit = limit.clamp(1, 2000) as usize;
    let mut rows = rows
        .iter()
        .filter(|row| filters.matches(row))
        .filter(|row| {
            cursor.as_ref().is_none_or(|cursor| {
                let position =
                    (row.timestamp, row.id.as_str()).cmp(&(cursor.timestamp, cursor.id.as_str()));
                match sort {
                    UsageReportSort::NewestFirst => position.is_lt(),
                    UsageReportSort::OldestFirst => position.is_gt(),
                }
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        let order = (a.timestamp, &a.id).cmp(&(b.timestamp, &b.id));
        match sort {
            UsageReportSort::NewestFirst => order.reverse(),
            UsageReportSort::OldestFirst => order,
        }
    });
    let more = rows.len() > limit;
    rows.truncate(limit);
    let next_cursor = if more {
        rows.last()
            .map(|row| {
                serde_json::to_string(&Cursor {
                    version: 1,
                    filters: filters.clone(),
                    sort,
                    timestamp: row.timestamp,
                    id: row.id.clone(),
                })
                .map_err(|_| UsageReportError::InvalidData)
            })
            .transpose()?
    } else {
        None
    };
    Ok(UsageReportPage {
        items: rows.into_iter().cloned().collect(),
        next_cursor,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageReportGroup {
    Day,
    Model,
    Provider,
    Character,
    Operation,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageReportTotals {
    pub requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub cancelled_requests: u64,
    pub interrupted_requests: u64,
    pub pending_requests: u64,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub total_cost: Option<f64>,
    pub unknown_token_requests: u64,
    pub unknown_cost_requests: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageReportGroupTotals {
    pub key: Option<String>,
    pub label: Option<String>,
    pub totals: UsageReportTotals,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageReportStats {
    pub totals: UsageReportTotals,
    pub groups: Vec<UsageReportGroupTotals>,
}

impl UsageReportTotals {
    fn add(&mut self, row: &UsageReportRow) -> Result<(), UsageReportError> {
        row.validate()?;
        let increment = |count: &mut u64| -> Result<(), UsageReportError> {
            *count = count.checked_add(1).ok_or(UsageReportError::Overflow)?;
            Ok(())
        };
        increment(&mut self.requests)?;
        increment(match row.status {
            UsageReportStatus::Succeeded => &mut self.successful_requests,
            UsageReportStatus::Failed => &mut self.failed_requests,
            UsageReportStatus::Cancelled => &mut self.cancelled_requests,
            UsageReportStatus::Interrupted => &mut self.interrupted_requests,
            UsageReportStatus::Pending => &mut self.pending_requests,
        })?;
        let sum = |total: &mut Option<u64>, value: Option<u64>| -> Result<(), UsageReportError> {
            if let Some(value) = value {
                *total = Some(
                    total
                        .unwrap_or(0)
                        .checked_add(value)
                        .ok_or(UsageReportError::Overflow)?,
                );
            }
            Ok(())
        };
        sum(&mut self.prompt_tokens, row.prompt_tokens)?;
        sum(&mut self.completion_tokens, row.completion_tokens)?;
        let tokens = match (row.total_tokens, row.prompt_tokens, row.completion_tokens) {
            (Some(total), _, _) => Some(total),
            (None, Some(input), Some(output)) => Some(
                input
                    .checked_add(output)
                    .ok_or(UsageReportError::Overflow)?,
            ),
            _ => None,
        };
        sum(&mut self.total_tokens, tokens)?;
        if tokens.is_none() {
            increment(&mut self.unknown_token_requests)?;
        }
        if let Some(cost) = row.total_cost {
            let total = self.total_cost.unwrap_or(0.0) + cost;
            if !total.is_finite() {
                return Err(UsageReportError::Overflow);
            }
            self.total_cost = Some(total);
        } else {
            increment(&mut self.unknown_cost_requests)?;
        }
        Ok(())
    }
}

pub fn usage_report_stats(
    rows: &[UsageReportRow],
    filters: &UsageReportFilters,
    group_by: UsageReportGroup,
    time_zone: &str,
) -> Result<UsageReportStats, UsageReportError> {
    filters.validate()?;
    let zone = jiff::tz::TimeZone::get(time_zone).map_err(|_| UsageReportError::InvalidTimeZone)?;
    let mut totals = UsageReportTotals::default();
    let mut groups = BTreeMap::<Option<String>, UsageReportGroupTotals>::new();
    for row in rows.iter().filter(|row| filters.matches(row)) {
        totals.add(row)?;
        let (key, label) = match group_by {
            UsageReportGroup::Day => {
                let day = jiff::Timestamp::from_millisecond(row.timestamp)
                    .map_err(|_| UsageReportError::InvalidData)?
                    .to_zoned(zone.clone())
                    .date()
                    .to_string();
                (Some(day.clone()), Some(day))
            }
            UsageReportGroup::Model => (row.model_id.clone(), row.model_name.clone()),
            UsageReportGroup::Provider => (row.provider_kind.clone(), row.provider_kind.clone()),
            UsageReportGroup::Character => (row.character_id.clone(), row.character_name.clone()),
            UsageReportGroup::Operation => (row.operation_type.clone(), row.operation_type.clone()),
        };
        let group = groups
            .entry(key.clone())
            .or_insert_with(|| UsageReportGroupTotals {
                key,
                label: label.clone(),
                totals: UsageReportTotals::default(),
            });
        if group.label.is_none() {
            group.label = label;
        }
        group.totals.add(row)?;
    }
    Ok(UsageReportStats {
        totals,
        groups: groups.into_values().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{UsageReportRow, UsageReportStatus};

    #[test]
    fn query_pages_tied_times_binds_the_cursor_and_uses_inclusive_legacy_dates() {
        let rows = (0..5)
            .map(|id| UsageReportRow {
                id: id.to_string(),
                timestamp: 10,
                status: UsageReportStatus::Succeeded,
                ..UsageReportRow::default()
            })
            .collect::<Vec<_>>();
        let filters = UsageReportFilters {
            start: Some(10),
            end: Some(10),
            ..UsageReportFilters::default()
        };
        let page = usage_report_page(&rows, &filters, UsageReportSort::NewestFirst, None, 2)
            .expect("page");
        assert_eq!(
            page.items
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["4", "3"]
        );
        let next = usage_report_page(
            &rows,
            &filters,
            UsageReportSort::NewestFirst,
            page.next_cursor.as_deref(),
            2,
        )
        .expect("next page");
        assert_eq!(
            next.items
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["2", "1"]
        );
        assert!(
            usage_report_page(
                &rows,
                &UsageReportFilters::default(),
                UsageReportSort::NewestFirst,
                page.next_cursor.as_deref(),
                2
            )
            .is_err()
        );
        assert!(
            usage_report_page(
                &rows,
                &filters,
                UsageReportSort::OldestFirst,
                page.next_cursor.as_deref(),
                2
            )
            .is_err()
        );
        assert!(
            usage_report_page(
                &rows,
                &filters,
                UsageReportSort::NewestFirst,
                Some("garbage"),
                2
            )
            .is_err()
        );
    }

    #[test]
    fn local_day_groups_follow_dst_instead_of_a_fixed_offset() {
        let times = [
            "2026-03-28T23:30:00Z",
            "2026-03-29T22:30:00Z",
            "2026-10-24T22:30:00Z",
            "2026-10-25T22:30:00Z",
        ];
        let rows = times
            .iter()
            .enumerate()
            .map(|(id, time)| UsageReportRow {
                id: id.to_string(),
                timestamp: time
                    .parse::<jiff::Timestamp>()
                    .expect("timestamp")
                    .as_millisecond(),
                status: UsageReportStatus::Succeeded,
                prompt_tokens: Some(10),
                completion_tokens: Some(5),
                ..UsageReportRow::default()
            })
            .collect::<Vec<_>>();
        let stats = usage_report_stats(
            &rows,
            &UsageReportFilters::default(),
            UsageReportGroup::Day,
            "Europe/Berlin",
        )
        .expect("local stats");
        assert_eq!(
            stats
                .groups
                .iter()
                .map(|group| group.key.as_deref())
                .collect::<Vec<_>>(),
            [Some("2026-03-29"), Some("2026-03-30"), Some("2026-10-25")]
        );
        assert_eq!(stats.groups[2].totals.requests, 2);
        assert_eq!(stats.totals.requests, 4);
        assert_eq!(stats.totals.prompt_tokens, Some(40));
        assert_eq!(stats.totals.total_cost, None);
        assert_eq!(stats.totals.unknown_cost_requests, 4);
        assert!(
            usage_report_stats(
                &rows,
                &UsageReportFilters::default(),
                UsageReportGroup::Day,
                "invalid/timezone"
            )
            .is_err()
        );
    }
}
