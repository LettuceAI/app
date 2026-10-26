use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};

use chrono::Timelike;
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_observability::{LineMatcher, LogSearchOptions};

use crate::api::{
    ApiContext, FileAccessError,
    error::{IntoApiError, invalid_field},
};

use super::{io_error, log_error};

struct ParsedLine<'a> {
    level: Option<dto::LogLevel>,
    component: Option<&'a str>,
    time: Option<u32>,
}

fn parse(line: &str) -> ParsedLine<'_> {
    let mut parsed = ParsedLine {
        level: None,
        component: None,
        time: None,
    };
    let Some((timestamp, rest)) = line.strip_prefix('[').and_then(|line| line.split_once(']'))
    else {
        return parsed;
    };
    if let Ok(time) = chrono::DateTime::parse_from_rfc3339(timestamp) {
        parsed.time = Some(time.time().num_seconds_from_midnight());
    }
    let mut fields = rest.split_whitespace();
    parsed.level = match fields.next() {
        Some("DEBUG") => Some(dto::LogLevel::Debug),
        Some("INFO") => Some(dto::LogLevel::Info),
        Some("WARN") => Some(dto::LogLevel::Warn),
        Some("ERROR") => Some(dto::LogLevel::Error),
        _ => None,
    };
    parsed.component = fields.next().filter(|component| *component != "|");
    parsed
}

fn boundary(value: Option<&str>, field: &str, end: bool) -> Result<Option<u32>, ApiError> {
    value
        .map(|value| {
            let format = match value.len() {
                5 => "%H:%M",
                8 => "%H:%M:%S",
                _ => return Err(invalid_field(field, "invalid log time")),
            };
            let time = chrono::NaiveTime::parse_from_str(value, format)
                .map_err(|_| invalid_field(field, "invalid log time"))?;
            Ok(time.num_seconds_from_midnight() + u32::from(end && value.len() == 5) * 59)
        })
        .transpose()
}

struct Filter {
    value: dto::LogViewFilter,
    matcher: Option<LineMatcher>,
    isolation: Option<BTreeSet<u64>>,
    from: Option<u32>,
    to: Option<u32>,
}

impl Filter {
    fn new(value: dto::LogViewFilter) -> Result<Self, ApiError> {
        let from = boundary(value.from.as_deref(), "from", false)?;
        let to = boundary(value.to.as_deref(), "to", true)?;
        if from.zip(to).is_some_and(|(from, to)| from > to) {
            return Err(invalid_field("to", "log time range is reversed"));
        }
        let matcher = if value.query.trim().is_empty() {
            None
        } else {
            Some(
                LineMatcher::build(
                    &value.query,
                    LogSearchOptions {
                        case_sensitive: value.case_sensitive,
                        whole_word: value.whole_word,
                        regex: value.regex,
                    },
                )
                .map_err(log_error)?,
            )
        };
        let isolation = value
            .isolation
            .as_ref()
            .map(|indices| indices.iter().copied().collect());
        Ok(Self {
            value,
            matcher,
            isolation,
            from,
            to,
        })
    }

    fn includes(&self, index: u64, parsed: &ParsedLine<'_>) -> bool {
        let included = if let Some(indices) = &self.isolation {
            indices.contains(&index)
        } else {
            parsed
                .level
                .is_none_or(|level| self.value.levels.contains(&level))
                && parsed.component.is_none_or(|component| {
                    !self
                        .value
                        .hidden_components
                        .iter()
                        .any(|hidden| hidden == component)
                })
        };
        included
            && parsed.time.is_none_or(|time| {
                self.from.is_none_or(|from| time >= from) && self.to.is_none_or(|to| time <= to)
            })
    }

    fn matches(&self, line: &str) -> bool {
        self.matcher
            .as_ref()
            .is_some_and(|matcher| matcher.is_match(line))
    }

    fn visible(&self, index: u64, line: &str) -> bool {
        self.includes(index, &parse(line))
            && (!self.value.only_matches || self.matcher.is_none() || self.matches(line))
    }
}

pub async fn log_query(
    context: &ApiContext,
    request: dto::LogQueryRequest,
) -> Result<dto::LogQueryView, ApiError> {
    let filter = Filter::new(request.filter)?;
    context
        .blocking(move |context| {
            let input = context
                .logs()?
                .directory
                .open(&request.name)
                .map_err(log_error)?;
            let mut view = dto::LogQueryView {
                total: 0,
                filtered_total: 0,
                lines: Vec::new(),
                components: Vec::new(),
                counts: dto::LogLevelCounts::default(),
                matches: Vec::new(),
                problems: Vec::new(),
            };
            let mut components = BTreeMap::<String, u64>::new();
            let limit = request.limit.clamp(1, 2000) as usize;
            for line in BufReader::new(input).lines() {
                let line = line.map_err(|error| {
                    log_error(lettuce_observability::LogFileError::ReadLine(error))
                })?;
                let index = view.total;
                view.total += 1;
                let parsed = parse(&line);
                if let Some(component) = parsed.component {
                    *components.entry(component.to_owned()).or_default() += 1;
                }
                match parsed.level {
                    Some(dto::LogLevel::Debug) => view.counts.debug += 1,
                    Some(dto::LogLevel::Info) => view.counts.info += 1,
                    Some(dto::LogLevel::Warn) => view.counts.warn += 1,
                    Some(dto::LogLevel::Error) => view.counts.error += 1,
                    None => {}
                }
                if !filter.includes(index, &parsed) {
                    continue;
                }
                let matched = filter.matches(&line);
                if filter.value.only_matches && filter.matcher.is_some() && !matched {
                    continue;
                }
                let position = view.filtered_total;
                view.filtered_total += 1;
                if matched {
                    view.matches.push(dto::LogPosition { index, position });
                }
                if matches!(
                    parsed.level,
                    Some(dto::LogLevel::Warn | dto::LogLevel::Error)
                ) {
                    view.problems.push(dto::LogPosition { index, position });
                }
                if position >= request.offset && view.lines.len() < limit {
                    view.lines.push(dto::LogLineView {
                        index,
                        position,
                        raw: line,
                    });
                }
            }
            view.components = components
                .into_iter()
                .map(|(component, count)| dto::LogComponentCount { component, count })
                .collect();
            Ok(view)
        })
        .await
}

