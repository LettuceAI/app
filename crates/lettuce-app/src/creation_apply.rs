use lettuce_creation::{
    ConfirmedPersonaApply, ConfirmedPersonaRevisionApply, CreationApplyReceipt,
    CreationApplyRepository, CreationRepositoryError,
};

pub fn apply_confirmed_new_persona<R: CreationApplyRepository + ?Sized>(
    repository: &R,
    request: ConfirmedPersonaApply,
) -> Result<CreationApplyReceipt, CreationRepositoryError> {
    repository.apply_new_persona(request)
}

pub fn apply_confirmed_existing_persona<R: CreationApplyRepository + ?Sized>(
    repository: &R,
    request: ConfirmedPersonaRevisionApply,
) -> Result<CreationApplyReceipt, CreationRepositoryError> {
    repository.apply_existing_persona(request)
}
