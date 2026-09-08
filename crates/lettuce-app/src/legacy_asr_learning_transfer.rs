use std::{
    collections::BTreeMap,
    fs::File,
    path::{Component, Path, PathBuf},
};

use chrono::{DateTime, NaiveDateTime};
use lettuce_media::{
    AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, IngestedMedia, LocalMediaBlobStore,
    MediaAssetRepository, MediaBlobRepository, MediaStoreError, RetentionClass,
};
use lettuce_speech::{
    AsrCorrectionRule, AsrIgnoredSuggestion, AsrLearningError, AsrLearningImportReceipt,
    AsrLearningRepository, AsrVocabularyTerm, AsrVoiceExample,
};
use lettuce_transfer::{
    ASR_LEARNING_DOCUMENT_VERSION, AsrLearningAudioAsset, AsrLearningDocument,
    LegacyAsrCorrectionRecord, LegacyAsrIgnoredSuggestionRecord, LegacyAsrLearningDocument,
    LegacyAsrVocabularyRecord, LegacyAsrVoiceExampleRecord,
};
use lettuce_types::{AsrCorrectionId, AsrVocabularyTermId, TimestampMillis};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LegacyAsrLearningTransferError {
    #[error("legacy ASR learning document is invalid")]
    InvalidDocument,
    #[error("legacy ASR voice audio path is unsafe")]
    UnsafeAudioPath,
    #[error("legacy ASR voice audio is unavailable")]
    AudioUnavailable,
    #[error("legacy ASR voice audio could not be read")]
    AudioRead,
    #[error("legacy ASR voice audio ingest failed: {0}")]
    Media(MediaStoreError),
    #[error("ASR learning import failed: {0}")]
    Learning(AsrLearningError),
}

#[derive(Debug)]
pub struct LegacyAsrLearningTransferCoordinator<'a, R: ?Sized, BR, AR> {
    repository: &'a R,
    media_store: &'a LocalMediaBlobStore<BR, AR>,
}

