use std::collections::BTreeMap;

use lettuce_settings::{SecretPurpose, SecretState, SecretStore, SecretStoreError};
use lettuce_transfer::{
    LegacyImportAdmission, LegacyImportAssignment, LegacyImportPlan,
    LegacyImportProviderSecretSource, LegacyImportRepository, LegacyImportRepositoryError,
    LegacyPendingProviderSecret, LegacyProviderModelMaterializationRequest,
    LegacyProviderModelReceipt,
};
use lettuce_types::TimestampMillis;

#[derive(Debug)]
pub struct LegacyProviderModelImportCoordinator<'a, R: ?Sized, S: ?Sized> {
    repository: &'a R,
    secret_store: &'a S,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyProviderModelImportError {
    InvalidAdmission,
    Repository(LegacyImportRepositoryError),
    SecretStore(SecretStoreError),
    SecretChanged,
}

impl<'a, R, S> LegacyProviderModelImportCoordinator<'a, R, S>
where
    R: LegacyImportRepository + ?Sized,
    S: SecretStore + ?Sized,
{
    #[must_use]
    pub const fn new(repository: &'a R, secret_store: &'a S) -> Self {
        Self {
            repository,
            secret_store,
        }
    }

    pub async fn execute(
        &self,
        admission: &LegacyImportAdmission,
        plan: &LegacyImportPlan,
        completed_at: TimestampMillis,
    ) -> Result<LegacyProviderModelReceipt, LegacyProviderModelImportError> {
        let fingerprint = super::legacy_import::plan_fingerprint(
            &plan.provider_models,
            &plan.prompts,
            &plan.personas,
            &plan.lorebooks,
            &plan.asr,
            &plan.media,
        );
        if fingerprint != admission.plan_fingerprint {
            return Err(LegacyProviderModelImportError::InvalidAdmission);
        }
        let mut owners = BTreeMap::new();
        let mut secret_refs = BTreeMap::new();
        for assignment in &admission.assignments {
            let duplicate = match assignment {
                LegacyImportAssignment::ProviderAccount {
                    legacy_id,
                    secret_owner_id,
                    ..
                } => owners.insert(*legacy_id, *secret_owner_id).is_some(),
                LegacyImportAssignment::ProviderSecret {
                    source,
                    destination_ref,
                } => secret_refs
                    .insert(source.clone(), *destination_ref)
                    .is_some(),
                _ => false,
            };
            if duplicate {
                return Err(LegacyProviderModelImportError::InvalidAdmission);
            }
        }
        for (source, destination_ref) in secret_refs {
            let owner = *owners
                .get(&source.provider_account_id)
                .ok_or(LegacyProviderModelImportError::InvalidAdmission)?;
            let purpose = secret_purpose(owner, &source);
            let completion = self
                .repository
                .get_secret_completion(admission.run_id, &source)
                .map_err(LegacyProviderModelImportError::Repository)?
                .ok_or(LegacyProviderModelImportError::SecretChanged)?;
            if completion.destination_ref != destination_ref {
                return Err(LegacyProviderModelImportError::SecretChanged);
            }
            let status = self
                .secret_store
                .status(&destination_ref, &purpose)
                .await
                .map_err(LegacyProviderModelImportError::SecretStore)?;
            if status.reference != destination_ref
                || status.purpose != purpose
                || status.generation != completion.generation
                || status.state != SecretState::Present
            {
                return Err(LegacyProviderModelImportError::SecretChanged);
            }
        }
        self.repository
            .materialize_provider_models(LegacyProviderModelMaterializationRequest {
                run_id: admission.run_id,
                plan_fingerprint: fingerprint,
                provider_models: plan.provider_models.clone(),
                prompts: plan.prompts.clone(),
                completed_at,
            })
            .map_err(LegacyProviderModelImportError::Repository)
    }
}

fn secret_purpose(
    owner: lettuce_settings::SecretOwnerId,
    source: &LegacyImportProviderSecretSource,
) -> SecretPurpose {
    match &source.secret {
        LegacyPendingProviderSecret::ApiKey => SecretPurpose::ProviderApiKey { owner },
        LegacyPendingProviderSecret::Header { name } => SecretPurpose::ProviderSecretHeader {
            owner,
            name: name.clone(),
        },
    }
}
