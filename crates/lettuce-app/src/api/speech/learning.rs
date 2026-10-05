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
        .map(|values| values.into_iter().filter(|rule| request.user_approved_only != Some(true) || rule.user_approved).map(correction_view).collect()).map_err(IntoApiError::into_api_error)).await
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

pub async fn asr_vocabulary_save(context: &ApiContext, request: dto::AsrVocabularySaveRequest) -> Result<dto::AsrVocabularyView, ApiError> {
    super::operations::validate_key(&request.client_operation_id)?;
    let digest = super::operations::digest(&request)?;
    context.blocking(move |context| {
        let record = super::operations::commit(context, "asr_vocabulary_save", &request.client_operation_id, &digest, |transaction| {
        let mut term = lettuce_speech::AsrVocabularyTerm::new(request.term, request.language.as_deref(), request.category.as_deref(), request.scope.as_deref(), request.priority.unwrap_or(50), context.now()).map_err(IntoApiError::into_api_error)?;
        if let Some(id) = request.id {
            term.id = parse_id(&id, "id")?;
            let existing = transaction.get_vocabulary(term.id).map_err(super::operations::learning_error)?
                .ok_or_else(|| crate::api::error::api_error(dto::ApiErrorCode::NotFound, "the vocabulary term was not found"))?;
            term.created_at = existing.created_at;
            term.updated_at = term.updated_at.max(existing.updated_at);
        }
        term.use_count = request.use_count.unwrap_or(0);
        transaction.save_vocabulary(term).map(|value| super::operations::StoredRecord::new(value.id, None)).map_err(super::operations::learning_error)
    })?;
        vocabulary_result(context, "asr_vocabulary_save", record)
    }).await
}

pub async fn asr_correction_save(context: &ApiContext, request: dto::AsrCorrectionSaveRequest) -> Result<dto::AsrCorrectionView, ApiError> {
    super::operations::validate_key(&request.client_operation_id)?;
    let digest = super::operations::digest(&request)?;
    context.blocking(move |context| {
        let record = super::operations::commit(context, "asr_correction_save", &request.client_operation_id, &digest, |transaction| {
        let draft = lettuce_speech::AsrCorrectionDraft {
            id: request.id.map(|id| parse_id(&id, "id")).transpose()?, wrong: request.wrong, correct: request.correct,
            language: request.language, scope: request.scope, confidence: request.confidence,
            use_count: request.use_count, accepted_count: request.accepted_count, rejected_count: request.rejected_count,
            seen_count: request.seen_count, last_seen_at: request.last_seen_at.map(lettuce_types::TimestampMillis::new), user_approved: request.user_approved,
        };
        transaction.save_correction_draft(draft, context.now()).map(|value| super::operations::StoredRecord::new(value.id, None)).map_err(super::operations::learning_error)
    })?;
        correction_result(context, "asr_correction_save", record)
    }).await
}

fn authored_suggestion(value: dto::AsrSuggestionView, now: lettuce_types::TimestampMillis) -> Result<lettuce_speech::AsrLearnedSuggestion, ApiError> {
    let rule = lettuce_speech::AsrCorrectionDraft {
        wrong: value.wrong, correct: value.correct, language: value.language, scope: Some(value.scope),
        confidence: Some(value.confidence), ..Default::default()
    }.materialize(None, false, now).map_err(IntoApiError::into_api_error)?;
    let suggestion = lettuce_speech::AsrLearnedSuggestion {
        wrong: rule.wrong, normalized_wrong: rule.normalized_wrong, correct: rule.correct, normalized_correct: rule.normalized_correct,
        language: rule.language, scope: rule.scope, confidence: rule.confidence,
        accepted_count: value.accepted_count, rejected_count: value.rejected_count, seen_count: value.seen_count,
    };
    suggestion.validate().map_err(IntoApiError::into_api_error)?;
    Ok(suggestion)
}

pub async fn asr_suggestion_approve(context: &ApiContext, request: dto::AsrSuggestionWriteRequest) -> Result<dto::AsrCorrectionView, ApiError> {
    super::operations::validate_key(&request.client_operation_id)?;
    let digest = super::operations::digest(&request)?;
    context.blocking(move |context| {
        let record = super::operations::commit(context, "asr_suggestion_approve", &request.client_operation_id, &digest, |transaction| {
        let suggestion = authored_suggestion(request.suggestion, context.now())?;
        transaction.save_correction_draft(lettuce_speech::AsrCorrectionDraft {
            wrong: suggestion.wrong, correct: suggestion.correct, language: suggestion.language,
            scope: Some(suggestion.scope), confidence: Some(suggestion.confidence), user_approved: Some(true), ..Default::default()
        }, context.now()).map(|value| super::operations::StoredRecord::new(value.id, None)).map_err(super::operations::learning_error)
    })?;
        correction_result(context, "asr_suggestion_approve", record)
    }).await
}

pub async fn asr_suggestion_ignore(context: &ApiContext, request: dto::AsrSuggestionWriteRequest) -> Result<dto::AsrIgnoredSuggestionView, ApiError> {
    super::operations::validate_key(&request.client_operation_id)?;
    let digest = super::operations::digest(&request)?;
    context.blocking(move |context| {
        let record = super::operations::commit(context, "asr_suggestion_ignore", &request.client_operation_id, &digest, |transaction| {
        let suggestion = authored_suggestion(request.suggestion, context.now())?;
        transaction.ignore_suggestion(suggestion, context.now()).map(|value| super::operations::StoredRecord::new(value.id, None)).map_err(super::operations::learning_error)
    })?;
        ignored_result(context, "asr_suggestion_ignore", record)
    }).await
}

