use std::collections::{BTreeMap, BTreeSet};

use lettuce_types::ContentHash;
use serde::Deserialize;
use serde_json::Value;

use crate::{
    LegacyBackupAsrError, LegacyBackupAuthoredError, LegacyBackupConfigurationError,
    LegacyBackupConversionNotice, LegacyBackupCreationHelperError, LegacyBackupCreationHelperPlan,
    LegacyBackupDocumentKind, LegacyBackupGroupSessionError, LegacyBackupInventory,
    LegacyBackupMediaPlanError, LegacyBackupMediaRoot, LegacyBackupMemoryEmbeddingError,
    LegacyBackupPricingError, LegacyBackupScheduledNoteError, LegacyBackupSessionError,
    LegacyBackupUsageError, plan_legacy_backup_asr, plan_legacy_backup_authored,
    plan_legacy_backup_authored_media, plan_legacy_backup_companion_shared_memory,
    plan_legacy_backup_configuration, plan_legacy_backup_creation_helpers,
    plan_legacy_backup_direct_sessions, plan_legacy_backup_group_sessions,
    plan_legacy_backup_memory_embeddings, plan_legacy_backup_pricing,
    plan_legacy_backup_scheduled_notes, plan_legacy_backup_usage,
};

const DOCUMENT_KINDS: [LegacyBackupDocumentKind; 23] = [
    LegacyBackupDocumentKind::Meta,
    LegacyBackupDocumentKind::Settings,
    LegacyBackupDocumentKind::ProviderCredentials,
    LegacyBackupDocumentKind::Models,
    LegacyBackupDocumentKind::AudioProviders,
    LegacyBackupDocumentKind::UserVoices,
    LegacyBackupDocumentKind::ModelPricingCache,
    LegacyBackupDocumentKind::Secrets,
    LegacyBackupDocumentKind::PromptTemplates,
    LegacyBackupDocumentKind::ChatTemplates,
    LegacyBackupDocumentKind::Personas,
    LegacyBackupDocumentKind::Characters,
    LegacyBackupDocumentKind::CompanionScheduledNotes,
    LegacyBackupDocumentKind::CompanionSharedMemory,
    LegacyBackupDocumentKind::MemoryEmbeddings,
    LegacyBackupDocumentKind::Sessions,
    LegacyBackupDocumentKind::CreationHelperSessions,
    LegacyBackupDocumentKind::AsrLearning,
    LegacyBackupDocumentKind::GroupCharacters,
    LegacyBackupDocumentKind::GroupSessions,
    LegacyBackupDocumentKind::UsageRecords,
    LegacyBackupDocumentKind::Lorebooks,
    LegacyBackupDocumentKind::CharacterLorebooks,
];
const META_ENTRY_LIMIT: usize = 10_000;
const META_TEXT_LIMIT: usize = 1_000_000;

#[derive(Debug)]
pub struct LegacyBackupCompatibilityPlan {
    pub fingerprint: ContentHash,
    pub coverage: LegacyBackupCompatibilityCoverage,
    pub notices: Vec<LegacyBackupConversionNotice>,
    pub creation_helpers: LegacyBackupCreationHelperPlan,
}

impl LegacyBackupCompatibilityPlan {
    pub(crate) fn inventory(&self) -> &LegacyBackupInventory {
        &self.configuration().source
    }

    pub(crate) fn secret_count(&self) -> usize {
        self.configuration().secrets.len()
    }

    fn configuration(&self) -> &crate::LegacyBackupConfigurationPlan {
        &self
            .creation_helpers
            .source
            .source
            .source
            .source
            .source
            .source
            .source
            .source
            .source
            .authored
            .configuration
    }

