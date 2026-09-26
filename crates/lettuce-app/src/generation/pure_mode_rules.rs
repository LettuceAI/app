//! The Pure mode rules a chat prompt carries and the rules a new character
//! starts with, from the chat runtime catalog.

use lettuce_settings::PureMode;

use crate::BuiltInPromptId;
use crate::generation::runtime_text::{RuntimeText, RuntimeTextSource};

/// The `{{content_rules}}` text for `level`; empty when Pure mode is off or
/// the text cannot be read.
pub(crate) fn content_rules<R: RuntimeTextSource + ?Sized>(
    repository: &R,
    level: PureMode,
) -> String {
    let key = match level {
        PureMode::Off => return String::new(),
        PureMode::Low => "runtime_content_rules_low",
        PureMode::Standard => "runtime_content_rules_standard",
        PureMode::Strict => "runtime_content_rules_strict",
    };
    RuntimeText::load(repository, BuiltInPromptId::ChatRuntime)
        .and_then(|text| text.render_with(key, []))
        .unwrap_or_default()
}

/// The rules a character without rules starts with: the base rules, then
/// the level's.
pub fn default_character_rules<R: RuntimeTextSource + ?Sized>(
    repository: &R,
    level: PureMode,
) -> Vec<String> {
    let Ok(text) = RuntimeText::load(repository, BuiltInPromptId::ChatRuntime) else {
        return Vec::new();
    };
    let level_key = match level {
        PureMode::Off => None,
        PureMode::Low => Some("character_rules_low"),
        PureMode::Standard => Some("character_rules_standard"),
        PureMode::Strict => Some("character_rules_strict"),
    };
    std::iter::once("character_rules_base")
        .chain(level_key)
        .flat_map(|key| {
            text.render_with(key, [])
                .unwrap_or_default()
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_types::TimestampMillis;

    #[test]
    fn each_level_has_the_old_rules() {
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let database = backend.database();
        assert_eq!(content_rules(database, PureMode::Off), "");
        assert_eq!(
            content_rules(database, PureMode::Low),
            "**Content Guidelines:**\n- Avoid explicit sexual content"
        );
        let standard = content_rules(database, PureMode::Standard);
        let strict = content_rules(database, PureMode::Strict);
        assert!(standard.starts_with("**Content Guidelines (STRICT"));
        assert_eq!(standard.lines().count(), 8);
        assert!(strict.starts_with(&standard));
        assert!(
            strict
                .ends_with("- Do not use suggestive, flirty, or sexually charged language or tone")
        );
        assert_eq!(default_character_rules(database, PureMode::Off).len(), 5);
        assert_eq!(default_character_rules(database, PureMode::Low).len(), 6);
        assert_eq!(
            default_character_rules(database, PureMode::Standard).len(),
            10
        );
        assert_eq!(
            default_character_rules(database, PureMode::Strict).len(),
            11
        );
    }
}
