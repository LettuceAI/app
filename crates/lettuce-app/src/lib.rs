//! LettuceAI composition root, desktop boundary, and optional host API.
//!
//! The intended ownership, boundaries, migration path, and acceptance gates are
//! specified in the crate PLAN.md. This crate starts behavior-empty so the
//! legacy monolith cannot leak in through premature compatibility APIs.

#![deny(unsafe_op_in_unsafe_fn)]

mod built_in_prompts;
mod companion_memory_continuation;
mod companion_memory_effect;
mod companion_memory_execution;
mod companion_memory_inference;
mod companion_memory_job;
mod companion_memory_job_runner;
mod companion_memory_loop;
mod companion_memory_run;
mod companion_memory_terminal;
mod companion_turn;
mod composition;
mod context_assembler;
mod creation_apply;
mod creation_continuation;
mod dynamic_memory;
mod dynamic_memory_continuation;
mod embeddings;
mod launch;
mod provider_runtime;

pub use built_in_prompts::*;
pub use companion_memory_continuation::*;
pub use companion_memory_effect::*;
pub use companion_memory_execution::*;
pub use companion_memory_inference::*;
pub use companion_memory_job::*;
pub use companion_memory_job_runner::*;
pub use companion_memory_loop::*;
pub use companion_memory_run::*;
pub use companion_memory_terminal::*;
pub use companion_turn::*;
pub use composition::*;
pub use context_assembler::*;
pub use creation_apply::*;
pub use creation_continuation::*;
pub use dynamic_memory::*;
pub use dynamic_memory_continuation::*;
pub use embeddings::*;
pub use launch::*;
pub use provider_runtime::*;

#[cfg(test)]
mod tests {
    use lettuce_context::{
        DetectionPolicy, LorebookBehaviorVersion, LorebookMetadataDraft, LorebookRepository,
        PromptBehaviorVersion, PromptMetadataDraft, PromptPurpose, PromptRepository,
    };
    use lettuce_database::Database;
    use lettuce_media::{
        AssetKind, AssetOrigin, AssetProvenanceV1, BlobState, MediaAsset, MediaAssetRepository,
        MediaBlob, MediaBlobRepository, MediaKind, RetentionClass,
    };
    use lettuce_models::{
        ModelKind, ModelProfile, ModelProfileConfig, ModelProfileRepository, ProviderAccount,
        ProviderAccountRepository, ProviderConfig, ProviderProtocol,
    };
    use lettuce_settings::{GlobalSettingsStore, HeaderName, SecretOwnerId, SecretRef};
    use lettuce_types::{
        ContentHash, MediaBlobId, ModelProfileId, ProviderAccountId, Revision, TimestampMillis,
    };

