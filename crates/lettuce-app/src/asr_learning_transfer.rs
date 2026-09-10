use std::collections::{BTreeMap, BTreeSet};

use lettuce_media::{BlobState, MediaAssetRepository, MediaBlobRepository, MediaKind};
use lettuce_speech::{
    AsrLearningBatch, AsrLearningError, AsrLearningImportReceipt, AsrLearningLibrary,
    AsrLearningRepository,
};
use lettuce_transfer::{ASR_LEARNING_DOCUMENT_VERSION, AsrLearningAudioAsset, AsrLearningDocument};
use lettuce_types::{
    AsrCorrectionId, AsrIgnoredSuggestionId, AsrVocabularyTermId, AsrVoiceExampleId,
};

#[derive(Debug)]
pub struct AsrLearningTransferCoordinator<'a, R: ?Sized> {
    repository: &'a R,
    library: AsrLearningLibrary<'a, R>,
}

impl<'a, R: ?Sized> AsrLearningTransferCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self {
            repository,
            library: AsrLearningLibrary::new(repository),
        }
    }
}

impl<R: AsrLearningRepository + MediaAssetRepository + MediaBlobRepository + ?Sized>
    AsrLearningTransferCoordinator<'_, R>
{
    pub fn export(
        &self,
        language: Option<&str>,
        scopes: &[String],
    ) -> Result<AsrLearningDocument, AsrLearningError> {
        let mut vocabulary = self.library.list_vocabulary(language, scopes)?;
        let mut corrections = self.library.list_corrections(language, scopes)?;
        let voice_examples = self.library.list_voice_examples(language, scopes)?;
        let mut vocabulary_ids = vocabulary
            .iter()
            .map(|term| term.id)
            .collect::<BTreeSet<_>>();
        let mut correction_ids = corrections
            .iter()
            .map(|rule| rule.id)
            .collect::<BTreeSet<_>>();
        for example in &voice_examples {
            if let Some(id) = example.vocabulary_term_id
                && vocabulary_ids.insert(id)
            {
                vocabulary.push(
                    self.library
                        .get_vocabulary(id)?
                        .ok_or(AsrLearningError::InvalidData)?,
                );
            }
            if let Some(id) = example.correction_id
                && correction_ids.insert(id)
            {
                corrections.push(
                    self.library
                        .get_correction(id)?
                        .ok_or(AsrLearningError::InvalidData)?,
                );
            }
        }
        let audio_asset_ids = voice_examples
            .iter()
            .map(|example| example.audio_asset_id)
            .collect::<BTreeSet<_>>();
        let audio_assets = audio_asset_ids
            .into_iter()
            .map(|asset_id| self.export_audio_asset(asset_id))
            .collect::<Result<Vec<_>, _>>()?;
        let document = AsrLearningDocument {
            version: ASR_LEARNING_DOCUMENT_VERSION,
            vocabulary,
            corrections,
            ignored_suggestions: self.library.list_ignored_suggestions(language, scopes)?,
            voice_examples,
            audio_assets,
        };
        document.validate()?;
        Ok(document)
    }

    pub fn import(
        &self,
        document: AsrLearningDocument,
    ) -> Result<AsrLearningImportReceipt, AsrLearningError> {
        document.validate()?;
        for expected in &document.audio_assets {
            self.validate_audio_asset(expected)?;
        }
        let vocabulary_ids = document
            .vocabulary
            .iter()
            .map(|term| (term.id, AsrVocabularyTermId::new()))
            .collect::<BTreeMap<_, _>>();
        let correction_ids = document
            .corrections
            .iter()
            .map(|rule| (rule.id, AsrCorrectionId::new()))
            .collect::<BTreeMap<_, _>>();
        let vocabulary = document
            .vocabulary
            .into_iter()
            .map(|mut term| {
                term.id = vocabulary_ids[&term.id];
                term
            })
            .collect();
        let corrections = document
            .corrections
            .into_iter()
            .map(|mut correction| {
                correction.id = correction_ids[&correction.id];
                correction
            })
            .collect();
        let ignored_suggestions = document
            .ignored_suggestions
            .into_iter()
            .map(|mut ignored| {
                ignored.id = AsrIgnoredSuggestionId::new();
                ignored
            })
            .collect();
        let voice_examples = document
            .voice_examples
            .into_iter()
            .map(|mut example| {
                example.id = AsrVoiceExampleId::new();
                example.vocabulary_term_id =
                    example.vocabulary_term_id.map(|id| vocabulary_ids[&id]);
                example.correction_id = example.correction_id.map(|id| correction_ids[&id]);
                example
            })
            .collect();
        self.library.import_learning_batch(AsrLearningBatch {
            vocabulary,
            corrections,
            ignored_suggestions,
            voice_examples,
        })
    }

    fn export_audio_asset(
        &self,
        asset_id: lettuce_types::AssetId,
    ) -> Result<AsrLearningAudioAsset, AsrLearningError> {
        let asset = MediaAssetRepository::get(self.repository, asset_id)
            .map_err(|_| {
                AsrLearningError::Repository(lettuce_speech::AsrLearningRepositoryError::Storage)
            })?
            .ok_or(AsrLearningError::InvalidData)?;
        let blob = MediaBlobRepository::get(self.repository, asset.blob_id)
            .map_err(|_| {
                AsrLearningError::Repository(lettuce_speech::AsrLearningRepositoryError::Storage)
            })?
            .ok_or(AsrLearningError::InvalidData)?;
        if blob.state != BlobState::Ready || blob.kind != MediaKind::Audio {
            return Err(AsrLearningError::InvalidData);
        }
        Ok(AsrLearningAudioAsset {
            asset_id,
            kind: asset.kind,
            origin: asset.origin,
            provenance: asset.provenance,
            content_hash: blob.content_hash,
            byte_size: blob.byte_size,
            mime_type: blob.mime_type,
            duration_ms: blob.duration_ms,
        })
    }

    fn validate_audio_asset(
        &self,
        expected: &AsrLearningAudioAsset,
    ) -> Result<(), AsrLearningError> {
        let actual = self.export_audio_asset(expected.asset_id)?;
        if actual != *expected {
            return Err(AsrLearningError::InvalidData);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use lettuce_database::Database;
    use lettuce_media::{
        AssetKind, AssetOrigin, AssetProvenanceV1, BlobState, MediaAsset, MediaAssetRepository,
        MediaBlob, MediaBlobRepository, MediaKind, RetentionClass,
    };
    use lettuce_speech::{
        AsrCorrectionRule, AsrIgnoredSuggestion, AsrLearningError, AsrLearningRepository,
        AsrVocabularyTerm, AsrVoiceExample,
    };
    use lettuce_transfer::AsrLearningDocument;
    use lettuce_types::{
        AsrVocabularyTermId, AssetId, ContentHash, MediaBlobId, Revision, TimestampMillis,
    };

    use crate::AsrLearningTransferCoordinator;

    fn audio_asset(database: &Database, id: AssetId, hash_byte: &str) {
        let blob = MediaBlob {
            id: MediaBlobId::new(),
            content_hash: ContentHash::parse(hash_byte.repeat(32)).expect("content hash"),
            kind: MediaKind::Audio,
            mime_type: "audio/wav".to_owned(),
            byte_size: 1,
            width: None,
            height: None,
            duration_ms: Some(1),
            validation_version: 1,
            state: BlobState::Staged,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        let blob = MediaBlobRepository::register(database, blob).expect("register audio blob");
        MediaBlobRepository::finalize_staged_to_ready(database, blob.id, TimestampMillis::new(2))
            .expect("finalize audio blob");
        MediaAssetRepository::create(
            database,
            MediaAsset::new(
                id,
                blob.id,
                AssetKind::OtherAudio,
                AssetOrigin::Upload,
                RetentionClass::Library,
                AssetProvenanceV1::default(),
                Revision::INITIAL,
                TimestampMillis::new(2),
                TimestampMillis::new(2),
            )
            .expect("valid audio asset"),
        )
        .expect("create audio asset");
    }

    #[test]
    fn versioned_export_import_remaps_links_atomically() {
        let source = Database::open_in_memory().expect("open source database");
        let audio_asset_id = AssetId::new();
        audio_asset(&source, audio_asset_id, "ba");
        let term = source
            .save_vocabulary(
                AsrVocabularyTerm::new(
                    "Lettuce AI",
                    Some("en"),
                    Some("product"),
                    Some("workspace"),
                    80,
                    TimestampMillis::new(10),
                )
                .expect("valid vocabulary"),
            )
            .expect("save vocabulary");
        let correction = source
            .save_correction(
                AsrCorrectionRule::new(
                    "lettuce a eye",
                    "Lettuce AI",
                    Some("en"),
                    Some("global"),
                    true,
                    TimestampMillis::new(11),
                )
                .expect("valid correction"),
            )
            .expect("save correction");
        source
            .save_ignored_suggestion(AsrIgnoredSuggestion {
                id: lettuce_types::AsrIgnoredSuggestionId::new(),
                wrong: "green salad".to_owned(),
                normalized_wrong: "green salad".to_owned(),
                correct: "green solid".to_owned(),
                normalized_correct: "green solid".to_owned(),
                language: Some("en".to_owned()),
                scope: "global".to_owned(),
                ignored_count: 2,
                last_ignored_at: TimestampMillis::new(12),
                created_at: TimestampMillis::new(12),
                updated_at: TimestampMillis::new(12),
            })
            .expect("save ignored suggestion");
        let mut example = AsrVoiceExample::new(
            audio_asset_id,
            "Lettuce AI",
            Some("lettuce a eye".to_owned()),
            Some("en"),
            Some("global"),
            TimestampMillis::new(13),
        )
        .expect("valid voice example");
        example.vocabulary_term_id = Some(term.id);
        example.correction_id = Some(correction.id);
        source
            .save_voice_example(example)
            .expect("save voice example");

        let document = AsrLearningTransferCoordinator::new(&source)
            .export(Some("en"), &["global".to_owned()])
            .expect("export learning document");
        assert_eq!(document.audio_assets.len(), 1);
        assert_eq!(document.audio_assets[0].asset_id, audio_asset_id);
        assert_eq!(
            document.audio_assets[0].content_hash.as_str(),
            "ba".repeat(32)
        );
        let encoded = serde_json::to_string(&document).expect("encode learning document");
        let decoded: AsrLearningDocument =
            serde_json::from_str(&encoded).expect("decode learning document");
        let destination = Database::open_in_memory().expect("open destination database");
        audio_asset(&destination, audio_asset_id, "ba");
        let receipt = AsrLearningTransferCoordinator::new(&destination)
            .import(decoded)
            .expect("import learning document");
        assert_eq!(
            (
                receipt.vocabulary_count,
                receipt.correction_count,
                receipt.ignored_suggestion_count,
                receipt.voice_example_count,
            ),
            (1, 1, 1, 1)
        );
        let imported_terms = destination
            .list_vocabulary(Some("en"), &["workspace".to_owned()])
            .expect("list imported vocabulary");
        let imported_corrections = destination
            .list_corrections(Some("en"), &["global".to_owned()])
            .expect("list imported corrections");
        let imported_examples = destination
            .list_voice_examples(Some("en"), &["global".to_owned()])
            .expect("list imported voice examples");
        assert_ne!(imported_terms[0].id, term.id);
        assert_ne!(imported_corrections[0].id, correction.id);
        assert_eq!(
            imported_examples[0].vocabulary_term_id,
            Some(imported_terms[0].id)
        );
        assert_eq!(
            imported_examples[0].correction_id,
            Some(imported_corrections[0].id)
        );

        let missing_audio = Database::open_in_memory().expect("open missing-audio database");
        assert!(
            AsrLearningTransferCoordinator::new(&missing_audio)
                .import(document.clone())
                .is_err()
        );

        let changed_audio = Database::open_in_memory().expect("open changed-audio database");
        audio_asset(&changed_audio, audio_asset_id, "ab");
        assert_eq!(
            AsrLearningTransferCoordinator::new(&changed_audio).import(document.clone()),
            Err(AsrLearningError::InvalidData)
        );
        assert!(
            changed_audio
                .list_vocabulary(Some("en"), &["workspace".to_owned()])
                .expect("list unchanged vocabulary")
                .is_empty()
        );
        assert!(
            missing_audio
                .list_vocabulary(Some("en"), &["workspace".to_owned()])
                .expect("list rolled-back vocabulary")
                .is_empty()
        );

        let mut wrong_version = document.clone();
        wrong_version.version += 1;
        assert_eq!(
            AsrLearningTransferCoordinator::new(&destination).import(wrong_version),
            Err(AsrLearningError::InvalidData)
        );

        let mut invalid = document;
        invalid.voice_examples[0].vocabulary_term_id = Some(AsrVocabularyTermId::new());
        let empty = Database::open_in_memory().expect("open empty database");
        audio_asset(&empty, audio_asset_id, "ba");
        assert_eq!(
            AsrLearningTransferCoordinator::new(&empty).import(invalid),
            Err(AsrLearningError::InvalidData)
        );
        assert!(
            empty
                .list_vocabulary(Some("en"), &["workspace".to_owned()])
                .expect("list empty vocabulary")
                .is_empty()
        );
    }
}
