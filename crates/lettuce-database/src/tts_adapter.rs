use std::str::FromStr;

use lettuce_settings::{SecretOwnerId, SecretRef};
use lettuce_speech::{
    AudioProvider, AudioProviderConfig, AudioProviderKind, TtsConfigurationRepository,
    TtsConfigurationRepositoryError, UserVoice,
};
use lettuce_types::{AudioProviderId, Revision, TimestampMillis, VoiceProfileId};
use rusqlite::{OptionalExtension, Row, params};

use crate::{Database, decode_versioned, encode_versioned};

const AUDIO_PROVIDER_CONFIG_FORMAT_VERSION: u32 = 1;

fn storage(_: impl std::fmt::Debug) -> TtsConfigurationRepositoryError {
    TtsConfigurationRepositoryError::Storage
}

fn corrupt(_: impl std::fmt::Debug) -> TtsConfigurationRepositoryError {
    TtsConfigurationRepositoryError::InvalidData
}

fn kind_name(kind: AudioProviderKind) -> &'static str {
    match kind {
        AudioProviderKind::GeminiTts => "gemini_tts",
        AudioProviderKind::Elevenlabs => "elevenlabs",
        AudioProviderKind::FishTts => "fish_tts",
        AudioProviderKind::FishSpeech => "fish_speech",
        AudioProviderKind::OpenAiTts => "open_ai_tts",
        AudioProviderKind::Kokoro => "kokoro",
    }
}

