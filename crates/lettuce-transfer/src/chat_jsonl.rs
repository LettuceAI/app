//! SillyTavern JSONL chat transcripts: a header line, then one message per
//! line with swipes for alternatives.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, NaiveDateTime, SecondsFormat, TimeZone, Utc};
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChatJsonlError {
    #[error("JSONL_INVALID_LINE")]
    InvalidLine,
    #[error("JSONL_INVALID_ENTRY")]
    InvalidEntry,
    #[error("JSONL_EMPTY")]
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatJsonlHeader {
    pub user_name: String,
    pub character_name: String,
    pub created_at: i64,
    pub group: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatJsonlLine {
    pub name: String,
    pub is_user: bool,
    pub is_system: bool,
    pub created_at: i64,
    pub content: String,
    pub swipes: Option<(Vec<String>, usize)>,
}

/// UTC RFC 3339 with milliseconds, as SillyTavern writes `send_date`.
#[must_use]
pub fn chat_jsonl_send_date(created_at: i64) -> String {
    Utc.timestamp_millis_opt(created_at)
        .single()
        .unwrap_or_else(Utc::now)
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// The transcript, lines joined by `\n`; a line with blank content is left
/// out.
#[must_use]
pub fn export_chat_jsonl(header: &ChatJsonlHeader, lines: &[ChatJsonlLine]) -> String {
    let chat_metadata = if header.group {
        json!({ "group": true })
    } else {
        json!({})
    };
    std::iter::once(json!({
        "user_name": header.user_name,
        "character_name": header.character_name,
        "create_date": chat_jsonl_send_date(header.created_at),
        "chat_metadata": chat_metadata,
    }))
    .chain(
        lines
            .iter()
            .filter(|line| !line.content.trim().is_empty())
            .map(|line| {
                let mut value = json!({
                    "name": line.name,
                    "is_user": line.is_user,
                    "is_system": line.is_system,
                    "send_date": chat_jsonl_send_date(line.created_at),
                    "mes": line.content,
                    "extra": {},
                    "original_avatar": "",
                });
                if let Some((swipes, selected)) = &line.swipes {
                    value["swipe_id"] = json!(selected);
                    value["swipes"] = json!(swipes);
                }
                value
            }),
    )
    .map(|value| value.to_string())
    .collect::<Vec<_>>()
    .join("\n")
}

/// A direct chat message's swipes: its variants with the selected one
/// marked, else the one matching the shown content, else the shown content
/// prepended; `None` without variants.
#[must_use]
pub fn direct_chat_swipes(
    content: &str,
    variants: &[(Option<String>, String)],
    selected_variant_id: Option<&str>,
) -> Option<(Vec<String>, usize)> {
    if variants.is_empty() {
        return None;
    }
    let mut swipes = variants.to_vec();
    let selected = selected_variant_id
        .and_then(|id| {
            swipes
                .iter()
                .position(|(variant, _)| variant.as_deref() == Some(id))
        })
        .or_else(|| swipes.iter().position(|(_, variant)| variant == content))
        .unwrap_or_else(|| {
            swipes.insert(0, (None, content.to_owned()));
            0
        });
    Some((
        swipes.into_iter().map(|(_, content)| content).collect(),
        selected,
    ))
}

/// A group chat message's shown content and swipes: the selected variant, or
/// the first, over the stored content.
#[must_use]
pub fn group_chat_content(
    content: String,
    variants: Vec<(String, String)>,
    selected_variant_id: Option<&str>,
) -> (String, Option<(Vec<String>, usize)>) {
    let selected = selected_variant_id
        .and_then(|id| variants.iter().position(|(variant, _)| variant == id))
        .unwrap_or(0);
    let shown = variants
        .get(selected)
        .map_or(content, |(_, content)| content.clone());
    let swipes = (!variants.is_empty()).then(|| {
        (
            variants.into_iter().map(|(_, content)| content).collect(),
            selected,
        )
    });
    (shown, swipes)
}

fn sanitize_filename(input: &str) -> String {
    let sanitized = input
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let trimmed = sanitized.trim_matches('_').to_lowercase();
    if trimmed.is_empty() {
        "chat".to_owned()
    } else {
        trimmed
    }
}

/// `chat_<title>_<YYYYmmdd_HHMMSS>.jsonl`, or `group_chat_…`, in UTC.
#[must_use]
pub fn chat_jsonl_filename(title: &str, group: bool, now: i64) -> String {
    let stamp = Utc
        .timestamp_millis_opt(now)
        .single()
        .unwrap_or_else(Utc::now)
        .format("%Y%m%d_%H%M%S");
    let prefix = if group { "group_chat" } else { "chat" };
    format!("{prefix}_{}_{stamp}.jsonl", sanitize_filename(title))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatJsonlRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatJsonlMessage {
    pub role: ChatJsonlRole,
    pub name: Option<String>,
    pub content: Option<String>,
    pub created_at: i64,
    pub swipes: Vec<String>,
    pub swipe_id: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatJsonl {
    pub metadata: Option<Value>,
    pub messages: Vec<ChatJsonlMessage>,
}

fn parse_created_at(value: Option<&Value>) -> Option<i64> {
    let scale = |value: i64| {
        if value < 10_000_000_000 {
            value * 1000
        } else {
            value
        }
    };
    let raw = value?;
    if let Some(value) = raw.as_i64() {
        return Some(scale(value));
    }
    let text = raw.as_str()?.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(value) = text.parse::<i64>() {
        return Some(scale(value));
    }
    if let Ok(time) = DateTime::parse_from_rfc3339(text) {
        return Some(time.timestamp_millis());
    }
    ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"]
        .iter()
        .find_map(|format| NaiveDateTime::parse_from_str(text, format).ok())
        .map(|time| time.and_utc().timestamp_millis())
}

/// Reads a transcript; the first line is a header when it has no `mes` but a
/// `chat_metadata`, `user_name` or `character_name`. A message without a
/// readable time gets `now`.
pub fn parse_chat_jsonl(raw: &str, now: i64) -> Result<ChatJsonl, ChatJsonlError> {
    let mut entries = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: Value =
            serde_json::from_str(trimmed).map_err(|_| ChatJsonlError::InvalidLine)?;
        if !value.is_object() {
            return Err(ChatJsonlError::InvalidEntry);
        }
        entries.push(value);
    }
    if entries.is_empty() {
        return Err(ChatJsonlError::Empty);
    }
    let first = &entries[0];
    let metadata = (first.get("mes").is_none()
        && ["chat_metadata", "user_name", "character_name"]
            .iter()
            .any(|key| first.get(*key).is_some()))
    .then(|| entries.remove(0));
    let messages = entries
        .iter()
        .map(|entry| {
            let flag = |key: &str| entry.get(key).and_then(Value::as_bool).unwrap_or(false);
            ChatJsonlMessage {
                role: if flag("is_system") {
                    ChatJsonlRole::System
                } else if flag("is_user") {
                    ChatJsonlRole::User
                } else {
                    ChatJsonlRole::Assistant
                },
                name: entry.get("name").and_then(Value::as_str).map(str::to_owned),
                content: ["mes", "content", "text", "message"]
                    .iter()
                    .find_map(|key| {
                        entry
                            .get(*key)
                            .and_then(Value::as_str)
                            .filter(|text| !text.trim().is_empty())
                            .map(str::to_owned)
                    }),
                created_at: ["send_date", "createdAt", "timestamp", "time"]
                    .iter()
                    .find_map(|key| parse_created_at(entry.get(*key)))
                    .unwrap_or(now),
                swipes: entry
                    .get("swipes")
                    .and_then(Value::as_array)
                    .map(|swipes| {
                        swipes
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default(),
                swipe_id: entry
                    .get("swipe_id")
                    .and_then(Value::as_u64)
                    .and_then(|index| usize::try_from(index).ok()),
            }
        })
        .collect();
    Ok(ChatJsonl { metadata, messages })
}

impl ChatJsonl {
    /// The header's `character_name`, else the file stem, else
    /// `Imported Chat`.
    #[must_use]
    pub fn title(&self, file_stem: Option<&str>) -> String {
        self.metadata
            .as_ref()
            .and_then(|metadata| metadata.get("character_name"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| file_stem.unwrap_or("Imported Chat").to_owned())
    }

    /// Distinct names of assistant messages that carry one.
    #[must_use]
    pub fn speakers(&self) -> BTreeSet<String> {
        self.messages
            .iter()
            .filter(|message| message.role == ChatJsonlRole::Assistant)
            .filter_map(|message| message.name.clone())
            .collect()
    }

    /// More than one assistant speaker.
    #[must_use]
    pub fn is_group(&self) -> bool {
        self.speakers().len() > 1
    }

    /// Assistant message counts per name (`Character` when unnamed), as the
    /// inspection lists participants.
    #[must_use]
    pub fn participant_counts(&self) -> BTreeMap<String, i64> {
        let mut counts = BTreeMap::new();
        for message in self
            .messages
            .iter()
            .filter(|message| message.role == ChatJsonlRole::Assistant)
        {
            *counts
                .entry(
                    message
                        .name
                        .clone()
                        .unwrap_or_else(|| "Character".to_owned()),
                )
                .or_insert(0) += 1;
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_match_sillytavern() {
        assert_eq!(chat_jsonl_send_date(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(
            parse_created_at(Some(&json!(chat_jsonl_send_date(0)))),
            Some(0)
        );
        assert_eq!(parse_created_at(Some(&json!(5))), Some(5_000));
        assert_eq!(
            parse_created_at(Some(&json!("2026-01-02 03:04:05"))),
            Some(1_767_323_045_000)
        );
    }

    #[test]
    fn a_transcript_round_trips_with_swipes() {
        let swipes = direct_chat_swipes(
            "Second",
            &[
                (Some("a".into()), "First".into()),
                (Some("b".into()), "Second".into()),
            ],
            Some("b"),
        );
        assert_eq!(
            swipes,
            Some((vec!["First".to_owned(), "Second".to_owned()], 1))
        );
        assert_eq!(
            direct_chat_swipes("Shown", &[(Some("a".into()), "Other".into())], None),
            Some((vec!["Shown".to_owned(), "Other".to_owned()], 0))
        );
        let raw = export_chat_jsonl(
            &ChatJsonlHeader {
                user_name: "User".into(),
                character_name: "Ada".into(),
                created_at: 0,
                group: false,
            },
            &[
                ChatJsonlLine {
                    name: "User".into(),
                    is_user: true,
                    is_system: false,
                    created_at: 1_000,
                    content: "Hi".into(),
                    swipes: None,
                },
                ChatJsonlLine {
                    name: "Ada".into(),
                    is_user: false,
                    is_system: false,
                    created_at: 2_000,
                    content: "  ".into(),
                    swipes: None,
                },
                ChatJsonlLine {
                    name: "Ada".into(),
                    is_user: false,
                    is_system: false,
                    created_at: 3_000,
                    content: "Second".into(),
                    swipes,
                },
            ],
        );
        let lines = raw.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 3);
        assert_eq!(
            lines[0],
            r#"{"character_name":"Ada","chat_metadata":{},"create_date":"1970-01-01T00:00:00.000Z","user_name":"User"}"#
        );
        assert_eq!(
            lines[1],
            r#"{"extra":{},"is_system":false,"is_user":true,"mes":"Hi","name":"User","original_avatar":"","send_date":"1970-01-01T00:00:01.000Z"}"#
        );
        let parsed = parse_chat_jsonl(&raw, 99).expect("parse");
        assert!(parsed.metadata.is_some());
        assert_eq!(parsed.messages.len(), 2);
        assert_eq!(parsed.messages[0].role, ChatJsonlRole::User);
        assert_eq!(parsed.messages[1].swipes.len(), 2);
        assert_eq!(parsed.messages[1].swipe_id, Some(1));
        assert_eq!(parsed.title(Some("file")), "Ada");
        assert!(!parsed.is_group());
    }

    #[test]
    fn group_content_and_inspection_follow_the_old_rules() {
        let (shown, swipes) = group_chat_content(
            "stored".into(),
            vec![("a".into(), "one".into()), ("b".into(), "two".into())],
            Some("b"),
        );
        assert_eq!(shown, "two");
        assert_eq!(swipes, Some((vec!["one".to_owned(), "two".to_owned()], 1)));
        let parsed = parse_chat_jsonl(
            "{\"name\":\"A\",\"mes\":\"x\"}\n{\"name\":\"B\",\"mes\":\"y\",\"send_date\":\"bad\"}\n{\"is_system\":true,\"text\":\"z\"}",
            42,
        )
        .expect("parse");
        assert!(parsed.metadata.is_none());
        assert!(parsed.is_group());
        assert_eq!(parsed.messages[1].created_at, 42);
        assert_eq!(parsed.messages[2].role, ChatJsonlRole::System);
        assert_eq!(parsed.title(None), "Imported Chat");
        assert_eq!(parse_chat_jsonl("[]", 0), Err(ChatJsonlError::InvalidEntry));
        assert_eq!(parse_chat_jsonl("  \n", 0), Err(ChatJsonlError::Empty));
        assert_eq!(parse_chat_jsonl("{", 0), Err(ChatJsonlError::InvalidLine));
        assert_eq!(
            chat_jsonl_filename(" My Chat! ", true, 0),
            "group_chat_my_chat_19700101_000000.jsonl"
        );
    }
}
