use super::{ApiContext, error::parse_id};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_database::ApiOperationError;
use lettuce_types::{RequestId, TimestampMillis};

fn report_error(error: lettuce_usage::UsageReportError) -> ApiError {
    use lettuce_usage::UsageReportError;
    let field = match error {
        UsageReportError::InvalidCursor => Some("cursor"),
        UsageReportError::InvalidRange => Some("range"),
        UsageReportError::InvalidTimeZone => Some("time_zone"),
        _ => None,
    };
    if let Some(field) = field {
        return super::error::invalid_field(field, error.to_string());
    }
    ApiError {
        code: ApiErrorCode::Unavailable,
        message: error.to_string(),
        details: Some(dto::ApiErrorDetails::UsageStorage),
    }
}

fn filters(request: dto::UsageFilters) -> Result<lettuce_usage::UsageReportFilters, ApiError> {
    let model = request
        .model_id
        .map(|value| {
            super::error::parse_id::<lettuce_types::ModelProfileId>(&value, "filters.model_id")
                .map(|id| id.to_string())
        })
        .transpose()?;
    let character = request
        .character_id
        .map(|value| {
            super::error::parse_id::<lettuce_types::CharacterId>(&value, "filters.character_id")
                .map(|id| id.to_string())
        })
        .transpose()?;
    let filters = lettuce_usage::UsageReportFilters {
        start: request.range.start,
        end: request.range.end,
        provider_kind: request.provider_kind,
        model,
        character,
        operation: request.operation_kind,
        status: request.status.map(|status| match status {
            dto::UsageStatus::Pending => lettuce_usage::UsageReportStatus::Pending,
            dto::UsageStatus::Succeeded => lettuce_usage::UsageReportStatus::Succeeded,
            dto::UsageStatus::Failed => lettuce_usage::UsageReportStatus::Failed,
            dto::UsageStatus::Cancelled => lettuce_usage::UsageReportStatus::Cancelled,
            dto::UsageStatus::Interrupted => lettuce_usage::UsageReportStatus::Interrupted,
        }),
    };
    filters.validate().map_err(report_error)?;
    Ok(filters)
}

fn row_view(row: lettuce_usage::UsageReportRow) -> dto::UsageRow {
    dto::UsageRow {
        id: row.id,
        timestamp: row.timestamp,
        status: match row.status {
            lettuce_usage::UsageReportStatus::Pending => dto::UsageStatus::Pending,
            lettuce_usage::UsageReportStatus::Succeeded => dto::UsageStatus::Succeeded,
            lettuce_usage::UsageReportStatus::Failed => dto::UsageStatus::Failed,
            lettuce_usage::UsageReportStatus::Cancelled => dto::UsageStatus::Cancelled,
            lettuce_usage::UsageReportStatus::Interrupted => dto::UsageStatus::Interrupted,
        },
        session_id: row.session_id,
        character_id: row.character_id,
        character_name: row.character_name,
        model_id: row.model_id,
        model_name: row.model_name,
        provider_kind: row.provider_kind,
        provider_label: row.provider_label,
        operation_type: row.operation_type,
        finish_reason: row.finish_reason,
        provider_response_id: row.provider_response_id,
        prompt_tokens: row.prompt_tokens,
        cached_prompt_tokens: row.cached_prompt_tokens,
        cache_write_tokens: row.cache_write_tokens,
        completion_tokens: row.completion_tokens,
        reasoning_tokens: row.reasoning_tokens,
        image_tokens: row.image_tokens,
        audio_tokens: row.audio_tokens,
        web_search_requests: row.web_search_requests,
        total_tokens: row.total_tokens,
        memory_tokens: row.memory_tokens,
        summary_tokens: row.summary_tokens,
        input_image_count: row.input_image_count,
        output_image_count: row.output_image_count,
        prompt_cost: row.prompt_cost,
        cache_read_cost: row.cache_read_cost,
        cache_write_cost: row.cache_write_cost,
        completion_cost: row.completion_cost,
        reasoning_cost: row.reasoning_cost,
        request_cost: row.request_cost,
        web_search_cost: row.web_search_cost,
        total_cost: row.total_cost,
        api_cost: row.api_cost,
        error_message: row.error_message,
    }
}

fn totals_view(totals: lettuce_usage::UsageReportTotals) -> dto::UsageTotals {
    dto::UsageTotals {
        requests: totals.requests,
        successful_requests: totals.successful_requests,
        failed_requests: totals.failed_requests,
        cancelled_requests: totals.cancelled_requests,
        interrupted_requests: totals.interrupted_requests,
        pending_requests: totals.pending_requests,
        prompt_tokens: totals.prompt_tokens,
        completion_tokens: totals.completion_tokens,
        total_tokens: totals.total_tokens,
        total_cost: totals.total_cost,
        unknown_token_requests: totals.unknown_token_requests,
        unknown_cost_requests: totals.unknown_cost_requests,
    }
}

