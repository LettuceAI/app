//! Settings that describe this device rather than the user: they never sync
//! or enter backups, and a restore keeps the previous file's unless an
//! imported legacy install brought its own.

use serde::{Deserialize, Serialize};

use crate::GlobalSettingsStoreError;

const MAX_TRUSTED_CERTIFICATES: usize = 64;
const MAX_CERTIFICATE_PEM_BYTES: usize = 1024 * 1024;
/// The longest certificate name.
pub const MAX_DEVICE_NAME_BYTES: usize = 256;
const MAX_PATH_BYTES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DeviceSettings {
    pub trusted_certificates: Vec<TrustedCertificate>,
    pub embedding: DeviceEmbeddingSettings,
    /// Legacy `customLlmModelsDir`: where GGUF downloads go (image models in
    /// its `image` folder); unset means the app's own models folder.
    pub llm_models_dir: Option<String>,
}

/// A root certificate every provider request trusts (legacy
/// `trustedCertificates`); an entry the TLS stack cannot parse is skipped
/// when clients are built, as legacy did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedCertificate {
    pub id: uuid::Uuid,
    pub name: String,
    pub pem: String,
    pub imported_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingModelVersion {
    V3,
    V4,
}

/// Legacy `embeddingModelVersion` (which installed model loads),
/// `embeddingMaxTokens` (unset means 4096; legacy clamped it to 512..=4096 at
/// load, and the import stores it clamped) and
/// `embeddingKeepModelLoaded`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DeviceEmbeddingSettings {
    pub model_version: Option<EmbeddingModelVersion>,
    pub max_tokens: Option<u16>,
    pub keep_model_loaded: bool,
}

impl DeviceSettings {
    pub fn validate(&self) -> Result<(), GlobalSettingsStoreError> {
        let text = |value: &str, max: usize| !value.trim().is_empty() && value.len() <= max;
        let mut pems = std::collections::BTreeSet::new();
        let mut ids = std::collections::BTreeSet::new();
        let certificates = self.trusted_certificates.len() <= MAX_TRUSTED_CERTIFICATES
            && self.trusted_certificates.iter().all(|certificate| {
                text(&certificate.name, MAX_DEVICE_NAME_BYTES)
                    && certificate.pem.len() <= MAX_CERTIFICATE_PEM_BYTES
                    && certificate.pem.contains("BEGIN CERTIFICATE")
                    && certificate.pem.contains("END CERTIFICATE")
                    && certificate.imported_at >= 0
                    && ids.insert(certificate.id)
                    && pems.insert(certificate.pem.trim())
            });
        let folder = self
            .llm_models_dir
            .as_deref()
            .is_none_or(|folder| text(folder, MAX_PATH_BYTES));
        if certificates && folder && self.embedding.max_tokens != Some(0) {
            Ok(())
        } else {
            Err(GlobalSettingsStoreError::InvalidData)
        }
    }

    /// The PEM roots every provider client trusts.
    #[must_use]
    pub fn trusted_roots_pem(&self) -> Vec<String> {
        self.trusted_certificates
            .iter()
            .map(|certificate| certificate.pem.clone())
            .collect()
    }
}

pub trait DeviceSettingsStore: Send + Sync {
    fn load_device_settings(&self) -> Result<DeviceSettings, GlobalSettingsStoreError>;

    fn save_device_settings(
        &self,
        settings: DeviceSettings,
    ) -> Result<(), GlobalSettingsStoreError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn certificate(pem: &str) -> TrustedCertificate {
        TrustedCertificate {
            id: uuid::Uuid::new_v4(),
            name: "corp.pem".into(),
            pem: pem.into(),
            imported_at: 1,
        }
    }

    #[test]
    fn device_settings_follow_the_legacy_import_checks() {
        let pem = "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----";
        let mut settings = DeviceSettings::default();
        settings.trusted_certificates.push(certificate(pem));
        assert!(settings.validate().is_ok());
        assert_eq!(settings.trusted_roots_pem(), vec![pem.to_owned()]);
        settings.trusted_certificates.push(certificate(pem));
        assert!(settings.validate().is_err());
        settings.trusted_certificates.pop();
        settings
            .trusted_certificates
            .push(certificate("not a certificate"));
        assert!(settings.validate().is_err());
        settings.trusted_certificates.pop();
        settings.llm_models_dir = Some("  ".into());
        assert!(settings.validate().is_err());
    }
}
