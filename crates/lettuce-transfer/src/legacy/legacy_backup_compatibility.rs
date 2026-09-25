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

const DOCUMENT_KINDS: [LegacyBackupDocumentKind; 26] = [
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
    LegacyBackupDocumentKind::ImageLoras,
    LegacyBackupDocumentKind::PlaygroundGenerations,
    LegacyBackupDocumentKind::LlmGenerationMetrics,
];
const META_TEXT_LIMIT: usize = 1_000_000;

#[derive(Debug)]
pub struct LegacyBackupCompatibilityPlan {
    pub fingerprint: ContentHash,
    pub coverage: LegacyBackupCompatibilityCoverage,
    pub notices: Vec<LegacyBackupConversionNotice>,
    pub creation_helpers: LegacyBackupCreationHelperPlan,
    pub images: crate::LegacyBackupImagePlan,
    pub llm_metrics: crate::LegacyBackupLlmMetricsPlan,
}

impl LegacyBackupCompatibilityPlan {
    #[must_use]
    pub fn legacy_import_plan(&self) -> crate::LegacyImportPlan {
        let asr = &self
            .creation_helpers
            .source
            .source
            .source
            .source
            .source
            .source
            .source
            .source;
        let authored = &asr.source.authored;
        let group_sessions = &self.creation_helpers.source.source.source.source;
        let direct_sessions = &group_sessions.source;
        let mut plan = crate::LegacyImportPlan {
            provider_models: authored.configuration.provider_models.clone(),
            prompts: authored.configuration.prompts.clone(),
            personas: authored.personas.clone(),
            lorebooks: authored.lorebooks.clone(),
            asr: asr.asr.clone(),
            media: asr.source.media.clone(),
            source_fingerprint: Some(self.fingerprint.clone()),
            later_skips: Vec::new(),
        };
        let sealed = [
            &plan.provider_models.skipped,
            &plan.prompts.skipped,
            &plan.personas.skipped,
            &plan.lorebooks.skipped,
            &plan.media.skipped,
        ]
        .into_iter()
        .flatten()
        .map(|skip| (skip.kind, skip.source_key.clone()))
        .collect::<BTreeSet<_>>();
        let mut later_skips = authored
            .configuration
            .skipped
            .iter()
            .chain(&authored.skipped)
            .chain(&direct_sessions.skipped)
            .chain(&group_sessions.skipped)
            .chain(&self.images.skipped)
            .chain(&self.llm_metrics.skipped)
            .filter(|skip| !sealed.contains(&(skip.kind, skip.source_key.clone())))
            .cloned()
            .collect::<Vec<_>>();
        later_skips.sort();
        later_skips
            .dedup_by(|left, right| left.kind == right.kind && left.source_key == right.source_key);
        plan.later_skips = later_skips;
        plan
    }

    #[must_use]
    pub fn authored_plan(&self) -> &crate::LegacyBackupAuthoredPlan {
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
    }

    #[must_use]
    pub fn usage_records(&self) -> &crate::LegacyBackupUsagePlan {
        &self.direct_sessions().source.source
    }

    #[must_use]
    pub fn memory_embeddings(&self) -> &crate::LegacyBackupMemoryEmbeddingPlan {
        &self.creation_helpers.source
    }

    #[must_use]
    pub fn group_sessions(&self) -> &crate::LegacyBackupGroupSessionPlan {
        &self.creation_helpers.source.source.source.source
    }

    #[must_use]
    pub fn direct_sessions(&self) -> &crate::LegacyBackupDirectSessionPlan {
        &self.creation_helpers.source.source.source.source.source
    }

    /// One planned media object of the source, by its archive path.
    #[must_use]
    pub fn media(&self, relative_path: &str) -> Option<&crate::LegacyBackupMedia> {
        self.inventory()
            .media
            .iter()
            .find(|media| crate::legacy::legacy_backup_media::archive_path(media) == relative_path)
    }

