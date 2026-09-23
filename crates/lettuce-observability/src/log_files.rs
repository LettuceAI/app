//! The app's daily log files: one `app-YYYY-MM-DD.log` per local day, one
//! `[time] LEVEL component at=location | message` line per record, and the
//! reads the log viewer runs over them.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

const MAX_MESSAGE_CHARS: usize = 1200;
const UNTRUNCATED_COMPONENTS: [&str; 4] = [
    "api_request",
    "image_generator",
    "llama_cpp",
    "dynamic_memory",
];

/// Parameter names whose values are masked in log messages.
pub(crate) const SECRET_PARAMETERS: [&str; 7] = [
    "key",
    "api_key",
    "apikey",
    "access_token",
    "token",
    "authorization",
    "x-api-key",
];

/// One record as written to a log file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub component: String,
    pub function: Option<String>,
    pub message: String,
}

impl LogEntry {
    /// The record's line, newline included.
    #[must_use]
    pub fn line(&self) -> String {
        let mut line = format!("[{}] {} {}", self.timestamp, self.level, self.component);
        if let Some(function) = &self.function {
            line.push_str(" at=");
            line.push_str(function);
        }
        line.push_str(" | ");
        line.push_str(&self.message);
        line.push('\n');
        line
    }
}

fn find_ascii_ci(haystack: &str, needle: &str, from: usize) -> Option<usize> {
    let bytes = haystack.as_bytes();
    let needle = needle.as_bytes();
    (from..=bytes.len().checked_sub(needle.len())?)
        .find(|&start| bytes[start..start + needle.len()].eq_ignore_ascii_case(needle))
}

fn value_end(message: &str, start: usize, stop: impl Fn(char) -> bool) -> usize {
    message[start..]
        .char_indices()
        .find(|(_, ch)| stop(*ch))
        .map_or(message.len(), |(offset, _)| start + offset)
}

fn redact_param_value(message: &str, param: &str) -> String {
    let needle = format!("{param}=");
    let mut output = String::with_capacity(message.len());
    let mut cursor = 0;
    while let Some(start) = find_ascii_ci(message, &needle, cursor) {
        let value_start = start + needle.len();
        output.push_str(&message[cursor..value_start]);
        output.push_str("***");
        cursor = value_end(message, value_start, |ch| {
            ch.is_whitespace() || matches!(ch, '&' | '"' | '\'' | ')' | ']')
        });
    }
    output.push_str(&message[cursor..]);
    output
}

fn redact_authorization(message: &str) -> String {
    if find_ascii_ci(message, "authorization", 0).is_none()
        && find_ascii_ci(message, "bearer", 0).is_none()
    {
        return message.to_owned();
    }
    let mut output = message.to_owned();
    if let Some(index) = find_ascii_ci(message, "bearer ", 0) {
        let start = index + "bearer ".len();
        let end = value_end(message, start, |ch| {
            ch.is_whitespace() || matches!(ch, '"' | '\'' | ',')
        });
        output.replace_range(start..end, "***");
    }
    output
}

fn redact_body_payload(message: &str) -> Option<String> {
    [
        "request body:",
        "response body:",
        "setting body as json:",
        "request body",
    ]
    .into_iter()
    .find_map(|marker| {
        let split_at = find_ascii_ci(message, marker, 0)? + marker.len();
        let (prefix, rest) = message.split_at(split_at);
        Some(format!(
            "{} <redacted body len={}>",
            prefix.trim_end(),
            rest.len()
        ))
    })
}

