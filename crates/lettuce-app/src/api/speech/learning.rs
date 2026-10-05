//! The authored ASR vocabulary, corrections and audio examples.
use lettuce_contracts::{self as dto, ApiError};
use crate::api::ApiContext;
use crate::api::error::{IntoApiError, parse_id};

fn vocabulary_view(value: lettuce_speech::AsrVocabularyTerm) -> dto::AsrVocabularyView {
    dto::AsrVocabularyView {
        id: value.id.to_string(),
        term: value.term,
        language: value.language,
        category: value.category,
        scope: value.scope,
        priority: value.priority,
        use_count: value.use_count,
        created_at: value.created_at.get(),
        updated_at: value.updated_at.get(),
    }
}

fn correction_view(value: lettuce_speech::AsrCorrectionRule) -> dto::AsrCorrectionView {
    dto::AsrCorrectionView {
        id: value.id.to_string(),
        wrong: value.wrong,
        correct: value.correct,
        language: value.language,
        scope: value.scope,
        confidence: value.confidence,
        use_count: value.use_count,
        accepted_count: value.accepted_count,
        rejected_count: value.rejected_count,
        seen_count: value.seen_count,
        last_seen_at: value.last_seen_at.map(|at| at.get()),
        user_approved: value.user_approved,
        created_at: value.created_at.get(),
        updated_at: value.updated_at.get(),
    }
}

fn suggestion_view(value: lettuce_speech::AsrLearnedSuggestion) -> dto::AsrSuggestionView {
    dto::AsrSuggestionView {
        wrong: value.wrong,
        correct: value.correct,
        language: value.language,
        scope: value.scope,
        confidence: value.confidence,
        accepted_count: value.accepted_count,
        rejected_count: value.rejected_count,
        seen_count: value.seen_count,
    }
}

fn ignored_view(value: lettuce_speech::AsrIgnoredSuggestion) -> dto::AsrIgnoredSuggestionView {
    dto::AsrIgnoredSuggestionView {
        id: value.id.to_string(),
        wrong: value.wrong,
        correct: value.correct,
        language: value.language,
        scope: value.scope,
        ignored_count: value.ignored_count,
        last_ignored_at: value.last_ignored_at.get(),
        created_at: value.created_at.get(),
        updated_at: value.updated_at.get(),
    }
}

fn example_view(value: lettuce_speech::AsrVoiceExample, context: &ApiContext) -> dto::AsrVoiceExampleView {
    dto::AsrVoiceExampleView {
        id: value.id.to_string(),
        audio: context.asset_ref(value.audio_asset_id),
        expected_text: value.expected_text,
        whisper_output: value.whisper_output,
        language: value.language,
        scope: value.scope,
        vocabulary_term_id: value.vocabulary_term_id.map(|id| id.to_string()),
        correction_id: value.correction_id.map(|id| id.to_string()),
        created_at: value.created_at.get(),
        updated_at: value.updated_at.get(),
    }
}

pub async fn asr_vocabulary_list(context: &ApiContext, request: dto::AsrLearningFilter) -> Result<Vec<dto::AsrVocabularyView>, ApiError> {
    context.blocking(move |context| context.backend().asr_learning()
        .list_vocabulary(request.language.as_deref(), &request.scopes)
        .map(|values| values.into_iter().map(vocabulary_view).collect()).map_err(IntoApiError::into_api_error)).await
}

pub async fn asr_corrections_list(context: &ApiContext, request: dto::AsrLearningFilter) -> Result<Vec<dto::AsrCorrectionView>, ApiError> {
    context.blocking(move |context| context.backend().asr_learning()
        .list_corrections(request.language.as_deref(), &request.scopes)
        .map(|values| values.into_iter().map(correction_view).collect()).map_err(IntoApiError::into_api_error)).await
}