impl<'a, R, BR, AR> LegacyAsrLearningTransferCoordinator<'a, R, BR, AR>
where
    R: AsrLearningRepository + MediaAssetRepository + MediaBlobRepository + ?Sized,
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    #[must_use]
    pub const fn new(repository: &'a R, media_store: &'a LocalMediaBlobStore<BR, AR>) -> Self {
        Self {
            repository,
            media_store,
        }
    }

    pub fn import(
        &self,
        document_directory: impl AsRef<Path>,
        legacy: LegacyAsrLearningDocument,
        imported_at: TimestampMillis,
    ) -> Result<AsrLearningImportReceipt, LegacyAsrLearningTransferError> {
        if !legacy.within_bounds() {
            return Err(LegacyAsrLearningTransferError::InvalidDocument);
        }
        validate_source_record_ids(legacy.vocabulary.iter().map(|value| value.id))?;
        validate_source_record_ids(legacy.corrections.iter().map(|value| value.id))?;
        validate_source_record_ids(legacy.ignored_suggestions.iter().map(|value| value.id))?;
        validate_source_record_ids(legacy.voice_examples.iter().map(|value| value.id))?;
        let document_directory = std::fs::canonicalize(document_directory)
            .map_err(|_| LegacyAsrLearningTransferError::AudioUnavailable)?;
        if !document_directory.is_dir() {
            return Err(LegacyAsrLearningTransferError::AudioUnavailable);
        }
        let vocabulary = map_vocabulary(legacy.vocabulary, imported_at)?;
        let corrections = map_corrections(legacy.corrections, imported_at)?;
        let vocabulary_ids = source_vocabulary_ids(&vocabulary)?;
        let correction_ids = source_correction_ids(&corrections)?;
        let ignored_suggestions = map_ignored(legacy.ignored_suggestions, imported_at)?;
        let resolved_voice_examples = legacy
            .voice_examples
            .into_iter()
            .map(|source| {
                map_voice_example(
                    source.clone(),
                    lettuce_types::AssetId::new(),
                    &vocabulary_ids,
                    &correction_ids,
                    imported_at,
                )?;
                let path = resolve_audio_path(&document_directory, &source.audio_path)?;
                Ok((source, path))
            })
            .collect::<Result<Vec<_>, LegacyAsrLearningTransferError>>()?;
        let mut audio_assets = BTreeMap::<PathBuf, IngestedMedia>::new();
        let mut voice_examples = Vec::with_capacity(resolved_voice_examples.len());
        for (source, canonical) in resolved_voice_examples {
            let ingested = if let Some(existing) = audio_assets.get(&canonical) {
                existing.clone()
            } else {
                let file = File::open(&canonical)
                    .map_err(|_| LegacyAsrLearningTransferError::AudioRead)?;
                let ingested = self
                    .media_store
                    .ingest(
                        file,
                        IngestRequest::new(
                            AssetKind::OtherAudio,
                            AssetOrigin::Legacy,
                            RetentionClass::Library,
                            AssetProvenanceV1 {
                                source_label: Some("Legacy ASR learning import".to_owned()),
                                imported_format: Some("lettuceai-asr-v2".to_owned()),
                                ..AssetProvenanceV1::default()
                            },
                        ),
                    )
                    .map_err(LegacyAsrLearningTransferError::Media)?;
                audio_assets.insert(canonical, ingested.clone());
                ingested
            };
            voice_examples.push(map_voice_example(
                source,
                ingested.asset.id,
                &vocabulary_ids,
                &correction_ids,
                imported_at,
            )?);
        }
        let audio_assets = audio_assets
            .into_values()
            .map(|ingested| AsrLearningAudioAsset {
                asset_id: ingested.asset.id,
                kind: ingested.asset.kind,
                origin: ingested.asset.origin,
                provenance: ingested.asset.provenance,
                content_hash: ingested.blob.content_hash,
                byte_size: ingested.blob.byte_size,
                mime_type: ingested.blob.mime_type,
                duration_ms: ingested.blob.duration_ms,
            })
            .collect();
        crate::AsrLearningTransferCoordinator::new(self.repository)
            .import(AsrLearningDocument {
                version: ASR_LEARNING_DOCUMENT_VERSION,
                vocabulary: vocabulary.into_iter().map(|(_, value)| value).collect(),
                corrections: corrections.into_iter().map(|(_, value)| value).collect(),
                ignored_suggestions,
                voice_examples,
                audio_assets,
            })
            .map_err(LegacyAsrLearningTransferError::Learning)
    }
}

fn map_vocabulary(
    records: Vec<LegacyAsrVocabularyRecord>,
    imported_at: TimestampMillis,
) -> Result<Vec<(Option<i64>, AsrVocabularyTerm)>, LegacyAsrLearningTransferError> {
    records
        .into_iter()
        .map(|source| {
            let mut value = AsrVocabularyTerm::new(
                source.term,
                source.language.as_deref(),
                source.category.as_deref(),
                source.scope.as_deref(),
                source.priority.unwrap_or(50),
                imported_at,
            )
            .map_err(|_| LegacyAsrLearningTransferError::InvalidDocument)?;
            validate_normalized(source.normalized_term.as_deref(), &value.normalized_term)?;
            value.use_count = nonnegative(source.use_count.unwrap_or(0))?;
            let (created_at, updated_at) = timestamps(
                source.created_at.as_deref(),
                source.updated_at.as_deref(),
                imported_at,
            )?;
            value.created_at = created_at;
            value.updated_at = updated_at;
            value
                .validate()
                .map_err(|_| LegacyAsrLearningTransferError::InvalidDocument)?;
            Ok((source.id, value))
        })
        .collect()
}

