use lettuce_types::{CreationProposalId, CreationWorkflowId, Revision};

use crate::{
    CreationProposal, CreationTurn, CreationWorkflow, NewCreationTurn, NewCreationWorkflow,
};

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
