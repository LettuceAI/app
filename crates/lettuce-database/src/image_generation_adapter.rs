use std::str::FromStr;

use lettuce_image_generation::{
    ImageGenerationRecord, ImageGenerationRepository, ImageGenerationRepositoryError,
    ImageGenerationSource, ImageGenerationState,
};
use lettuce_types::{JobId, ModelProfileId, RequestId, TimestampMillis};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{Database, decode_versioned, encode_versioned};

const REQUEST_FORMAT_VERSION: u32 = 1;
const STATE_FORMAT_VERSION: u32 = 1;

fn storage(_: impl std::fmt::Debug) -> ImageGenerationRepositoryError {
    ImageGenerationRepositoryError::Storage
}

fn corrupt(_: impl std::fmt::Debug) -> ImageGenerationRepositoryError {
    ImageGenerationRepositoryError::InvalidData
}

const fn source_label(source: ImageGenerationSource) -> &'static str {
    match source {
        ImageGenerationSource::Direct => "direct",
        ImageGenerationSource::Scene => "scene",
        ImageGenerationSource::Playground => "playground",
        ImageGenerationSource::CreationHelper => "creation_helper",
    }
}

fn state_columns(state: &ImageGenerationState) -> (&'static str, Option<i64>) {
    match state {
        ImageGenerationState::Pending => ("pending", None),
        ImageGenerationState::Succeeded { result } => {
            ("succeeded", Some(result.completed_at.get()))
        }
        ImageGenerationState::Failed { completed_at, .. } => ("failed", Some(completed_at.get())),
        ImageGenerationState::Cancelled { completed_at } => ("cancelled", Some(completed_at.get())),
    }
}

fn load_in(
    transaction: &Transaction<'_>,
    job_id: JobId,
) -> Result<Option<ImageGenerationRecord>, ImageGenerationRepositoryError> {
    let row = transaction
        .query_row(
            "SELECT request_id, model_profile_id, source, admitted_at, request_json,
                    state, state_json, completed_at
               FROM image_generations WHERE job_id = ?1",
            [job_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                ))
            },
        )
        .optional()
        .map_err(corrupt)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let record = ImageGenerationRecord {
        job_id,
        request: decode_versioned(&row.4, REQUEST_FORMAT_VERSION).map_err(corrupt)?,
        state: decode_versioned(&row.6, STATE_FORMAT_VERSION).map_err(corrupt)?,
    };
    record.validate().map_err(corrupt)?;
    let (state, completed_at) = state_columns(&record.state);
    let outputs = match &record.state {
        ImageGenerationState::Succeeded { result } => result
            .images
            .iter()
            .map(|image| image.asset_id.to_string())
            .collect(),
        _ => Vec::new(),
    };
    if output_assets(transaction, job_id)? != outputs
        || record.request.id != RequestId::from_str(&row.0).map_err(corrupt)?
        || record.request.model_profile_id != ModelProfileId::from_str(&row.1).map_err(corrupt)?
        || source_label(record.request.source) != row.2
        || record.request.created_at != TimestampMillis::new(row.3)
        || state != row.5
        || completed_at != row.7
    {
        return Err(ImageGenerationRepositoryError::InvalidData);
    }
    Ok(Some(record))
}

fn insert_pending_row(
    transaction: &Transaction<'_>,
    record: &ImageGenerationRecord,
) -> Result<usize, ImageGenerationRepositoryError> {
    let request_json =
        encode_versioned(&record.request, REQUEST_FORMAT_VERSION).map_err(storage)?;
    let state_json =
        encode_versioned(&ImageGenerationState::Pending, STATE_FORMAT_VERSION).map_err(storage)?;
    transaction
        .execute(
            "INSERT OR IGNORE INTO image_generations (
                job_id, request_id, model_profile_id, source, admitted_at, request_json,
                state, state_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', ?7)",
            params![
                record.job_id.to_string(),
                record.request.id.to_string(),
                record.request.model_profile_id.to_string(),
                source_label(record.request.source),
                record.request.created_at.get(),
                request_json,
                state_json,
            ],
        )
        .map_err(storage)
}