fn export_error(error: FileAccessError) -> ApiError {
    if error == FileAccessError::SourceIsTarget {
        ApiError {
            code: ApiErrorCode::Conflict,
            message: error.to_string(),
            details: Some(dto::ApiErrorDetails::Logs {
                reason: dto::LogFailureReason::SourceIsTarget,
            }),
        }
    } else {
        let mut error = error.into_api_error();
        error.details = Some(dto::ApiErrorDetails::Logs {
            reason: dto::LogFailureReason::Storage,
        });
        error
    }
}

struct NativeLogStage {
    path: std::path::PathBuf,
    file: Option<std::fs::File>,
}

impl NativeLogStage {
    fn new(directory: &std::path::Path) -> std::io::Result<Self> {
        let path = directory.join(format!(
            ".lettuce-log-export-{}.partial",
            uuid::Uuid::new_v4()
        ));
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path)?;
        Ok(Self {
            path,
            file: Some(file),
        })
    }

    fn cleanup(&mut self) -> std::io::Result<()> {
        self.file.take();
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

impl Drop for NativeLogStage {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

pub async fn log_export_selection(
    context: &ApiContext,
    request: dto::LogExportSelectionRequest,
) -> Result<(), ApiError> {
    let (indices, filter) = match request.selection {
        dto::LogSelection::Indices { indices } => {
            (Some(indices.into_iter().collect::<BTreeSet<_>>()), None)
        }
        dto::LogSelection::Filter { filter } => (None, Some(Filter::new(filter)?)),
    };
    if request.target.uri.trim().is_empty() {
        return Err(invalid_field("target.uri", "export target is empty"));
    }
    context
        .blocking(move |context| {
            let logs = context.logs()?;
            let source = logs.directory.open(&request.name).map_err(log_error)?;
            let protection = crate::api::files::FileExportProtection::new(context)?;
            let protected =
                |uri: &str, file: Option<&std::fs::File>| protection.protects(uri, file);
            let files = context.files();
            let mut stage = NativeLogStage::new(logs.directory.path()).map_err(io_error)?;
            let result = (|| {
                let staged = stage.file.as_mut().expect("open log staging file");
                let mut writer = std::io::BufWriter::new(&mut *staged);
                let mut input = BufReader::new(&source);
                let mut buffer = Vec::new();
                let mut count = 0_u64;
                loop {
                    buffer.clear();
                    if input.read_until(b'\n', &mut buffer).map_err(io_error)? == 0 {
                        break;
                    }
                    let raw = std::str::from_utf8(&buffer).map_err(|_| {
                        super::unavailable(
                            dto::LogFailureReason::InvalidEncoding,
                            "log line is not UTF-8",
                        )
                    })?;
                    let line = raw.strip_suffix('\n').unwrap_or(raw);
                    let line = line.strip_suffix('\r').unwrap_or(line);
                    let include = indices
                        .as_ref()
                        .is_some_and(|indices| indices.contains(&count))
                        || filter
                            .as_ref()
                            .is_some_and(|filter| filter.visible(count, line));
                    if include {
                        writer.write_all(&buffer).map_err(io_error)?;
                    }
                    count += 1;
                }
                if indices
                    .as_ref()
                    .and_then(|indices| indices.last())
                    .is_some_and(|index| *index >= count)
                {
                    return Err(invalid_field("indices", "log line index is out of range"));
                }
                writer.flush().map_err(io_error)?;
                drop(writer);
                staged.sync_all().map_err(io_error)?;
                staged.seek(SeekFrom::Start(0)).map_err(io_error)?;
                let mut output = files
                    .create_export(&request.target.uri, Some(&source), &protected)
                    .map_err(export_error)?;
                std::io::copy(staged, &mut output).map_err(io_error)?;
                output.flush().map_err(io_error)
            })();
            match (result, stage.cleanup()) {
                (Err(error), _) => Err(error),
                (Ok(()), Ok(())) => Ok(()),
                (Ok(()), Err(error)) => Err(io_error(error)),
            }
        })
        .await
}
