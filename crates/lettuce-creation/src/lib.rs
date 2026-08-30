//! AI-assisted authoring, discovery, and import preparation.

#![deny(unsafe_op_in_unsafe_fn)]

mod model;
mod port;
mod proposal;

pub use model::{
    CreationDraft, CreationLorebookEntry, CreationScene, CreationStage, CreationTarget,
    CreationTargetKind, CreationTurn, CreationWorkflow, NewCreationTurn, NewCreationWorkflow,
};
pub use port::{CreationRepositoryError, CreationWorkflowRepository};
pub use proposal::{
    CreationOperation, CreationOperationError, CreationOperationOutcome, CreationProposal,
    CreationProposalError,
};