fn map_corrections(
    records: Vec<LegacyAsrCorrectionRecord>,
    imported_at: TimestampMillis,
) -> Result<Vec<(Option<i64>, AsrCorrectionRule)>, LegacyAsrLearningTransferError> {
    records
        .into_iter()
        .map(|source| {
            let approved = source.user_approved.unwrap_or(false)
                || source.accepted_count.is_some_and(|count| count > 0);
            let mut value = AsrCorrectionRule::new(
                source.wrong,
                source.correct,
                source.language.as_deref(),
                source.scope.as_deref(),
                approved,
                imported_at,
            )
            .map_err(|_| LegacyAsrLearningTransferError::InvalidDocument)?;
            validate_normalized(source.normalized_wrong.as_deref(), &value.normalized_wrong)?;
            validate_normalized(
                source.normalized_correct.as_deref(),
                &value.normalized_correct,
            )?;
            let accepted_count = source.accepted_count.unwrap_or(i64::from(approved));
            value.confidence = source.confidence.unwrap_or(0.75);
            value.use_count = nonnegative(source.use_count.unwrap_or(1))?.max(1);
            value.accepted_count = nonnegative(accepted_count)?;
            value.rejected_count = nonnegative(source.rejected_count.unwrap_or(0))?;
            value.seen_count = nonnegative(source.seen_count.unwrap_or(accepted_count))?;
            value.last_seen_at = optional_timestamp(source.last_seen_at.as_deref())?;
            value.user_approved = approved;
            let (created_at, updated_at) = timestamps(
                source.created_at.as_deref(),
                source.updated_at.as_deref(),
                imported_at,
            )?;
            value.created_at = created_at;
            value.updated_at = updated_at;
            value
                .validate()
                .map_err(|_| LegacyAsrLearningTransferError::InvalidDocument)?;
            Ok((source.id, value))
        })
        .collect()
}

fn map_ignored(
    records: Vec<LegacyAsrIgnoredSuggestionRecord>,
    imported_at: TimestampMillis,
) -> Result<Vec<AsrIgnoredSuggestion>, LegacyAsrLearningTransferError> {
    records
        .into_iter()
        .map(|source| {
            let correction = AsrCorrectionRule::new(
                source.wrong,
                source.correct,
                source.language.as_deref(),
                source.scope.as_deref(),
                false,
                imported_at,
            )
            .map_err(|_| LegacyAsrLearningTransferError::InvalidDocument)?;
            validate_normalized(
                source.normalized_wrong.as_deref(),
                &correction.normalized_wrong,
            )?;
            validate_normalized(
                source.normalized_correct.as_deref(),
                &correction.normalized_correct,
            )?;
            let (created_at, updated_at) = timestamps(
                source.created_at.as_deref(),
                source.updated_at.as_deref(),
                imported_at,
            )?;
            let value = AsrIgnoredSuggestion {
                id: lettuce_types::AsrIgnoredSuggestionId::new(),
                wrong: correction.wrong,
                normalized_wrong: correction.normalized_wrong,
                correct: correction.correct,
                normalized_correct: correction.normalized_correct,
                language: correction.language,
                scope: correction.scope,
                ignored_count: nonnegative(source.ignored_count.unwrap_or(1))?.max(1),
                last_ignored_at: optional_timestamp(source.last_ignored_at.as_deref())?
                    .unwrap_or(imported_at),
                created_at,
                updated_at,
            };
            value
                .validate()
                .map_err(|_| LegacyAsrLearningTransferError::InvalidDocument)?;
            Ok(value)
        })
        .collect()
}

fn map_voice_example(
    source: LegacyAsrVoiceExampleRecord,
    audio_asset_id: lettuce_types::AssetId,
    vocabulary_ids: &BTreeMap<i64, AsrVocabularyTermId>,
    correction_ids: &BTreeMap<i64, AsrCorrectionId>,
    imported_at: TimestampMillis,
) -> Result<AsrVoiceExample, LegacyAsrLearningTransferError> {
    let mut value = AsrVoiceExample::new(
        audio_asset_id,
        source.expected_text,
        source.whisper_output,
        source.language.as_deref(),
        source.scope.as_deref(),
        imported_at,
    )
    .map_err(|_| LegacyAsrLearningTransferError::InvalidDocument)?;
    validate_normalized(
        source.normalized_expected_text.as_deref(),
        &value.normalized_expected_text,
    )?;
    if source.normalized_whisper_output.as_deref() != value.normalized_whisper_output.as_deref()
        && source.normalized_whisper_output.is_some()
    {
        return Err(LegacyAsrLearningTransferError::InvalidDocument);
    }
    value.vocabulary_term_id = source
        .term_id
        .map(|id| {
            vocabulary_ids
                .get(&id)
                .copied()
                .ok_or(LegacyAsrLearningTransferError::InvalidDocument)
        })
        .transpose()?;
    value.correction_id = source
        .correction_id
        .map(|id| {
            correction_ids
                .get(&id)
                .copied()
                .ok_or(LegacyAsrLearningTransferError::InvalidDocument)
        })
        .transpose()?;
    value.created_at = optional_timestamp(source.created_at.as_deref())?.unwrap_or(imported_at);
    value.updated_at = value.created_at;
    value
        .validate()
        .map_err(|_| LegacyAsrLearningTransferError::InvalidDocument)?;
    Ok(value)
}

