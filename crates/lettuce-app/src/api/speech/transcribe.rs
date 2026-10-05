use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_jobs::{JobKind, JobSnapshot};
use lettuce_media::{AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, RetentionClass};
use lettuce_speech::{
    AsrModelDescriptor, TranscriptionOptions, TranscriptionRepository, TranscriptionRequest,
    TranscriptionState,
};
use lettuce_types::{AssetId, RequestId, TimestampMillis};

use super::whisper::{engine_options, resolve_model};
use crate::api::ApiContext;
use crate::api::error::{IntoApiError, api_error, invalid_field, parse_id};

/// How long a recording or a picked file lives when nothing keeps it.
const RECORDING_LIFETIME_MS: i64 = 24 * 60 * 60 * 1000;

/// Stores a picked audio file as a temporary asset.
pub(super) fn ingest_picked_audio(context: &ApiContext, uri: &str) -> Result<AssetId, ApiError> {
    let media = context
        .media()
        .ok_or_else(|| api_error(ApiErrorCode::Unavailable, "no media store is open"))?;
    if let Some(id) = context.asset_id_from_url(uri)? {
        // Retry the existing managed asset, without importing a duplicate.
        media.open_ready(id).map_err(IntoApiError::into_api_error)?;
        return Ok(id);
    }
    let reader = context
        .files()
        .open(uri)
        .map_err(IntoApiError::into_api_error)?;
    let expires_at =
        TimestampMillis::new(context.now().get().saturating_add(RECORDING_LIFETIME_MS));
    media
        .ingest(
            reader,
            IngestRequest::new(
                AssetKind::OtherAudio,
                AssetOrigin::Upload,
                RetentionClass::Temporary { expires_at },
                AssetProvenanceV1::default(),
            ),
        )
        .map(|ingested| ingested.asset.id)
        .map_err(|error| match error {
            lettuce_media::MediaStoreError::UnsupportedFormat
            | lettuce_media::MediaStoreError::KindMismatch
            | lettuce_media::MediaStoreError::InvalidHeader
            | lettuce_media::MediaStoreError::EmptyInput
            | lettuce_media::MediaStoreError::InputTooLarge => {
                invalid_field("source", error.to_string())
            }
            error => error.into_api_error(),
        })
}

/// Admits a transcription of `audio` and wakes the runner.
pub(super) fn admit_transcription(
    context: &ApiContext,
    request_id: RequestId,
    audio: AssetId,
    model: AsrModelDescriptor,
    options: TranscriptionOptions,
) -> Result<dto::JobAccepted, ApiError> {
    let request = TranscriptionRequest {
        id: request_id,
        audio_asset_id: Some(audio),
        model,
        options,
        created_at: context.now(),
    };
    let digest = serde_json::to_string(&(request_id, audio, &request.model, &request.options))
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
    let job = atomic_transcription(context, request, &digest)?;
    context.jobs().wake();
    Ok(dto::JobAccepted {
        job_id: job.id.to_string(),
    })
}

fn atomic_transcription(
    context: &ApiContext,
    request: TranscriptionRequest,
    digest: &str,
) -> Result<JobSnapshot, ApiError> {
    let spec = crate::speech::speech_transcription::transcription_job_spec(&request)
        .map_err(IntoApiError::into_api_error)?;
    let key = format!("transcribe_file:{}", request.id);
    context
        .backend()
        .database()
        .admit_speech_transcription(
            spec,
            &key,
            digest,
            &serde_json::json!({"kind": "speech_transcribe"}),
            request,
        )
        .map_err(IntoApiError::into_api_error)
}

/// Transcribes a picked audio file as a job. The file is read once per
/// request id; a replayed request returns its job.
pub async fn transcribe_file(
    context: &ApiContext,
    request: dto::TranscribeFileRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let request_id: RequestId = parse_id(&request.request_id, "request_id")?;
    let digest = serde_json::to_string(&request)
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
    let uri = request.source.uri.trim().to_owned();
    if uri.is_empty() {
        return Err(invalid_field("source", "source is empty"));
    }
    context
        .blocking(move |context| {
            let key = format!("transcribe_file:{request_id}");
            if let Some(prior) = context
                .backend()
                .database()
                .job_operation(&key)
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
            {
                if prior.request_digest != digest {
                    return Err(api_error(
                        ApiErrorCode::Conflict,
                        "another transcription uses this request id",
                    ));
                }
                return Ok(dto::JobAccepted {
                    job_id: prior.job_id.to_string(),
                });
            }
            let model = resolve_model(context, request.model_id.as_deref())?;
            let audio = ingest_picked_audio(context, &uri)?;
            let job = atomic_transcription(
                context,
                TranscriptionRequest {
                    id: request_id,
                    audio_asset_id: Some(audio),
                    model,
                    options: engine_options(&request.options),
                    created_at: context.now(),
                },
                &digest,
            )?;
            context.jobs().wake();
            Ok(dto::JobAccepted {
                job_id: job.id.to_string(),
            })
        })
        .await
}

/// What a succeeded transcription job produced.
pub(crate) fn transcription_view(
    context: &ApiContext,
    job: &JobSnapshot,
) -> Result<Option<dto::TranscriptionView>, ApiError> {
    if job.kind != JobKind::SpeechTranscribe || job.state != lettuce_jobs::JobState::Succeeded {
        return Ok(None);
    }
    let record = TranscriptionRepository::get(context.backend().database(), job.id)
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
    let TranscriptionState::Succeeded { result } = record.state else {
        return Ok(None);
    };
    Ok(Some(dto::TranscriptionView {
        request_id: result.request_id.to_string(),
        audio: result.audio_asset_id.map(|id| context.asset_ref(id)),
        model_id: result.model.id.as_str().to_owned(),
        raw_text: result.raw_text,
        text: result.corrected_text,
        detected_language: result.detected_language,
        segments: result
            .segments
            .into_iter()
            .map(|segment| dto::TranscriptSegment {
                start_ms: segment.start_ms,
                end_ms: segment.end_ms,
                text: segment.text,
            })
            .collect(),
        applied_corrections: result
            .applied_corrections
            .into_iter()
            .map(|correction| dto::AppliedCorrectionView {
                correction_id: correction.correction_id,
                wrong: correction.wrong,
                correct: correction.correct,
                matched_text: correction.matched_text,
            })
            .collect(),
    }))
}

/// Collects released inputs through the normal media entry point, with
/// protection for objects cataloged by another database on production hosts.
pub(crate) fn collect_recordings(context: &ApiContext) -> Result<(), ApiError> {
    let Some(media) = context.media() else {
        return Ok(());
    };
    if let Some(files) = context.database_files() {
        crate::collect_media_garbage(
            context.backend().database(),
            &crate::MediaGarbageScope {
                store: media,
                location: &files.location,
                open_database: &files.active,
            },
            context.now(),
        )
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
    } else {
        media
            .remove_released_objects(|| {
                context
                    .backend()
                    .database()
                    .collect_media_garbage(context.now())
                    .map_err(|_| lettuce_media::MediaStoreError::CatalogFailure)
            })
            .map_err(IntoApiError::into_api_error)?;
    }
    Ok(())
}
