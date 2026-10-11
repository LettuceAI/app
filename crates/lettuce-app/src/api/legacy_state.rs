use std::path::Path;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_transfer::LegacyDatabasePreflightError;

use super::{ApiContext, error::api_error};

pub async fn legacy_state(context: &ApiContext) -> Result<dto::LegacyState, ApiError> {
    context.blocking(detect).await
}

fn error(code: ApiErrorCode, reason: dto::LegacyDatabaseFailure) -> ApiError {
    let mut error = api_error(code, "legacy storage could not be read");
    error.details = Some(dto::ApiErrorDetails::LegacyDatabase { reason });
    error
}

pub(super) fn failure_state(failure: LegacyDatabasePreflightError) -> Result<dto::LegacyState, ApiError> {
    match failure {
        LegacyDatabasePreflightError::UnsupportedVersion { found, minimum, .. } if found < i64::from(minimum) => Ok(dto::LegacyState::RequiresLegacyUpgrade { reason: dto::LegacyUpgradeReason::Schema { found }, required_app_version: "2.2.5".into() }),
        LegacyDatabasePreflightError::UnsupportedVersion { found, .. } => Err(error(ApiErrorCode::Unsupported, dto::LegacyDatabaseFailure::UnsupportedSchema { found })),
        LegacyDatabasePreflightError::Unavailable => Err(error(ApiErrorCode::Unavailable, dto::LegacyDatabaseFailure::Unavailable)),
        _ => Err(error(ApiErrorCode::Malformed, dto::LegacyDatabaseFailure::InvalidSchema)),
    }
}

fn is_file(path: &Path) -> Result<bool, ApiError> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(failure) if failure.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(error(ApiErrorCode::Unavailable, dto::LegacyDatabaseFailure::Unavailable)),
    }
}

pub(super) fn detect(context: &ApiContext) -> Result<dto::LegacyState, ApiError> {
    Ok(detect_storage(context).unwrap_or_else(|error| dto::LegacyState::Failed { error }))
}

fn detect_storage(context: &ApiContext) -> Result<dto::LegacyState, ApiError> {
    let Some(folder) = context.app_folder() else { return Ok(dto::LegacyState::Absent); };
    let root = folder.join("lettuce");
    let path = root.join("app.db");
    if is_file(&path)? {
        return match context.backend().preflight_legacy_database(&path) {
            Ok(inventory) => Ok(dto::LegacyState::Ready { schema_version: inventory.schema_version }),
            Err(failure) => failure_state(failure),
        };
    }
    for name in ["settings.bin", "characters.bin", "personas.bin", "sessions/index.bin"] {
        if is_file(&root.join(name))? {
            return Ok(dto::LegacyState::RequiresLegacyUpgrade { reason: dto::LegacyUpgradeReason::BinStorage, required_app_version: "2.2.5".into() });
        }
    }
    match std::fs::read_dir(root.join("sessions")) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.map_err(|_| error(ApiErrorCode::Unavailable, dto::LegacyDatabaseFailure::Unavailable))?;
                if entry.path().extension().is_some_and(|extension| extension == "bin") && is_file(&entry.path())? {
                    return Ok(dto::LegacyState::RequiresLegacyUpgrade { reason: dto::LegacyUpgradeReason::BinStorage, required_app_version: "2.2.5".into() });
                }
            }
        }
        Err(failure) if failure.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(error(ApiErrorCode::Unavailable, dto::LegacyDatabaseFailure::Unavailable)),
    }
    Ok(dto::LegacyState::Absent)
}