fn source_vocabulary_ids(
    values: &[(Option<i64>, AsrVocabularyTerm)],
) -> Result<BTreeMap<i64, AsrVocabularyTermId>, LegacyAsrLearningTransferError> {
    source_ids(values.iter().map(|(id, value)| (*id, value.id)))
}

fn source_correction_ids(
    values: &[(Option<i64>, AsrCorrectionRule)],
) -> Result<BTreeMap<i64, AsrCorrectionId>, LegacyAsrLearningTransferError> {
    source_ids(values.iter().map(|(id, value)| (*id, value.id)))
}

fn source_ids<T: Copy>(
    values: impl Iterator<Item = (Option<i64>, T)>,
) -> Result<BTreeMap<i64, T>, LegacyAsrLearningTransferError> {
    let mut ids = BTreeMap::new();
    for (source_id, destination_id) in values {
        if let Some(source_id) = source_id {
            if source_id <= 0 || ids.insert(source_id, destination_id).is_some() {
                return Err(LegacyAsrLearningTransferError::InvalidDocument);
            }
        }
    }
    Ok(ids)
}

fn validate_source_record_ids(
    values: impl Iterator<Item = Option<i64>>,
) -> Result<(), LegacyAsrLearningTransferError> {
    let mut ids = BTreeMap::new();
    for id in values.flatten() {
        if id <= 0 || ids.insert(id, ()).is_some() {
            return Err(LegacyAsrLearningTransferError::InvalidDocument);
        }
    }
    Ok(())
}

fn validate_normalized(
    supplied: Option<&str>,
    expected: &str,
) -> Result<(), LegacyAsrLearningTransferError> {
    if supplied.is_some_and(|supplied| supplied != expected) {
        return Err(LegacyAsrLearningTransferError::InvalidDocument);
    }
    Ok(())
}

fn nonnegative(value: i64) -> Result<u64, LegacyAsrLearningTransferError> {
    u64::try_from(value).map_err(|_| LegacyAsrLearningTransferError::InvalidDocument)
}

fn timestamps(
    created_at: Option<&str>,
    updated_at: Option<&str>,
    fallback: TimestampMillis,
) -> Result<(TimestampMillis, TimestampMillis), LegacyAsrLearningTransferError> {
    let created_at = optional_timestamp(created_at)?.unwrap_or(fallback);
    let updated_at = optional_timestamp(updated_at)?.unwrap_or(created_at);
    Ok((created_at, updated_at))
}

fn optional_timestamp(
    value: Option<&str>,
) -> Result<Option<TimestampMillis>, LegacyAsrLearningTransferError> {
    value.map(parse_timestamp).transpose()
}

fn parse_timestamp(value: &str) -> Result<TimestampMillis, LegacyAsrLearningTransferError> {
    let millis = DateTime::parse_from_rfc3339(value)
        .map(|value| value.timestamp_millis())
        .or_else(|_| {
            NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
                .map(|value| value.and_utc().timestamp_millis())
        })
        .map_err(|_| LegacyAsrLearningTransferError::InvalidDocument)?;
    Ok(TimestampMillis::new(millis))
}

