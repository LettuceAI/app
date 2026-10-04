use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_speech::{SynthesisRequest, TtsConfigurationRepository, TtsOutputPolicy};
use lettuce_types::{AssetId, AudioProviderId, RequestId, TimestampMillis};

use crate::api::ApiContext;
use crate::api::error::{IntoApiError, api_error, parse_id};

const PREVIEW_LIFETIME_MS: i64 = 24 * 60 * 60 * 1000;

pub async fn tts_synthesize(
    context: &ApiContext,
    request: dto::TtsSynthesizeRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let id: RequestId = parse_id(&request.request_id, "request_id")?;
    let provider_id: AudioProviderId = parse_id(&request.provider_id, "provider_id")?;
    let digest = serde_json::to_string(&request)
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
    context.blocking(move |context| {
        let database = context.backend().database();
        let key = format!("tts_synthesize:{id}");
        if let Some(prior) = database.job_operation(&key)
            .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
        {
            if prior.request_digest != digest {
                return Err(api_error(ApiErrorCode::Conflict, "another synthesis uses this request id"));
            }
            return Ok(dto::JobAccepted { job_id: prior.job_id.to_string() });
        }
        let provider = database.get_audio_provider(provider_id)
            .map_err(IntoApiError::into_api_error)?
            .ok_or_else(|| api_error(ApiErrorCode::NotFound, "the audio provider was not found"))?;
        let now = context.now();
        let synthesis = SynthesisRequest {
            id, provider, model_id: request.model_id, voice_id: request.voice_id,
            prompt: request.prompt, text: request.text, output_asset_id: AssetId::new(),
            output_policy: if request.retained { TtsOutputPolicy::Retained } else {
                TtsOutputPolicy::Preview { expires_at: TimestampMillis::new(now.get().saturating_add(PREVIEW_LIFETIME_MS)) }
            },
            created_at: now,
        };
        let spec = crate::speech::tts_synthesis::synthesis_job_spec(&synthesis)
            .map_err(IntoApiError::into_api_error)?;
        let job = database.admit_speech_synthesis(spec, &key, &digest, synthesis)
            .map_err(IntoApiError::into_api_error)?;
        context.jobs().wake();
        Ok(dto::JobAccepted { job_id: job.id.to_string() })
    }).await
}
