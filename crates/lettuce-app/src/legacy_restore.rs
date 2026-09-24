use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
};

use lettuce_database::Database;
use lettuce_media::LocalMediaBlobStore;
use lettuce_platform::{FilesystemAuthority, ManagedRoot, PlatformError};
use lettuce_settings::{SecretPurpose, SecretRef, SecretStore};
use lettuce_transfer::{
    BackupRestoreAdmission, BackupRestoreAdmissionRepository, BackupRestoreAdmissionRequest,
    BackupRestoreWorkspace, LegacyImportAssignment, LegacyImportRunStatus,
};
use lettuce_types::{LegacyImportRunId, OperationId, TimestampMillis};

use crate::{
    AppBackend, AppDatabaseLocation, AppDatabaseLocationError, DATABASE_EXTENSION,
    LegacyDatabaseImportPlan,
};

#[derive(Debug)]
pub struct LegacyRestoreReceipt {
    pub database_path: PathBuf,
    pub previous_database_path: PathBuf,
    pub run_id: LegacyImportRunId,
    pub admission: Option<BackupRestoreAdmission>,
}

#[derive(Debug, thiserror::Error)]
pub enum LegacyRestoreError {
    #[error("legacy backup could not be decoded: {0:?}")]
    Inventory(lettuce_transfer::LegacyBackupInventoryError),
    #[error("legacy backup could not be planned: {0:?}")]
    Compatibility(lettuce_transfer::LegacyBackupCompatibilityError),
    #[error("legacy app data could not be planned: {0:?}")]
    Plan(crate::LegacyDatabaseImportPlanError),
    #[error("legacy backup could not be staged: {0:?}")]
    Workspace(lettuce_transfer::BackupRestoreWorkspaceError),
    #[error("restore admission is invalid: {0:?}")]
    Admission(lettuce_transfer::BackupRestoreAdmissionError),
    #[error("database location is unavailable: {0:?}")]
    Location(AppDatabaseLocationError),
    #[error("restore target database already exists")]
    TargetExists,
    #[error("restore target directory is unavailable")]
    TargetDirectory,
    #[error("restore database could not be opened: {0:?}")]
    Open(crate::AppInitializationError),
    #[error("restore database is unavailable: {0:?}")]
    Database(lettuce_database::DatabaseError),
    #[error("media store is unavailable: {0:?}")]
    MediaStore(PlatformError),
    #[error("legacy import stage {stage} failed: {detail}")]
    Stage { stage: &'static str, detail: String },
    #[error("legacy import did not complete")]
    Incomplete,
}

fn stage<E: fmt::Debug>(name: &'static str) -> impl FnOnce(E) -> LegacyRestoreError {
    move |error| LegacyRestoreError::Stage {
        stage: name,
        detail: format!("{error:?}"),
    }
}

/// Replaces the app data with a legacy source (user decision 2026-09-14: a
/// legacy backup replaces, never imports alongside). The whole legacy import
/// chain runs into a new database file; only a completed run switches the
/// active database, and the previous file is never deleted. Provider and audio
/// secrets use references unique to the import run, so a failed attempt deletes
/// them again and a successful one never shares a record with the previous
/// database.
pub struct LegacyRestoreCoordinator<'a, S: ?Sized> {
    location: &'a AppDatabaseLocation,
    authority: &'a FilesystemAuthority,
    workspace_root: &'a Path,
    secrets: &'a S,
}

impl<S: ?Sized> fmt::Debug for LegacyRestoreCoordinator<'_, S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LegacyRestoreCoordinator")
            .finish_non_exhaustive()
    }
}

impl<'a, S: SecretStore + ?Sized> LegacyRestoreCoordinator<'a, S> {
    #[must_use]
    pub const fn new(
        location: &'a AppDatabaseLocation,
        authority: &'a FilesystemAuthority,
        workspace_root: &'a Path,
        secrets: &'a S,
    ) -> Self {
        Self {
            location,
            authority,
            workspace_root,
            secrets,
        }
    }

    /// Replaces the app data with a version-1 legacy backup archive.
    pub async fn restore_backup(
        &self,
        restore_id: OperationId,
        backup: impl lettuce_transfer::BackupSource + 'static,
        password: &str,
        restored_at: TimestampMillis,
    ) -> Result<LegacyRestoreReceipt, LegacyRestoreError> {
        let inventory = lettuce_transfer::decode_legacy_backup_inventory(backup, password)
            .map_err(LegacyRestoreError::Inventory)?;
        let compatibility = lettuce_transfer::plan_legacy_backup_compatibility(inventory)
            .map_err(LegacyRestoreError::Compatibility)?;
        let staging =
            BackupRestoreWorkspace::open(self.workspace_root.join(restore_id.to_string()))
                .and_then(|workspace| workspace.stage_legacy(&compatibility))
                .map_err(LegacyRestoreError::Workspace)?;
        let admission = lettuce_transfer::legacy_backup_restore_admission(
            restore_id,
            &compatibility,
            &staging,
            restored_at,
        )
        .map_err(LegacyRestoreError::Admission)?;
        let plan = compatibility.legacy_import_plan();
        self.replace(
            restore_id,
            &LegacyDatabaseImportPlan {
                compatibility,
                plan,
            },
            None,
            Some(admission),
            restored_at,
        )
        .await
    }

