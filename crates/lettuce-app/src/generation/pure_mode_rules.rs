//! The Pure mode rules a chat prompt carries and the rules a new character
//! starts with, from the chat runtime catalog.

use lettuce_settings::PureMode;

use crate::BuiltInPromptId;
use crate::generation::runtime_text::{RuntimeText, RuntimeTextSource};

/// The `{{content_rules}}` text for `level`; empty when Pure mode is off or
/// fails when the text cannot be read.
pub(crate) fn content_rules<R: RuntimeTextSource + ?Sized>(
    repository: &R,
    level: PureMode,
) -> Result<String, crate::generation::runtime_text::RuntimeTextError> {
    let key = match level {
        PureMode::Off => return Ok(String::new()),
        PureMode::Low => "runtime_content_rules_low",
        PureMode::Standard => "runtime_content_rules_standard",
        PureMode::Strict => "runtime_content_rules_strict",
    };
    RuntimeText::load(repository, BuiltInPromptId::ChatRuntime)
        .and_then(|text| text.render_with(key, []))
}

/// The rules a character without rules starts with: the base rules, then
/// the level's. Fails when the chat runtime text cannot be read.
pub(crate) fn default_character_rules<R: RuntimeTextSource + ?Sized>(
    repository: &R,
    level: PureMode,
) -> Result<Vec<String>, crate::generation::runtime_text::RuntimeTextError> {
    let text = RuntimeText::load(repository, BuiltInPromptId::ChatRuntime)?;
    let level_key = match level {
        PureMode::Off => None,
        PureMode::Low => Some("character_rules_low"),
        PureMode::Standard => Some("character_rules_standard"),
        PureMode::Strict => Some("character_rules_strict"),
    };
    let mut rules = Vec::new();
    for key in std::iter::once("character_rules_base").chain(level_key) {
        rules.extend(
            text.render_with(key, [])?
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned),
        );
    }
    Ok(rules)
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
            default_character_rules(&MissingRuntimeText, PureMode::Standard),
            Err(crate::generation::runtime_text::RuntimeTextError::Unavailable)
        );
    }

    #[test]
    fn each_level_has_the_old_rules() {
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let database = backend.database();
        assert_eq!(content_rules(database, PureMode::Off).expect("rules"), "");
        assert_eq!(
            content_rules(database, PureMode::Low).expect("rules"),
            "**Content Guidelines:**\n- Avoid explicit sexual content"
        );
        let standard = content_rules(database, PureMode::Standard).expect("rules");
        let strict = content_rules(database, PureMode::Strict).expect("rules");
        assert!(standard.starts_with("**Content Guidelines (STRICT"));
        assert_eq!(standard.lines().count(), 8);
        assert!(strict.starts_with(&standard));
        assert!(
            strict
                .ends_with("- Do not use suggestive, flirty, or sexually charged language or tone")
        );
        assert_eq!(
            default_character_rules(database, PureMode::Off)
                .expect("rules")
                .len(),
            5
        );
        assert_eq!(
            default_character_rules(database, PureMode::Low)
                .expect("rules")
                .len(),
            6
        );
        assert_eq!(
            default_character_rules(database, PureMode::Standard)
                .expect("rules")
                .len(),
            10
        );
        assert_eq!(
            default_character_rules(database, PureMode::Strict)
                .expect("rules")
                .len(),
            11
        );
    }
}
