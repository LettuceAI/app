//! Kind policy registration. Domain crates provide executors in later slices;
//! this module only validates unique, typed policy declarations.

use std::collections::BTreeMap;

use crate::{CancellationPolicy, JobKind, RecoveryPolicy, ResourceClass};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobPolicy {
    pub recovery: RecoveryPolicy,
    pub cancellation: CancellationPolicy,
    pub resources: Vec<ResourceClass>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    #[error("job kind is already registered")]
    Duplicate,
    #[error("a registered job policy must declare at least one resource class")]
    EmptyResources,
    #[error("a registered job policy cannot duplicate a resource class")]
    DuplicateResource,
}

#[derive(Debug, Clone, Default)]
pub struct JobRegistry {
    policies: BTreeMap<JobKindKey, JobPolicy>,
}

// A separate key keeps the registry deterministic without requiring every
// future JobKind to change an external serialization contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct JobKindKey(u8);

impl From<JobKind> for JobKindKey {
    fn from(kind: JobKind) -> Self {
        Self(match kind {
            JobKind::ArtifactInstall => 0,
            JobKind::ArtifactVerify => 1,
            JobKind::RuntimePrepare => 2,
            JobKind::ModelLoad => 3,
            JobKind::MemoryExtraction => 4,
            JobKind::MemoryConsolidation => 5,
            JobKind::VectorIndexBuild => 6,
            JobKind::CreationRun => 7,
            JobKind::ImageGenerate => 8,
            JobKind::MediaTransform => 9,
            JobKind::TransferImport => 10,
            JobKind::TransferExport => 11,
            JobKind::BackupExport => 12,
            JobKind::BackupRestore => 13,
            JobKind::SyncSession => 14,
            JobKind::SpeechTranscribe => 15,
            JobKind::SpeechSynthesize => 16,
            JobKind::EmbeddingBenchmark => 17,
            JobKind::Maintenance => 18,
            JobKind::CompanionGrowth => 19,
            JobKind::CompanionConsolidation => 20,
            JobKind::CompanionSoulWriter => 21,
        })
    }
}

impl JobRegistry {
    pub fn register(&mut self, kind: JobKind, policy: JobPolicy) -> Result<(), RegistryError> {
        if policy.resources.is_empty() {
            return Err(RegistryError::EmptyResources);
        }
        if policy
            .resources
            .iter()
            .enumerate()
            .any(|(index, resource)| policy.resources[index + 1..].contains(resource))
        {
            return Err(RegistryError::DuplicateResource);
        }
        if self.policies.insert(kind.into(), policy).is_some() {
            return Err(RegistryError::Duplicate);
        }
        Ok(())
    }

    #[must_use]
    pub fn policy(&self, kind: JobKind) -> Option<&JobPolicy> {
        self.policies.get(&kind.into())
    }
}