/// A message as it may be stored: one line, request and response bodies
/// replaced by their length, credentials masked, and long messages cut.
#[must_use]
pub fn sanitize_message(component: &str, message: &str) -> String {
    let mut message = message.replace('\n', "\\n").replace('\r', "\\r");
    if let Some(redacted) = redact_body_payload(&message) {
        message = redacted;
    }
    message = redact_authorization(&message);
    for key in SECRET_PARAMETERS {
        message = redact_param_value(&message, key);
    }
    message = redact_param_value(&message, "key");
    if !UNTRUNCATED_COMPONENTS.contains(&component) && message.len() > MAX_MESSAGE_CHARS {
        let truncated: String = message.chars().take(MAX_MESSAGE_CHARS).collect();
        message = format!("{truncated}... <truncated>");
    }
    if component == "api_request" && message.contains("full_url=") {
        message = message.replace("full_url=", "url=");
    }
    message
}

/// The file a local date's records go to.
#[must_use]
pub fn daily_file_name(date: chrono::NaiveDate) -> String {
    format!("app-{}.log", date.format("%Y-%m-%d"))
}

/// Appends lines to the current local day's file, reopening it when the day
/// changes or the file was deleted.
#[derive(Debug)]
pub struct DailyLogWriter {
    directory: PathBuf,
    current: Option<(PathBuf, File)>,
}

impl DailyLogWriter {
    #[must_use]
    pub const fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            current: None,
        }
    }

    fn file(&mut self) -> io::Result<&mut File> {
        let path = self
            .directory
            .join(daily_file_name(chrono::Local::now().date_naive()));
        let reuse = self
            .current
            .as_ref()
            .is_some_and(|(open, _)| *open == path && path.exists());
        if !reuse {
            let file = OpenOptions::new().create(true).append(true).open(&path)?;
            self.current = Some((path, file));
        }
        Ok(&mut self.current.as_mut().expect("opened above").1)
    }
}

impl Write for DailyLogWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let file = self.file()?;
        file.write_all(bytes)?;
        file.flush()?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut self.current {
            Some((_, file)) => file.flush(),
            None => Ok(()),
        }
    }
}

/// A page of a log file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogPage {
    pub total: usize,
    pub lines: Vec<String>,
}

/// The 0-based indices of matching lines and the file's line count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogSearchResult {
    pub matches: Vec<usize>,
    pub total: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LogSearchOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
}

