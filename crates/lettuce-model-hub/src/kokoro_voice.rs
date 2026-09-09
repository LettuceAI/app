use crate::{KOKORO_SOURCE_REVISION, KokoroInstallError};

pub const MAX_KOKORO_VOICE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteKokoroVoice {
    pub id: String,
    pub remote_path: String,
    pub source_revision: String,
    pub byte_size: u64,
    pub sha256: String,
}

impl RemoteKokoroVoice {
    pub fn pinned(
        id: impl Into<String>,
        source_revision: impl Into<String>,
        byte_size: u64,
        sha256: impl Into<String>,
    ) -> Result<Self, KokoroInstallError> {
        let id = id.into();
        let voice = Self {
            remote_path: format!("voices/{id}.bin"),
            id,
            source_revision: source_revision.into(),
            byte_size,
            sha256: sha256.into(),
        };
        voice.validate()?;
        Ok(voice)
    }

    pub fn validate(&self) -> Result<(), KokoroInstallError> {
        if !is_valid_kokoro_voice_id(&self.id)
            || self.remote_path != format!("voices/{}.bin", self.id)
            || self.source_revision != KOKORO_SOURCE_REVISION
            || self.byte_size == 0
            || self.byte_size > MAX_KOKORO_VOICE_BYTES
            || self.sha256.len() != 64
            || !self.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            || self.sha256 != self.sha256.to_ascii_lowercase()
        {
            return Err(KokoroInstallError::InvalidManifest);
        }
        Ok(())
    }
}

#[must_use]
pub fn is_valid_kokoro_voice_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_voice_derives_the_only_managed_remote_path() {
        let voice = RemoteKokoroVoice::pinned(
            "af_heart",
            KOKORO_SOURCE_REVISION,
            522_240,
            "d583ccff3cdca2f7fae535cb998ac07e9fcb90f09737b9a41fa2734ec44a8f0b",
        )
        .expect("remote voice");
        assert_eq!(voice.remote_path, "voices/af_heart.bin");
        assert!(voice.validate().is_ok());
        assert!(
            RemoteKokoroVoice::pinned(
                "../af_heart",
                KOKORO_SOURCE_REVISION,
                522_240,
                "d5".repeat(32),
            )
            .is_err()
        );
        assert!(RemoteKokoroVoice::pinned("af_heart", "main", 522_240, "d5".repeat(32),).is_err());
    }
}
