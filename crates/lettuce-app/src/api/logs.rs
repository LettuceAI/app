use std::io::Write;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_observability::{LogDirectory, LogEntry, LogFileError, LogSearchOptions, LogSink};

use super::{
    ApiContext,
    error::{IntoApiError, invalid_field},
};

#[derive(Clone)]
pub(super) struct LogHost {
    pub directory: LogDirectory,
    pub sink: LogSink,
}

pub(super) fn unavailable(reason: dto::LogFailureReason, message: impl Into<String>) -> ApiError {
    ApiError {
        code: ApiErrorCode::Unavailable,
        message: message.into(),
        details: Some(dto::ApiErrorDetails::Logs { reason }),
    }
}

fn log_error(error: LogFileError) -> ApiError {
    let code = match error {
        LogFileError::NotFound => ApiErrorCode::NotFound,
        LogFileError::InvalidPattern(_) => return invalid_field("query", error.to_string()),
        _ => ApiErrorCode::Unavailable,
    };
    let reason = if matches!(&error, LogFileError::ReadLine(source) if source.kind() == std::io::ErrorKind::InvalidData)
    {
        dto::LogFailureReason::InvalidEncoding
    } else {
        dto::LogFailureReason::Storage
    };
    ApiError {
        code,
        message: error.to_string(),
        details: Some(dto::ApiErrorDetails::Logs { reason }),
    }
}

fn io_error(error: std::io::Error) -> ApiError {
    unavailable(dto::LogFailureReason::Storage, error.to_string())
}

pub async fn logs_list(context: &ApiContext) -> Result<dto::LogsList, ApiError> {
    context
        .blocking(|context| {
            Ok(dto::LogsList {
                files: context.logs()?.directory.list().map_err(log_error)?,
            })
        })
        .await
}

pub async fn log_read_page(
    context: &ApiContext,
    request: dto::LogReadPageRequest,
) -> Result<dto::LogPageView, ApiError> {
    context
        .blocking(move |context| {
            let offset = usize::try_from(request.offset)
                .map_err(|_| invalid_field("offset", "offset is out of range"))?;
            let page = context
                .logs()?
                .directory
                .page(&request.name, offset, request.limit.clamp(1, 2000) as usize)
                .map_err(log_error)?;
            Ok(dto::LogPageView {
                total: page.total as u64,
                lines: page.lines,
            })
        })
        .await
}

fn search_view(view: lettuce_observability::LogSearchResult) -> dto::LogSearchView {
    dto::LogSearchView {
        total: view.total as u64,
        matches: view.matches.into_iter().map(|index| index as u64).collect(),
    }
}

pub async fn log_search(
    context: &ApiContext,
    request: dto::LogSearchRequest,
) -> Result<dto::LogSearchView, ApiError> {
    context
        .blocking(move |context| {
            context
                .logs()?
                .directory
                .search(
                    &request.name,
                    &request.query,
                    LogSearchOptions {
                        case_sensitive: request.case_sensitive,
                        whole_word: request.whole_word,
                        regex: request.regex,
                    },
                )
                .map(search_view)
                .map_err(log_error)
        })
        .await
}

pub async fn log_relevant_lines(
    context: &ApiContext,
    request: dto::LogRelevantLinesRequest,
) -> Result<dto::LogSearchView, ApiError> {
    context
        .blocking(move |context| {
            context
                .logs()?
                .directory
                .relevant_lines(&request.name, &request.reference_line)
                .map(search_view)
                .map_err(log_error)
        })
        .await
}

pub async fn log_delete(
    context: &ApiContext,
    request: dto::LogNameRequest,
) -> Result<(), ApiError> {
    context
        .blocking(move |context| {
            context
                .logs()?
                .directory
                .delete(&request.name)
                .map_err(log_error)
        })
        .await
}

pub async fn logs_clear(context: &ApiContext) -> Result<(), ApiError> {
    context
        .blocking(|context| context.logs()?.directory.clear().map_err(log_error))
        .await
}

pub async fn log_export(
    context: &ApiContext,
    request: dto::LogExportRequest,
) -> Result<(), ApiError> {
    context
        .blocking(move |context| {
            let logs = context.logs()?;
            let mut input = logs.directory.open(&request.name).map_err(log_error)?;
            if request.target.uri.trim().is_empty() {
                return Err(invalid_field("target.uri", "export target is empty"));
            }
            let protection = super::files::FileExportProtection::new(context)?;
            let mut output = context
                .files()
                .create_export(&request.target.uri, Some(&input), &|uri, target| {
                    protection.protects(uri, target)
                })
                .map_err(|error| {
                    if error == super::FileAccessError::SourceIsTarget {
                        ApiError {
                            code: ApiErrorCode::Conflict,
                            message: error.to_string(),
                            details: Some(dto::ApiErrorDetails::Logs {
                                reason: dto::LogFailureReason::SourceIsTarget,
                            }),
                        }
                    } else {
                        let mut api = error.into_api_error();
                        api.details = Some(dto::ApiErrorDetails::Logs {
                            reason: dto::LogFailureReason::Storage,
                        });
                        api
                    }
                })?;
            std::io::copy(&mut input, &mut output).map_err(io_error)?;
            output.flush().map_err(io_error)
        })
        .await
}

pub async fn log_append(
    context: &ApiContext,
    request: dto::LogAppendRequest,
) -> Result<(), ApiError> {
    context
        .blocking(move |context| {
            if !super::content_filter::refresh_logging(context)? && !cfg!(debug_assertions) {
                return Err(ApiError {
                    code: ApiErrorCode::Unsupported,
                    message: "developer mode is required".into(),
                    details: Some(dto::ApiErrorDetails::Settings {
                        reason: dto::SettingsFailureReason::DeveloperModeRequired,
                    }),
                });
            }
            chrono::DateTime::parse_from_rfc3339(&request.timestamp)
                .map_err(|_| invalid_field("timestamp", "invalid log timestamp"))?;
            for (field, value) in [
                ("component", request.component.as_str()),
                ("function", request.function.as_deref().unwrap_or("")),
            ] {
                if value.contains(char::is_whitespace) {
                    return Err(invalid_field(field, "log label contains whitespace"));
                }
            }
            let level = match request.level {
                dto::LogLevel::Debug => "DEBUG",
                dto::LogLevel::Info => "INFO",
                dto::LogLevel::Warn => "WARN",
                dto::LogLevel::Error => "ERROR",
            };
            context
                .logs()?
                .sink
                .append(&LogEntry {
                    timestamp: request.timestamp,
                    level: level.into(),
                    component: request.component,
                    function: request.function,
                    message: request.message.replace('\n', "\\n").replace('\r', "\\r"),
                })
                .map_err(io_error)
        })
        .await
}
