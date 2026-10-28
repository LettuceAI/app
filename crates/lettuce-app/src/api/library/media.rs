use crate::api::{
    ApiContext,
    error::{IntoApiError, api_error, invalid_field, parse_id},
    lorebooks::Failure,
};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_media::{
    AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, IngestedMedia, MediaStoreError,
    RetentionClass,
};
use lettuce_types::AssetId;

fn input_error(error: MediaStoreError) -> ApiError {
    match error {
        MediaStoreError::UnsupportedFormat
        | MediaStoreError::KindMismatch
        | MediaStoreError::InvalidHeader { .. }
        | MediaStoreError::EmptyInput
        | MediaStoreError::InputTooLarge
        | MediaStoreError::PixelLimitExceeded
        | MediaStoreError::InvalidDimensions
        | MediaStoreError::MimeMismatch => invalid_field("input", error.to_string()),
        error => error.into_api_error(),
    }
}

fn commit_media<T: serde::Serialize + serde::de::DeserializeOwned>(
    context: &ApiContext,
    operation: &str,
    key: &str,
    digest: &str,
    input: dto::LibraryMediaInput,
    kind: AssetKind,
    apply: impl FnOnce(
        &lettuce_database::ApiOperationTransaction<'_, '_>,
        Option<AssetId>,
    ) -> Result<T, ApiError>,
) -> Result<T, ApiError> {
    if context.shutdown_token().is_cancelled() {
        return Err(api_error(
            ApiErrorCode::Cancelled,
            "shutdown interrupted media change",
        ));
    }
    let database = context.backend().database();
    if let Some(receipt) = database
        .lookup_api_operation(operation, key)
        .map_err(|error| Failure::from(error).0)?
    {
        if receipt.request_digest != digest {
            return Err(api_error(
                ApiErrorCode::Conflict,
                "the operation key was used for another request",
            ));
        }
        return serde_json::from_value(receipt.result)
            .map_err(|_| api_error(ApiErrorCode::Internal, "invalid library media receipt"));
    }
    let commit = |upload: Option<IngestedMedia>, asset: Option<AssetId>| {
        database
            .commit_api_operation(operation, key, digest, context.now(), |scope| {
                let asset = match upload {
                    Some(upload) => Some(
                        scope
                            .install_library_upload(upload)
                            .map_err(|error| Failure(input_error(error)))?,
                    ),
                    None => asset,
                };
                if let Some(id) = asset {
                    scope
                        .require_library_image(id)
                        .map_err(|error| Failure(input_error(error)))?;
                }
                apply(scope, asset).map_err(Failure)
            })
            .map_err(|failure: Failure| failure.0)
    };
    match input {
        dto::LibraryMediaInput::None => commit(None, None),
        dto::LibraryMediaInput::Asset { asset_id } => {
            let id = parse_id(&asset_id, "asset_id")?;
            let store = context
                .media()
                .ok_or_else(|| api_error(ApiErrorCode::Unavailable, "no media store is open"))?;
            store.open_ready(id).map_err(input_error)?;
            commit(None, Some(id))
        }
        dto::LibraryMediaInput::Source { source } => {
            let uri = source.uri.trim();
            if uri.is_empty() {
                return Err(invalid_field("source", "source is empty"));
            }
            let store = context
                .media()
                .ok_or_else(|| api_error(ApiErrorCode::Unavailable, "no media store is open"))?;
            let reader = context
                .files()
                .open(uri)
                .map_err(IntoApiError::into_api_error)?;
            store
                .ingest_with_commit(
                    reader,
                    IngestRequest::new(
                        kind,
                        AssetOrigin::Upload,
                        RetentionClass::Persistent,
                        AssetProvenanceV1::default(),
                    ),
                    |upload| commit(Some(upload), None),
                )
                .map_err(input_error)?
        }
    }
}

pub async fn character_media_set(
    context: &ApiContext,
    request: dto::CharacterMediaSetRequest,
) -> Result<dto::CharacterView, ApiError> {
    let id = parse_id(&request.character_id, "character_id")?;
    let expected = crate::api::lorebooks::revision(request.expected_revision, "expected_revision")?;
    let digest = crate::api::jobs::local::digest(&request)?;
    let slot = match request.slot {
        dto::LibraryCharacterMediaSlot::AvatarOriginal => {
            lettuce_characters::CharacterMediaSlot::AvatarOriginal
        }
        dto::LibraryCharacterMediaSlot::Banner => lettuce_characters::CharacterMediaSlot::Banner,
        dto::LibraryCharacterMediaSlot::Background => {
            lettuce_characters::CharacterMediaSlot::Background
        }
        dto::LibraryCharacterMediaSlot::DesignReference => {
            lettuce_characters::CharacterMediaSlot::DesignReference
        }
    };
    let kind = match slot {
        lettuce_characters::CharacterMediaSlot::AvatarOriginal => AssetKind::AvatarOriginal,
        lettuce_characters::CharacterMediaSlot::Background => AssetKind::BackgroundImage,
        _ => AssetKind::OtherImage,
    };
    context
        .blocking(move |context| {
            commit_media(
                context,
                "character_media_set",
                &request.client_operation_id,
                &digest,
                request.input,
                kind,
                |scope, asset| {
                    let details = scope
                        .set_library_character_media(
                            id,
                            expected,
                            slot,
                            request.ordinal,
                            asset,
                            context.now(),
                        )
                        .map_err(IntoApiError::into_api_error)?;
                    super::characters::view(context, details)
                },
            )
        })
        .await
}

pub async fn persona_media_set(
    context: &ApiContext,
    request: dto::PersonaMediaSetRequest,
) -> Result<dto::PersonaView, ApiError> {
    let id = parse_id(&request.persona_id, "persona_id")?;
    let expected = crate::api::lorebooks::revision(request.expected_revision, "expected_revision")?;
    let digest = crate::api::jobs::local::digest(&request)?;
    let slot = match request.slot {
        dto::PersonaMediaSlot::Avatar => lettuce_characters::PersonaMediaSlot::Avatar,
        dto::PersonaMediaSlot::DesignReference => {
            lettuce_characters::PersonaMediaSlot::DesignReference
        }
    };
    let kind = if slot == lettuce_characters::PersonaMediaSlot::Avatar {
        AssetKind::AvatarOriginal
    } else {
        AssetKind::OtherImage
    };
    context
        .blocking(move |context| {
            commit_media(
                context,
                "persona_media_set",
                &request.client_operation_id,
                &digest,
                request.input,
                kind,
                |scope, asset| {
                    let persona = scope
                        .set_library_persona_media(
                            id,
                            expected,
                            slot,
                            request.ordinal,
                            asset,
                            context.now(),
                        )
                        .map_err(IntoApiError::into_api_error)?;
                    Ok(super::personas::view(context, persona))
                },
            )
        })
        .await
}
