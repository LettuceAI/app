use super::*;
use lettuce_jobs::{JobKind, JobSpec, JobStore, JobSubject, OutcomeRef, SubjectKind};
use lettuce_media::{
    AssetKind, AssetOrigin, AssetProvenanceV1, BlobState, MediaAsset, MediaAssetRepository,
    MediaBlob, MediaBlobRepository, MediaKind, RetentionClass,
};
use lettuce_types::{AssetId, MediaBlobId, Revision};

fn asset(database: &Database, digit: char, retention: RetentionClass) -> (AssetId, ContentHash) {
    let hash = ContentHash::parse(digit.to_string().repeat(64)).expect("hash");
    let blob = MediaBlobRepository::register(
        database,
        MediaBlob {
            id: MediaBlobId::new(),
            content_hash: hash.clone(),
            kind: MediaKind::Image,
            mime_type: "image/png".into(),
            byte_size: 42,
            width: None,
            height: None,
            duration_ms: None,
            validation_version: 1,
            state: BlobState::Staged,
            created_at: TimestampMillis::new(10),
            updated_at: TimestampMillis::new(10),
        },
    )
    .expect("register blob");
    MediaBlobRepository::finalize_staged_to_ready(database, blob.id, blob.updated_at)
        .expect("ready blob");
    let id = AssetId::new();
    MediaAssetRepository::create(
        database,
        MediaAsset::new(
            id,
            blob.id,
            AssetKind::OtherImage,
            AssetOrigin::Upload,
            retention,
            AssetProvenanceV1::default(),
            Revision::INITIAL,
            TimestampMillis::new(10),
            TimestampMillis::new(10),
        )
        .expect("valid asset"),
    )
    .expect("create asset");
    (id, hash)
}

#[test]
fn kept_media_set_uses_reference_and_library_rules_instead_of_the_whole_catalog() {
    let path =
        std::env::temp_dir().join(format!("lettuce-kept-set-{}.sqlite3", uuid::Uuid::new_v4()));
    let database = Database::open(&path).expect("database");
    let (referenced, reference_hash) = asset(&database, 'a', RetentionClass::Persistent);
    let (_, library_hash) = asset(&database, 'b', RetentionClass::Library);
    let (_, garbage_hash) = asset(&database, 'c', RetentionClass::Persistent);
    database
        .create_or_get(
            JobSpec::new(
                JobKind::ArtifactInstall,
                JobSubject::new(SubjectKind::ArtifactInstall, "retained-media").expect("subject"),
                OutcomeRef::ArtifactInstallation(referenced),
            )
            .with_resources(vec![lettuce_jobs::ResourceClass::DiskWrite]),
        )
        .expect("reference job");
    assert_eq!(
        Database::media_objects_in_file(&path).expect("kept hashes before collection"),
        BTreeSet::from([reference_hash.clone(), library_hash.clone()])
    );
    database
        .connection()
        .expect("connection")
        .execute(
            "INSERT INTO media_gc_candidates SELECT id, 11 FROM media_assets",
            [],
        )
        .expect("queue all assets");
    let released = database
        .collect_media_garbage(TimestampMillis::new(11))
        .expect("collect active media");
    assert_eq!(
        released
            .iter()
            .map(|object| object.content_hash.clone())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([garbage_hash.clone()])
    );
    drop(database);
    let retained = Database::media_objects_in_file(&path).expect("kept hashes");
    assert_eq!(retained, BTreeSet::from([reference_hash, library_hash]));
    assert!(!retained.contains(&garbage_hash));
    std::fs::remove_file(path).expect("remove test database");
}
