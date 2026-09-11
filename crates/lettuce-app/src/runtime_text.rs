use lettuce_context::{
    PromptDocument, PromptPurpose, PromptRenderValues, PromptRepository, PromptVariable,
    render_prompt_text,
};

use crate::BuiltInPromptId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum RuntimeTextError {
    #[error("built-in runtime prompt text is unavailable")]
    Unavailable,
    #[error("runtime prompt text could not be rendered")]
    Render,
}

/// A built-in `runtimeText` document whose entries are fragments the runtime
/// renders by stable entry key.
#[derive(Debug, Clone)]
pub(crate) struct RuntimeText {
    document: PromptDocument,
}

impl RuntimeText {
    pub(crate) fn load<R: PromptRepository + ?Sized>(
        repository: &R,
        id: BuiltInPromptId,
    ) -> Result<Self, RuntimeTextError> {
        crate::built_in_prompts::active_built_in_prompt(repository, id)
            .map_err(|_| RuntimeTextError::Unavailable)?
            .map(|document| Self { document })
            .ok_or(RuntimeTextError::Unavailable)
    }

    pub(crate) const fn document(&self) -> &PromptDocument {
        &self.document
    }

    /// The rendered fragment, or `None` when its entry was removed or disabled.
    pub(crate) fn render(
        &self,
        key: &str,
        values: &PromptRenderValues,
    ) -> Result<Option<String>, RuntimeTextError> {
        let Some(entry) = self
            .document
            .entries
            .iter()
            .find(|entry| entry.enabled && entry.built_in_entry_key.as_deref() == Some(key))
        else {
            return Ok(None);
        };
        render_prompt_text(PromptPurpose::RuntimeText, &entry.content, values)
            .map(Some)
            .map_err(|_| RuntimeTextError::Render)
    }

    pub(crate) fn render_with(
        &self,
        key: &str,
        variables: impl IntoIterator<Item = (PromptVariable, String)>,
    ) -> Result<String, RuntimeTextError> {
        let mut values = PromptRenderValues::default();
        values.purpose_values.extend(variables);
        self.render(key, &values).map(Option::unwrap_or_default)
    }
}
