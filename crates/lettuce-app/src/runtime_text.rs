use lettuce_context::{
    PromptDocument, PromptPurpose, PromptRenderValues, PromptRepository, PromptRepositoryError,
    PromptVariable, render_prompt_text,
};

use crate::BuiltInPromptId;

fn bundled_catalog() -> Option<&'static crate::BuiltInPromptCatalog> {
    static CATALOG: std::sync::OnceLock<Option<crate::BuiltInPromptCatalog>> =
        std::sync::OnceLock::new();
    CATALOG
        .get_or_init(|| crate::BuiltInPromptCatalog::bundled().ok())
        .as_ref()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum RuntimeTextError {
    #[error("built-in runtime prompt text is unavailable")]
    Unavailable,
    #[error("runtime prompt text could not be rendered")]
    Render,
}

/// Where runtime text documents are read from; every prompt repository is one.
pub trait RuntimeTextSource {
    fn runtime_text_document(
        &self,
        id: BuiltInPromptId,
    ) -> Result<Option<PromptDocument>, PromptRepositoryError>;
}

impl<T: PromptRepository + ?Sized> RuntimeTextSource for T {
    fn runtime_text_document(
        &self,
        id: BuiltInPromptId,
    ) -> Result<Option<PromptDocument>, PromptRepositoryError> {
        crate::built_in_prompts::active_built_in_prompt(self, id)
    }
}

/// A built-in `runtimeText` document whose entries are fragments the runtime
/// renders by stable entry key.
#[derive(Debug, Clone)]
pub(crate) struct RuntimeText {
    id: BuiltInPromptId,
    document: PromptDocument,
}

impl RuntimeText {
    pub(crate) fn load<R: RuntimeTextSource + ?Sized>(
        repository: &R,
        id: BuiltInPromptId,
    ) -> Result<Self, RuntimeTextError> {
        repository
            .runtime_text_document(id)
            .map_err(|_| RuntimeTextError::Unavailable)?
            .map(|document| Self { id, document })
            .ok_or(RuntimeTextError::Unavailable)
    }

    #[cfg(test)]
    pub(crate) fn from_seed(id: BuiltInPromptId) -> Self {
        Self {
            id,
            document: crate::built_in_prompts::seed_document(id),
        }
    }

    #[cfg(test)]
    pub(crate) fn disable_for_test(&mut self, key: &str) {
        for entry in &mut self.document.entries {
            if entry.built_in_entry_key.as_deref() == Some(key) {
                entry.enabled = false;
            }
        }
    }

    pub(crate) const fn document(&self) -> &PromptDocument {
        &self.document
    }

    /// The rendered fragment, or `None` when its entry is disabled. A key the
    /// stored document lacks (a user-edited copy kept across a catalog update)
    /// renders the bundled catalog text.
    pub(crate) fn render(
        &self,
        key: &str,
        values: &PromptRenderValues,
    ) -> Result<Option<String>, RuntimeTextError> {
        let content = match self
            .document
            .entries
            .iter()
            .find(|entry| entry.built_in_entry_key.as_deref() == Some(key))
        {
            Some(entry) if !entry.enabled => return Ok(None),
            Some(entry) => entry.content.clone(),
            None => bundled_catalog()
                .ok_or(RuntimeTextError::Unavailable)?
                .seed(self.id)
                .entries
                .iter()
                .find(|entry| entry.built_in_entry_key.as_deref() == Some(key))
                .map(|entry| entry.content.clone())
                .ok_or(RuntimeTextError::Unavailable)?,
        };
        render_prompt_text(PromptPurpose::RuntimeText, &content, values)
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

#[cfg(test)]
mod tests {
    use super::RuntimeText;
    use crate::BuiltInPromptId;

    #[test]
    fn missing_keys_fall_back_to_the_bundled_text_but_disabled_entries_stay_off() {
        let mut text = RuntimeText::from_seed(BuiltInPromptId::MemoryRuntime);
        text.document
            .entries
            .retain(|entry| entry.built_in_entry_key.as_deref() != Some("memory_none"));
        assert_eq!(
            text.render_with("memory_none", []).expect("fallback"),
            "none"
        );
        for entry in &mut text.document.entries {
            if entry.built_in_entry_key.as_deref() == Some("summary_no_previous") {
                entry.enabled = false;
            }
        }
        assert_eq!(
            text.render_with("summary_no_previous", [])
                .expect("disabled"),
            ""
        );
        assert!(text.render_with("unknown_key", []).is_err());
    }
}
