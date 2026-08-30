use lettuce_creation::{
    ConfirmedCharacterApply, ConfirmedLorebookApply, ConfirmedLorebookRevisionApply,
    ConfirmedPersonaApply, ConfirmedPersonaRevisionApply, CreationApplyReceipt,
    CreationApplyRepository, CreationCharacterApplyReceipt, CreationLorebookApplyReceipt,
    CreationRepositoryError,
};

pub fn apply_confirmed_new_persona<R: CreationApplyRepository + ?Sized>(
    repository: &R,
    request: ConfirmedPersonaApply,
) -> Result<CreationApplyReceipt, CreationRepositoryError> {
    repository.apply_new_persona(request)
}

pub fn apply_confirmed_new_lorebook<R: CreationApplyRepository + ?Sized>(
    repository: &R,
    request: ConfirmedLorebookApply,
) -> Result<CreationLorebookApplyReceipt, CreationRepositoryError> {
    repository.apply_new_lorebook(request)
}

pub fn apply_confirmed_existing_lorebook<R: CreationApplyRepository + ?Sized>(
    repository: &R,
    request: ConfirmedLorebookRevisionApply,
) -> Result<CreationLorebookApplyReceipt, CreationRepositoryError> {
    repository.apply_existing_lorebook(request)
}

pub fn apply_confirmed_new_character<R: CreationApplyRepository + ?Sized>(
    repository: &R,
    request: ConfirmedCharacterApply,
) -> Result<CreationCharacterApplyReceipt, CreationRepositoryError> {
    repository.apply_new_character(request)
}

pub fn apply_confirmed_existing_persona<R: CreationApplyRepository + ?Sized>(
    repository: &R,
    request: ConfirmedPersonaRevisionApply,
) -> Result<CreationApplyReceipt, CreationRepositoryError> {
    repository.apply_existing_persona(request)
}
