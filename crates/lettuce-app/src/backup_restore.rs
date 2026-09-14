use std::{
    fmt,
    path::{Path, PathBuf},
};

use lettuce_database::Database;
use lettuce_platform::{
    FilesystemAuthority, ManagedFiles, ManagedRoot, ObjectKey, PlatformError, ReadCapability,
    WriteCapability,
};
use lettuce_settings::{SecretPurpose, SecretRecord, SecretRef, SecretStore};
use lettuce_transfer::{
    BackupConversationArtifact, BackupRestoreAdmission, BackupRestoreAdmissionRepository,
    BackupRestoreAdmissionRequest, BackupRestoreWorkspace, ProviderBackupGraph,
    ProviderBackupRestorePlan, ProviderBackupRestoreWriter, ProviderBackupSecret,
    ProviderBackupSource,
};
use lettuce_types::{OperationId, TimestampMillis};

const ACTIVE_DATABASE_KEY: &str = "active-database";
const DATABASE_DIRECTORY: &str = "databases";
const INITIAL_DATABASE_NAME: &str = "lettuce.sqlite3";
const DATABASE_EXTENSION: &str = ".sqlite3";

/// The app's database files under private persistent storage and the pointer
/// naming the active one. Files are never deleted here: a restore adds a new
/// database file and then moves the pointer.
pub struct AppDatabaseLocation {
    private_persistent: PathBuf,
    files: ManagedFiles,
    read: ReadCapability,
    write: WriteCapability,
}

impl fmt::Debug for AppDatabaseLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AppDatabaseLocation")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AppDatabaseLocationError {
    #[error("database pointer is corrupt")]
    Corrupt,
    #[error("database location storage failed: {0}")]
    Platform(PlatformError),
}

impl From<PlatformError> for AppDatabaseLocationError {
    fn from(error: PlatformError) -> Self {
        Self::Platform(error)
    }
}

impl AppDatabaseLocation {
    pub fn new(
        private_persistent: impl Into<PathBuf>,
        authority: &FilesystemAuthority,
    ) -> Result<Self, AppDatabaseLocationError> {
        Ok(Self {
            private_persistent: private_persistent.into(),
            files: authority.managed_files(),
            read: authority.read_capability(ManagedRoot::PrivatePersistent)?,
            write: authority.write_capability(ManagedRoot::PrivatePersistent)?,
        })
    }

    pub fn active_path(&self) -> Result<PathBuf, AppDatabaseLocationError> {
        let name = match self
            .files
            .read(&self.read, &ObjectKey::single(ACTIVE_DATABASE_KEY)?)
        {
            Ok(bytes) => String::from_utf8(bytes).map_err(|_| AppDatabaseLocationError::Corrupt)?,
            Err(PlatformError::NotFound) => INITIAL_DATABASE_NAME.to_owned(),
            Err(error) => return Err(error.into()),
        };
        self.database_path(&name)
    }

    fn database_path(&self, name: &str) -> Result<PathBuf, AppDatabaseLocationError> {
        let stem = name
            .strip_suffix(DATABASE_EXTENSION)
            .ok_or(AppDatabaseLocationError::Corrupt)?;
        if stem.is_empty()
            || stem.len() > 64
            || !stem
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
        {
            return Err(AppDatabaseLocationError::Corrupt);
        }
        Ok(self.private_persistent.join(DATABASE_DIRECTORY).join(name))
    }

