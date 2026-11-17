use lettuce_models::{
    LOCAL_DIFFUSION_PROVIDER_KIND, LOCAL_DIFFUSION_PROVIDER_LABEL,
    LOCAL_LLAMA_PROVIDER_KIND, LOCAL_LLAMA_PROVIDER_LABEL,
    LEGACY_LOCAL_DIFFUSION_PROVIDER_LABEL,
    ModelLookup, ModelRepositoryError, ProviderAccount, ProviderAccountRepository, ProviderConfig,
    ProviderProtocol,
};
use lettuce_settings::SecretOwnerId;
use lettuce_types::{ProviderAccountId, Revision, TimestampMillis};

#[derive(Debug, Clone, Copy)]
pub(crate) enum LocalModelProvider {
    LlamaCpp,
    StableDiffusion,
}

impl LocalModelProvider {
    pub(crate) fn from_kind(kind: &str) -> Option<Self> {
        if kind.eq_ignore_ascii_case(LOCAL_LLAMA_PROVIDER_KIND) {
            Some(Self::LlamaCpp)
        } else if kind.eq_ignore_ascii_case(LOCAL_DIFFUSION_PROVIDER_KIND) {
            Some(Self::StableDiffusion)
        } else {
            None
        }
    }

    fn kind(self) -> &'static str {
        match self {
            Self::LlamaCpp => LOCAL_LLAMA_PROVIDER_KIND,
            Self::StableDiffusion => LOCAL_DIFFUSION_PROVIDER_KIND,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::LlamaCpp => LOCAL_LLAMA_PROVIDER_LABEL,
            Self::StableDiffusion => LOCAL_DIFFUSION_PROVIDER_LABEL,
        }
    }

    pub(crate) fn managed_labels(self) -> &'static [&'static str] {
        match self {
            Self::LlamaCpp => &[LOCAL_LLAMA_PROVIDER_LABEL],
            Self::StableDiffusion => &[
                LOCAL_DIFFUSION_PROVIDER_LABEL, LEGACY_LOCAL_DIFFUSION_PROVIDER_LABEL,
            ],
        }
    }

    pub(crate) fn protocol(self) -> ProviderProtocol {
        match self {
            Self::LlamaCpp => ProviderProtocol::LlamaCpp,
            Self::StableDiffusion => ProviderProtocol::StableDiffusion,
        }
    }

    pub(crate) fn new_account(self, now: TimestampMillis) -> ProviderAccount {
        ProviderAccount {
            id: ProviderAccountId::new(),
            secret_owner_id: SecretOwnerId::new(),
            provider_kind: self.kind().to_owned(),
            protocol: self.protocol(),
            label: self.label().to_owned(),
            endpoint: None,
            enabled: true,
            streaming_enabled: true,
            allow_invalid_tls: false,
            api_key_ref: None,
            secret_headers: Vec::new(),
            config: ProviderConfig::Standard,
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        }
    }

    /// The managed account used by model downloads, created or re-enabled.
    pub(crate) fn ensure_account<R>(
        self,
        repository: &R,
        now: TimestampMillis,
    ) -> Result<ProviderAccount, ModelRepositoryError>
    where
        R: ModelLookup + ProviderAccountRepository + ?Sized,
    {
        for label in self.managed_labels() {
            if let Some(account) = repository.account_by_kind_and_label(self.kind(), label)? {
                if account.enabled {
                    return Ok(account);
                }
                let revision = account.revision;
                return ProviderAccountRepository::upsert(
                    repository,
                    ProviderAccount {
                        enabled: true,
                        updated_at: now,
                        ..account
                    },
                    Some(revision),
                );
            }
        }
        ProviderAccountRepository::upsert(repository, self.new_account(now), None)
    }
}