fn settle_row(
    transaction: &Transaction<'_>,
    job_id: JobId,
    state: &ImageGenerationState,
) -> Result<usize, ImageGenerationRepositoryError> {
    let state_json = encode_versioned(state, STATE_FORMAT_VERSION).map_err(storage)?;
    let (label, completed_at) = state_columns(state);
    let updated = transaction
        .execute(
            "UPDATE image_generations
                SET state = ?2, state_json = ?3, completed_at = ?4
              WHERE job_id = ?1 AND state = 'pending'",
            params![job_id.to_string(), label, state_json, completed_at],
        )
        .map_err(storage)?;
    if updated == 1
        && let ImageGenerationState::Succeeded { result } = state
    {
        for (ordinal, image) in result.images.iter().enumerate() {
            transaction
                .execute(
                    "INSERT INTO image_generation_outputs (job_id, ordinal, asset_id)
                     VALUES (?1, ?2, ?3)",
                    params![
                        job_id.to_string(),
                        i64::try_from(ordinal).map_err(storage)?,
                        image.asset_id.to_string()
                    ],
                )
                .map_err(storage)?;
        }
    }
    Ok(updated)
}

fn output_assets(
    transaction: &Transaction<'_>,
    job_id: JobId,
) -> Result<Vec<String>, ImageGenerationRepositoryError> {
    transaction
        .prepare("SELECT asset_id FROM image_generation_outputs WHERE job_id = ?1 ORDER BY ordinal")
        .and_then(|mut statement| {
            statement
                .query_map([job_id.to_string()], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(corrupt)
}

pub(crate) fn list_in(
    transaction: &Transaction<'_>,
) -> Result<Vec<ImageGenerationRecord>, ImageGenerationRepositoryError> {
    let job_ids = transaction
        .prepare("SELECT job_id FROM image_generations ORDER BY admitted_at, job_id")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(storage)?;
    job_ids
        .iter()
        .map(|job_id| {
            load_in(transaction, JobId::from_str(job_id).map_err(corrupt)?)?
                .ok_or(ImageGenerationRepositoryError::Storage)
        })
        .collect()
}

pub(crate) fn insert_restored_in(
    transaction: &Transaction<'_>,
    record: &ImageGenerationRecord,
) -> Result<(), ImageGenerationRepositoryError> {
    record.validate().map_err(corrupt)?;
    if insert_pending_row(transaction, record)? != 1 {
        return Err(ImageGenerationRepositoryError::Conflict);
    }
    if record.state != ImageGenerationState::Pending
        && settle_row(transaction, record.job_id, &record.state)? != 1
    {
        return Err(ImageGenerationRepositoryError::Conflict);
    }
    if load_in(transaction, record.job_id)?.as_ref() != Some(record) {
        return Err(ImageGenerationRepositoryError::InvalidData);
    }
    Ok(())
}

impl ImageGenerationRepository for Database {
    fn admit(
        &self,
        record: ImageGenerationRecord,
    ) -> Result<ImageGenerationRecord, ImageGenerationRepositoryError> {
        record.validate().map_err(corrupt)?;
        if record.state != ImageGenerationState::Pending {
            return Err(ImageGenerationRepositoryError::InvalidData);
        }
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let inserted = insert_pending_row(&transaction, &record)?;
        let stored =
            load_in(&transaction, record.job_id)?.ok_or(ImageGenerationRepositoryError::Storage)?;
        if inserted == 0 && stored.request != record.request {
            return Err(ImageGenerationRepositoryError::Conflict);
        }
        transaction.commit().map_err(storage)?;
        Ok(stored)
    }

    fn get(&self, job_id: JobId) -> Result<ImageGenerationRecord, ImageGenerationRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let record =
            load_in(&transaction, job_id)?.ok_or(ImageGenerationRepositoryError::NotFound)?;
        transaction.commit().map_err(storage)?;
        Ok(record)
    }

    fn settle(
        &self,
        job_id: JobId,
        state: ImageGenerationState,
    ) -> Result<ImageGenerationRecord, ImageGenerationRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let current =
            load_in(&transaction, job_id)?.ok_or(ImageGenerationRepositoryError::NotFound)?;
        let settled = ImageGenerationRecord {
            state,
            ..current.clone()
        };
        settled.validate().map_err(corrupt)?;
        if settled.state == ImageGenerationState::Pending {
            return Err(ImageGenerationRepositoryError::InvalidData);
        }
        if current.state != ImageGenerationState::Pending {
            if current == settled {
                transaction.commit().map_err(storage)?;
                return Ok(current);
            }
            return Err(ImageGenerationRepositoryError::Conflict);
        }
        if settle_row(&transaction, job_id, &settled.state)? != 1 {
            return Err(ImageGenerationRepositoryError::Conflict);
        }
        let stored =
            load_in(&transaction, job_id)?.ok_or(ImageGenerationRepositoryError::Storage)?;
        transaction.commit().map_err(storage)?;
        Ok(stored)
    }
}

#[cfg(test)]
mod tests {
    use lettuce_image_generation::{
        GeneratedImage, ImageAttribution, ImageGenerationRequest, ImageGenerationResult,
        ImageOutputPolicy,
    };
    use lettuce_jobs::{
        CancellationPolicy, JobKind, JobSpec, JobStore, JobSubject, OutcomeRef, RecoveryPolicy,
        ResourceClass, SubjectKind,
    };
    use lettuce_media::{
        AssetKind, AssetOrigin, AssetProvenanceV1, BlobState, MediaAsset, MediaAssetRepository,
        MediaBlob, MediaBlobRepository, MediaKind, RetentionClass,
    };
    use lettuce_models::StableDiffusionSettings;
    use lettuce_types::{AssetId, ContentHash, MediaBlobId, Revision};

    use super::*;

    fn request() -> ImageGenerationRequest {
        ImageGenerationRequest {
            id: RequestId::new(),
            model_profile_id: ModelProfileId::new(),
            prompt: "a lighthouse".to_owned(),
            settings: StableDiffusionSettings::default(),
            input_images: Vec::new(),
            mask_image: None,
            loras: Vec::new(),
            size: None,
            quality: None,
            style: None,
            count: 2,
            source: ImageGenerationSource::Playground,
            attribution: ImageAttribution::default(),
            output_policy: ImageOutputPolicy::Retained,
            created_at: TimestampMillis::new(10),
        }
    }

    fn admitted_job(database: &Database, request: &ImageGenerationRequest) -> JobId {
        database
            .create_or_get(
                JobSpec::new(
                    JobKind::ImageGenerate,
                    JobSubject::new(SubjectKind::ImageRequest, request.id.to_string())
                        .expect("subject"),
                    OutcomeRef::Request(request.id),
                )
                .with_resources(vec![ResourceClass::Network])
                .with_policies(
                    RecoveryPolicy::MarkInterrupted,
                    CancellationPolicy::Cooperative,
                ),
            )
            .expect("job")
            .job
            .id
    }

    fn generated_asset(database: &Database, job_id: JobId, hash: char) -> GeneratedImage {
        let blob = MediaBlobRepository::register(
            database,
            MediaBlob {
                id: MediaBlobId::new(),
                content_hash: ContentHash::parse(hash.to_string().repeat(64)).expect("hash"),
                kind: MediaKind::Image,
                mime_type: "image/png".into(),
                byte_size: 42,
                width: Some(2),
                height: Some(3),
                duration_ms: None,
                validation_version: 1,
                state: BlobState::Staged,
                created_at: TimestampMillis::new(10),
                updated_at: TimestampMillis::new(10),
            },
        )
        .expect("blob");
        let blob =
            MediaBlobRepository::finalize_staged_to_ready(database, blob.id, blob.updated_at)
                .expect("ready blob");
        let asset = MediaAssetRepository::create(
            database,
            MediaAsset::new(
                AssetId::new(),
                blob.id,
                AssetKind::GeneratedImage,
                AssetOrigin::Generated,
                RetentionClass::Persistent,
                AssetProvenanceV1 {
                    producing_job_id: Some(job_id),
                    ..AssetProvenanceV1::default()
                },
                Revision::INITIAL,
                TimestampMillis::new(20),
                TimestampMillis::new(20),
            )
            .expect("asset"),
        )
        .expect("create asset");
        GeneratedImage {
            asset_id: asset.id,
            content_hash: blob.content_hash,
            mime_type: blob.mime_type,
            byte_size: blob.byte_size,
            width: blob.width,
            height: blob.height,
            text: None,
        }
    }

    fn succeeded(
        request: &ImageGenerationRequest,
        images: Vec<GeneratedImage>,
    ) -> ImageGenerationState {
        ImageGenerationState::Succeeded {
            result: ImageGenerationResult {
                request_id: request.id,
                images,
                rejected_outputs: 0,
                completed_at: TimestampMillis::new(30),
            },
        }
    }

    #[test]
    fn generations_bind_to_their_job_and_settle_once_with_their_own_images() {
        let database = Database::open_in_memory().expect("database");
        let request = request();
        let unbound = ImageGenerationRecord {
            job_id: JobId::new(),
            request: request.clone(),
            state: ImageGenerationState::Pending,
        };
        assert_eq!(
            database.admit(unbound),
            Err(ImageGenerationRepositoryError::Storage)
        );
        let job_id = admitted_job(&database, &request);
        let record = ImageGenerationRecord {
            job_id,
            request: request.clone(),
            state: ImageGenerationState::Pending,
        };
        assert_eq!(database.admit(record.clone()), Ok(record.clone()));
        assert_eq!(database.admit(record.clone()), Ok(record.clone()));
        let other_job = admitted_job(&database, &super::tests::request());
        let foreign = generated_asset(&database, other_job, 'a');
        assert_eq!(
            database.settle(job_id, succeeded(&request, vec![foreign])),
            Err(ImageGenerationRepositoryError::Storage)
        );
        let pending_delete = database.connection().expect("lock").execute(
            "DELETE FROM image_generations WHERE job_id = ?1",
            [job_id.to_string()],
        );
        assert!(pending_delete.is_err());
        let own = generated_asset(&database, job_id, 'b');
        let settled = database
            .settle(job_id, succeeded(&request, vec![own.clone()]))
            .expect("settle");
        assert_eq!(settled.state, succeeded(&request, vec![own.clone()]));
        assert_eq!(
            database.settle(job_id, succeeded(&request, vec![own])),
            Ok(settled.clone())
        );
        assert_eq!(
            database.settle(
                job_id,
                ImageGenerationState::Cancelled {
                    completed_at: TimestampMillis::new(40)
                }
            ),
            Err(ImageGenerationRepositoryError::Conflict)
        );
        assert_eq!(
            ImageGenerationRepository::get(&database, job_id),
            Ok(settled.clone())
        );
        let ImageGenerationState::Succeeded { result } = &settled.state else {
            panic!("expected result");
        };
        let connection = database.connection().expect("lock");
        assert!(
            connection
                .execute(
                    "DELETE FROM media_assets WHERE id = ?1",
                    [result.images[0].asset_id.to_string()],
                )
                .is_err()
        );
        connection
            .execute(
                "DELETE FROM image_generations WHERE job_id = ?1",
                [job_id.to_string()],
            )
            .expect("settled generations can be deleted");
        let outputs: i64 = connection
            .query_row("SELECT count(*) FROM image_generation_outputs", [], |row| {
                row.get(0)
            })
            .expect("outputs");
        assert_eq!(outputs, 0);
    }

    #[test]
    fn failed_generations_restore_and_list_in_admission_order() {
        let database = Database::open_in_memory().expect("database");
        let request = request();
        let job_id = admitted_job(&database, &request);
        database
            .admit(ImageGenerationRecord {
                job_id,
                request: request.clone(),
                state: ImageGenerationState::Pending,
            })
            .expect("admit");
        let failed = database
            .settle(
                job_id,
                ImageGenerationState::Failed {
                    message: "API error 500: upstream".to_owned(),
                    completed_at: TimestampMillis::new(30),
                },
            )
            .expect("settle failure");
        let mut connection = database.connection().expect("lock");
        let transaction = connection.transaction().expect("transaction");
        assert_eq!(list_in(&transaction), Ok(vec![failed.clone()]));
        transaction
            .execute(
                "DELETE FROM image_generations WHERE job_id = ?1",
                [job_id.to_string()],
            )
            .expect("terminal generations can be deleted");
        assert_eq!(insert_restored_in(&transaction, &failed), Ok(()));
        assert_eq!(
            insert_restored_in(&transaction, &failed),
            Err(ImageGenerationRepositoryError::Conflict)
        );
        assert_eq!(list_in(&transaction), Ok(vec![failed]));
    }
}
