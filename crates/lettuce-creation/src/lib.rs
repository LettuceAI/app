//! AI-assisted authoring, discovery, and import preparation.

#![deny(unsafe_op_in_unsafe_fn)]

mod model;
mod port;
mod proposal;
mod tool;

pub use model::{
    CreationDraft, CreationLorebookEntry, CreationScene, CreationStage, CreationTarget,
    CreationTargetKind, CreationTurn, CreationWorkflow, NewCreationTurn, NewCreationWorkflow,
};
pub use port::{CreationRepositoryError, CreationWorkflowRepository};
pub use proposal::{
    CreationOperation, CreationOperationError, CreationOperationOutcome, CreationProposal,
    CreationProposalError,
};
pub use tool::{
    AdmittedCreationToolCall, CreationToolApply, CreationToolBatch, CreationToolCommit,
    CreationToolContractError, apply_creation_tool_calls, creation_tool_request,
    reduce_creation_tool_calls,
};
