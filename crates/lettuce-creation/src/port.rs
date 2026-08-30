use lettuce_types::{
    CreationProposalId, CreationWorkflowId, GenerationAttemptId, Revision, TimestampMillis,
};

use crate::{
    ConfirmedPersonaApply, ConfirmedPersonaRevisionApply, CreationApplyReceipt,
    CreationAttemptFailureCode, CreationAttemptOwner, CreationAttemptRecovery,
    CreationAttemptStatus, CreationAttemptSuccess, CreationAttemptSuccessSettlement,
    CreationInferenceAttempt, CreationInferenceRound, CreationProposal, CreationToolCallEvidence,
    CreationTurn, CreationTurnAttemptAdmission, CreationWorkflow, NewCreationAttempt,
    NewCreationAttemptRecovery, NewCreationInferenceRound, NewCreationTurn, NewCreationTurnAttempt,
    NewCreationWorkflow,
};

pub trait CreationApplyRepository: Send + Sync {
    fn apply_new_persona(
        &self,
        request: ConfirmedPersonaApply,
    ) -> Result<CreationApplyReceipt, CreationRepositoryError>;

    fn apply_existing_persona(
        &self,
        request: ConfirmedPersonaRevisionApply,
    ) -> Result<CreationApplyReceipt, CreationRepositoryError>;
}

pub trait CreationAttemptRepository: Send + Sync {
    fn admit_creation_turn_attempt(
        &self,
        admission: NewCreationTurnAttempt,
    ) -> Result<CreationTurnAttemptAdmission, CreationRepositoryError>;

    fn create_creation_attempt(
        &self,
        attempt: NewCreationAttempt,
    ) -> Result<CreationInferenceAttempt, CreationRepositoryError>;

    fn recover_creation_attempt(
        &self,
        recovery: NewCreationAttemptRecovery,
    ) -> Result<CreationAttemptRecovery, CreationRepositoryError>;

    fn settle_creation_attempt_success(
        &self,
        settlement: CreationAttemptSuccessSettlement,
    ) -> Result<CreationAttemptSuccess, CreationRepositoryError>;

    fn load_creation_attempt(
        &self,
        id: GenerationAttemptId,
    ) -> Result<CreationInferenceAttempt, CreationRepositoryError>;

    fn transition_creation_attempt(
        &self,
        id: GenerationAttemptId,
        expected_revision: Revision,
        next: CreationAttemptStatus,
        failure: Option<CreationAttemptFailureCode>,
        at: TimestampMillis,
    ) -> Result<CreationInferenceAttempt, CreationRepositoryError>;

    fn admit_creation_inference_round(
        &self,
        owner: CreationAttemptOwner,
        attempt_id: GenerationAttemptId,
        expected_round_ordinal: u8,
        expected_next_ordinal: u16,
        round: NewCreationInferenceRound,
    ) -> Result<CreationInferenceRound, CreationRepositoryError>;

    fn list_creation_inference_rounds(
        &self,
        owner: CreationAttemptOwner,
        attempt_id: GenerationAttemptId,
    ) -> Result<Vec<CreationInferenceRound>, CreationRepositoryError>;

    fn list_creation_tool_calls(
        &self,
        owner: CreationAttemptOwner,
        attempt_id: GenerationAttemptId,
    ) -> Result<Vec<CreationToolCallEvidence>, CreationRepositoryError>;
}

pub trait CreationWorkflowRepository: Send + Sync {
    fn create_workflow(
        &self,
        workflow: NewCreationWorkflow,
    ) -> Result<CreationWorkflow, CreationRepositoryError>;

    fn load_workflow(
        &self,
        id: CreationWorkflowId,
    ) -> Result<CreationWorkflow, CreationRepositoryError>;

    fn load_proposal(
        &self,
        id: CreationProposalId,
    ) -> Result<CreationProposal, CreationRepositoryError>;

    fn load_turn(
        &self,
        id: lettuce_types::CreationTurnId,
    ) -> Result<CreationTurn, CreationRepositoryError>;

    fn record_user_turn(
        &self,
        turn: NewCreationTurn,
    ) -> Result<CreationTurn, CreationRepositoryError>;

    fn append_proposal(
        &self,
        workflow_id: CreationWorkflowId,
        expected_workflow_revision: Revision,
        proposal: CreationProposal,
    ) -> Result<CreationWorkflow, CreationRepositoryError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CreationRepositoryError {
    #[error("creation record was not found")]
    NotFound,
    #[error("creation operation conflicts with durable state")]
    Conflict,
    #[error("creation record is invalid")]
    Invalid,
    #[error("creation storage failed")]
    Storage,
}