#[derive(Debug, Error)]
pub enum LogFileError {
    #[error("Log file not found")]
    NotFound,
    #[error("Failed to read log directory: {0}")]
    ReadDirectory(#[source] io::Error),
    #[error("Failed to read log file: {0}")]
    Read(#[source] io::Error),
    #[error("Failed to open log file: {0}")]
    Open(#[source] io::Error),
    #[error("Failed to read line: {0}")]
    ReadLine(#[source] io::Error),
    #[error("Invalid search pattern: {0}")]
    InvalidPattern(#[source] regex::Error),
    #[error("Failed to delete log file: {0}")]
    Delete(#[source] io::Error),
}

enum LineMatcher {
    Plain(String),
    PlainIgnoringCase(String),
    Pattern(regex::Regex),
}

impl LineMatcher {
    fn build(query: &str, options: LogSearchOptions) -> Result<Self, LogFileError> {
        if options.regex || options.whole_word {
            let pattern = if options.whole_word && !options.regex {
                format!(r"\b{}\b", regex::escape(query))
            } else {
                query.to_owned()
            };
            regex::RegexBuilder::new(&pattern)
                .case_insensitive(!options.case_sensitive)
                .build()
                .map(Self::Pattern)
                .map_err(LogFileError::InvalidPattern)
        } else if options.case_sensitive {
            Ok(Self::Plain(query.to_owned()))
        } else {
            Ok(Self::PlainIgnoringCase(query.to_lowercase()))
        }
    }

    fn is_match(&self, line: &str) -> bool {
        match self {
            Self::Plain(query) => line.contains(query.as_str()),
            Self::PlainIgnoringCase(query) => line.to_lowercase().contains(query.as_str()),
            Self::Pattern(pattern) => pattern.is_match(line),
        }
    }
}

/// The log viewer's reads over the log directory. File names are single
/// names inside the directory.
#[derive(Debug, Clone)]
pub struct LogDirectory {
    directory: PathBuf,
}

impl LogDirectory {
    #[must_use]
    pub const fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.directory
    }

    fn file(&self, filename: &str) -> Result<PathBuf, LogFileError> {
        let mut components = Path::new(filename).components();
        let single = matches!(
            (components.next(), components.next()),
            (Some(Component::Normal(_)), None)
        );
        let path = self.directory.join(filename);
        if single && path.is_file() {
            Ok(path)
        } else {
            Err(LogFileError::NotFound)
        }
    }

    fn lines(&self, filename: &str) -> Result<io::Lines<BufReader<File>>, LogFileError> {
        let path = self.file(filename)?;
        let file = File::open(path).map_err(LogFileError::Open)?;
        Ok(BufReader::new(file).lines())
    }

    /// The `.log` files, newest name first.
    pub fn list(&self) -> Result<Vec<String>, LogFileError> {
        let mut names: Vec<String> = fs::read_dir(&self.directory)
            .map_err(LogFileError::ReadDirectory)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("log")
            })
            .filter_map(|path| path.file_name()?.to_str().map(str::to_owned))
            .collect();
        names.sort_by(|left, right| right.cmp(left));
        Ok(names)
    }

    pub fn read(&self, filename: &str) -> Result<String, LogFileError> {
        fs::read_to_string(self.file(filename)?).map_err(LogFileError::Read)
    }

    /// Lines `offset..offset + limit` and the file's line count.
    pub fn page(
        &self,
        filename: &str,
        offset: usize,
        limit: usize,
    ) -> Result<LogPage, LogFileError> {
        let mut total = 0;
        let mut lines = Vec::with_capacity(limit.min(2000));
        for line in self.lines(filename)? {
            let line = line.map_err(LogFileError::ReadLine)?;
            if total >= offset && lines.len() < limit {
                lines.push(line);
            }
            total += 1;
        }
        Ok(LogPage { total, lines })
    }

    pub fn search(
        &self,
        filename: &str,
        query: &str,
        options: LogSearchOptions,
    ) -> Result<LogSearchResult, LogFileError> {
        let path = self.file(filename)?;
        let matcher = LineMatcher::build(query, options)?;
        let lines = BufReader::new(File::open(path).map_err(LogFileError::Open)?).lines();
        let mut matches = Vec::new();
        let mut total = 0;
        for line in lines {
            if matcher.is_match(&line.map_err(LogFileError::ReadLine)?) {
                matches.push(total);
            }
            total += 1;
        }
        Ok(LogSearchResult { matches, total })
    }

    /// Lines related to `reference_line`: sharing one of its UUIDs, or from
    /// its component within 30 seconds of it or sharing enough of its words.
    pub fn relevant_lines(
        &self,
        filename: &str,
        reference_line: &str,
    ) -> Result<LogSearchResult, LogFileError> {
        let lines = self.lines(filename)?;
        let uuid = regex::Regex::new(
            r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
        )
        .expect("the UUID pattern compiles");
        let reference_component = reference_line.split_whitespace().nth(2).unwrap_or("");
        let reference_uuids: Vec<String> = uuid
            .find_iter(reference_line)
            .map(|found| found.as_str().to_lowercase())
            .collect();
        let reference_words: Vec<String> = reference_line
            .split('|')
            .nth(1)
            .unwrap_or("")
            .split_whitespace()
            .filter(|word| word.len() >= 5)
            .map(str::to_lowercase)
            .collect();
        let epoch = |line: &str| {
            chrono::NaiveDateTime::parse_from_str(line.get(1..20)?, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|time| time.and_utc().timestamp())
        };
        let reference_epoch = epoch(reference_line);
        let mut matches = Vec::new();
        let mut total = 0;
        for line in lines {
            let line = line.map_err(LogFileError::ReadLine)?;
            let lower = line.to_lowercase();
            let mut matched = reference_uuids.iter().any(|id| lower.contains(id.as_str()));
            if !matched
                && !reference_component.is_empty()
                && line.split_whitespace().nth(2) == Some(reference_component)
            {
                matched = matches!(
                    (reference_epoch, epoch(&line)),
                    (Some(reference), Some(at)) if (at - reference).abs() <= 30
                );
                if !matched
                    && !reference_words.is_empty()
                    && let Some(message) = line.split('|').nth(1)
                {
                    let message = message.to_lowercase();
                    let shared = reference_words
                        .iter()
                        .filter(|word| message.contains(word.as_str()))
                        .count();
                    matched = shared >= 2 || shared * 100 / reference_words.len() > 40;
                }
            }
            if matched {
                matches.push(total);
            }
            total += 1;
        }
        Ok(LogSearchResult { matches, total })
    }

    pub fn delete(&self, filename: &str) -> Result<(), LogFileError> {
        fs::remove_file(self.file(filename)?).map_err(LogFileError::Delete)
    }

    /// Deletes every `.log` file, stopping at the first failure.
    pub fn clear(&self) -> Result<(), LogFileError> {
        for entry in fs::read_dir(&self.directory)
            .map_err(LogFileError::ReadDirectory)?
            .flatten()
        {
            let path = entry.path();
            if path.is_file() && path.extension() == Some("log".as_ref()) {
                fs::remove_file(path).map_err(LogFileError::Delete)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directory() -> LogDirectory {
        let path = std::env::temp_dir().join(format!(
            "lettuce-log-files-{}",
            lettuce_types::OperationId::new()
        ));
        fs::create_dir(&path).expect("temporary directory");
        LogDirectory::new(path)
    }

    fn entry(time: &str, component: &str, message: &str) -> String {
        LogEntry {
            timestamp: format!("2026-09-24T{time}.000+03:00"),
            level: "INFO".to_owned(),
            component: component.to_owned(),
            function: Some("src/lib.rs:10:1".to_owned()),
            message: message.to_owned(),
        }
        .line()
    }

    #[test]
    fn lines_keep_the_old_layout() {
        assert_eq!(
            entry("10:00:00", "sync", "hello"),
            "[2026-09-24T10:00:00.000+03:00] INFO sync at=src/lib.rs:10:1 | hello\n"
        );
        let plain = LogEntry {
            timestamp: "t".to_owned(),
            level: "warn".to_owned(),
            component: "ui".to_owned(),
            function: None,
            message: "m".to_owned(),
        };
        assert_eq!(plain.line(), "[t] warn ui | m\n");
        assert_eq!(
            daily_file_name(chrono::NaiveDate::from_ymd_opt(2026, 9, 4).expect("date")),
            "app-2026-09-04.log"
        );
    }

    #[test]
    fn messages_are_redacted_and_cut() {
        assert_eq!(
            sanitize_message("x", "GET /v1?key=abc&x=1 Authorization: Bearer sk-1,rest"),
            "GET /v1?key=***&x=1 Authorization: Bearer ***,rest"
        );
        assert_eq!(
            sanitize_message("x", "line one\nRequest Body: {\"a\":1}"),
            "line one\\nRequest Body: <redacted body len=8>"
        );
        assert_eq!(
            sanitize_message("x", "İstanbul TOKEN=secret done"),
            "İstanbul TOKEN=*** done"
        );
        let long = "a".repeat(1300);
        assert_eq!(
            sanitize_message("x", &long),
            format!("{}... <truncated>", "a".repeat(1200))
        );
        assert_eq!(sanitize_message("llama_cpp", &long), long);
        assert_eq!(
            sanitize_message("api_request", "full_url=https://h/p"),
            "url=https://h/p"
        );
    }

    #[test]
    fn the_viewer_pages_searches_and_relates_lines() {
        let logs = directory();
        let id = "0f8fad5b-d9cb-469f-a165-70867728950e";
        let content = [
            entry("10:00:00", "sync", &format!("started {id}")),
            entry("10:00:10", "sync", "pushed changes"),
            entry("10:05:00", "sync", "remote refused upload request"),
            entry(
                "10:09:00",
                "media",
                &format!("stored {}", id.to_uppercase()),
            ),
            entry("10:20:00", "media", "Stored thumbnail"),
            entry("11:00:00", "sync", "remote refused another upload"),
        ]
        .concat();
        fs::write(logs.path().join("app-2026-09-24.log"), content).expect("write");
        fs::write(logs.path().join("app-2026-09-23.log"), "").expect("write");
        fs::write(logs.path().join("notes.txt"), "").expect("write");
        assert_eq!(
            logs.list().expect("list"),
            ["app-2026-09-24.log", "app-2026-09-23.log"]
        );
        let page = logs.page("app-2026-09-24.log", 1, 2).expect("page");
        assert_eq!(page.total, 6);
        assert!(page.lines[0].ends_with("pushed changes"));
        let search = |query: &str, options| {
            logs.search("app-2026-09-24.log", query, options)
                .expect("search")
                .matches
        };
        assert_eq!(search("stored", LogSearchOptions::default()), [3, 4]);
        let refused = entry("10:05:00", "sync", "remote refused upload request");
        assert_eq!(
            logs.relevant_lines("app-2026-09-24.log", refused.trim_end())
                .expect("relevant")
                .matches,
            [2, 5]
        );
        let sensitive = LogSearchOptions {
            case_sensitive: true,
            ..LogSearchOptions::default()
        };
        assert_eq!(search("stored", sensitive), [3]);
        let whole = LogSearchOptions {
            whole_word: true,
            ..LogSearchOptions::default()
        };
        assert_eq!(search("push", whole), Vec::<usize>::new());
        assert!(matches!(
            logs.search(
                "app-2026-09-24.log",
                "(",
                LogSearchOptions {
                    regex: true,
                    ..LogSearchOptions::default()
                }
            ),
            Err(LogFileError::InvalidPattern(_))
        ));
        let reference = entry("10:00:00", "sync", &format!("started {id}"));
        assert_eq!(
            logs.relevant_lines("app-2026-09-24.log", reference.trim_end())
                .expect("relevant")
                .matches,
            [0, 1, 3]
        );
        fs::remove_dir_all(logs.path()).expect("cleanup");
    }

    #[test]
    fn names_outside_the_directory_are_not_found() {
        let logs = directory();
        let outside = logs.path().with_extension("log");
        fs::write(&outside, "secret").expect("write");
        let escape = format!(
            "../{}",
            outside
                .file_name()
                .and_then(|name| name.to_str())
                .expect("name")
        );
        assert!(matches!(logs.read(&escape), Err(LogFileError::NotFound)));
        assert!(matches!(logs.delete(&escape), Err(LogFileError::NotFound)));
        let absolute = outside.to_str().expect("path");
        assert!(matches!(logs.read(absolute), Err(LogFileError::NotFound)));
        assert!(matches!(logs.delete(absolute), Err(LogFileError::NotFound)));
        assert!(outside.exists());
        fs::remove_file(outside).expect("cleanup");
        fs::remove_dir_all(logs.path()).expect("cleanup");
    }

    #[test]
    fn the_daily_writer_reopens_a_deleted_file_and_clear_removes_logs() {
        let logs = directory();
        let mut writer = DailyLogWriter::new(logs.path().to_path_buf());
        writer.write_all(b"one\n").expect("write");
        let name = logs.list().expect("list").remove(0);
        logs.delete(&name).expect("delete");
        writer.write_all(b"two\n").expect("write");
        assert_eq!(logs.read(&name).expect("read"), "two\n");
        fs::write(logs.path().join("keep.txt"), "").expect("write");
        logs.clear().expect("clear");
        assert!(logs.list().expect("list").is_empty());
        assert!(logs.path().join("keep.txt").exists());
        fs::remove_dir_all(logs.path()).expect("cleanup");
    }
}