    pub(crate) fn verify_seal(&self) -> Result<(), LegacyBackupCompatibilityError> {
        let coverage = build_coverage(self.inventory())?;
        let mut notices = validate_meta(self.inventory())?;
        notices.extend(self.creation_helpers.notices.iter().cloned());
        notices.sort();
        notices.dedup();
        let fingerprint = fingerprint(&self.creation_helpers, &coverage, &notices)?;
        if coverage != self.coverage || notices != self.notices || fingerprint != self.fingerprint {
            return Err(LegacyBackupCompatibilityError::InvalidSeal);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBackupCompatibilityCoverage {
    pub documents: Vec<LegacyBackupDocumentCoverage>,
    pub media: Vec<LegacyBackupMediaCoverage>,
    pub present_document_count: u64,
    pub absent_document_count: u64,
    pub media_object_count: u64,
    pub media_byte_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBackupDocumentCoverage {
    pub kind: LegacyBackupDocumentKind,
    pub present: bool,
    pub byte_count: u64,
    pub content_hash: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBackupMediaCoverage {
    pub root: LegacyBackupMediaRoot,
    pub relative_segments: Vec<String>,
    pub byte_count: u64,
    pub content_hash: ContentHash,
}

#[derive(Debug, thiserror::Error)]
pub enum LegacyBackupCompatibilityError {
    #[error("legacy backup inventory contains duplicate document or media identities")]
    DuplicateInventory,
    #[error("legacy backup inventory size cannot be represented")]
    LimitExceeded,
    #[error("legacy backup metadata document is malformed")]
    InvalidMeta,
    #[error("legacy backup compatibility seal is invalid")]
    InvalidSeal,
    #[error(transparent)]
    Configuration(#[from] LegacyBackupConfigurationError),
    #[error(transparent)]
    Authored(#[from] LegacyBackupAuthoredError),
    #[error(transparent)]
    Media(#[from] LegacyBackupMediaPlanError),
    #[error(transparent)]
    Asr(#[from] LegacyBackupAsrError),
    #[error(transparent)]
    Usage(#[from] LegacyBackupUsageError),
    #[error(transparent)]
    Pricing(#[from] LegacyBackupPricingError),
    #[error(transparent)]
    DirectSessions(#[from] LegacyBackupSessionError),
    #[error(transparent)]
    GroupSessions(#[from] LegacyBackupGroupSessionError),
    #[error(transparent)]
    ScheduledNotes(#[from] LegacyBackupScheduledNoteError),
    #[error(transparent)]
    CompanionSharedMemory(#[from] crate::LegacyBackupCompanionSharedMemoryError),
    #[error(transparent)]
    MemoryEmbeddings(#[from] LegacyBackupMemoryEmbeddingError),
    #[error(transparent)]
    CreationHelpers(#[from] LegacyBackupCreationHelperError),
}

pub fn plan_legacy_backup_compatibility(
    inventory: LegacyBackupInventory,
) -> Result<LegacyBackupCompatibilityPlan, LegacyBackupCompatibilityError> {
    let coverage = build_coverage(&inventory)?;
    let mut meta_notices = validate_meta(&inventory)?;
    let configuration = plan_legacy_backup_configuration(inventory)?;
    let authored = plan_legacy_backup_authored(configuration)?;
    let media = plan_legacy_backup_authored_media(authored)?;
    let asr = plan_legacy_backup_asr(media)?;
    let usage = plan_legacy_backup_usage(asr)?;
    let pricing = plan_legacy_backup_pricing(usage)?;
    let direct = plan_legacy_backup_direct_sessions(pricing)?;
    let group = plan_legacy_backup_group_sessions(direct)?;
    let notes = plan_legacy_backup_scheduled_notes(group)?;
    let shared = plan_legacy_backup_companion_shared_memory(notes)?;
    let embeddings = plan_legacy_backup_memory_embeddings(shared)?;
    let creation_helpers = plan_legacy_backup_creation_helpers(embeddings)?;
    meta_notices.extend(creation_helpers.notices.iter().cloned());
    meta_notices.sort();
    meta_notices.dedup();
    let notices = meta_notices;
    let fingerprint = fingerprint(&creation_helpers, &coverage, &notices)?;
    Ok(LegacyBackupCompatibilityPlan {
        fingerprint,
        coverage,
        notices,
        creation_helpers,
    })
}

#[derive(Deserialize)]
struct MetaEntry {
    key: String,
    value: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

fn validate_meta(
    inventory: &LegacyBackupInventory,
) -> Result<Vec<LegacyBackupConversionNotice>, LegacyBackupCompatibilityError> {
    let Some(document) = inventory
        .documents
        .iter()
        .find(|document| document.kind == LegacyBackupDocumentKind::Meta)
    else {
        return Ok(vec![LegacyBackupConversionNotice {
            kind: crate::LegacyBackupConversionNoticeKind::Absent,
            document: LegacyBackupDocumentKind::Meta,
            field: "$".into(),
        }]);
    };
    let rows: Vec<MetaEntry> = serde_json::from_slice(&document.bytes)
        .map_err(|_| LegacyBackupCompatibilityError::InvalidMeta)?;
    if rows.len() > META_ENTRY_LIMIT {
        return Err(LegacyBackupCompatibilityError::LimitExceeded);
    }
    let mut keys = BTreeSet::new();
    let mut notices = Vec::new();
    for (index, row) in rows.into_iter().enumerate() {
        if row.key.is_empty()
            || row.key.len() > META_TEXT_LIMIT
            || row
                .value
                .as_ref()
                .is_some_and(|value| value.len() > META_TEXT_LIMIT)
            || !keys.insert(row.key)
        {
            return Err(LegacyBackupCompatibilityError::InvalidMeta);
        }
        for field in row.extra.keys() {
            notices.push(LegacyBackupConversionNotice {
                kind: crate::LegacyBackupConversionNoticeKind::Unsupported,
                document: LegacyBackupDocumentKind::Meta,
                field: format!("[{index}].{field}"),
            });
        }
    }
    notices.push(LegacyBackupConversionNotice {
        kind: crate::LegacyBackupConversionNoticeKind::Unsupported,
        document: LegacyBackupDocumentKind::Meta,
        field: "$[].legacy_database_metadata".into(),
    });
    Ok(notices)
}

fn build_coverage(
    inventory: &LegacyBackupInventory,
) -> Result<LegacyBackupCompatibilityCoverage, LegacyBackupCompatibilityError> {
    let mut seen_documents = BTreeSet::new();
    for document in &inventory.documents {
        if !seen_documents.insert(document.kind) {
            return Err(LegacyBackupCompatibilityError::DuplicateInventory);
        }
    }
    let documents = DOCUMENT_KINDS
        .iter()
        .map(|kind| {
            let document = inventory
                .documents
                .iter()
                .find(|document| document.kind == *kind);
            Ok(LegacyBackupDocumentCoverage {
                kind: *kind,
                present: document.is_some(),
                byte_count: document
                    .map(|document| u64::try_from(document.bytes.len()))
                    .transpose()
                    .map_err(|_| LegacyBackupCompatibilityError::LimitExceeded)?
                    .unwrap_or(0),
                content_hash: document.map(|document| hash_bytes(&document.bytes)),
            })
        })
        .collect::<Result<Vec<_>, LegacyBackupCompatibilityError>>()?;
    if seen_documents.len() != inventory.documents.len() {
        return Err(LegacyBackupCompatibilityError::DuplicateInventory);
    }

    let mut seen_media = BTreeSet::new();
    let mut media = Vec::with_capacity(inventory.media.len());
    let mut media_byte_count = 0_u64;
    for item in &inventory.media {
        if !seen_media.insert((item.root, item.relative_segments.clone())) {
            return Err(LegacyBackupCompatibilityError::DuplicateInventory);
        }
        let byte_count = u64::try_from(item.bytes.len())
            .map_err(|_| LegacyBackupCompatibilityError::LimitExceeded)?;
        media_byte_count = media_byte_count
            .checked_add(byte_count)
            .ok_or(LegacyBackupCompatibilityError::LimitExceeded)?;
        media.push(LegacyBackupMediaCoverage {
            root: item.root,
            relative_segments: item.relative_segments.clone(),
            byte_count,
            content_hash: hash_bytes(&item.bytes),
        });
    }
    media.sort_by(|left, right| {
        (left.root, &left.relative_segments).cmp(&(right.root, &right.relative_segments))
    });

    let present_document_count = u64::try_from(seen_documents.len())
        .map_err(|_| LegacyBackupCompatibilityError::LimitExceeded)?;
    let document_count = u64::try_from(DOCUMENT_KINDS.len())
        .map_err(|_| LegacyBackupCompatibilityError::LimitExceeded)?;
    let media_object_count =
        u64::try_from(media.len()).map_err(|_| LegacyBackupCompatibilityError::LimitExceeded)?;
    Ok(LegacyBackupCompatibilityCoverage {
        documents,
        media,
        present_document_count,
        absent_document_count: document_count - present_document_count,
        media_object_count,
        media_byte_count,
    })
}

fn fingerprint(
    plan: &LegacyBackupCreationHelperPlan,
    coverage: &LegacyBackupCompatibilityCoverage,
    notices: &[LegacyBackupConversionNotice],
) -> Result<ContentHash, LegacyBackupCompatibilityError> {
    let inventory = &plan
        .source
        .source
        .source
        .source
        .source
        .source
        .source
        .source
        .source
        .authored
        .configuration
        .source;
    let mut hasher = blake3::Hasher::new();
    add_bytes(&mut hasher, b"lettuce.legacy-backup.compatibility-plan.v1");
    add_u64(&mut hasher, u64::from(inventory.version));
    add_u64(&mut hasher, inventory.created_at);
    add_bytes(&mut hasher, inventory.app_version.as_bytes());
    add_bytes(&mut hasher, inventory.source_hash.as_str().as_bytes());
    for document in &coverage.documents {
        add_bytes(&mut hasher, document_name(document.kind).as_bytes());
        add_u64(&mut hasher, u64::from(document.present));
        add_u64(&mut hasher, document.byte_count);
        add_bytes(
            &mut hasher,
            document
                .content_hash
                .as_ref()
                .map_or(&[][..], |value| value.as_str().as_bytes()),
        );
    }
    for item in &coverage.media {
        add_bytes(&mut hasher, media_root_name(item.root).as_bytes());
        add_u64(
            &mut hasher,
            u64::try_from(item.relative_segments.len())
                .map_err(|_| LegacyBackupCompatibilityError::LimitExceeded)?,
        );
        for segment in &item.relative_segments {
            add_bytes(&mut hasher, segment.as_bytes());
        }
        add_u64(&mut hasher, item.byte_count);
        add_bytes(&mut hasher, item.content_hash.as_str().as_bytes());
    }
    for notice in notices {
        add_bytes(&mut hasher, notice_kind_name(notice.kind).as_bytes());
        add_bytes(&mut hasher, document_name(notice.document).as_bytes());
        add_bytes(&mut hasher, notice.field.as_bytes());
    }
    Ok(ContentHash::parse(hasher.finalize().to_hex().to_string()).expect("BLAKE3 is valid"))
}

fn add_bytes(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    add_u64(hasher, bytes.len() as u64);
    hasher.update(bytes);
}

fn add_u64(hasher: &mut blake3::Hasher, value: u64) {
    hasher.update(&value.to_le_bytes());
}

fn hash_bytes(bytes: &[u8]) -> ContentHash {
    ContentHash::parse(blake3::hash(bytes).to_hex().to_string()).expect("BLAKE3 is valid")
}

fn notice_kind_name(kind: crate::LegacyBackupConversionNoticeKind) -> &'static str {
    match kind {
        crate::LegacyBackupConversionNoticeKind::Absent => "absent",
        crate::LegacyBackupConversionNoticeKind::Unsupported => "unsupported",
        crate::LegacyBackupConversionNoticeKind::Lossy => "lossy",
    }
}

fn media_root_name(root: LegacyBackupMediaRoot) -> &'static str {
    match root {
        LegacyBackupMediaRoot::Images => "images",
        LegacyBackupMediaRoot::Avatars => "avatars",
        LegacyBackupMediaRoot::Attachments => "attachments",
        LegacyBackupMediaRoot::Sessions => "sessions",
        LegacyBackupMediaRoot::GeneratedImages => "generated_images",
    }
}

fn document_name(kind: LegacyBackupDocumentKind) -> &'static str {
    match kind {
        LegacyBackupDocumentKind::Meta => "meta",
        LegacyBackupDocumentKind::Settings => "settings",
        LegacyBackupDocumentKind::ProviderCredentials => "provider_credentials",
        LegacyBackupDocumentKind::Models => "models",
        LegacyBackupDocumentKind::AudioProviders => "audio_providers",
        LegacyBackupDocumentKind::UserVoices => "user_voices",
        LegacyBackupDocumentKind::ModelPricingCache => "model_pricing_cache",
        LegacyBackupDocumentKind::Secrets => "secrets",
        LegacyBackupDocumentKind::PromptTemplates => "prompt_templates",
        LegacyBackupDocumentKind::ChatTemplates => "chat_templates",
        LegacyBackupDocumentKind::Personas => "personas",
        LegacyBackupDocumentKind::Characters => "characters",
        LegacyBackupDocumentKind::CompanionScheduledNotes => "companion_scheduled_notes",
        LegacyBackupDocumentKind::CompanionSharedMemory => "companion_shared_memory",
        LegacyBackupDocumentKind::MemoryEmbeddings => "memory_embeddings",
        LegacyBackupDocumentKind::Sessions => "sessions",
        LegacyBackupDocumentKind::CreationHelperSessions => "creation_helper_sessions",
        LegacyBackupDocumentKind::AsrLearning => "asr_learning",
        LegacyBackupDocumentKind::GroupCharacters => "group_characters",
        LegacyBackupDocumentKind::GroupSessions => "group_sessions",
        LegacyBackupDocumentKind::UsageRecords => "usage_records",
        LegacyBackupDocumentKind::Lorebooks => "lorebooks",
        LegacyBackupDocumentKind::CharacterLorebooks => "character_lorebooks",
    }
}

#[cfg(test)]
mod tests {
    use lettuce_types::ContentHash;
    use zeroize::Zeroizing;

    use super::*;
    use crate::{LegacyBackupDocument, LegacyBackupMedia};

    fn inventory() -> LegacyBackupInventory {
        LegacyBackupInventory {
            version: 2,
            created_at: 1_700_000_000_000,
            app_version: "1.0.0".into(),
            source_hash: ContentHash::parse("ab".repeat(32)).expect("source hash"),
            documents: Vec::new(),
            media: Vec::new(),
        }
    }

    #[test]
    fn empty_optional_backup_has_complete_explicit_coverage() {
        let plan = plan_legacy_backup_compatibility(inventory()).expect("compatibility plan");

        assert_eq!(plan.coverage.documents.len(), DOCUMENT_KINDS.len());
        assert_eq!(plan.coverage.present_document_count, 0);
        assert_eq!(plan.coverage.absent_document_count, 23);
        assert_eq!(plan.coverage.media_object_count, 0);
        assert_eq!(plan.coverage.media_byte_count, 0);
        assert!(plan.coverage.documents.iter().all(|item| !item.present));
        assert!(plan.notices.iter().any(|notice| {
            notice.kind == crate::LegacyBackupConversionNoticeKind::Absent
                && notice.document == LegacyBackupDocumentKind::Meta
        }));
    }

    #[test]
    fn every_retained_media_object_changes_the_source_bound_fingerprint() {
        let empty = plan_legacy_backup_compatibility(inventory()).expect("empty plan");
        let mut with_media = inventory();
        with_media.media.push(LegacyBackupMedia {
            root: LegacyBackupMediaRoot::Attachments,
            relative_segments: vec!["session".into(), "file.bin".into()],
            bytes: Zeroizing::new(b"retained attachment".to_vec()),
        });

        let plan = plan_legacy_backup_compatibility(with_media).expect("media plan");

        assert_ne!(plan.fingerprint, empty.fingerprint);
        assert_eq!(plan.coverage.media_object_count, 1);
        assert_eq!(plan.coverage.media_byte_count, 19);
        assert_eq!(plan.coverage.media[0].relative_segments.len(), 2);
    }

    #[test]
    fn duplicate_document_and_media_identities_reject_before_planning() {
        let mut duplicate_document = inventory();
        duplicate_document.documents = vec![
            LegacyBackupDocument {
                kind: LegacyBackupDocumentKind::Meta,
                bytes: Zeroizing::new(b"{}".to_vec()),
            },
            LegacyBackupDocument {
                kind: LegacyBackupDocumentKind::Meta,
                bytes: Zeroizing::new(b"{}".to_vec()),
            },
        ];
        assert!(matches!(
            plan_legacy_backup_compatibility(duplicate_document),
            Err(LegacyBackupCompatibilityError::DuplicateInventory)
        ));

        let media = LegacyBackupMedia {
            root: LegacyBackupMediaRoot::Images,
            relative_segments: vec!["same.png".into()],
            bytes: Zeroizing::new(vec![1]),
        };
        let mut duplicate_media = inventory();
        duplicate_media.media = vec![media.clone(), media];
        assert!(matches!(
            plan_legacy_backup_compatibility(duplicate_media),
            Err(LegacyBackupCompatibilityError::DuplicateInventory)
        ));
    }

    #[test]
    fn metadata_is_validated_and_bound_without_becoming_live_state() {
        let mut source = inventory();
        source.documents.push(LegacyBackupDocument {
            kind: LegacyBackupDocumentKind::Meta,
            bytes: Zeroizing::new(br#"[{"key":"schema_version","value":"61"}]"#.to_vec()),
        });

        let plan = plan_legacy_backup_compatibility(source).expect("metadata plan");

        let meta = plan
            .coverage
            .documents
            .iter()
            .find(|item| item.kind == LegacyBackupDocumentKind::Meta)
            .expect("metadata coverage");
        assert!(meta.present);
        assert!(plan.notices.iter().any(|notice| {
            notice.kind == crate::LegacyBackupConversionNoticeKind::Unsupported
                && notice.document == LegacyBackupDocumentKind::Meta
        }));

        let mut duplicate = inventory();
        duplicate.documents.push(LegacyBackupDocument {
            kind: LegacyBackupDocumentKind::Meta,
            bytes: Zeroizing::new(
                br#"[{"key":"same","value":null},{"key":"same","value":"x"}]"#.to_vec(),
            ),
        });
        assert!(matches!(
            plan_legacy_backup_compatibility(duplicate),
            Err(LegacyBackupCompatibilityError::InvalidMeta)
        ));
    }
}