fn resolve_audio_path(
    document_directory: &Path,
    value: &str,
) -> Result<PathBuf, LegacyAsrLearningTransferError> {
    let path = Path::new(value);
    if value.trim().is_empty()
        || (!path.is_absolute()
            && !path
                .components()
                .all(|component| matches!(component, Component::Normal(_))))
    {
        return Err(LegacyAsrLearningTransferError::UnsafeAudioPath);
    }
    let source = if path.is_absolute() {
        path.to_path_buf()
    } else {
        document_directory.join(path)
    };
    let metadata = std::fs::symlink_metadata(&source)
        .map_err(|_| LegacyAsrLearningTransferError::AudioUnavailable)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(LegacyAsrLearningTransferError::UnsafeAudioPath);
    }
    let canonical = std::fs::canonicalize(source)
        .map_err(|_| LegacyAsrLearningTransferError::AudioUnavailable)?;
    if !path.is_absolute() && !canonical.starts_with(document_directory) {
        return Err(LegacyAsrLearningTransferError::UnsafeAudioPath);
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use lettuce_database::Database;
    use lettuce_media::LocalMediaBlobStore;
    use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
    use lettuce_speech::AsrLearningRepository;
    use lettuce_transfer::LegacyAsrLearningDocument;
    use lettuce_types::{AssetId, TimestampMillis};

    use crate::{AppBackend, LegacyAsrLearningTransferError};

    fn wav_fixture() -> Vec<u8> {
        let samples = [0_i16, 1, -1, 0];
        let data_size = u32::try_from(samples.len() * 2).expect("WAV data size");
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_size).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&16_000_u32.to_le_bytes());
        wav.extend_from_slice(&32_000_u32.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        for sample in samples {
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        wav
    }

    fn media_store(
        database_path: &std::path::Path,
        destination_root: &std::path::Path,
    ) -> LocalMediaBlobStore<Database, Database> {
        let snapshot = DirectorySnapshot::new(destination_root).expect("directory snapshot");
        let authority = FilesystemAuthority::new(&snapshot).expect("filesystem authority");
        LocalMediaBlobStore::new(
            authority.managed_files(),
            authority
                .read_capability(ManagedRoot::MediaBlobs)
                .expect("read capability"),
            authority
                .write_capability(ManagedRoot::MediaBlobs)
                .expect("write capability"),
            Database::open(database_path).expect("blob database"),
            Database::open(database_path).expect("asset database"),
        )
    }

    fn legacy_document(audio_path: &str) -> LegacyAsrLearningDocument {
        serde_json::from_value(serde_json::json!({
            "version": 2,
            "vocabulary": [{
                "id": 1,
                "term": "Lettuce AI",
                "normalizedTerm": "lettuce ai",
                "language": "en",
                "category": "product",
                "scope": "workspace",
                "priority": 80,
                "useCount": 7,
                "createdAt": "2026-01-01 00:00:00",
                "updatedAt": "2026-01-02 00:00:00"
            }],
            "corrections": [{
                "id": 2,
                "wrong": "lettuce a eye",
                "normalizedWrong": "lettuce a eye",
                "correct": "Lettuce AI",
                "normalizedCorrect": "lettuce ai",
                "language": "en",
                "scope": "global",
                "confidence": 0.9,
                "useCount": 4,
                "acceptedCount": 3,
                "rejectedCount": 1,
                "seenCount": 5,
                "lastSeenAt": "2026-01-03 00:00:00",
                "userApproved": true,
                "createdAt": "2026-01-01 00:00:00",
                "updatedAt": "2026-01-04 00:00:00"
            }],
            "voiceExamples": [{
                "id": 3,
                "audioPath": audio_path,
                "expectedText": "Lettuce AI",
                "normalizedExpectedText": "lettuce ai",
                "whisperOutput": "lettuce a eye",
                "normalizedWhisperOutput": "lettuce a eye",
                "language": "en",
                "scope": "global",
                "termId": 1,
                "correctionId": 2,
                "createdAt": "2026-01-04 00:00:00"
            }],
            "ignoredSuggestions": [{
                "id": 4,
                "wrong": "green salad",
                "normalizedWrong": "green salad",
                "correct": "green solid",
                "normalizedCorrect": "green solid",
                "language": "en",
                "scope": "global",
                "ignoredCount": 2,
                "lastIgnoredAt": "2026-01-03 00:00:00",
                "createdAt": "2026-01-01 00:00:00",
                "updatedAt": "2026-01-04 00:00:00"
            }]
        }))
        .expect("legacy version 2 document")
    }

    #[test]
    fn legacy_v2_import_ingests_audio_and_commits_the_complete_graph() {
        let root = std::env::temp_dir().join(format!("lettuce-asr-v2-{}", AssetId::new()));
        let source = root.join("source");
        fs::create_dir_all(&source).expect("create source directory");
        fs::write(source.join("voice.wav"), wav_fixture()).expect("write voice audio");
        let database_path = root.join("app.sqlite3");
        let backend =
            AppBackend::open(&database_path, TimestampMillis::new(1)).expect("open application");
        let store = media_store(&database_path, &root.join("destination"));
        let receipt = backend
            .legacy_asr_learning_transfer(&store)
            .import(
                &source,
                legacy_document("voice.wav"),
                TimestampMillis::new(2_000_000_000_000),
            )
            .expect("import legacy learning document");
        assert_eq!(
            (
                receipt.vocabulary_count,
                receipt.correction_count,
                receipt.ignored_suggestion_count,
                receipt.voice_example_count,
            ),
            (1, 1, 1, 1)
        );
        let database = Database::open(&database_path).expect("reopen database");
        let terms = database
            .list_vocabulary(Some("en"), &["workspace".to_owned()])
            .expect("list vocabulary");
        let corrections = database
            .list_corrections(Some("en"), &["global".to_owned()])
            .expect("list corrections");
        let ignored = database
            .list_ignored_suggestions(Some("en"), &["global".to_owned()])
            .expect("list ignored suggestions");
        let voices = database
            .list_voice_examples(Some("en"), &["global".to_owned()])
            .expect("list voice examples");
        assert_eq!((terms[0].priority, terms[0].use_count), (80, 7));
        assert_eq!(
            (
                corrections[0].confidence,
                corrections[0].use_count,
                corrections[0].accepted_count,
                corrections[0].rejected_count,
                corrections[0].seen_count,
            ),
            (0.9, 4, 3, 1, 5)
        );
        assert_eq!(ignored[0].ignored_count, 2);
        assert_eq!(voices[0].vocabulary_term_id, Some(terms[0].id));
        assert_eq!(voices[0].correction_id, Some(corrections[0].id));
    }

    #[test]
    fn legacy_v2_import_rejects_bad_versions_paths_and_links_before_learning_writes() {
        let root = std::env::temp_dir().join(format!("lettuce-asr-v2-invalid-{}", AssetId::new()));
        let source = root.join("source");
        fs::create_dir_all(&source).expect("create source directory");
        fs::write(source.join("voice.wav"), wav_fixture()).expect("write voice audio");
        let database_path = root.join("app.sqlite3");
        let backend =
            AppBackend::open(&database_path, TimestampMillis::new(1)).expect("open application");
        let store = media_store(&database_path, &root.join("destination"));
        let coordinator = backend.legacy_asr_learning_transfer(&store);

        let mut wrong_version = legacy_document("voice.wav");
        wrong_version.version = 1;
        assert_eq!(
            coordinator.import(&source, wrong_version, TimestampMillis::new(2)),
            Err(LegacyAsrLearningTransferError::InvalidDocument)
        );
        let mut dangling = legacy_document("voice.wav");
        dangling.voice_examples[0].term_id = Some(99);
        assert_eq!(
            coordinator.import(&source, dangling, TimestampMillis::new(2)),
            Err(LegacyAsrLearningTransferError::InvalidDocument)
        );
        assert_eq!(
            coordinator.import(
                &source,
                legacy_document("../voice.wav"),
                TimestampMillis::new(2),
            ),
            Err(LegacyAsrLearningTransferError::UnsafeAudioPath)
        );
        let database = Database::open(&database_path).expect("reopen database");
        assert!(
            database
                .list_vocabulary(Some("en"), &["workspace".to_owned()])
                .expect("list unchanged vocabulary")
                .is_empty()
        );
    }
}