    fn activate(&self, name: &str) -> Result<(), AppDatabaseLocationError> {
        self.database_path(name)?;
        self.files.write_atomic(
            &self.write,
            ObjectKey::single(ACTIVE_DATABASE_KEY)?,
            name.as_bytes(),
        )?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct BackupRestoreReceipt {
    pub database_path: PathBuf,
    pub previous_database_path: PathBuf,
    pub admission: BackupRestoreAdmission,
}

#[derive(Debug, thiserror::Error)]
pub enum BackupRestoreError {
    #[error("backup could not be decoded: {0}")]
    Plan(#[from] lettuce_transfer::ProviderBackupRestorePlanError),
    #[error("backup could not be staged: {0}")]
    Workspace(#[from] lettuce_transfer::BackupRestoreWorkspaceError),
    #[error("backup graph is invalid: {0}")]
    Graph(#[from] lettuce_transfer::ProviderBackupGraphError),
    #[error("database location is unavailable: {0}")]
    Location(#[from] AppDatabaseLocationError),
    #[error("restore target database already exists")]
    TargetExists,
    #[error("restore target directory is unavailable")]
    TargetDirectory,
    #[error("restore database is unavailable: {0}")]
    Database(#[from] lettuce_database::DatabaseError),
    #[error("backup media could not be installed")]
    Media(lettuce_media::MediaStoreError),
    #[error("backup secrets could not be stored")]
    Secret(lettuce_settings::SecretStoreError),
    #[error("backup could not be written: {0}")]
    Restore(#[from] lettuce_transfer::ProviderBackupRestoreWriteError),
    #[error("restored database could not be read back: {0}")]
    ReadBack(#[from] lettuce_transfer::ProviderBackupSourceError),
    #[error("restored database does not match the backup")]
    VerificationFailed,
    #[error("restore admission failed: {0}")]
    Admission(#[from] lettuce_transfer::BackupRestoreAdmissionError),
}

/// Restores a v2 backup into a new database file and switches the active
/// database to it once the restored data reads back equal to the backup.
pub struct BackupRestoreCoordinator<'a, S: ?Sized> {
    location: &'a AppDatabaseLocation,
    workspace_root: &'a Path,
    media_root: &'a Path,
    secrets: &'a S,
}

impl<S: ?Sized> fmt::Debug for BackupRestoreCoordinator<'_, S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackupRestoreCoordinator")
            .finish_non_exhaustive()
    }
}

impl<'a, S: SecretStore + ?Sized> BackupRestoreCoordinator<'a, S> {
    #[must_use]
    pub const fn new(
        location: &'a AppDatabaseLocation,
        workspace_root: &'a Path,
        media_root: &'a Path,
        secrets: &'a S,
    ) -> Self {
        Self {
            location,
            workspace_root,
            media_root,
            secrets,
        }
    }

    pub async fn restore(
        &self,
        restore_id: OperationId,
        bytes: &[u8],
        password: &str,
        restored_at: TimestampMillis,
    ) -> Result<BackupRestoreReceipt, BackupRestoreError> {
        let plan = lettuce_transfer::decode_provider_backup_restore_plan(bytes, password)?;
        let staging = BackupRestoreWorkspace::open(self.workspace_root)?.stage(&plan)?;
        let admission = lettuce_transfer::current_backup_restore_admission(
            restore_id,
            &plan,
            &staging,
            restored_at,
        )?;
        let previous_database_path = self.location.active_path()?;
        let name = format!("{restore_id}{DATABASE_EXTENSION}");
        let database_path = self.location.database_path(&name)?;
        if database_path
            .try_exists()
            .map_err(|_| BackupRestoreError::TargetDirectory)?
        {
            return Err(BackupRestoreError::TargetExists);
        }
        std::fs::create_dir_all(
            database_path
                .parent()
                .ok_or(BackupRestoreError::TargetDirectory)?,
        )
        .map_err(|_| BackupRestoreError::TargetDirectory)?;
        let ProviderBackupRestorePlan {
            mut graph,
            secrets,
            media,
            artifacts,
            ..
        } = plan;
        for object in &media {
            lettuce_media::install_backup_media_object(
                self.media_root,
                &object.content_hash,
                &object.bytes,
            )
            .map_err(BackupRestoreError::Media)?;
        }
        let secrets = lettuce_transfer::rebind_provider_backup_secrets(&mut graph, secrets)?;
        let mut written = Vec::new();
        let outcome = self
            .write_database(
                &database_path,
                &previous_database_path,
                &graph,
                secrets,
                &artifacts,
                admission,
                &mut written,
            )
            .await
            .and_then(|admission| {
                self.location
                    .activate(&name)
                    .map(|()| admission)
                    .map_err(BackupRestoreError::from)
            });
        let admission = match outcome {
            Ok(admission) => admission,
            Err(error) => {
                for (reference, purpose) in written {
                    let _ = self.secrets.delete(&reference, &purpose, None).await;
                }
                return Err(error);
            }
        };
        Ok(BackupRestoreReceipt {
            database_path,
            previous_database_path,
            admission,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn write_database(
        &self,
        database_path: &Path,
        previous_database_path: &Path,
        graph: &ProviderBackupGraph,
        secrets: Vec<ProviderBackupSecret>,
        artifacts: &[BackupConversationArtifact],
        admission: BackupRestoreAdmissionRequest,
        written: &mut Vec<(SecretRef, SecretPurpose)>,
    ) -> Result<BackupRestoreAdmission, BackupRestoreError> {
        for secret in secrets {
            self.secrets
                .put(
                    SecretRecord::new(secret.reference, secret.purpose.clone()),
                    secret.value,
                    None,
                )
                .await
                .map_err(BackupRestoreError::Secret)?;
            written.push((secret.reference, secret.purpose));
        }
        let database = Database::open(database_path)?;
        database.restore_provider_backup_graph(graph, artifacts)?;
        if previous_database_path
            .try_exists()
            .map_err(|_| BackupRestoreError::TargetDirectory)?
        {
            database.carry_device_local_state_from(previous_database_path)?;
        }
        let mut restored = database.read_provider_backup_graph()?;
        lettuce_transfer::canonicalize_and_validate(&mut restored)?;
        if restored != *graph {
            return Err(BackupRestoreError::VerificationFailed);
        }
        Ok(database.admit_backup_restore(admission)?)
    }
}