pub async fn asr_ignored_suggestions_list(context: &ApiContext, request: dto::AsrLearningFilter) -> Result<Vec<dto::AsrIgnoredSuggestionView>, ApiError> {
    context.blocking(move |context| context.backend().asr_learning()
        .list_ignored_suggestions(request.language.as_deref(), &request.scopes)
        .map(|values| values.into_iter().map(ignored_view).collect()).map_err(IntoApiError::into_api_error)).await
}

pub async fn asr_voice_examples_list(context: &ApiContext, request: dto::AsrLearningFilter) -> Result<Vec<dto::AsrVoiceExampleView>, ApiError> {
    context.blocking(move |context| context.backend().asr_learning()
        .list_voice_examples(request.language.as_deref(), &request.scopes)
        .map(|values| values.into_iter().map(|value| example_view(value, context)).collect()).map_err(IntoApiError::into_api_error)).await
}

pub async fn asr_vocabulary_delete(context: &ApiContext, request: dto::AsrLearningItemRequest) -> Result<(), ApiError> {
    let id = parse_id(&request.id, "id")?;
    context.blocking(move |context| context.backend().asr_learning().delete_vocabulary(id).map_err(IntoApiError::into_api_error)).await
}

pub async fn asr_correction_delete(context: &ApiContext, request: dto::AsrLearningItemRequest) -> Result<(), ApiError> {
    let id = parse_id(&request.id, "id")?;
    context.blocking(move |context| context.backend().asr_learning().delete_correction(id).map_err(IntoApiError::into_api_error)).await
}

pub async fn asr_voice_example_delete(context: &ApiContext, request: dto::AsrLearningItemRequest) -> Result<(), ApiError> {
    let id = parse_id(&request.id, "id")?;
    context.blocking(move |context| context.backend().asr_learning().delete_voice_example(id).map_err(IntoApiError::into_api_error)).await
}

pub async fn asr_suggestions(context: &ApiContext, request: dto::AsrSuggestionsRequest) -> Result<Vec<dto::AsrSuggestionView>, ApiError> {
    context.blocking(move |context| context.backend().asr_learning()
        .suggest_corrections_from_edit(&request.before, &request.after, request.language.as_deref(), request.scope.as_deref())
        .map(|values| values.into_iter().map(suggestion_view).collect()).map_err(IntoApiError::into_api_error)).await
}

pub async fn asr_learning_export(context: &ApiContext, request: dto::AsrLearningExportRequest) -> Result<(), ApiError> {
    let uri = request.target.uri.trim().to_owned();
    if uri.is_empty() { return Err(crate::api::error::invalid_field("target", "target is empty")); }
    context.blocking(move |context| {
        let document = context.backend().asr_learning_transfer()
            .export(request.filter.language.as_deref(), &request.filter.scopes).map_err(IntoApiError::into_api_error)?;
        let mut writer = context.files().create(&uri).map_err(IntoApiError::into_api_error)?;
        serde_json::to_writer_pretty(&mut writer, &document)
            .map_err(|error| crate::api::error::api_error(dto::ApiErrorCode::Unavailable, error.to_string()))?;
        writer.flush().map_err(|error| crate::api::error::api_error(dto::ApiErrorCode::Unavailable, error.to_string()))?;
        Ok(())
    }).await
}

pub async fn asr_voice_example_suggest(context: &ApiContext, request: dto::AsrLearningItemRequest) -> Result<Option<dto::AsrSuggestionView>, ApiError> {
    let id = parse_id(&request.id, "id")?;
    context.blocking(move |context| {
        let library = context.backend().asr_learning();
        let example = library.get_voice_example(id).map_err(IntoApiError::into_api_error)?
            .ok_or_else(|| crate::api::error::api_error(dto::ApiErrorCode::NotFound, "the voice example was not found"))?;
        library.suggest_voice_example_correction(&example).map(|suggestion| suggestion.map(suggestion_view))
            .map_err(IntoApiError::into_api_error)
    }).await
}
