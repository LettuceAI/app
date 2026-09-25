use std::sync::OnceLock;

use chrono::{Datelike, Local, LocalResult, TimeZone};
use lettuce_characters::{CharacterRepository, InteractionMode, RepositoryError};
use lettuce_companions::{
    CompanionStateOwner, CompanionStateRepository, CompanionStateRepositoryError,
};
use lettuce_conversations::{
    CompanionClockSettings, Conversation, ConversationKind, PromptRuntimeValues,
};
use lettuce_types::TimestampMillis;
use regex::Regex;

/// Legacy `is_companion_mode` and the session time preferences of one
/// conversation: a direct chat is a companion chat when its live character is
/// a companion or it carries companion session state, and its clock counts
/// only while time awareness is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct CompanionClockContext {
    pub(crate) companion: bool,
    pub(crate) clock: Option<CompanionClockSettings>,
}

impl CompanionClockContext {
    pub(crate) fn effective_now(&self, now: TimestampMillis) -> TimestampMillis {
        self.clock.map_or(now, |clock| clock.effective_now(now))
    }

    pub(crate) const fn time_awareness_enabled(&self) -> bool {
        self.clock.is_some()
    }
}

#[derive(Debug)]
pub(crate) enum CompanionClockError {
    Character(RepositoryError),
    MissingCharacter,
    Companion(CompanionStateRepositoryError),
}

pub(crate) fn companion_clock_context<R>(
    repository: &R,
    conversation: &Conversation,
) -> Result<CompanionClockContext, CompanionClockError>
where
    R: CharacterRepository + CompanionStateRepository + ?Sized,
{
    let ConversationKind::Direct(details) = &conversation.kind else {
        return Ok(CompanionClockContext::default());
    };
    let character = CharacterRepository::get(repository, details.character.source_id)
        .map_err(CompanionClockError::Character)?
        .ok_or(CompanionClockError::MissingCharacter)?;
    let companion = character.character.defaults.interaction_mode == InteractionMode::Companion
        || match companion_state_owner(conversation) {
            Some(owner) => CompanionStateRepository::get(repository, owner)
                .map_err(CompanionClockError::Companion)?
                .is_some(),
            None => false,
        };
    let clock = conversation
        .current_settings
        .as_ref()
        .and_then(|settings| settings.companion_clock)
        .filter(|clock| companion && clock.time_awareness_enabled);
    Ok(CompanionClockContext { companion, clock })
}

/// The companion state a direct conversation reads and writes: its character
/// with the persona the conversation uses now, so a persona switched mid-chat
/// brings that persona's relationship.
pub(crate) fn companion_state_owner(conversation: &Conversation) -> Option<CompanionStateOwner> {
    let ConversationKind::Direct(details) = &conversation.kind else {
        return None;
    };
    Some(CompanionStateOwner {
        conversation_id: conversation.id,
        character_id: details.character.source_id,
        persona_id: lettuce_conversations::effective_persona(conversation)
            .map(|persona| persona.source_id),
    })
}

/// Legacy `time_placeholder_values`: every time placeholder the caller left
/// unset is rendered from the reference time in the local timezone.
pub(crate) fn fill_time_values(values: &mut PromptRuntimeValues, reference: TimestampMillis) {
    let now = match Local.timestamp_millis_opt(reference.get()) {
        LocalResult::Single(datetime) | LocalResult::Ambiguous(datetime, _) => datetime,
        LocalResult::None => Local::now(),
    };
    let date_full = format!(
        "{}, {} {}, {}",
        now.format("%A"),
        now.format("%B"),
        now.day(),
        now.year()
    );
    for (slot, value) in [
        (&mut values.date, now.format("%Y-%m-%d").to_string()),
        (&mut values.date_full, date_full),
        (&mut values.weekday, now.format("%A").to_string()),
        (&mut values.time_hour, now.format("%H").to_string()),
        (&mut values.time_minute, now.format("%M").to_string()),
        (&mut values.time_second, now.format("%S").to_string()),
        (
            &mut values.time_full,
            now.format("%H:%M:%S %:z").to_string(),
        ),
        (
            &mut values.time_12hour_format,
            now.format("%I:%M %p").to_string(),
        ),
        (&mut values.time_timezone, now.format("%:z").to_string()),
        (&mut values.time_timezone_name, now.format("%Z").to_string()),
        (&mut values.datetime_iso, now.to_rfc3339()),
    ] {
        if slot.is_none() {
            *slot = Some(value);
        }
    }
}

