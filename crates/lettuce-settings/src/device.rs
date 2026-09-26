//! Settings that describe this device rather than the user: they never sync.
//! A backup carries the trusted certificates and the embedding choice; the
//! models folder is a path on this device, so a restore keeps the previous
//! file's.

use serde::{Deserialize, Serialize};

use crate::GlobalSettingsStoreError;

const MAX_TRUSTED_CERTIFICATES: usize = 64;
const MAX_CERTIFICATE_PEM_BYTES: usize = 1024 * 1024;
/// The longest certificate name.
pub const MAX_CERTIFICATE_NAME_BYTES: usize = 1024;
const MAX_PATH_BYTES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DeviceSettings {
    pub trusted_certificates: Vec<TrustedCertificate>,
    pub embedding: DeviceEmbeddingSettings,
    /// Where GGUF downloads go (image models in its `image` folder); unset
    /// means the app's own models folder.
    pub llm_models_dir: Option<String>,
}

/// A root certificate every provider request trusts; an entry the TLS stack
/// cannot parse is skipped when clients are built.
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
    /// Lettuce Eidos.
    V5,
}

/// Which installed embedding model loads (unset means Eidos), its token
/// window (unset means 4096; imports store it clamped to 512..=4096) and
/// whether the model stays loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DeviceEmbeddingSettings {
    pub model_version: Option<EmbeddingModelVersion>,
    pub max_tokens: Option<u16>,
    pub keep_model_loaded: bool,
}

impl DeviceEmbeddingSettings {
    /// The model version to load when it is installed: the stored choice,
    /// or Eidos on a device that never chose one.
    #[must_use]
    pub fn preferred_model_version(&self) -> EmbeddingModelVersion {
        self.model_version.unwrap_or(EmbeddingModelVersion::V5)
    }
}

impl DeviceSettings {
    pub fn validate(&self) -> Result<(), GlobalSettingsStoreError> {
        let text = |value: &str, max: usize| !value.trim().is_empty() && value.len() <= max;
        let mut pems = std::collections::BTreeSet::new();
        let mut ids = std::collections::BTreeSet::new();
        let certificates = self.trusted_certificates.len() <= MAX_TRUSTED_CERTIFICATES
            && self.trusted_certificates.iter().all(|certificate| {
                text(&certificate.name, MAX_CERTIFICATE_NAME_BYTES)
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
        settings.trusted_certificates[0].name = "c".repeat(600);
        assert!(settings.validate().is_ok());
        settings.llm_models_dir = Some("  ".into());
        assert!(settings.validate().is_err());
    }

    #[test]
    fn new_devices_prefer_eidos_and_stored_choices_are_kept() {
        let fresh = DeviceSettings::default();
        assert_eq!(fresh.embedding.model_version, None);
        assert_eq!(
            fresh.embedding.preferred_model_version(),
            EmbeddingModelVersion::V5
        );
        let stored: DeviceSettings =
            serde_json::from_str(r#"{"embedding":{"model_version":"v4"}}"#).expect("stored");
        assert_eq!(
            stored.embedding.preferred_model_version(),
            EmbeddingModelVersion::V4
        );
        assert_eq!(
            serde_json::to_string(&EmbeddingModelVersion::V5).expect("json"),
            "\"v5\""
        );
    }
}