fn provider_from_row(row: &Row<'_>) -> rusqlite::Result<AudioProvider> {
    let id = row.get::<_, String>(0)?;
    let owner = row.get::<_, String>(1)?;
    let stored_kind = row.get::<_, String>(2)?;
    let label = row.get(3)?;
    let secret_ref = row.get::<_, Option<String>>(4)?;
    let config_json = row.get::<_, String>(5)?;
    let revision = row.get::<_, i64>(6)?;
    let created_at = row.get(7)?;
    let updated_at = row.get(8)?;
    let config =
        decode_versioned::<AudioProviderConfig>(&config_json, AUDIO_PROVIDER_CONFIG_FORMAT_VERSION)
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let provider = AudioProvider {
        id: AudioProviderId::from_str(&id).map_err(|_| rusqlite::Error::InvalidQuery)?,
        secret_owner_id: SecretOwnerId::from_uuid(
            uuid::Uuid::parse_str(&owner).map_err(|_| rusqlite::Error::InvalidQuery)?,
        ),
        label,
        api_key_ref: secret_ref
            .map(|value| {
                uuid::Uuid::parse_str(&value)
                    .map(SecretRef::from_uuid)
                    .map_err(|_| rusqlite::Error::InvalidQuery)
            })
            .transpose()?,
        config,
        revision: Revision::new(
            u64::try_from(revision).map_err(|_| rusqlite::Error::InvalidQuery)?,
        ),
        created_at: TimestampMillis::new(created_at),
        updated_at: TimestampMillis::new(updated_at),
    };
    provider
        .validate()
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    if kind_name(provider.config.provider_kind()) != stored_kind {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(provider)
}

fn voice_from_row(row: &Row<'_>) -> rusqlite::Result<UserVoice> {
    let voice = UserVoice {
        id: VoiceProfileId::from_str(&row.get::<_, String>(0)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        provider_id: AudioProviderId::from_str(&row.get::<_, String>(1)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        name: row.get(2)?,
        model_id: row.get(3)?,
        voice_id: row.get(4)?,
        prompt: row.get(5)?,
        revision: Revision::new(
            u64::try_from(row.get::<_, i64>(6)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        ),
        created_at: TimestampMillis::new(row.get(7)?),
        updated_at: TimestampMillis::new(row.get(8)?),
    };
    voice
        .validate()
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(voice)
}

const PROVIDER_SELECT: &str = "SELECT id, secret_owner_id, provider_kind, label,
    api_key_secret_ref, config_json, revision, created_at, updated_at FROM audio_providers";
const VOICE_SELECT: &str = "SELECT id, provider_id, name, model_id, voice_id, prompt,
    revision, created_at, updated_at FROM user_voices";

impl TtsConfigurationRepository for Database {
    fn upsert_audio_provider(
        &self,
        provider: AudioProvider,
        expected_revision: Option<Revision>,
    ) -> Result<AudioProvider, TtsConfigurationRepositoryError> {
        provider.validate().map_err(corrupt)?;
        let config_json = encode_versioned(&provider.config, AUDIO_PROVIDER_CONFIG_FORMAT_VERSION)
            .map_err(corrupt)?;
        let connection = self.connection().map_err(storage)?;
        let changed = if let Some(expected) = expected_revision {
            if provider.revision != expected {
                return Err(TtsConfigurationRepositoryError::InvalidData);
            }
            let next = expected.next().map_err(storage)?;
            connection
                .execute(
                    "UPDATE audio_providers SET provider_kind=?2, label=?3,
                        api_key_secret_ref=?4, config_json=?5, revision=?6, updated_at=?7
                     WHERE id=?1 AND revision=?8 AND secret_owner_id=?9 AND created_at=?10",
                    params![
                        provider.id.to_string(),
                        kind_name(provider.config.provider_kind()),
                        provider.label,
                        provider.api_key_ref.map(|value| value.to_string()),
                        config_json,
                        i64::try_from(next.get()).map_err(corrupt)?,
                        provider.updated_at.get(),
                        i64::try_from(expected.get()).map_err(corrupt)?,
                        provider.secret_owner_id.as_uuid().to_string(),
                        provider.created_at.get(),
                    ],
                )
                .map_err(storage)?
        } else {
            if provider.revision != Revision::INITIAL {
                return Err(TtsConfigurationRepositoryError::InvalidData);
            }
            let exists = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM audio_providers WHERE id=?1)",
                    [provider.id.to_string()],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(storage)?;
            if exists {
                return Err(TtsConfigurationRepositoryError::AlreadyExists);
            }
            connection
                .execute(
                    "INSERT INTO audio_providers (
                        id, secret_owner_id, provider_kind, label, api_key_secret_ref,
                        config_json, revision, created_at, updated_at
                     ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                    params![
                        provider.id.to_string(),
                        provider.secret_owner_id.as_uuid().to_string(),
                        kind_name(provider.config.provider_kind()),
                        provider.label,
                        provider.api_key_ref.map(|value| value.to_string()),
                        config_json,
                        i64::try_from(provider.revision.get()).map_err(corrupt)?,
                        provider.created_at.get(),
                        provider.updated_at.get(),
                    ],
                )
                .map_err(storage)?
        };
        if changed == 0 {
            return Err(TtsConfigurationRepositoryError::StaleRevision);
        }
        drop(connection);
        self.get_audio_provider(provider.id)?
            .ok_or(TtsConfigurationRepositoryError::NotFound)
    }

    fn get_audio_provider(
        &self,
        id: AudioProviderId,
    ) -> Result<Option<AudioProvider>, TtsConfigurationRepositoryError> {
        self.connection()
            .map_err(storage)?
            .query_row(
                &format!("{PROVIDER_SELECT} WHERE id=?1"),
                [id.to_string()],
                provider_from_row,
            )
            .optional()
            .map_err(corrupt)
    }

    fn list_audio_providers(&self) -> Result<Vec<AudioProvider>, TtsConfigurationRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        let mut statement = connection
            .prepare(&format!(
                "{PROVIDER_SELECT} ORDER BY created_at DESC, id DESC"
            ))
            .map_err(storage)?;
        statement
            .query_map([], provider_from_row)
            .map_err(storage)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(corrupt)
    }

    fn delete_audio_provider(
        &self,
        id: AudioProviderId,
        expected_revision: Revision,
    ) -> Result<AudioProvider, TtsConfigurationRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(storage)?;
        let provider = transaction
            .query_row(
                &format!("{PROVIDER_SELECT} WHERE id=?1"),
                [id.to_string()],
                provider_from_row,
            )
            .optional()
            .map_err(corrupt)?
            .ok_or(TtsConfigurationRepositoryError::NotFound)?;
        if provider.revision != expected_revision {
            return Err(TtsConfigurationRepositoryError::StaleRevision);
        }
        transaction
            .execute(
                "DELETE FROM audio_providers WHERE id=?1 AND revision=?2",
                params![
                    id.to_string(),
                    i64::try_from(expected_revision.get()).map_err(corrupt)?
                ],
            )
            .map_err(storage)?;
        transaction.commit().map_err(storage)?;
        Ok(provider)
    }

    fn upsert_user_voice(
        &self,
        voice: UserVoice,
        expected_revision: Option<Revision>,
    ) -> Result<UserVoice, TtsConfigurationRepositoryError> {
        voice.validate().map_err(corrupt)?;
        let connection = self.connection().map_err(storage)?;
        let provider_exists = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM audio_providers WHERE id=?1)",
                [voice.provider_id.to_string()],
                |row| row.get::<_, bool>(0),
            )
            .map_err(storage)?;
        if !provider_exists {
            return Err(TtsConfigurationRepositoryError::ProviderMissing);
        }
        let changed = if let Some(expected) = expected_revision {
            if voice.revision != expected {
                return Err(TtsConfigurationRepositoryError::InvalidData);
            }
            let next = expected.next().map_err(storage)?;
            connection
                .execute(
                    "UPDATE user_voices SET provider_id=?2, name=?3, model_id=?4,
                        voice_id=?5, prompt=?6, revision=?7, updated_at=?8
                     WHERE id=?1 AND revision=?9 AND created_at=?10",
                    params![
                        voice.id.to_string(),
                        voice.provider_id.to_string(),
                        voice.name,
                        voice.model_id,
                        voice.voice_id,
                        voice.prompt,
                        i64::try_from(next.get()).map_err(corrupt)?,
                        voice.updated_at.get(),
                        i64::try_from(expected.get()).map_err(corrupt)?,
                        voice.created_at.get(),
                    ],
                )
                .map_err(storage)?
        } else {
            if voice.revision != Revision::INITIAL {
                return Err(TtsConfigurationRepositoryError::InvalidData);
            }
            let exists = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM user_voices WHERE id=?1)",
                    [voice.id.to_string()],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(storage)?;
            if exists {
                return Err(TtsConfigurationRepositoryError::AlreadyExists);
            }
            connection
                .execute(
                    "INSERT INTO user_voices (
                        id, provider_id, name, model_id, voice_id, prompt,
                        revision, created_at, updated_at
                     ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                    params![
                        voice.id.to_string(),
                        voice.provider_id.to_string(),
                        voice.name,
                        voice.model_id,
                        voice.voice_id,
                        voice.prompt,
                        i64::try_from(voice.revision.get()).map_err(corrupt)?,
                        voice.created_at.get(),
                        voice.updated_at.get(),
                    ],
                )
                .map_err(storage)?
        };
        if changed == 0 {
            return Err(TtsConfigurationRepositoryError::StaleRevision);
        }
        drop(connection);
        self.get_user_voice(voice.id)?
            .ok_or(TtsConfigurationRepositoryError::NotFound)
    }

    fn get_user_voice(
        &self,
        id: VoiceProfileId,
    ) -> Result<Option<UserVoice>, TtsConfigurationRepositoryError> {
        self.connection()
            .map_err(storage)?
            .query_row(
                &format!("{VOICE_SELECT} WHERE id=?1"),
                [id.to_string()],
                voice_from_row,
            )
            .optional()
            .map_err(corrupt)
    }

    fn list_user_voices(&self) -> Result<Vec<UserVoice>, TtsConfigurationRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        let mut statement = connection
            .prepare(&format!("{VOICE_SELECT} ORDER BY created_at DESC, id DESC"))
            .map_err(storage)?;
        statement
            .query_map([], voice_from_row)
            .map_err(storage)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(corrupt)
    }

    fn delete_user_voice(&self, id: VoiceProfileId) -> Result<(), TtsConfigurationRepositoryError> {
        let changed = self
            .connection()
            .map_err(storage)?
            .execute("DELETE FROM user_voices WHERE id=?1", [id.to_string()])
            .map_err(storage)?;
        if changed == 0 {
            return Err(TtsConfigurationRepositoryError::NotFound);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(now: i64) -> AudioProvider {
        AudioProvider {
            id: AudioProviderId::new(),
            secret_owner_id: SecretOwnerId::new(),
            label: "Primary".into(),
            api_key_ref: Some(SecretRef::new()),
            config: AudioProviderConfig::OpenAiCompatible {
                base_url: Some("https://audio.example".into()),
                request_path: Some("/v1/audio/speech".into()),
            },
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(now),
            updated_at: TimestampMillis::new(now),
        }
    }

    fn voice(provider_id: AudioProviderId, now: i64) -> UserVoice {
        UserVoice {
            id: VoiceProfileId::new(),
            provider_id,
            name: "Narrator".into(),
            model_id: "voice-model".into(),
            voice_id: "voice-one".into(),
            prompt: Some("Warm and clear".into()),
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(now),
            updated_at: TimestampMillis::new(now),
        }
    }

    #[test]
    fn provider_and_voice_round_trip_with_cas_and_cascade() {
        let database = Database::open_in_memory().expect("database");
        let mut provider = provider(10);
        let stored = database
            .upsert_audio_provider(provider.clone(), None)
            .expect("provider insert");
        assert_eq!(stored, provider);
        let mut voice = voice(provider.id, 11);
        database
            .upsert_user_voice(voice.clone(), None)
            .expect("voice insert");
        voice.prompt = Some("Calm and precise".into());
        voice.updated_at = TimestampMillis::new(12);
        let updated_voice = database
            .upsert_user_voice(voice.clone(), Some(Revision::INITIAL))
            .expect("voice update");
        assert_eq!(updated_voice.revision, Revision::new(2));
        assert_eq!(updated_voice.created_at, TimestampMillis::new(11));

        provider.label = "Updated".into();
        provider.updated_at = TimestampMillis::new(13);
        let mut updated = provider.clone();
        updated.revision = Revision::new(2);
        assert_eq!(
            database
                .upsert_audio_provider(provider.clone(), Some(Revision::INITIAL))
                .expect("provider update"),
            updated
        );
        assert_eq!(
            database.upsert_audio_provider(provider.clone(), Some(Revision::INITIAL)),
            Err(TtsConfigurationRepositoryError::StaleRevision)
        );
        let removed = database
            .delete_audio_provider(provider.id, Revision::new(2))
            .expect("provider delete");
        assert_eq!(removed, updated);
        assert_eq!(database.get_user_voice(voice.id).expect("voice get"), None);
        assert!(removed.api_key_ref.is_some());
    }

    #[test]
    fn voice_requires_an_existing_provider() {
        let database = Database::open_in_memory().expect("database");
        let voice = voice(AudioProviderId::new(), 1);
        assert_eq!(
            database.upsert_user_voice(voice, None),
            Err(TtsConfigurationRepositoryError::ProviderMissing)
        );
    }

    #[test]
    fn corrupt_provider_kind_is_rejected_on_read() {
        let database = Database::open_in_memory().expect("database");
        let provider = provider(1);
        database
            .upsert_audio_provider(provider.clone(), None)
            .expect("provider insert");
        database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE audio_providers SET provider_kind='fish_tts', revision=2 WHERE id=?1",
                [provider.id.to_string()],
            )
            .expect("corrupt provider kind");
        assert_eq!(
            database.get_audio_provider(provider.id),
            Err(TtsConfigurationRepositoryError::InvalidData)
        );
    }
}
