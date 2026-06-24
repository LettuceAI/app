use crate::chat_manager::storage::resolve_credential_for_model;
use crate::chat_manager::types::{Model, ProviderCredential, Settings};

pub(crate) fn find_model_with_credential<'a>(
    settings: &'a Settings,
    model_id: &str,
) -> Option<(&'a Model, &'a ProviderCredential)> {
    let model = settings.models.iter().find(|m| m.id == model_id)?;
    let credential = resolve_credential_for_model(settings, model)?;
    Some((model, credential))
}
