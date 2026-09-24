use lettuce_transfer::{
    BackupLorebookBindings, LegacyBackupGroupCandidate, LegacyGroupMaterializationRequest,
    LegacyImportAdmission, LegacyImportPlan, LegacyImportRepository, LegacyImportRepositoryError,
    LegacyImportStageReceipt,
};
use lettuce_types::{GroupId, TimestampMillis};

#[derive(Debug)]
pub struct LegacyGroupImportCoordinator<'a, R: ?Sized> {
    repository: &'a R,
}

impl<'a, R: LegacyImportRepository + ?Sized> LegacyGroupImportCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    /// Writes the planned reusable legacy groups of an admitted run once its
    /// characters stage completed.
    pub fn execute(
        &self,
        admission: &LegacyImportAdmission,
        plan: &LegacyImportPlan,
        groups: &[LegacyBackupGroupCandidate],
        group_lorebooks: &[BackupLorebookBindings<GroupId>],
        completed_at: TimestampMillis,
    ) -> Result<LegacyImportStageReceipt, LegacyImportRepositoryError> {
        let plan_fingerprint = crate::legacy::legacy_import::plan_fingerprint(plan);
        if plan_fingerprint != admission.plan_fingerprint {
            return Err(LegacyImportRepositoryError::Conflict);
        }
        let source_fingerprint = plan
            .source_fingerprint
            .clone()
            .ok_or(LegacyImportRepositoryError::InvalidInput)?;
        self.repository
            .materialize_groups(LegacyGroupMaterializationRequest {
                run_id: admission.run_id,
                plan_fingerprint,
                source_fingerprint,
                media: plan.media.clone(),
                groups: groups.to_vec(),
                group_lorebooks: group_lorebooks.to_vec(),
                completed_at,
            })
    }

    pub fn execute_database_import(
        &self,
        admission: &LegacyImportAdmission,
        import: &crate::LegacyDatabaseImportPlan,
        completed_at: TimestampMillis,
    ) -> Result<LegacyImportStageReceipt, LegacyImportRepositoryError> {
        let authored = import.compatibility.authored_plan();
        self.execute(
            admission,
            &import.plan,
            &authored.groups,
            &authored.group_lorebooks,
            completed_at,
        )
    }
}
