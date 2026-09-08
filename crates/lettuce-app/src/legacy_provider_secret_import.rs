use std::collections::BTreeMap;

use lettuce_settings::{SecretPurpose, SecretRecord, SecretState, SecretStore, SecretStoreError};
use lettuce_transfer::{
    LegacyImportAdmission, LegacyImportAssignment, LegacyImportProviderSecretSource,
    LegacyImportRepository, LegacyImportRepositoryError, LegacyImportSecretCompletion,
    LegacyImportSecretCompletionRequest, LegacyPendingProviderSecret, LegacyProviderSecretSource,
    LegacyProviderSecretSourceError,
};
use lettuce_types::TimestampMillis;

#[derive(Debug)]
pub struct LegacyProviderSecretImportCoordinator<'a, R: ?Sized, S: ?Sized, V: ?Sized> {
    repository: &'a R,
    source: &'a S,
    secret_store: &'a V,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyProviderSecretImportError {
    InvalidAdmission,
    Source(LegacyProviderSecretSourceError),
    Repository(LegacyImportRepositoryError),
    SecretStore(SecretStoreError),
    SourceChanged,
}

impl<'a, R, S, V> LegacyProviderSecretImportCoordinator<'a, R, S, V>
where
    R: LegacyImportRepository + ?Sized,
    S: LegacyProviderSecretSource + ?Sized,
    V: SecretStore + ?Sized,
{
    #[must_use]
    pub const fn new(repository: &'a R, source: &'a S, secret_store: &'a V) -> Self {
        Self {
            repository,
            source,
            secret_store,
        }
    }

    pub async fn execute(
        &self,
        admission: &LegacyImportAdmission,
        completed_at: TimestampMillis,
    ) -> Result<Vec<LegacyImportSecretCompletion>, LegacyProviderSecretImportError> {
        let mut owners = BTreeMap::new();
        let mut assignments = Vec::new();
        for assignment in &admission.assignments {
            match assignment {
                LegacyImportAssignment::ProviderAccount {
                    legacy_id,
                    secret_owner_id,
                    ..
                } => {
                    if owners.insert(*legacy_id, *secret_owner_id).is_some() {
                        return Err(LegacyProviderSecretImportError::InvalidAdmission);
                    }
                }
                LegacyImportAssignment::ProviderSecret {
                    source,
                    destination_ref,
                } => assignments.push((source.clone(), *destination_ref)),
                _ => {}
            }
        }
        assignments.sort_by(|left, right| left.0.cmp(&right.0));
        let mut actual_sources = self
            .source
            .sources()
            .map_err(LegacyProviderSecretImportError::Source)?;
        actual_sources.sort();
        if assignments
            .iter()
            .map(|(source, _)| source)
            .ne(actual_sources.iter())
        {
            return Err(LegacyProviderSecretImportError::SourceChanged);
        }

        let mut completions = Vec::with_capacity(assignments.len());
        for (source, destination_ref) in assignments {
            let owner = *owners
                .get(&source.provider_account_id)
                .ok_or(LegacyProviderSecretImportError::InvalidAdmission)?;
            let purpose = secret_purpose(owner, &source);
            let source_value = self
                .source
                .load(&source)
                .map_err(LegacyProviderSecretImportError::Source)?;
            let existing = self
                .repository
                .get_secret_completion(admission.run_id, &source)
                .map_err(LegacyProviderSecretImportError::Repository)?;
            let status = self
                .secret_store
                .status(&destination_ref, &purpose)
                .await
                .map_err(LegacyProviderSecretImportError::SecretStore)?;
            let generation = match status.state {
                SecretState::Present => {
                    let stored = self
                        .secret_store
                        .load(&destination_ref, &purpose)
                        .await
                        .map_err(LegacyProviderSecretImportError::SecretStore)?;
                    let matches =
                        stored.with(|stored| source_value.with(|source| stored == source));
                    if !matches {
                        return Err(LegacyProviderSecretImportError::SourceChanged);
                    }
                    status.generation
                }
                SecretState::Missing if existing.is_none() => {
                    self.secret_store
                        .put(
                            SecretRecord::new(destination_ref, purpose.clone()),
                            source_value,
                            None,
                        )
                        .await
                        .map_err(LegacyProviderSecretImportError::SecretStore)?
                        .generation
                }
                SecretState::Missing => {
                    return Err(LegacyProviderSecretImportError::SourceChanged);
                }
                SecretState::Unavailable { reason } => {
                    return Err(LegacyProviderSecretImportError::SecretStore(
                        SecretStoreError::Unavailable(reason),
                    ));
                }
            };
            if existing
                .as_ref()
                .is_some_and(|completion| completion.generation != generation)
            {
                return Err(LegacyProviderSecretImportError::SourceChanged);
            }
            completions.push(
                self.repository
                    .complete_secret(LegacyImportSecretCompletionRequest {
                        run_id: admission.run_id,
                        source,
                        destination_ref,
                        generation,
                        completed_at,
                    })
                    .map_err(LegacyProviderSecretImportError::Repository)?,
            );
        }
        Ok(completions)
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

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs, sync::Mutex};

    use lettuce_settings::{HeaderName, InMemorySecretStore, SecretStore};
    use lettuce_transfer::{
        LEGACY_DATABASE_SCHEMA_VERSION, LegacyImportAdmissionRequest,
        LegacyImportProviderSecretSource, LegacyImportRepository, LegacyImportSources,
        LegacyPendingProviderSecret, LegacyProviderSecretSource, LegacyProviderSecretSourceError,
    };
    use lettuce_types::{ContentHash, LegacyImportRunId, ProviderAccountId};

    use super::*;
    use crate::AppBackend;

    struct TestSource {
        values: BTreeMap<LegacyImportProviderSecretSource, String>,
        fail_once: Mutex<Option<LegacyImportProviderSecretSource>>,
    }

    impl LegacyProviderSecretSource for TestSource {
        fn sources(
            &self,
        ) -> Result<Vec<LegacyImportProviderSecretSource>, LegacyProviderSecretSourceError>
        {
            Ok(self.values.keys().cloned().collect())
        }

        fn load(
            &self,
            source: &LegacyImportProviderSecretSource,
        ) -> Result<lettuce_settings::SecretValue, LegacyProviderSecretSourceError> {
            let mut fail_once = self
                .fail_once
                .lock()
                .map_err(|_| LegacyProviderSecretSourceError::Unavailable)?;
            if fail_once.as_ref() == Some(source) {
                *fail_once = None;
                return Err(LegacyProviderSecretSourceError::Unavailable);
            }
            lettuce_settings::SecretValue::new(
                self.values
                    .get(source)
                    .ok_or(LegacyProviderSecretSourceError::Missing)?
                    .clone(),
            )
            .map_err(|_| LegacyProviderSecretSourceError::Invalid)
        }
    }

    #[tokio::test]
    async fn secret_transfer_resumes_reopens_and_keeps_plaintext_out_of_sqlite() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-legacy-secret-transfer-{}.sqlite3",
            LegacyImportRunId::new()
        ));
        let run_id = LegacyImportRunId::new();
        let provider_id = ProviderAccountId::new();
        let api_source = LegacyImportProviderSecretSource {
            provider_account_id: provider_id,
            secret: LegacyPendingProviderSecret::ApiKey,
        };
        let header_source = LegacyImportProviderSecretSource {
            provider_account_id: provider_id,
            secret: LegacyPendingProviderSecret::Header {
                name: HeaderName::new("x-private-token").expect("header name"),
            },
        };
        let mut source = TestSource {
            values: BTreeMap::from([
                (api_source.clone(), "api-canary-47".to_owned()),
                (header_source.clone(), "header-canary-83".to_owned()),
            ]),
            fail_once: Mutex::new(Some(header_source.clone())),
        };
        let store = InMemorySecretStore::new();
        let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("open backend");
        let admission = backend
            .database()
            .admit(LegacyImportAdmissionRequest {
                run_id,
                source_schema_version: LEGACY_DATABASE_SCHEMA_VERSION,
                inventory_fingerprint: ContentHash::parse("ab".repeat(32)).expect("inventory hash"),
                plan_fingerprint: ContentHash::parse("cd".repeat(32)).expect("plan hash"),
                sources: LegacyImportSources {
                    provider_account_ids: vec![provider_id],
                    model_profile_ids: Vec::new(),
                    prompt_ids: Vec::new(),
                    provider_secrets: vec![header_source.clone(), api_source.clone()],
                    persona_ids: Vec::new(),
                    lorebook_ids: Vec::new(),
                    lorebook_entry_ids: Vec::new(),
                    asr_vocabulary_ids: Vec::new(),
                    asr_correction_ids: Vec::new(),
                    asr_ignored_suggestion_ids: Vec::new(),
                    asr_voice_example_ids: Vec::new(),
                    media: Vec::new(),
                },
                admitted_at: TimestampMillis::new(2),
            })
            .expect("admit import");
        let api_assignment = admission
            .assignments
            .iter()
            .find_map(|assignment| match assignment {
                LegacyImportAssignment::ProviderSecret {
                    source,
                    destination_ref,
                } if source == &api_source => Some(*destination_ref),
                _ => None,
            })
            .expect("API assignment");
        let owner = admission
            .assignments
            .iter()
            .find_map(|assignment| match assignment {
                LegacyImportAssignment::ProviderAccount {
                    secret_owner_id, ..
                } => Some(*secret_owner_id),
                _ => None,
            })
            .expect("secret owner");
        store
            .put(
                SecretRecord::new(api_assignment, SecretPurpose::ProviderApiKey { owner }),
                lettuce_settings::SecretValue::new("api-canary-47").expect("API key"),
                None,
            )
            .await
            .expect("simulate write before receipt");
        assert_eq!(
            backend
                .legacy_provider_secret_importer(&source, &store)
                .execute(&admission, TimestampMillis::new(3))
                .await,
            Err(LegacyProviderSecretImportError::Source(
                LegacyProviderSecretSourceError::Unavailable
            ))
        );
        let resumed = backend
            .legacy_provider_secret_importer(&source, &store)
            .execute(&admission, TimestampMillis::new(4))
            .await
            .expect("resume secret import");
        assert_eq!(resumed.len(), 2);
        assert!(resumed[0].replayed);
        assert!(!resumed[1].replayed);
        drop(backend);
        let bytes = fs::read(&path).expect("read destination database");
        assert!(!bytes.windows(13).any(|window| window == b"api-canary-47"));
        assert!(
            !bytes
                .windows(16)
                .any(|window| window == b"header-canary-83")
        );

        let reopened = AppBackend::open(&path, TimestampMillis::new(5)).expect("reopen backend");
        let replayed_admission = reopened
            .database()
            .admit(LegacyImportAdmissionRequest {
                run_id,
                source_schema_version: LEGACY_DATABASE_SCHEMA_VERSION,
                inventory_fingerprint: ContentHash::parse("ab".repeat(32)).expect("inventory hash"),
                plan_fingerprint: ContentHash::parse("cd".repeat(32)).expect("plan hash"),
                sources: LegacyImportSources {
                    provider_account_ids: vec![provider_id],
                    model_profile_ids: Vec::new(),
                    prompt_ids: Vec::new(),
                    provider_secrets: vec![api_source.clone(), header_source.clone()],
                    persona_ids: Vec::new(),
                    lorebook_ids: Vec::new(),
                    lorebook_entry_ids: Vec::new(),
                    asr_vocabulary_ids: Vec::new(),
                    asr_correction_ids: Vec::new(),
                    asr_ignored_suggestion_ids: Vec::new(),
                    asr_voice_example_ids: Vec::new(),
                    media: Vec::new(),
                },
                admitted_at: TimestampMillis::new(6),
            })
            .expect("replay admission");
        let replay = reopened
            .legacy_provider_secret_importer(&source, &store)
            .execute(&replayed_admission, TimestampMillis::new(7))
            .await
            .expect("replay secret import");
        assert!(replay.iter().all(|completion| completion.replayed));

        let loaded = store
            .load(&api_assignment, &SecretPurpose::ProviderApiKey { owner })
            .await
            .expect("load imported API key");
        assert!(loaded.with(|value| value == "api-canary-47"));
        source
            .values
            .insert(api_source, "changed-api-canary".to_owned());
        assert_eq!(
            reopened
                .legacy_provider_secret_importer(&source, &store)
                .execute(&replayed_admission, TimestampMillis::new(8))
                .await,
            Err(LegacyProviderSecretImportError::SourceChanged)
        );
        drop(reopened);
        fs::remove_file(path).expect("remove database");
    }
}
