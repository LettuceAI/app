use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LogsList {
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LogNameRequest {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LogReadPageRequest {
    pub name: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub offset: u64,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LogPageView {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub total: u64,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LogSearchRequest {
    pub name: String,
    pub query: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LogRelevantLinesRequest {
    pub name: String,
    pub reference_line: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LogSearchView {
    #[cfg_attr(feature = "specta", specta(type = Vec<specta_typescript::Number>))]
    pub matches: Vec<u64>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LogExportRequest {
    pub name: String,
    pub target: crate::FileTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LogAppendRequest {
    pub timestamp: String,
    pub level: LogLevel,
    pub component: String,
    pub function: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LogFailureReason {
    HostUnavailable,
    Storage,
    InvalidEncoding,
    SourceIsTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LogViewFilter {
    pub levels: Vec<LogLevel>,
    pub hidden_components: Vec<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub query: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
    pub only_matches: bool,
    #[cfg_attr(feature = "specta", specta(type = Option<Vec<specta_typescript::Number>>))]
    pub isolation: Option<Vec<u64>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LogQueryRequest {
    pub name: String,
    pub filter: LogViewFilter,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub offset: u64,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LogLineView {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub index: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub position: u64,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LogPosition {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub index: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub position: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LogComponentCount {
    pub component: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub count: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LogLevelCounts {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub debug: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub info: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub warn: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub error: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LogQueryView {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub total: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub filtered_total: u64,
    pub lines: Vec<LogLineView>,
    pub components: Vec<LogComponentCount>,
    pub counts: LogLevelCounts,
    pub matches: Vec<LogPosition>,
    pub problems: Vec<LogPosition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LogSelection {
    Indices {
        #[cfg_attr(feature = "specta", specta(type = Vec<specta_typescript::Number>))]
        indices: Vec<u64>,
    },
    Filter {
        filter: LogViewFilter,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LogExportSelectionRequest {
    pub name: String,
    pub target: crate::FileTarget,
    pub selection: LogSelection,
}