    #[test]
    fn database_composes_all_foundation_domain_ports() {
        let database = Database::open_in_memory().expect("open database");
        let account_id = ProviderAccountId::new();
        let account = ProviderAccount {
            id: account_id,
            secret_owner_id: SecretOwnerId::new(),
            provider_kind: "openrouter".into(),
            protocol: ProviderProtocol::OpenAiCompatible,
            label: "OpenRouter".into(),
            endpoint: Some("https://openrouter.ai/api/v1".into()),
            enabled: true,
            streaming_enabled: true,
            allow_invalid_tls: false,
            api_key_ref: Some(SecretRef::new()),
            secret_headers: vec![lettuce_models::SecretHeader {
                name: HeaderName::new("X-Private").expect("valid header"),
                secret_ref: SecretRef::new(),
            }],
            config: ProviderConfig::Standard,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        let account =
            ProviderAccountRepository::upsert(&database, account, None).expect("insert account");
        let profile = ModelProfile {
            id: ModelProfileId::new(),
            provider_account_id: account.id,
            external_model_id: "example/model".into(),
            display_name: "Example".into(),
            kind: ModelKind::Chat,
            config: ModelProfileConfig {
                chat_parameters: lettuce_models::ChatParameterProfile {
                    temperature: Some(0.7),
                    context_length: Some(4096),
                    max_output_tokens: Some(512),
                    ..Default::default()
                },
                capabilities: lettuce_models::ModelCapabilities {
                    input_modalities: lettuce_models::ModalityCapabilities {
                        text: lettuce_models::CapabilityStatus::Supported,
                        ..Default::default()
                    },
                    output_modalities: lettuce_models::ModalityCapabilities {
                        text: lettuce_models::CapabilityStatus::Supported,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            },
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(2),
            updated_at: TimestampMillis::new(2),
        };
        let profile =
            ModelProfileRepository::upsert(&database, profile, None).expect("insert profile");
        let settings = GlobalSettingsStore::load(&database).expect("load settings");
        GlobalSettingsStore::save(
            &database,
            settings.settings,
            Some(profile.id),
            settings.revision,
        )
        .expect("select default profile");
        let blob = MediaBlob {
            id: MediaBlobId::new(),
            content_hash: ContentHash::parse("ab".repeat(32)).expect("valid hash"),
            kind: MediaKind::Image,
            mime_type: "image/webp".into(),
            byte_size: 4,
            width: Some(2),
            height: Some(2),
            duration_ms: None,
            validation_version: 1,
            state: BlobState::Staged,
            created_at: TimestampMillis::new(3),
            updated_at: TimestampMillis::new(3),
        };
        let blob = MediaBlobRepository::register(&database, blob).expect("register blob");
        let blob = MediaBlobRepository::finalize_staged_to_ready(
            &database,
            blob.id,
            TimestampMillis::new(3),
        )
        .expect("ready blob");
        let asset = MediaAsset::new(
            lettuce_types::AssetId::new(),
            blob.id,
            AssetKind::Illustration,
            AssetOrigin::Upload,
            RetentionClass::Library,
            AssetProvenanceV1::default(),
            Revision::INITIAL,
            TimestampMillis::new(4),
            TimestampMillis::new(4),
        )
        .expect("valid asset");
        let asset = MediaAssetRepository::create(&database, asset).expect("create asset");

        assert_eq!(
            GlobalSettingsStore::load(&database)
                .expect("load settings")
                .default_model_profile_id,
            Some(profile.id)
        );
        assert_eq!(
            ProviderAccountRepository::get(&database, account.id)
                .expect("get account")
                .map(|value| value.id),
            Some(account.id)
        );
        assert_eq!(
            ModelProfileRepository::get(&database, profile.id)
                .expect("get profile")
                .map(|value| value.id),
            Some(profile.id)
        );
        assert_eq!(
            MediaBlobRepository::get(&database, blob.id)
                .expect("get blob")
                .map(|value| value.id),
            Some(blob.id)
        );
        assert_eq!(
            MediaAssetRepository::get(&database, asset.id)
                .expect("get asset")
                .map(|value| value.id),
            Some(asset.id)
        );
    }

    fn assert_context_ports<
        T: PromptRepository
            + lettuce_context::PromptBootstrapPort
            + lettuce_context::PromptDependencyReader
            + LorebookRepository
            + lettuce_context::LorebookDependencyReader
            + lettuce_context::CharacterLorebookBindingRepository
            + lettuce_context::PersonaLorebookBindingRepository
            + lettuce_context::GroupLorebookBindingRepository,
    >() {
    }

    #[test]
    fn database_composes_context_ports_through_domain_traits() {
        assert_context_ports::<Database>();
        let database = Database::open_in_memory().expect("open database");
        let prompt = PromptRepository::create_user_draft(
            &database,
            PromptMetadataDraft {
                name: "Context smoke".into(),
                purpose: PromptPurpose::DirectChat,
                condense: false,
                behavior_version: PromptBehaviorVersion::LegacyV1,
            },
            vec![],
            lettuce_types::TimestampMillis::new(1),
        );
        let prompt = prompt.expect("create context prompt");
        assert_eq!(
            PromptRepository::get(&database, prompt.id)
                .expect("get context prompt")
                .map(|value| value.id),
            Some(prompt.id)
        );
        let lorebook = LorebookRepository::create(
            &database,
            LorebookMetadataDraft {
                name: "Context smoke".into(),
                detection_policy: DetectionPolicy::RecentMessageWindow,
                icon_asset_id: None,
                behavior_version: LorebookBehaviorVersion::LegacyV1,
            },
            vec![],
            lettuce_types::TimestampMillis::new(1),
        );
        let lorebook = lorebook.expect("create context lorebook");
        assert_eq!(
            LorebookRepository::get(&database, lorebook.book.id)
                .expect("get context lorebook")
                .map(|value| value.book.id),
            Some(lorebook.book.id)
        );
    }
}
