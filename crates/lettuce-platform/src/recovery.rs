use std::{collections::BTreeSet, path::Path, sync::Arc};

use cap_std::fs::Dir;

use crate::{
    authority::{map_symlink_error, open_dir_nofollow_from},
    directories::ManagedRoot,
    error::PlatformError,
    keys::is_owned_stage_name,
    managed::{MAX_RECOVERY_ARTIFACTS, MAX_RECOVERY_DEPTH, ManagedFiles},
    model::RecoveryReport,
};

impl ManagedFiles {
    /// Inspect only tool-owned stage names. Recovery never guesses at a
    /// destructive cleanup; identity includes the leaf root and full relative
    /// location so equal basenames in different roots remain distinct.
    pub fn recover_incomplete(&self) -> Result<RecoveryReport, PlatformError> {
        let mut report = RecoveryReport::default();
        let mut seen: BTreeSet<(ManagedRoot, Vec<String>)> = BTreeSet::new();
        for root in ManagedRoot::ALL {
            if report.truncated {
                break;
            }
            let handle = self
                .inner
                .roots
                .get(&root)
                .ok_or(PlatformError::InvalidRoot)?;
            scan_recovery(
                &handle.dir,
                root,
                0,
                &mut Vec::new(),
                &mut report,
                &mut seen,
            )?;
        }
        Ok(report)
    }
}

fn scan_recovery(
    directory: &Arc<Dir>,
    root: ManagedRoot,
    depth: usize,
    relative: &mut Vec<String>,
    report: &mut RecoveryReport,
    seen: &mut BTreeSet<(ManagedRoot, Vec<String>)>,
) -> Result<(), PlatformError> {
    if depth > MAX_RECOVERY_DEPTH || report.truncated {
        report.truncated = true;
        return Ok(());
    }
    for entry in directory.read_dir(".").map_err(PlatformError::from)? {
        if report.inspected >= MAX_RECOVERY_ARTIFACTS {
            report.truncated = true;
            break;
        }
        let entry = entry.map_err(PlatformError::from)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| PlatformError::InvalidKey)?;
        report.inspected += 1;
        let file_type = entry.file_type().map_err(PlatformError::from)?;
        relative.push(name.clone());
        if is_owned_stage_name(&name) {
            if seen.insert((root, relative.clone())) {
                report.retained += 1;
            }
        } else if file_type.is_dir() {
            let child =
                open_dir_nofollow_from(directory, Path::new(&name)).map_err(map_symlink_error)?;
            scan_recovery(&Arc::new(child), root, depth + 1, relative, report, seen)?;
        }
        relative.pop();
    }
    Ok(())
}