    /// Replaces the app data with a live legacy app data directory.
    pub async fn restore_database(
        &self,
        restore_id: OperationId,
        app_data_dir: &Path,
        restored_at: TimestampMillis,
    ) -> Result<LegacyRestoreReceipt, LegacyRestoreError> {
        let import =
            crate::plan_legacy_database_import(app_data_dir).map_err(LegacyRestoreError::Plan)?;
        self.replace(
            restore_id,
            &import,
            Some(&app_data_dir.join("lettuce")),
            None,
            restored_at,
        )
        .await
    }

    async fn replace(
        &self,
        restore_id: OperationId,
        import: &LegacyDatabaseImportPlan,
        storage_root: Option<&Path>,
        admission: Option<BackupRestoreAdmissionRequest>,
        restored_at: TimestampMillis,
    ) -> Result<LegacyRestoreReceipt, LegacyRestoreError> {
        let previous_database_path = self
            .location
            .active_path()
            .map_err(LegacyRestoreError::Location)?;
        let name = format!("{restore_id}{DATABASE_EXTENSION}");
        let database_path = self
            .location
            .database_path(&name)
            .map_err(LegacyRestoreError::Location)?;
        if database_path
            .try_exists()
            .map_err(|_| LegacyRestoreError::TargetDirectory)?
        {
            return Err(LegacyRestoreError::TargetExists);
        }
        std::fs::create_dir_all(
            database_path
                .parent()
                .ok_or(LegacyRestoreError::TargetDirectory)?,
        )
        .map_err(|_| LegacyRestoreError::TargetDirectory)?;
        let backend =
            AppBackend::open(&database_path, restored_at).map_err(LegacyRestoreError::Open)?;
        let run_id = LegacyImportRunId::new();
        let mut written = Vec::new();
        let outcome = async {
            self.import(
                &backend,
                &database_path,
                import,
                storage_root,
                run_id,
                restored_at,
                &mut written,
            )
            .await?;
            if previous_database_path
                .try_exists()
                .map_err(|_| LegacyRestoreError::TargetDirectory)?
            {
                backend
                    .database()
                    .carry_device_local_state_from(&previous_database_path)
                    .map_err(LegacyRestoreError::Database)?;
            }
            let admission = admission
                .map(|request| backend.database().admit_backup_restore(request))
                .transpose()
                .map_err(LegacyRestoreError::Admission)?;
            self.location
                .activate(&name)
                .map_err(LegacyRestoreError::Location)?;
            Ok(admission)
        }
        .await;
        drop(backend);
        let admission = match outcome {
            Ok(admission) => admission,
            Err(error) => {
                for (reference, purpose) in written {
                    let _ = self.secrets.delete(&reference, &purpose, None).await;
                }
                return Err(error);
            }
        };
        Ok(LegacyRestoreReceipt {
            database_path,
            previous_database_path,
            run_id,
            admission,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn import(
        &self,
        backend: &AppBackend,
        database_path: &Path,
        import: &LegacyDatabaseImportPlan,
        storage_root: Option<&Path>,
        run_id: LegacyImportRunId,
        at: TimestampMillis,
        written: &mut Vec<(SecretRef, SecretPurpose)>,
    ) -> Result<(), LegacyRestoreError> {
        let source = &import.compatibility;
        let plan = &import.plan;
        let admission = backend
            .legacy_import_admission()
            .admit(run_id, &source.database_inventory(), plan, at)
            .map_err(stage("admission"))?;
        let owners = admission
            .assignments
            .iter()
            .filter_map(|assignment| match assignment {
                LegacyImportAssignment::ProviderAccount {
                    legacy_id,
                    secret_owner_id,
                    ..
                } => Some((*legacy_id, *secret_owner_id)),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        written.extend(
            admission
                .assignments
                .iter()
                .filter_map(|assignment| match assignment {
                    LegacyImportAssignment::ProviderSecret {
                        source,
                        destination_ref,
                    } => owners.get(&source.provider_account_id).map(|owner| {
                        let purpose = source.secret.purpose(*owner);
                        (*destination_ref, purpose)
                    }),
                    _ => None,
                }),
        );
        if let Some(fingerprint) = &plan.source_fingerprint {
            let scope = lettuce_transfer::LegacyIdScope::new(fingerprint);
            written.extend(
                source
                    .authored_plan()
                    .configuration
                    .audio_providers
                    .iter()
                    .filter_map(|provider| {
                        crate::legacy_audio_import::legacy_audio_secret(run_id, scope, provider)
                    }),
            );
        }
        let media_store = LocalMediaBlobStore::new(
            self.authority.managed_files(),
            self.authority
                .read_capability(ManagedRoot::MediaBlobs)
                .map_err(LegacyRestoreError::MediaStore)?,
            self.authority
                .write_capability(ManagedRoot::MediaBlobs)
                .map_err(LegacyRestoreError::MediaStore)?,
            Database::open(database_path).map_err(LegacyRestoreError::Database)?,
            Database::open(database_path).map_err(LegacyRestoreError::Database)?,
        );
        backend
            .legacy_media_importer(&media_store)
            .execute_from_source(source, storage_root, &admission, &plan.media, at)
            .map_err(stage("media"))?;
        backend
            .legacy_provider_secret_importer(&source.authored_plan().configuration, self.secrets)
            .execute(&admission, at)
            .await
            .map_err(stage("provider secrets"))?;
        backend
            .legacy_import_executor()
            .execute(&admission, plan, at)
            .map_err(stage("authored graph"))?;
        backend
            .legacy_asr_importer()
            .execute(&admission, plan, at)
            .map_err(stage("speech learning"))?;
        backend
            .legacy_provider_model_importer(self.secrets)
            .execute(&admission, plan, at)
            .await
            .map_err(stage("provider models"))?;
        backend
            .legacy_character_importer()
            .execute_database_import(&admission, import, at)
            .map_err(stage("characters"))?;
        backend
            .legacy_group_importer()
            .execute_database_import(&admission, import, at)
            .map_err(stage("groups"))?;
        backend
            .legacy_audio_importer(self.secrets)
            .execute_database_import(&admission, import, at)
            .await
            .map_err(stage("audio"))?;
        crate::legacy_app_secret_import::store_legacy_app_secrets(
            self.secrets,
            &source.authored_plan().configuration.secrets,
            written,
        )
        .await
        .map_err(stage("app secrets"))?;
        backend
            .legacy_settings_importer()
            .execute_database_import(&admission, import, at)
            .map_err(stage("settings"))?;
        backend
            .legacy_direct_conversation_importer()
            .execute_database_import(&admission, import, at)
            .map_err(stage("direct conversations"))?;
        backend
            .legacy_group_conversation_importer()
            .execute_database_import(&admission, import, at)
            .map_err(stage("group conversations"))?;
        backend
            .legacy_usage_importer()
            .execute_database_import(&admission, import, at)
            .map_err(stage("usage records"))?;
        backend
            .legacy_creation_importer()
            .execute_database_import(&admission, import, at)
            .map_err(stage("creation helper"))?;
        backend
            .legacy_image_importer()
            .execute_database_import(&admission, import, at)
            .map_err(stage("images"))?;
        backend
            .legacy_llm_metrics_importer()
            .execute_database_import(&admission, import, at)
            .map_err(stage("local generation metrics"))?;
        match backend
            .complete_legacy_import(run_id, at)
            .map_err(stage("completion"))?
        {
            LegacyImportRunStatus::Completed => Ok(()),
            _ => Err(LegacyRestoreError::Incomplete),
        }
    }
}

#[cfg(test)]
mod tests {
    use lettuce_platform::DirectorySnapshot;
    use lettuce_settings::InMemorySecretStore;
    use lettuce_transfer::{LegacyBackupInventory, ProviderBackupSource};
    use lettuce_types::ContentHash;

    use super::*;

    #[tokio::test]
    async fn a_legacy_source_with_provider_secrets_replaces_the_active_database() {
        let root = std::env::temp_dir().join(format!("legacy-restore-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("fixture root");
        let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
            .expect("filesystem authority");
        let location = AppDatabaseLocation::new(root.join("private-persistent-v2"), &authority)
            .expect("database location");
        let secrets = InMemorySecretStore::new();
        let workspace = root.join("legacy-restore-workspace");
        let coordinator =
            LegacyRestoreCoordinator::new(&location, &authority, &workspace, &secrets);
        let provider_id = lettuce_types::ProviderAccountId::new();
        let credentials = serde_json::json!([{
            "id": provider_id,
            "provider_id": "openrouter",
            "label": "Router",
            "api_key_ref": null,
            "api_key": "provider-secret",
            "base_url": "https://openrouter.ai/api/v1",
            "default_model": null,
            "headers": "{\"X-Client\":\"header-secret\"}",
            "config": null
        }]);
        let compatibility =
            lettuce_transfer::plan_legacy_backup_compatibility(LegacyBackupInventory {
                version: 2,
                created_at: 1_700_000_000_000,
                app_version: "1.0.0".into(),
                source_hash: ContentHash::parse("cd".repeat(32)).expect("source hash"),
                documents: vec![lettuce_transfer::LegacyBackupDocument {
                    kind: lettuce_transfer::LegacyBackupDocumentKind::ProviderCredentials,
                    bytes: zeroize::Zeroizing::new(
                        serde_json::to_vec(&credentials).expect("credentials document"),
                    ),
                }],
                media: Vec::new(),
            })
            .expect("compatibility plan");
        let plan = compatibility.legacy_import_plan();
        let receipt = coordinator
            .replace(
                OperationId::new(),
                &LegacyDatabaseImportPlan {
                    compatibility,
                    plan,
                },
                None,
                None,
                TimestampMillis::new(1_700_000_000_100),
            )
            .await
            .expect("replace with legacy source");
        let restored = Database::open(&receipt.database_path).expect("restored database");
        let graph = restored
            .read_provider_backup_graph()
            .expect("restored graph");
        assert_eq!(graph.accounts.len(), 1);
        let account = &graph.accounts[0];
        let api_key = secrets
            .load(
                &account.api_key_ref.expect("api key reference"),
                &lettuce_settings::SecretPurpose::ProviderApiKey {
                    owner: account.secret_owner_id,
                },
            )
            .await
            .expect("restored api key");
        assert!(api_key.with(|value| value == "provider-secret"));
        assert_eq!(account.secret_headers.len(), 1);
        assert_eq!(
            lettuce_transfer::backup_sql_text(&graph.legacy_imports.runs[0].run, "status"),
            Some("completed")
        );
    }

    const PNG: [u8; 70] = [
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 218, 99, 100, 96, 248, 95, 15,
        0, 2, 135, 1, 128, 235, 71, 186, 146, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];

    fn legacy_message(
        id: &str,
        role: &str,
        parent: Option<&str>,
        content: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "role": role,
            "content": content,
            "created_at": 10,
            "effective_at": 11,
            "visible_in_chat": true,
            "scene_edited": false,
            "prompt_tokens": null,
            "completion_tokens": null,
            "total_tokens": null,
            "first_token_ms": null,
            "tokens_per_second": null,
            "mtp_stats": null,
            "model_id": null,
            "selected_variant_id": null,
            "is_pinned": false,
            "memory_refs": "[]",
            "used_lorebook_entries": "[]",
            "attachments": "[]",
            "reasoning": null,
            "parent_message_id": parent,
            "variants": []
        })
    }

    #[tokio::test]
    async fn a_legacy_source_with_media_characters_and_conversations_replaces_the_database() {
        let root = std::env::temp_dir().join(format!("legacy-restore-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("fixture root");
        let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
            .expect("filesystem authority");
        let location = AppDatabaseLocation::new(root.join("private-persistent-v2"), &authority)
            .expect("database location");
        let secrets = InMemorySecretStore::new();
        let workspace = root.join("legacy-restore-workspace");
        let coordinator =
            LegacyRestoreCoordinator::new(&location, &authority, &workspace, &secrets);
        let persona = uuid::Uuid::from_u128(1).to_string();
        let character = uuid::Uuid::from_u128(2).to_string();
        let session = uuid::Uuid::from_u128(3).to_string();
        let first_message = uuid::Uuid::from_u128(4).to_string();
        let second_message = uuid::Uuid::from_u128(5).to_string();
        let attachment = uuid::Uuid::from_u128(6).to_string();
        let document = |kind, value: serde_json::Value| lettuce_transfer::LegacyBackupDocument {
            kind,
            bytes: zeroize::Zeroizing::new(serde_json::to_vec(&value).expect("document")),
        };
        let media = |root, segments: &[&str]| {
            lettuce_transfer::LegacyBackupMedia::from_bytes(
                root,
                segments
                    .iter()
                    .map(|segment| (*segment).to_owned())
                    .collect(),
                zeroize::Zeroizing::new(PNG.to_vec()),
            )
        };
        let mut reply = legacy_message(
            &second_message,
            "assistant",
            Some(&first_message),
            "Hello there",
        );
        reply["attachments"] = serde_json::json!([{
            "id": attachment,
            "data": "",
            "mimeType": "image/png",
            "filename": "a lighthouse at dusk",
            "storagePath": format!("sessions/{character}/{session}/ai_{second_message}_{attachment}.webp")
        }])
        .to_string()
        .into();
        let playground_image = uuid::Uuid::from_u128(7).to_string();
        let playground = serde_json::json!([{
            "id": "playground-1",
            "created_at": 30,
            "provider_id": "sdcpp",
            "model_id": "missing-model",
            "model_name": "Flux",
            "prompt": "a lighthouse",
            "negative_prompt": null,
            "seed": 42,
            "params_json": "{\"size\":\"512x512\"}",
            "status": "complete",
            "error": null,
            "images_json": serde_json::json!([
                {"assetId": playground_image, "filePath": "/old/images/x.png", "mimeType": "image/png", "width": 1, "height": 1},
                {"assetId": "gone-image", "filePath": "/old/images/gone.png", "mimeType": "image/png"}
            ])
            .to_string()
        }]);
        let background = format!(
            "data:image/png;base64,{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, PNG)
        );
        let mut greeting = legacy_message(&first_message, "user", None, "Hello Mira");
        greeting["attachments"] = reply["attachments"].clone();
        let compatibility =
            lettuce_transfer::plan_legacy_backup_compatibility(LegacyBackupInventory {
                version: 1,
                created_at: 1,
                app_version: "legacy".into(),
                source_hash: ContentHash::parse("ef".repeat(32)).expect("source hash"),
                documents: vec![
                    document(
                        lettuce_transfer::LegacyBackupDocumentKind::PlaygroundGenerations,
                        playground,
                    ),
                    document(
                        lettuce_transfer::LegacyBackupDocumentKind::Personas,
                        serde_json::json!([{
                            "id": persona,
                            "title": "User",
                            "description": "User profile",
                            "avatar_path": "persona.png",
                            "created_at": 1,
                            "updated_at": 1
                        }]),
                    ),
                    document(
                        lettuce_transfer::LegacyBackupDocumentKind::Characters,
                        serde_json::json!([{
                            "id": character,
                            "name": "Mira",
                            "avatar_path": "mira.png",
                            "created_at": 1,
                            "updated_at": 1
                        }]),
                    ),
                    document(
                        lettuce_transfer::LegacyBackupDocumentKind::Sessions,
                        serde_json::json!([{
                            "id": session,
                            "character_id": character,
                            "title": "Conversation",
                            "parent_session_id": null,
                            "branched_from_message_id": null,
                            "root_session_id": session,
                            "background_image_path": background,
                            "system_prompt": null,
                            "mode": "roleplay",
                            "selected_scene_id": null,
                            "author_note": null,
                            "persona_id": persona,
                            "persona_disabled": false,
                            "voice_autoplay": false,
                            "prompt_template_id": null,
                            "lorebook_ids_override": "[]",
                            "temperature": null,
                            "top_p": null,
                            "max_output_tokens": null,
                            "frequency_penalty": null,
                            "presence_penalty": null,
                            "top_k": null,
                            "advanced_model_settings": "{\"temperature\":0.55,\"llamaThreads\":8}",
                            "companion_state": "{\"preferences\":{\"timeAwarenessEnabled\":true,\"timeOverride\":{\"mode\":\"frozen\",\"anchorMs\":5000,\"setAtMs\":1}}}",
                            "memories": "[]",
                            "memory_embeddings": "[]",
                            "memory_summary": "They met at the lighthouse.",
                            "memory_summary_token_count": 0,
                            "memory_tool_events": serde_json::json!([
                                {"status": "success", "windowMessageIds": [first_message]},
                                {"status": "error", "windowMessageIds": [second_message]},
                                {"revertedAt": 5, "windowMessageIds": [second_message]}
                            ])
                            .to_string(),
                            "memory_status": null,
                            "memory_error": null,
                            "memory_progress_step": 0,
                            "archived": false,
                            "created_at": 1,
                            "updated_at": 20,
                            "messages": [
                                greeting,
                                reply
                            ]
                        }]),
                    ),
                ],
                media: vec![
                    media(
                        lettuce_transfer::LegacyBackupMediaRoot::Avatars,
                        &[&format!("persona-{persona}"), "persona.png"],
                    ),
                    media(
                        lettuce_transfer::LegacyBackupMediaRoot::Avatars,
                        &[&format!("character-{character}"), "mira.png"],
                    ),
                    media(
                        lettuce_transfer::LegacyBackupMediaRoot::Images,
                        &[&format!("{playground_image}.png")],
                    ),
                    media(
                        lettuce_transfer::LegacyBackupMediaRoot::Sessions,
                        &[
                            character.as_str(),
                            session.as_str(),
                            &format!("ai_{second_message}_{attachment}.webp"),
                        ],
                    ),
                ],
            })
            .expect("compatibility plan");
        let plan = compatibility.legacy_import_plan();
        assert_eq!(plan.media.media.len(), 5);
        let receipt = coordinator
            .replace(
                OperationId::new(),
                &LegacyDatabaseImportPlan {
                    compatibility,
                    plan,
                },
                None,
                None,
                TimestampMillis::new(1_700_000_000_100),
            )
            .await
            .expect("replace with legacy source");
        assert_eq!(
            location.active_path().expect("active database"),
            receipt.database_path
        );
        let restored = Database::open(&receipt.database_path).expect("restored database");
        let graph = restored
            .read_provider_backup_graph()
            .expect("restored graph");
        assert_eq!(graph.authored.personas.len(), 1);
        assert_eq!(graph.authored.characters.len(), 1);
        assert_eq!(graph.authored.media_assets.len(), 5);
        let playground = &graph.playground_history;
        assert_eq!(playground.entries.len(), 1);
        assert_eq!(
            lettuce_transfer::backup_sql_text(&playground.entries[0], "origin"),
            Some("imported")
        );
        assert_eq!(
            lettuce_transfer::backup_sql_text(&playground.entries[0], "params_json"),
            Some("{\"size\":\"512x512\"}")
        );
        assert_eq!(playground.images.len(), 2);
        let generated = lettuce_transfer::backup_sql_text(&playground.images[0], "asset_id")
            .expect("imported playground image")
            .parse::<lettuce_types::AssetId>()
            .expect("asset id");
        assert_eq!(
            lettuce_media::MediaAssetRepository::get(&restored, generated)
                .expect("read asset")
                .expect("playground asset")
                .kind,
            lettuce_media::AssetKind::GeneratedImage
        );
        assert_eq!(
            playground.images[1].get("asset_id"),
            Some(&lettuce_transfer::BackupSqlValue::Null)
        );
        assert_eq!(
            lettuce_transfer::backup_sql_text(&playground.images[1], "source_asset_id"),
            Some("gone-image")
        );
        let reply = graph.conversation_history.conversations[0]
            .messages
            .iter()
            .find(|message| message.message.role == lettuce_conversations::MessageRole::Assistant)
            .expect("assistant reply");
        let attached = reply.revisions[0]
            .parts
            .iter()
            .find_map(|part| match part {
                lettuce_conversations::MessagePart::MediaAsset { asset_id, role } => {
                    Some((*asset_id, *role))
                }
                _ => None,
            })
            .expect("legacy attachment part");
        assert_eq!(
            attached.1,
            lettuce_conversations::MediaAssetRole::Attachment
        );
        let greeting = graph.conversation_history.conversations[0]
            .messages
            .iter()
            .find(|message| message.message.role == lettuce_conversations::MessageRole::User)
            .expect("user greeting");
        assert!(greeting.revisions[0].parts.contains(
            &lettuce_conversations::MessagePart::MediaAsset {
                asset_id: attached.0,
                role: lettuce_conversations::MediaAssetRole::Attachment,
            }
        ));
        let asset = lettuce_media::MediaAssetRepository::get(&restored, attached.0)
            .expect("read asset")
            .expect("attachment asset");
        assert_eq!(asset.kind, lettuce_media::AssetKind::MessageImage);
        assert_eq!(
            asset.provenance.source_label.as_deref(),
            Some("a lighthouse at dusk")
        );
        assert_eq!(graph.conversation_history.conversations.len(), 1);
        assert_eq!(
            graph.conversation_history.conversations[0].messages.len(),
            2
        );
        let settings = graph.conversation_history.conversations[0]
            .aggregate
            .conversation
            .current_settings
            .as_ref()
            .expect("conversation settings");
        assert_eq!(
            settings.model_settings.chat_parameters.temperature,
            Some(0.55)
        );
        assert_eq!(settings.model_settings.llama_cpp.threads, Some(8));
        assert_eq!(
            settings.companion_clock,
            Some(lettuce_conversations::CompanionClockSettings {
                time_awareness_enabled: true,
                time_override: lettuce_conversations::CompanionTimeOverride::Frozen {
                    anchor_at: TimestampMillis::new(5_000),
                },
            })
        );
        let Some(lettuce_conversations::ConversationBackground::Image { asset_id }) =
            settings.background
        else {
            panic!("imported conversation background");
        };
        assert_eq!(
            lettuce_media::MediaAssetRepository::get(&restored, asset_id)
                .expect("read asset")
                .expect("background asset")
                .kind,
            lettuce_media::AssetKind::OtherImage
        );
        let summary = graph.memory.spaces[0]
            .summary
            .as_ref()
            .expect("imported memory summary");
        assert_eq!((summary.window_start, summary.window_end), (0, 1));
        let mut exported = graph.clone();
        lettuce_transfer::canonicalize_and_validate(&mut exported)
            .expect("a legacy-restored database backs up again");
        assert_eq!(
            lettuce_transfer::backup_sql_text(&graph.legacy_imports.runs[0].run, "status"),
            Some("completed")
        );
    }

    #[tokio::test]
    async fn legacy_metrics_follow_their_direct_and_group_messages_and_survive_a_second_restore() {
        let root = std::env::temp_dir().join(format!("legacy-restore-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("fixture root");
        let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
            .expect("filesystem authority");
        let location = AppDatabaseLocation::new(root.join("private-persistent-v2"), &authority)
            .expect("database location");
        let secrets = InMemorySecretStore::new();
        let workspace = root.join("legacy-restore-workspace");
        let coordinator =
            LegacyRestoreCoordinator::new(&location, &authority, &workspace, &secrets);
        let id = |value: u128| uuid::Uuid::from_u128(value).to_string();
        let (provider, model, ada, grace, session, group, group_session) =
            (id(1), id(2), id(3), id(4), id(5), id(6), id(7));
        let (greeting, reply, group_greeting, group_reply) = (id(10), id(11), id(12), id(13));
        let document = |kind, value: serde_json::Value| lettuce_transfer::LegacyBackupDocument {
            kind,
            bytes: zeroize::Zeroizing::new(serde_json::to_vec(&value).expect("document")),
        };
        let variant = |variant: &str, created_at: i64| {
            serde_json::json!({
                "id": variant,
                "content": format!("take {variant}"),
                "speaker_character_id": ada,
                "created_at": created_at,
                "attachments": "[]"
            })
        };
        let mut direct_reply =
            legacy_message(&reply, "assistant", Some(&greeting), "take variant-a");
        direct_reply["variants"] =
            serde_json::json!([variant("variant-a", 12), variant("variant-b", 13)]);
        direct_reply["selected_variant_id"] = "variant-a".into();
        let group_message = |message: &str, role: &str, parent: Option<&str>, variants| {
            serde_json::json!({
                "id": message,
                "role": role,
                "content": "hello",
                "speaker_character_id": (role == "assistant").then_some(&ada),
                "turn_number": 1,
                "created_at": 20,
                "selected_variant_id": (role == "assistant").then_some("group-variant"),
                "is_pinned": false,
                "attachments": "[]",
                "used_lorebook_entries": "[]",
                "memory_refs": "[]",
                "parent_message_id": parent,
                "variants": variants
            })
        };
        let members = serde_json::to_string(&[&ada, &grace]).expect("members");
        let metric = |metric: &str, created_at: i64, message: Option<&str>| {
            serde_json::json!({
                "id": metric,
                "created_at": created_at,
                "model_name": "/models/local.gguf",
                "summary_json": format!("{{\"completionTokens\":{created_at}}}"),
                "samples_json": "[]",
                "message_id": message
            })
        };
        let source = |title: &str| {
            lettuce_transfer::plan_legacy_backup_compatibility(LegacyBackupInventory {
                version: 1,
                created_at: 1,
                app_version: "legacy".into(),
                source_hash: ContentHash::parse("ef".repeat(32)).expect("source hash"),
                documents: vec![
                    document(
                        lettuce_transfer::LegacyBackupDocumentKind::ProviderCredentials,
                        serde_json::json!([{
                            "id": provider,
                            "provider_id": "openrouter",
                            "label": "Router",
                            "api_key_ref": null,
                            "api_key": "provider-secret",
                            "base_url": "https://openrouter.ai/api/v1",
                            "default_model": null,
                            "headers": null,
                            "config": null
                        }]),
                    ),
                    document(
                        lettuce_transfer::LegacyBackupDocumentKind::Models,
                        serde_json::json!([{
                            "id": model,
                            "name": "gpt-4o",
                            "provider_id": "openrouter",
                            "provider_credential_id": provider,
                            "provider_label": "Router",
                            "display_name": "GPT-4o",
                            "created_at": 1,
                            "model_type": "chat",
                            "input_scopes": "[\"text\"]",
                            "output_scopes": "[\"text\"]"
                        }]),
                    ),
                    document(
                        lettuce_transfer::LegacyBackupDocumentKind::Settings,
                        serde_json::json!({
                            "default_provider_credential_id": provider,
                            "default_model_id": model,
                            "app_state": {},
                            "advanced_model_settings": null,
                            "prompt_template_id": null,
                            "system_prompt": null,
                            "migration_version": 92,
                            "advanced_settings": null,
                            "created_at": 1,
                            "updated_at": 1
                        }),
                    ),
                    document(
                        lettuce_transfer::LegacyBackupDocumentKind::Characters,
                        serde_json::json!([
                            {"id": ada, "name": "Ada", "created_at": 1, "updated_at": 1},
                            {"id": grace, "name": "Grace", "created_at": 1, "updated_at": 1}
                        ]),
                    ),
                    document(
                        lettuce_transfer::LegacyBackupDocumentKind::Sessions,
                        serde_json::json!([{
                            "id": session,
                            "character_id": ada,
                            "title": title,
                            "mode": "roleplay",
                            "persona_disabled": false,
                            "lorebook_ids_override": "[]",
                            "memories": "[]",
                            "memory_embeddings": "[]",
                            "memory_summary_token_count": 0,
                            "memory_tool_events": "[]",
                            "archived": false,
                            "created_at": 1,
                            "updated_at": 20,
                            "messages": [
                                legacy_message(&greeting, "user", None, "Hello Ada"),
                                direct_reply.clone()
                            ]
                        }]),
                    ),
                    document(
                        lettuce_transfer::LegacyBackupDocumentKind::GroupCharacters,
                        serde_json::json!([{
                            "id": group,
                            "name": "Room",
                            "character_ids": members,
                            "muted_character_ids": "[]",
                            "created_at": 1,
                            "updated_at": 1,
                            "archived": false,
                            "chat_type": "conversation",
                            "lorebook_ids": "[]",
                            "disable_character_lorebooks": false,
                            "speaker_selection_method": "llm",
                            "memory_type": "manual"
                        }]),
                    ),
                    document(
                        lettuce_transfer::LegacyBackupDocumentKind::GroupSessions,
                        serde_json::json!([{
                            "id": group_session,
                            "group_character_id": group,
                            "name": "Room chat",
                            "character_ids": members,
                            "muted_character_ids": "[]",
                            "created_at": 1,
                            "updated_at": 1,
                            "archived": false,
                            "chat_type": "conversation",
                            "lorebook_ids": "[]",
                            "disable_character_lorebooks": false,
                            "memories": "[]",
                            "memory_embeddings": "[]",
                            "memory_summary": "",
                            "memory_summary_token_count": 0,
                            "memory_tool_events": "[]",
                            "speaker_selection_method": "llm",
                            "memory_type": "manual",
                            "config_overrides": "{\"version\":1}",
                            "character_model_overrides": "{}",
                            "participation": [],
                            "messages": [
                                group_message(&group_greeting, "user", None, serde_json::json!([])),
                                group_message(
                                    &group_reply,
                                    "assistant",
                                    Some(&group_greeting),
                                    serde_json::json!([variant("group-variant", 21)])
                                )
                            ]
                        }]),
                    ),
                    document(
                        lettuce_transfer::LegacyBackupDocumentKind::LlmGenerationMetrics,
                        serde_json::json!([
                            metric("reply-old", 30, Some(&reply)),
                            metric("reply-new", 31, Some(&reply)),
                            metric("group-reply", 32, Some(&group_reply)),
                            metric("greeting", 33, Some(&greeting)),
                            metric("gone", 34, Some("deleted-message")),
                            metric("free", 35, None)
                        ]),
                    ),
                ],
                media: Vec::new(),
            })
            .expect("compatibility plan")
        };
        let import_of = |title: &str| {
            let compatibility = source(title);
            assert_eq!(compatibility.llm_metrics.metrics.len(), 6);
            let plan = compatibility.legacy_import_plan();
            let scope = lettuce_transfer::LegacyIdScope::new(
                plan.source_fingerprint
                    .as_ref()
                    .expect("source fingerprint"),
            );
            (
                LegacyDatabaseImportPlan {
                    compatibility,
                    plan,
                },
                scope,
            )
        };
        let (import, scope) = import_of("Conversation");
        let (changed, changed_scope) = import_of("Renamed conversation");
        assert_ne!(scope, changed_scope);
        let check = |database: &Database, scope: lettuce_transfer::LegacyIdScope| {
            let graph = database
                .read_provider_backup_graph()
                .expect("restored graph");
            let found = |legacy: &str| {
                let message_id = lettuce_types::MessageId::from_uuid(scope.source(legacy));
                let conversation = graph
                    .conversation_history
                    .conversations
                    .iter()
                    .find(|conversation| {
                        conversation
                            .messages
                            .iter()
                            .any(|message| message.message.id == message_id)
                    })
                    .expect("imported message");
                let message = conversation
                    .messages
                    .iter()
                    .find(|message| message.message.id == message_id)
                    .expect("imported message");
                let attempt = match message.message.active_render_source {
                    lettuce_conversations::MessageRenderSource::Candidate(active) => message
                        .candidates
                        .iter()
                        .find(|candidate| candidate.id == active)
                        .map(|candidate| candidate.attempt_id.to_string()),
                    lettuce_conversations::MessageRenderSource::Revision(_) => None,
                };
                let metric = database
                    .llm_generation_metric_for_message(
                        &conversation.aggregate.conversation.id.to_string(),
                        &message_id.to_string(),
                    )
                    .expect("metric by message");
                if let Some(metric) = &metric {
                    assert_eq!(Some(&metric.id), attempt.as_ref());
                }
                metric
            };
            let reply_metric = found(&reply).expect("the reply's metric");
            assert_eq!(reply_metric.created_at, 31);
            assert_eq!(
                reply_metric.summary,
                serde_json::json!({"completionTokens": 31})
            );
            assert_eq!(
                reply_metric.id,
                lettuce_types::GenerationAttemptId::from_uuid(
                    scope.derived("variant-a", "attempt")
                )
                .to_string()
            );
            let group_metric = found(&group_reply).expect("the group reply's metric");
            assert_eq!(group_metric.created_at, 32);
            assert_ne!(group_metric.id, "group-reply");
            assert_eq!(found(&greeting), None);
            let mut ids = database
                .llm_generation_metrics(None)
                .expect("metrics")
                .into_iter()
                .map(|metric| metric.id)
                .collect::<Vec<_>>();
            ids.sort();
            let mut expected = vec![
                "free".to_owned(),
                "gone".to_owned(),
                "greeting".to_owned(),
                "reply-old".to_owned(),
                reply_metric.id,
                group_metric.id,
            ];
            expected.sort();
            assert_eq!(ids, expected);
        };
        let first = coordinator
            .replace(
                OperationId::new(),
                &import,
                None,
                None,
                TimestampMillis::new(1_700_000_000_100),
            )
            .await
            .expect("replace with legacy source");
        let restored = Database::open(&first.database_path).expect("restored database");
        check(&restored, scope);
        let backend = AppBackend::open(
            &first.database_path,
            TimestampMillis::new(1_700_000_000_200),
        )
        .expect("reopen backend");
        let admission = backend
            .legacy_import_admission()
            .admit(
                first.run_id,
                &import.compatibility.database_inventory(),
                &import.plan,
                TimestampMillis::new(1_700_000_000_200),
            )
            .expect("replayed admission");
        assert!(
            backend
                .legacy_llm_metrics_importer()
                .execute_database_import(
                    &admission,
                    &import,
                    TimestampMillis::new(1_700_000_000_200)
                )
                .expect("replayed metrics")
                .replayed
        );
        drop(backend);
        check(&restored, scope);
        drop(restored);
        let second = coordinator
            .replace(
                OperationId::new(),
                &import,
                None,
                None,
                TimestampMillis::new(1_700_000_000_300),
            )
            .await
            .expect("second replace carries the first database's metrics");
        assert_eq!(second.previous_database_path, first.database_path);
        check(
            &Database::open(&second.database_path).expect("second database"),
            scope,
        );
        let third = coordinator
            .replace(
                OperationId::new(),
                &changed,
                None,
                None,
                TimestampMillis::new(1_700_000_000_400),
            )
            .await
            .expect("a changed source replaces the second database");
        assert_eq!(third.previous_database_path, second.database_path);
        check(
            &Database::open(&third.database_path).expect("third database"),
            changed_scope,
        );
    }

    #[tokio::test]
    async fn an_empty_legacy_source_replaces_the_active_database_with_a_completed_run() {
        let root = std::env::temp_dir().join(format!("legacy-restore-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("fixture root");
        let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
            .expect("filesystem authority");
        let location = AppDatabaseLocation::new(root.join("private-persistent-v2"), &authority)
            .expect("database location");
        let secrets = InMemorySecretStore::new();
        let workspace = root.join("legacy-restore-workspace");
        let coordinator =
            LegacyRestoreCoordinator::new(&location, &authority, &workspace, &secrets);
        let compatibility =
            lettuce_transfer::plan_legacy_backup_compatibility(LegacyBackupInventory {
                version: 2,
                created_at: 1_700_000_000_000,
                app_version: "1.0.0".into(),
                source_hash: ContentHash::parse("ab".repeat(32)).expect("source hash"),
                documents: Vec::new(),
                media: Vec::new(),
            })
            .expect("compatibility plan");
        let plan = compatibility.legacy_import_plan();
        let receipt = coordinator
            .replace(
                OperationId::new(),
                &LegacyDatabaseImportPlan {
                    compatibility,
                    plan,
                },
                None,
                None,
                TimestampMillis::new(1_700_000_000_100),
            )
            .await
            .expect("replace with legacy source");
        assert_eq!(
            location.active_path().expect("active database"),
            receipt.database_path
        );
        let restored = Database::open(&receipt.database_path).expect("restored database");
        let graph = restored
            .read_provider_backup_graph()
            .expect("restored graph");
        assert_eq!(graph.legacy_imports.runs.len(), 1);
        assert_eq!(
            lettuce_transfer::backup_sql_text(&graph.legacy_imports.runs[0].run, "status"),
            Some("completed")
        );
    }
}