pub async fn usage_query(
    context: &ApiContext,
    request: dto::UsageQueryRequest,
) -> Result<dto::UsagePage, ApiError> {
    let filters = filters(request.filters)?;
    let sort = match request.sort {
        dto::UsageSort::NewestFirst => lettuce_usage::UsageReportSort::NewestFirst,
        dto::UsageSort::OldestFirst => lettuce_usage::UsageReportSort::OldestFirst,
    };
    context
        .blocking(move |context| {
            use lettuce_usage::UsageReportRepository;
            let rows = context
                .backend()
                .database()
                .read_usage_report()
                .map_err(report_error)?;
            let page = lettuce_usage::usage_report_page(
                &rows,
                &filters,
                sort,
                request.cursor.as_deref(),
                request.limit,
            )
            .map_err(report_error)?;
            Ok(dto::UsagePage {
                items: page.items.into_iter().map(row_view).collect(),
                next_cursor: page.next_cursor,
            })
        })
        .await
}

pub async fn usage_stats(
    context: &ApiContext,
    request: dto::UsageStatsRequest,
) -> Result<dto::UsageStats, ApiError> {
    let group = match request.group_by {
        dto::UsageGroupBy::Day => lettuce_usage::UsageReportGroup::Day,
        dto::UsageGroupBy::Model => lettuce_usage::UsageReportGroup::Model,
        dto::UsageGroupBy::Provider => lettuce_usage::UsageReportGroup::Provider,
        dto::UsageGroupBy::Character => lettuce_usage::UsageReportGroup::Character,
        dto::UsageGroupBy::Operation => lettuce_usage::UsageReportGroup::Operation,
    };
    let filters = lettuce_usage::UsageReportFilters {
        start: request.range.start,
        end: request.range.end,
        provider_kind: request.provider_kind,
        ..lettuce_usage::UsageReportFilters::default()
    };
    filters.validate().map_err(report_error)?;
    context
        .blocking(move |context| {
            use lettuce_usage::UsageReportRepository;
            let rows = context
                .backend()
                .database()
                .read_usage_report()
                .map_err(report_error)?;
            let stats =
                lettuce_usage::usage_report_stats(&rows, &filters, group, &request.time_zone)
                    .map_err(report_error)?;
            Ok(dto::UsageStats {
                totals: totals_view(stats.totals),
                groups: stats
                    .groups
                    .into_iter()
                    .map(|group| dto::UsageGroupTotals {
                        key: group.key,
                        label: group.label,
                        totals: totals_view(group.totals),
                    })
                    .collect(),
            })
        })
        .await
}

pub async fn usage_export_csv(
    context: &ApiContext,
    request: dto::UsageExportCsvRequest,
) -> Result<(), ApiError> {
    use super::error::IntoApiError;
    let filters = filters(request.filters)?;
    if request.target.uri.trim().is_empty() {
        return Err(super::error::invalid_field(
            "target.uri",
            "export target is empty",
        ));
    }
    context
        .blocking(move |context| {
            use lettuce_usage::UsageReportRepository;
            use std::io::Write;
            let rows = context
                .backend()
                .database()
                .read_usage_report()
                .map_err(report_error)?
                .into_iter()
                .filter(|row| filters.matches(row))
                .collect::<Vec<_>>();
            let csv = lettuce_usage::usage_csv(&rows).map_err(report_error)?;
            let protection = super::files::FileExportProtection::new(context)?;
            let mut output = context
                .files()
                .create_export(&request.target.uri, None, &|uri, target| {
                    protection.protects(uri, target)
                })
                .map_err(IntoApiError::into_api_error)?;
            output.write_all(csv.as_bytes()).map_err(|error| ApiError {
                code: ApiErrorCode::Unavailable,
                message: error.to_string(),
                details: Some(dto::ApiErrorDetails::UsageStorage),
            })?;
            output.flush().map_err(|error| ApiError {
                code: ApiErrorCode::Unavailable,
                message: error.to_string(),
                details: Some(dto::ApiErrorDetails::UsageStorage),
            })
        })
        .await
}

pub async fn usage_clear_before(
    context: &ApiContext,
    request: dto::UsageClearBeforeRequest,
) -> Result<dto::UsageCleared, ApiError> {
    context
        .blocking(move |context| {
            let key: RequestId = parse_id(&request.client_operation_id, "client_operation_id")?;
            let removed = context
                .backend()
                .database()
                .commit_api_operation::<u64, ApiOperationError>(
                    "usage_clear_before",
                    &key.to_string(),
                    &request.before.to_string(),
                    context.now(),
                    |transaction| {
                        transaction.clear_usage_before(TimestampMillis::new(request.before))
                    },
                )
                .map_err(|error| ApiError {
                    code: match error {
                        ApiOperationError::Conflict => ApiErrorCode::Conflict,
                        ApiOperationError::InvalidData => ApiErrorCode::InvalidInput,
                        ApiOperationError::Storage => ApiErrorCode::Unavailable,
                    },
                    message: error.to_string(),
                    details: Some(dto::ApiErrorDetails::UsageStorage),
                })?;
            Ok(dto::UsageCleared { removed })
        })
        .await
}
