use lettuce_creation::{
    ConfirmedPersonaApply, CreationApplyReceipt, CreationApplyRepository, CreationRepositoryError,
};

pub fn apply_confirmed_new_persona<R: CreationApplyRepository + ?Sized>(
    repository: &R,
    request: ConfirmedPersonaApply,
) -> Result<CreationApplyReceipt, CreationRepositoryError> {
    repository.apply_new_persona(request)
}