fn tagged_time_stamp() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?im)<\s*time\s*>[^<>\n]{0,48}(?:<\s*/\s*time\s*>|$)\s*")
            .expect("valid tagged timestamp regex")
    })
}

fn bracket_time_stamp() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(?i)\[\s*(?:[a-z]{3,9}\s+)?\d{1,2}:\d{2}\s*(?:am|pm)\s*,?\s*\d{4}-\d{2}-\d{2}\s*\]\s*",
        )
        .expect("valid bracket timestamp regex")
    })
}

fn leading_invented_time_stamp() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(?i)^\s*[\[\(\*]{1,2}\s*(?:[a-z]{3,9},?\s+){0,2}\d{1,2}:\d{2}(?::\d{2})?\s*(?:am|pm)?\s*,?\s*(?:\d{4}-\d{2}-\d{2}|\d{1,2}[/-]\d{1,2}[/-]\d{2,4}|(?:[a-z]{3,9}\s+)?\d{1,2},?\s*\d{4})?\s*[\]\)\*]{1,2}[\s,:\-]*",
        )
        .expect("valid leading invented timestamp regex")
    })
}

/// Legacy `strip_echoed_time_stamps`: removes `<time>` tags and the older
/// bracket stamp anywhere, plus one invented stamp at the start of the reply,
/// so a model's own timestamps are never persisted; roleplay brackets stay.
pub(crate) fn strip_echoed_time_stamps(text: &str) -> String {
    let without_tags = tagged_time_stamp().replace_all(text, "");
    let without_brackets = bracket_time_stamp().replace_all(&without_tags, "");
    leading_invented_time_stamp()
        .replace(&without_brackets, "")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Local, TimeZone};
    use lettuce_conversations::PromptRuntimeValues;
    use lettuce_types::TimestampMillis;

    use super::{fill_time_values, strip_echoed_time_stamps};

    #[test]
    fn echoed_time_stamps_are_stripped_like_legacy() {
        assert_eq!(
            strip_echoed_time_stamps("<time>Thu 6:50 PM, 2026-03-12</time> Hey, you're up late."),
            "Hey, you're up late."
        );
        assert_eq!(
            strip_echoed_time_stamps("<time>Thu 6:50 PM, 2026-03-12\nHey, you're up late."),
            "Hey, you're up late."
        );
        assert_eq!(
            strip_echoed_time_stamps("[Thu 6:50 PM, 2026-03-12] Hey."),
            "Hey."
        );
        assert_eq!(strip_echoed_time_stamps("[6:50 PM] Hey."), "Hey.");
        assert_eq!(strip_echoed_time_stamps("(Thursday, 6:50 PM) Hey."), "Hey.");
        assert_eq!(strip_echoed_time_stamps("**6:50 AM** Hey."), "Hey.");
        assert_eq!(
            strip_echoed_time_stamps("[2026-03-12 18:50] Hey."),
            "[2026-03-12 18:50] Hey."
        );
    }

    #[test]
    fn roleplay_brackets_and_mid_message_clocks_survive_stamp_stripping() {
        assert_eq!(
            strip_echoed_time_stamps("[she smiles] Hey."),
            "[she smiles] Hey."
        );
        assert_eq!(
            strip_echoed_time_stamps("[she checks her watch at 6:50] Hey."),
            "[she checks her watch at 6:50] Hey."
        );
        assert_eq!(
            strip_echoed_time_stamps("*she smiles* Hey."),
            "*she smiles* Hey."
        );
        let text = "I'll be there at 6:50 PM, promise.";
        assert_eq!(strip_echoed_time_stamps(text), text);
    }

    #[test]
    fn time_values_follow_the_reference_time_and_keep_caller_values() {
        let reference = TimestampMillis::new(1_700_000_000_000);
        let mut values = PromptRuntimeValues {
            date: Some("caller".into()),
            ..PromptRuntimeValues::default()
        };
        fill_time_values(&mut values, reference);
        assert_eq!(values.date.as_deref(), Some("caller"));
        let local = Local
            .timestamp_millis_opt(reference.get())
            .single()
            .expect("local time");
        assert_eq!(values.weekday, Some(local.format("%A").to_string()));
        assert_eq!(
            values.time_12hour_format,
            Some(local.format("%I:%M %p").to_string())
        );
        assert_eq!(
            values.date_full,
            Some(local.format("%A, %B %-d, %Y").to_string())
        );
        let iso = values.datetime_iso.expect("iso timestamp");
        assert_eq!(
            DateTime::parse_from_rfc3339(&iso)
                .expect("rfc3339")
                .timestamp_millis(),
            reference.get()
        );
    }
}
