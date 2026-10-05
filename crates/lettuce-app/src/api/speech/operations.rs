use lettuce_contracts::{ApiError, ApiErrorCode};
use lettuce_database::{ApiOperationError, ApiOperationTransaction};
use serde::{Serialize, de::DeserializeOwned};

use crate::api::ApiContext;
use crate::api::error::{api_error, invalid_field};

#[derive(Debug)]
struct OperationFailure(ApiError);

impl From<ApiOperationError> for OperationFailure {
    fn from(error: ApiOperationError) -> Self {
        Self(api_error(match error {
            ApiOperationError::Conflict => ApiErrorCode::Conflict,
            ApiOperationError::InvalidData | ApiOperationError::Storage => ApiErrorCode::Internal,
        }, error.to_string()))
    }
}

pub(super) fn validate_key(key: &str) -> Result<(), ApiError> {
    if key.trim().is_empty() || key.trim() != key || key.len() > 1_024 {
        return Err(invalid_field("client_operation_id", "the operation key is invalid"));
    }
    Ok(())
}

pub(super) fn digest(value: &impl Serialize) -> Result<String, ApiError> {
    let bytes = zeroize::Zeroizing::new(serde_json::to_vec(value)
        .map_err(|_| api_error(ApiErrorCode::Internal, "the operation request could not be encoded"))?);
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

pub(super) fn replay<T: DeserializeOwned>(context: &ApiContext, command: &str, key: &str, digest: &str) -> Result<Option<T>, ApiError> {
    let receipt = context.backend().database().lookup_api_operation(command, key)
        .map_err(|error| OperationFailure::from(error).0)?;
    receipt.map(|receipt| {
        if receipt.request_digest != digest {
            return Err(api_error(ApiErrorCode::Conflict, "the operation key was already used for a different request"));
        }
        serde_json::from_value(receipt.result).map_err(|_| api_error(ApiErrorCode::Internal, "the operation receipt is invalid"))
    }).transpose()
}

pub(super) fn commit<T: Serialize + DeserializeOwned>(context: &ApiContext, command: &str, key: &str, digest: &str,
    apply: impl FnOnce(&ApiOperationTransaction<'_, '_>) -> Result<T, ApiError>) -> Result<T, ApiError> {
    context.backend().database().commit_api_operation(command, key, digest, context.now(),
        |transaction| apply(transaction).map_err(OperationFailure)).map_err(|failure| failure.0)
}

pub(super) fn learning_error(error: lettuce_speech::AsrLearningRepositoryError) -> ApiError {
    use crate::api::error::IntoApiError;
    lettuce_speech::AsrLearningError::Repository(error).into_api_error()
}
