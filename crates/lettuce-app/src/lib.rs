//! LettuceAI composition root, desktop boundary, and optional host API.
//!
//! The intended ownership, boundaries, migration path, and acceptance gates are
//! specified in the crate PLAN.md. This crate starts behavior-empty so the
//! legacy monolith cannot leak in through premature compatibility APIs.

#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(test)]
mod tests {
    use lettuce_database::Database;
    use lettuce_media::{BlobState, MediaBlob, MediaBlobRepository, MediaKind};
    use lettuce_models::{
        Modality, ModelKind, ModelProfile, ModelProfileConfig, ModelProfileRepository,
        ProviderAccount, ProviderAccountRepository, ProviderConfig, ProviderProtocol,
    };
    use lettuce_settings::{GlobalSettingsStore, HeaderName, SecretRef};
    use lettuce_types::{
        ContentHash, MediaBlobId, ModelProfileId, ProviderAccountId, Revision, TimestampMillis,
    };

    #[test]
    fn database_composes_all_foundation_domain_ports() {
        let database = Database::open_in_memory().expect("open database");
        let account_id = ProviderAccountId::new();
        let account = ProviderAccount {
            id: account_id,
            provider_kind: "openrouter".into(),
            protocol: ProviderProtocol::OpenAiCompatible,
            label: "OpenRouter".into(),
            endpoint: Some("https://openrouter.ai/api/v1".into()),
            enabled: true,
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
                input_modalities: vec![Modality::Text],
                output_modalities: vec![Modality::Text],
                temperature: Some(0.7),
                context_length: Some(4096),
                max_output_tokens: Some(512),
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
            state: BlobState::Ready,
            created_at: TimestampMillis::new(3),
            updated_at: TimestampMillis::new(3),
        };
        let blob = MediaBlobRepository::register(&database, blob).expect("register blob");

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
    }
}