pub async fn asr_voice_example_save(context: &ApiContext, request: dto::AsrVoiceExampleSaveRequest) -> Result<dto::AsrVoiceExampleView, ApiError> {
    super::operations::validate_key(&request.client_operation_id)?;
    let digest = super::operations::digest(&request)?;
    context.blocking(move |context| {
        let record = super::operations::commit(context, "asr_voice_example_save", &request.client_operation_id, &digest, |transaction| {
        let mut example = lettuce_speech::AsrVoiceExample::new(parse_id(&request.audio_asset_id, "audio_asset_id")?, request.expected_text, request.whisper_output, request.language.as_deref(), request.scope.as_deref(), context.now()).map_err(IntoApiError::into_api_error)?;
        if let Some(id) = request.id {
            example.id = parse_id(&id, "id")?;
            let existing = transaction.get_voice_example(example.id).map_err(super::operations::learning_error)?
                .ok_or_else(|| crate::api::error::api_error(dto::ApiErrorCode::NotFound, "the voice example was not found"))?;
            example.created_at = existing.created_at;
            example.updated_at = example.updated_at.max(existing.updated_at);
        }
        example.vocabulary_term_id = request.vocabulary_term_id.map(|id| parse_id(&id, "vocabulary_term_id")).transpose()?;
        example.correction_id = request.correction_id.map(|id| parse_id(&id, "correction_id")).transpose()?;
        transaction.save_voice_example(example).map(|value| super::operations::StoredRecord::new(value.id, None)).map_err(super::operations::learning_error)
    })?;
        example_result(context, "asr_voice_example_save", record)
    }).await
}

pub async fn asr_learning_import(context: &ApiContext, request: dto::AsrLearningImportRequest) -> Result<dto::AsrLearningImportView, ApiError> {
    use std::io::Read;
    super::operations::validate_key(&request.client_operation_id)?;
    context.blocking(move |context| {
        let _import = context.speech_state().learning_import();
        let uri = request.source.uri.trim();
        if uri.is_empty() { return Err(crate::api::error::invalid_field("source", "source is empty")); }
        let reader = context.files().open(uri).map_err(IntoApiError::into_api_error)?;
        let mut bytes = Vec::new();
        reader.take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes)
            .map_err(|_| crate::api::error::api_error(dto::ApiErrorCode::Unavailable, "the library could not be read"))?;
        if bytes.len() > 16 * 1024 * 1024 { return Err(crate::api::error::invalid_field("source", "the library is too large")); }
        let digest = blake3::hash(&bytes).to_hex().to_string();
        if let Some(result) = super::operations::replay(context, "asr_learning_import", &request.client_operation_id, &digest)? { return Ok(result); }
        let document: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| crate::api::error::invalid_field("source", "the library document is invalid"))?;
        let batch = if document.get("version").and_then(serde_json::Value::as_u64) == Some(2) {
            let media = context.media().ok_or_else(|| crate::api::error::api_error(dto::ApiErrorCode::Unavailable, "no media store is open"))?;
            let path = crate::api::local_models::local_path(&request.source, "source")?;
            let legacy = serde_json::from_value(document).map_err(|_| crate::api::error::invalid_field("source", "the legacy library is invalid"))?;
            context.backend().legacy_asr_learning_transfer(media).prepare_import(
                std::path::Path::new(&path).parent().unwrap_or(std::path::Path::new(".")), legacy, context.now())
                .map_err(|error| crate::api::error::invalid_field("source", error.to_string()))?
        } else {
            let document = serde_json::from_value(document).map_err(|_| crate::api::error::invalid_field("source", "the library document is invalid"))?;
            context.backend().asr_learning_transfer().prepare_import(document).map_err(IntoApiError::into_api_error)?
        };
        super::operations::commit(context, "asr_learning_import", &request.client_operation_id, &digest, |transaction| {
            transaction.import_learning_batch(batch).map(|receipt| dto::AsrLearningImportView {
                vocabulary_count: receipt.vocabulary_count, correction_count: receipt.correction_count,
                ignored_suggestion_count: receipt.ignored_suggestion_count, voice_example_count: receipt.voice_example_count,
            }).map_err(super::operations::learning_error)
        })
    }).await
}

fn vocabulary_result(context: &ApiContext, command: &str, record: super::operations::StoredRecord) -> Result<dto::AsrVocabularyView, ApiError> {
    context.backend().asr_learning().get_vocabulary(parse_id(&record.id, "id")?).map_err(IntoApiError::into_api_error)?
        .map(vocabulary_view).ok_or_else(|| super::operations::applied_deleted(command, &record.id))
}
fn correction_result(context: &ApiContext, command: &str, record: super::operations::StoredRecord) -> Result<dto::AsrCorrectionView, ApiError> {
    context.backend().asr_learning().get_correction(parse_id(&record.id, "id")?).map_err(IntoApiError::into_api_error)?
        .map(correction_view).ok_or_else(|| super::operations::applied_deleted(command, &record.id))
}
fn ignored_result(context: &ApiContext, command: &str, record: super::operations::StoredRecord) -> Result<dto::AsrIgnoredSuggestionView, ApiError> {
    context.backend().asr_learning().get_ignored_suggestion(parse_id(&record.id, "id")?).map_err(IntoApiError::into_api_error)?
        .map(ignored_view)
        .ok_or_else(|| super::operations::applied_deleted(command, &record.id))
}
fn example_result(context: &ApiContext, command: &str, record: super::operations::StoredRecord) -> Result<dto::AsrVoiceExampleView, ApiError> {
    context.backend().asr_learning().get_voice_example(parse_id(&record.id, "id")?).map_err(IntoApiError::into_api_error)?
        .map(|value| example_view(value, context)).ok_or_else(|| super::operations::applied_deleted(command, &record.id))
}
