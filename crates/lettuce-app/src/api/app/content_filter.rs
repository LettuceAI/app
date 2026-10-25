use crate::api::{ApiContext, app::settings_error};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_inference::content_filter::PureModeLevel;

pub(super) fn filter_error() -> ApiError {
    ApiError {
        code: ApiErrorCode::Unavailable,
        message: "filter log is unavailable".to_owned(),
        details: Some(dto::ApiErrorDetails::Settings {
            reason: dto::SettingsFailureReason::Storage,
        }),
    }
}

pub(super) fn refresh_logging(context: &ApiContext) -> Result<bool, ApiError> {
    let (generation, enabled, mode) = context
        .backend()
        .database()
        .settings_filter_state()
        .map_err(settings_error)?;
    context
        .content_filter()
        .apply_settings(
            generation,
            enabled,
            crate::generation::provider_runtime::pure_mode_level(mode),
        )
        .map_err(|_| filter_error())?;
    Ok(enabled)
}

fn require_debug(context: &ApiContext) -> Result<(), ApiError> {
    let enabled = refresh_logging(context)?;
    if enabled {
        Ok(())
    } else {
        Err(ApiError {
            code: ApiErrorCode::Unsupported,
            message: "developer mode is required".to_owned(),
            details: Some(dto::ApiErrorDetails::Settings {
                reason: dto::SettingsFailureReason::DeveloperModeRequired,
            }),
        })
    }
}

pub async fn content_filter_log(
    context: &ApiContext,
) -> Result<dto::ContentFilterLogView, ApiError> {
    context
        .blocking(|context| {
            require_debug(context)?;
            let entries = context
                .content_filter()
                .hit_log()
                .map_err(|_| filter_error())?;
            Ok(dto::ContentFilterLogView {
                entries: entries
                    .into_iter()
                    .map(|entry| dto::ContentFilterLogEntry {
                        timestamp_ms: entry.timestamp_ms,
                        text_snippet: entry.text_snippet,
                        score: entry.score,
                        blocked: entry.blocked,
                        matched_terms: entry.matched_terms,
                        level: match entry.level {
                            PureModeLevel::Off => dto::SettingsPureMode::Off,
                            PureModeLevel::Low => dto::SettingsPureMode::Low,
                            PureModeLevel::Standard => dto::SettingsPureMode::Standard,
                            PureModeLevel::Strict => dto::SettingsPureMode::Strict,
                        },
                    })
                    .collect(),
            })
        })
        .await
}

pub async fn content_filter_clear(context: &ApiContext) -> Result<(), ApiError> {
    context
        .blocking(|context| {
            require_debug(context)?;
            context
                .content_filter()
                .clear_hit_log()
                .map_err(|_| filter_error())
        })
        .await
}

pub(super) fn publish_settings_changes(context: &ApiContext) -> Result<(), ApiError> {
    let sections = context.take_settings_sections();
    if sections.contains(&"device") || sections.contains(&"embedding") {
        context.onnx_models().refresh_settings(||
            lettuce_settings::DeviceSettingsStore::load_device_settings(context.backend().database())
        ).map_err(settings_error)?;
    }
    let refreshed = refresh_logging(context).map(|_| ());
    for section in sections {
        context.emit(dto::ApiEvent::SettingsChanged {
            section: section.to_owned(),
        });
    }
    refreshed
}

pub(in crate::api) async fn publish_pending(context: &ApiContext) {
    if let Err(error) = context.blocking(publish_settings_changes).await {
        tracing::error!(?error, "settings change delivery failed");
    }
}

pub(super) async fn run_events(context: ApiContext, stopped: impl Future<Output = ()>) {
    #[cfg(not(test))]
    let _pressure_monitor = {
        let manager = context.onnx_models().clone();
        lettuce_platform::MemoryPressureMonitor::subscribe(move || manager.memory_warning())
            .map_err(|error| tracing::debug!(%error, "native memory pressure notifications are unavailable"))
            .ok()
    };
    let signal = context.content_filter().hit_signal();
    let stopped = std::pin::pin!(stopped);
    let mut stopped = stopped;
    let mut emitted = 0;
    loop {
        tokio::select! {
            () = &mut stopped => break,
            () = context.shutdown_token().cancelled() => break,
            () = context.settings_changed() => {
                if let Err(error) = context.blocking(publish_settings_changes).await {
                    tracing::error!(?error, "settings change delivery failed");
                }
                continue;
            }
            () = context.onnx_models().notified() => {
                if let Err(error) = context.blocking(|context| {
                    context.onnx_models().check();
                    Ok(())
                }).await {
                    tracing::warn!(?error, "ONNX memory pressure check failed");
                }
                continue;
            }
            () = signal.notified() => {}
        }
        if context.content_filter().hit_revision().ok() == Some(emitted) {
            continue;
        }
        if let Ok(revision) = context.content_filter().hit_revision() {
            emitted = revision;
            if context.content_filter().logging_enabled() == Ok(true) {
                context.emit(dto::ApiEvent::ContentFilterHit);
            }
        }
    }
}
