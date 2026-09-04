//! AI-assisted authoring, discovery, and import preparation.

#![deny(unsafe_op_in_unsafe_fn)]

mod attempt;
mod lorebook_entry;
mod model;
mod port;
mod proposal;
mod tool;

pub use attempt::{
    CreationAttemptError, CreationAttemptFailureCode, CreationAttemptOwner,
    CreationAttemptRecovery, CreationAttemptStatus, CreationAttemptSuccess,
    CreationAttemptSuccessSettlement, CreationInferenceAttempt,
    CreationInferenceProfileFingerprint, CreationInferenceRound, CreationRoundFinishReason,
    CreationToolCallEvidence, CreationTurnAttemptAdmission, MAX_CREATION_INFERENCE_ROUNDS,
    NewCreationAttempt, NewCreationAttemptRecovery, NewCreationInferenceRound, NewCreationToolCall,
    NewCreationTurnAttempt, creation_inference_profile_fingerprint,
};
pub use lorebook_entry::*;
pub use model::{
    ConfirmedCharacterApply, ConfirmedCharacterRevisionApply, ConfirmedLorebookApply,
    ConfirmedLorebookRevisionApply, ConfirmedPersonaApply, ConfirmedPersonaRevisionApply,
    CreationApplyReceipt, CreationCharacterApplyReceipt, CreationDraft,
    CreationLorebookApplyReceipt, CreationLorebookEntry, CreationScene, CreationStage,
    CreationTarget, CreationTargetKind, CreationTurn, CreationWorkflow, NewCreationTurn,
    NewCreationWorkflow,
};
pub use port::{
    CreationApplyRepository, CreationAttemptRepository, CreationRepositoryError,
    CreationWorkflowRepository,
};
pub use proposal::{
    CreationOperation, CreationOperationError, CreationOperationOutcome, CreationProposal,
    CreationProposalError,
};
pub use tool::{
    AdmittedCreationToolCall, CreationToolApply, CreationToolBatch, CreationToolCommit,
    CreationToolContractError, apply_creation_tool_calls, creation_tool_request,
    reduce_creation_tool_calls, validate_creation_tool_calls,
};