    /// The source counts a legacy import admission validates, derived from the
    /// planned source.
    #[must_use]
    pub fn database_inventory(&self) -> crate::LegacyDatabaseInventory {
        let authored = self.authored_plan();
        let configuration = &authored.configuration;
        let count = |value: usize| u64::try_from(value).unwrap_or(u64::MAX);
        crate::LegacyDatabaseInventory {
            schema_version: configuration.source_schema_version,
            provider_accounts: count(
                configuration
                    .provider_models
                    .provider_accounts
                    .iter()
                    .filter(|provider| {
                        provider.origin == crate::LegacyProviderAccountOrigin::Stored
                    })
                    .count(),
            ),
            models: count(configuration.provider_models.model_profiles.len()),
            prompts: count(configuration.prompts.prompts.len()),
            personas: count(authored.personas.personas.len()),
            characters: count(authored.characters.len()),
            lorebooks: count(authored.lorebooks.lorebooks.len()),
            chat_templates: count(configuration.chat_templates.len()),
            direct_conversations: count(self.direct_sessions().sessions.len()),
            group_profiles: count(authored.groups.len()),
            group_conversations: count(self.group_sessions().sessions.len()),
        }
    }

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
        let images = crate::plan_legacy_backup_images(self.inventory())?;
        let llm_metrics = crate::plan_legacy_backup_llm_metrics(self.inventory())?;
        let mut notices = validate_meta(self.inventory())?;
        notices.extend(self.creation_helpers.notices.iter().cloned());
        notices.extend(images.notices.iter().cloned());
        notices.extend(llm_metrics.notices.iter().cloned());
        notices.sort();
        notices.dedup();
        let fingerprint = fingerprint(&self.creation_helpers, &coverage, &notices)?;
        if coverage != self.coverage
            || notices != self.notices
            || fingerprint != self.fingerprint
            || images != self.images
            || llm_metrics != self.llm_metrics
        {
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
    #[error(transparent)]
    Images(#[from] crate::LegacyBackupImageError),
    #[error(transparent)]
    LlmMetrics(#[from] crate::LegacyBackupLlmMetricsError),
}

pub fn plan_legacy_backup_compatibility(
    inventory: LegacyBackupInventory,
) -> Result<LegacyBackupCompatibilityPlan, LegacyBackupCompatibilityError> {
    let coverage = build_coverage(&inventory)?;
    let mut meta_notices = validate_meta(&inventory)?;
    let images = crate::plan_legacy_backup_images(&inventory)?;
    let llm_metrics = crate::plan_legacy_backup_llm_metrics(&inventory)?;
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
    meta_notices.extend(images.notices.iter().cloned());
    meta_notices.extend(llm_metrics.notices.iter().cloned());
    meta_notices.sort();
    meta_notices.dedup();
    let notices = meta_notices;
    let fingerprint = fingerprint(&creation_helpers, &coverage, &notices)?;
    Ok(LegacyBackupCompatibilityPlan {
        fingerprint,
        coverage,
        notices,
        creation_helpers,
        images,
        llm_metrics,
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
    for item in inventory
        .media
        .iter()
        .filter(|item| item.root != LegacyBackupMediaRoot::Inline)
    {
        if !seen_media.insert((item.root, item.relative_segments.clone())) {
            return Err(LegacyBackupCompatibilityError::DuplicateInventory);
        }
        let byte_count = item.byte_len;
        media_byte_count = media_byte_count
            .checked_add(byte_count)
            .ok_or(LegacyBackupCompatibilityError::LimitExceeded)?;
        media.push(LegacyBackupMediaCoverage {
            root: item.root,
            relative_segments: item.relative_segments.clone(),
            byte_count,
            content_hash: item.content_hash.clone(),
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
        LegacyBackupMediaRoot::Inline => "inline",
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
        LegacyBackupDocumentKind::ImageLoras => "image_loras",
        LegacyBackupDocumentKind::PlaygroundGenerations => "playground_generations",
        LegacyBackupDocumentKind::LlmGenerationMetrics => "llm_generation_metrics",
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
        assert_eq!(plan.coverage.absent_document_count, 26);
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
        with_media.media.push(LegacyBackupMedia::from_bytes(
            LegacyBackupMediaRoot::Attachments,
            vec!["session".into(), "file.bin".into()],
            Zeroizing::new(b"retained attachment".to_vec()),
        ));

        let plan = plan_legacy_backup_compatibility(with_media).expect("media plan");

        assert_ne!(plan.fingerprint, empty.fingerprint);
        assert_eq!(plan.coverage.media_object_count, 1);
        assert_eq!(plan.coverage.media_byte_count, 19);
        assert_eq!(plan.coverage.media[0].relative_segments.len(), 2);
    }

    #[test]
    fn decoded_data_url_images_keep_the_plan_sealed() {
        use base64::Engine as _;
        let mut png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
        png.extend_from_slice(&[0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0]);
        let mut source = inventory();
        source.documents.push(LegacyBackupDocument {
            kind: LegacyBackupDocumentKind::Lorebooks,
            bytes: Zeroizing::new(
                serde_json::to_vec(&serde_json::json!([{
                    "id": uuid::Uuid::from_u128(1).to_string(),
                    "name": "World",
                    "avatar_path": format!(
                        "data:image/png;base64,{}",
                        base64::engine::general_purpose::STANDARD.encode(&png)
                    ),
                    "created_at": 1,
                    "updated_at": 1
                }]))
                .expect("lorebooks"),
            ),
        });

        let plan = plan_legacy_backup_compatibility(source).expect("compatibility plan");

        assert_eq!(plan.coverage.media_object_count, 0);
        assert_eq!(plan.legacy_import_plan().media.media.len(), 1);
        assert!(
            plan.media(&plan.legacy_import_plan().media.media[0].relative_path)
                .is_some()
        );
        plan.verify_seal()
            .expect("inline media stays outside the sealed coverage");
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

        let media = LegacyBackupMedia::from_bytes(
            LegacyBackupMediaRoot::Images,
            vec!["same.png".into()],
            Zeroizing::new(vec![1]),
        );
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
