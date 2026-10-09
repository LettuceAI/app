use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_database::{PurgeNoticeEntity, PurgeNoticeReason};
use lettuce_settings::{DeviceUiStateStore, GlobalSettingsStoreError};

use super::ApiContext;
use super::error::{api_error, invalid_field};

pub(super) fn settings_error(error: GlobalSettingsStoreError) -> ApiError {
    let code = match error {
        GlobalSettingsStoreError::InvalidData => ApiErrorCode::InvalidInput,
        GlobalSettingsStoreError::StaleRevision => ApiErrorCode::Conflict,
        GlobalSettingsStoreError::ModelProfileMissing => ApiErrorCode::NotFound,
        GlobalSettingsStoreError::Storage => ApiErrorCode::Internal,
    };
    let reason = match error {
        GlobalSettingsStoreError::InvalidData => dto::SettingsFailureReason::InvalidData,
        GlobalSettingsStoreError::StaleRevision => dto::SettingsFailureReason::StaleRevision,
        GlobalSettingsStoreError::ModelProfileMissing => {
            dto::SettingsFailureReason::ModelProfileMissing
        }
        GlobalSettingsStoreError::Storage => dto::SettingsFailureReason::Storage,
    };
    ApiError {
        code,
        message: error.to_string(),
        details: Some(dto::ApiErrorDetails::Settings { reason }),
    }
}

fn purge_error(error: lettuce_database::PurgeError) -> ApiError {
    api_error(ApiErrorCode::Internal, error.to_string())
}

const fn platform() -> dto::AppPlatform {
    if cfg!(target_os = "windows") {
        dto::AppPlatform::Windows
    } else if cfg!(target_os = "macos") {
        dto::AppPlatform::Macos
    } else if cfg!(target_os = "android") {
        dto::AppPlatform::Android
    } else if cfg!(target_os = "ios") {
        dto::AppPlatform::Ios
    } else if cfg!(target_os = "linux") {
        dto::AppPlatform::Linux
    } else {
        dto::AppPlatform::Other
    }
}

/// What the shell shows before any feature: version, build, platform, this
/// device's UI state, and the counts that ask for the user's attention.
pub async fn app_status(context: &ApiContext) -> Result<dto::AppStatus, ApiError> {
    context
        .blocking(|context| {
            let database = context.backend().database();
            let version = crate::app_version(env!("CARGO_PKG_VERSION"));
            let build_variant = if version.ends_with("-cuda") {
                dto::BuildVariant::Cuda
            } else {
                dto::BuildVariant::Normal
            };
            Ok(dto::AppStatus {
                version,
                build_variant,
                platform: platform(),
                ui_state: database.load_device_ui_state().map_err(settings_error)?,
                legacy_database_detected: context.legacy_database_detected(),
                unresolved_sync_conflicts: database
                    .unresolved_sync_conflict_count()
                    .map_err(purge_error)?,
                purge_notices: u64::try_from(database.purge_notices().map_err(purge_error)?.len())
                    .unwrap_or(u64::MAX),
            })
        })
        .await
}

/// Sets the patch's keys in this device's UI state; a `null` value removes
/// its key.
pub async fn app_ui_state_update(
    context: &ApiContext,
    request: dto::AppUiStateUpdateRequest,
) -> Result<dto::AppUiStateView, ApiError> {
    context
        .blocking(move |context| {
            if request.patch.keys().any(|key| key.trim().is_empty()) {
                return Err(invalid_field("patch", "a UI state key is empty"));
            }
            let state = context
                .backend()
                .database()
                .patch_device_ui_state(request.patch)
                .map_err(settings_error)?;
            Ok(super::settings::finish_committed(
                dto::AppUiStateView { state },
                || super::content_filter::publish_settings_changes(context),
            ))
        })
        .await
}

/// Undismissed purge notices, oldest first.
pub async fn purge_notices_list(context: &ApiContext) -> Result<dto::PurgeNoticeList, ApiError> {
    context
        .blocking(|context| {
            let notices = context
                .backend()
                .database()
                .purge_notices()
                .map_err(purge_error)?;
            Ok(dto::PurgeNoticeList {
                items: notices
                    .into_iter()
                    .map(|notice| dto::PurgeNoticeView {
                        id: notice.id.to_string(),
                        entity: entity(notice.entity),
                        entity_id: notice.entity_id,
                        reason: reason(notice.reason),
                        recorded_at: notice.recorded_at.get(),
                    })
                    .collect(),
            })
        })
        .await
}

/// Dismisses a notice; dismissing one already dismissed is `NotFound`.
pub async fn purge_notice_dismiss(
    context: &ApiContext,
    request: dto::PurgeNoticeDismissRequest,
) -> Result<(), ApiError> {
    let id = request
        .id
        .parse::<i64>()
        .map_err(|_| invalid_field("id", "id is not a valid notice id"))?;
    context
        .blocking(move |context| {
            let dismissed = context
                .backend()
                .database()
                .dismiss_purge_notice(id, context.now())
                .map_err(purge_error)?;
            if dismissed {
                Ok(())
            } else {
                Err(api_error(
                    ApiErrorCode::NotFound,
                    "no open notice has this id",
                ))
            }
        })
        .await
}

const fn entity(entity: PurgeNoticeEntity) -> dto::PurgeNoticeEntityDto {
    match entity {
        PurgeNoticeEntity::Conversation => dto::PurgeNoticeEntityDto::Conversation,
        PurgeNoticeEntity::Character => dto::PurgeNoticeEntityDto::Character,
        PurgeNoticeEntity::Group => dto::PurgeNoticeEntityDto::Group,
        PurgeNoticeEntity::DatabaseFile => dto::PurgeNoticeEntityDto::DatabaseFile,
        PurgeNoticeEntity::MediaAsset => dto::PurgeNoticeEntityDto::MediaAsset,
        PurgeNoticeEntity::SyncEntity => dto::PurgeNoticeEntityDto::SyncEntity,
    }
}

const fn reason(reason: PurgeNoticeReason) -> dto::PurgeNoticeReasonDto {
    match reason {
        PurgeNoticeReason::KeptUnsentLocalChanges => {
            dto::PurgeNoticeReasonDto::KeptUnsentLocalChanges
        }
        PurgeNoticeReason::RejournalIncomplete => dto::PurgeNoticeReasonDto::RejournalIncomplete,
        PurgeNoticeReason::RejournalDropped => dto::PurgeNoticeReasonDto::RejournalDropped,
        PurgeNoticeReason::DroppedAfterFailures => dto::PurgeNoticeReasonDto::DroppedAfterFailures,
        PurgeNoticeReason::GroupBelowTwoMembers => dto::PurgeNoticeReasonDto::GroupBelowTwoMembers,
        PurgeNoticeReason::MediaCollectionSkipped => {
            dto::PurgeNoticeReasonDto::MediaCollectionSkipped
        }
        PurgeNoticeReason::NotSynced => dto::PurgeNoticeReasonDto::NotSynced,
        PurgeNoticeReason::ConflictCarried => dto::PurgeNoticeReasonDto::ConflictCarried,
    }
}
