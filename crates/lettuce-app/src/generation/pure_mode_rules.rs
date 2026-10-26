//! The Pure mode rules a chat prompt carries, from the chat runtime catalog.

use lettuce_settings::PureMode;

use crate::BuiltInPromptId;
use crate::generation::runtime_text::{RuntimeText, RuntimeTextSource};

/// The `{{content_rules}}` text for `level`; none when Pure mode is off or
/// fails when the text cannot be read.
pub(crate) fn content_rules<R: RuntimeTextSource + ?Sized>(
    repository: &R,
    level: PureMode,
) -> Result<Option<String>, crate::generation::runtime_text::RuntimeTextError> {
    let key = match level {
        PureMode::Off => return Ok(None),
        PureMode::Low => "runtime_content_rules_low",
        PureMode::Standard => "runtime_content_rules_standard",
        PureMode::Strict => "runtime_content_rules_strict",
    };
    RuntimeText::load(repository, BuiltInPromptId::ChatRuntime)
        .and_then(|text| text.render_with(key, []))
        .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_types::TimestampMillis;

    struct MissingRuntimeText;

    impl RuntimeTextSource for MissingRuntimeText {
        fn runtime_text_document(
            &self,
            _: BuiltInPromptId,
        ) -> Result<Option<lettuce_context::PromptDocument>, lettuce_context::PromptRepositoryError>
        {
            Ok(None)
        }
    }

    #[test]
    fn unreadable_runtime_text_is_an_error_not_empty_rules() {
        assert_eq!(
            content_rules(&MissingRuntimeText, PureMode::Standard),
            Err(crate::generation::runtime_text::RuntimeTextError::Unavailable)
        );
    }

    #[test]
    fn each_level_has_the_old_rules() {
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let database = backend.database();
        assert_eq!(content_rules(database, PureMode::Off).expect("rules"), None);
        assert_eq!(
            content_rules(database, PureMode::Low)
                .expect("rules")
                .as_deref(),
            Some("**Content Guidelines:**\n- Avoid explicit sexual content")
        );
        let standard = content_rules(database, PureMode::Standard)
            .expect("rules")
            .expect("standard");
        let strict = content_rules(database, PureMode::Strict)
            .expect("rules")
            .expect("strict");
        assert!(standard.starts_with("**Content Guidelines (STRICT"));
        assert_eq!(standard.lines().count(), 8);
        assert!(strict.starts_with(&standard));
        assert!(
            strict
                .ends_with("- Do not use suggestive, flirty, or sexually charged language or tone")
        );
    }
}
