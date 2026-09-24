//! Legacy's app-wide tokens (Hugging Face, CivitAI) go to their
//! fixed references in the secret store. A token the new install already has
//! is kept, so importing never replaces one the user set here.

use lettuce_settings::{
    SecretPurpose, SecretRecord, SecretRef, SecretState, SecretStore, SecretStoreError, SecretValue,
};
use lettuce_transfer::ProviderBackupSecret;

/// Stores the missing tokens and records each one it wrote in `written`, so a
/// failed restore can remove them again.
pub(crate) async fn store_legacy_app_secrets<S: SecretStore + ?Sized>(
    store: &S,
    secrets: &[ProviderBackupSecret],
    written: &mut Vec<(SecretRef, SecretPurpose)>,
) -> Result<usize, SecretStoreError> {
    let mut stored = 0;
    for secret in secrets {
        let Some(reference) = secret.purpose.app_secret_ref() else {
            continue;
        };
        match store.status(&reference, &secret.purpose).await?.state {
            SecretState::Present => {}
            SecretState::Missing => {
                let value = secret
                    .value
                    .with(|value| SecretValue::new(value))
                    .map_err(|_| {
                        SecretStoreError::Backend(lettuce_settings::SecretBackendError::Corrupt)
                    })?;
                store
                    .put(
                        SecretRecord::new(reference, secret.purpose.clone()),
                        value,
                        None,
                    )
                    .await?;
                written.push((reference, secret.purpose.clone()));
                stored += 1;
            }
            SecretState::Unavailable { reason } => {
                return Err(SecretStoreError::Unavailable(reason));
            }
        }
    }
    Ok(stored)
}

#[cfg(test)]
mod tests {
    use lettuce_settings::InMemorySecretStore;

    use super::*;

    fn secret(purpose: SecretPurpose, value: &str) -> ProviderBackupSecret {
        ProviderBackupSecret {
            reference: purpose.app_secret_ref().expect("app secret"),
            purpose,
            generation: 1,
            value: SecretValue::new(value.to_owned()).expect("value"),
        }
    }

    #[tokio::test]
    async fn legacy_app_tokens_fill_only_missing_app_secrets() {
        let store = InMemorySecretStore::default();
        let host = SecretPurpose::CivitaiAccessToken;
        let hugging_face = SecretPurpose::HuggingFaceAccessToken;
        store
            .put(
                SecretRecord::new(host.app_secret_ref().expect("ref"), host.clone()),
                SecretValue::new("current".to_owned()).expect("value"),
                None,
            )
            .await
            .expect("existing host token");
        let mut written = Vec::new();
        let stored = store_legacy_app_secrets(
            &store,
            &[
                secret(host.clone(), "legacy"),
                secret(hugging_face.clone(), "hf_token"),
            ],
            &mut written,
        )
        .await
        .expect("import");
        assert_eq!(stored, 1);
        assert_eq!(
            written,
            [(
                hugging_face.app_secret_ref().expect("ref"),
                hugging_face.clone()
            )]
        );
        let read = |purpose: SecretPurpose| {
            let store = &store;
            async move {
                store
                    .load(&purpose.app_secret_ref().expect("ref"), &purpose)
                    .await
                    .expect("load")
                    .with(str::to_owned)
            }
        };
        assert_eq!(read(host).await, "current");
        assert_eq!(read(hugging_face).await, "hf_token");
    }
}
