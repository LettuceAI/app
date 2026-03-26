use crate::storage_manager::db::DbConnection;
use crate::storage_manager::lorebook::{get_enabled_character_lorebook_entries, LorebookEntry};

pub(crate) fn keyword_matches(keyword: &str, text: &str, case_sensitive: bool) -> bool {
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return false;
    }

    let normalize = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if c.is_alphanumeric() || c.is_whitespace() {
                    c
                } else {
                    ' '
                }
            })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };

    let (search_keyword, search_text) = if case_sensitive {
        (keyword.to_string(), text.to_string())
    } else {
        (keyword.to_lowercase(), text.to_lowercase())
    };

    if search_keyword.ends_with('*') {
        let prefix = &search_keyword[..search_keyword.len() - 1];
        if prefix.is_empty() {
            return false;
        }

        let normalized_text = normalize(&search_text);

        for word in normalized_text.split_whitespace() {
            if word.starts_with(prefix) {
                return true;
            }
        }
        return false;
    }

    let normalized_keyword = normalize(&search_keyword);
    let normalized_text = normalize(&search_text);

    if normalized_keyword.contains(' ') {
        return normalized_text.contains(&normalized_keyword);
    }

    let text_words: Vec<&str> = normalized_text.split_whitespace().collect();
    text_words.iter().any(|word| *word == normalized_keyword)
}

pub fn activate_lorebook_entries(
    entries: Vec<LorebookEntry>,
    recent_messages: &[String],
) -> Vec<LorebookEntry> {
    if entries.is_empty() {
        return vec![];
    }
    let context = recent_messages.join("\n");

    let mut active_entries: Vec<LorebookEntry> = vec![];

    for entry in entries {
        let should_activate = if entry.always_active {
            true
        } else if entry.keywords.is_empty() {
            false
        } else {
            entry
                .keywords
                .iter()
                .any(|keyword| keyword_matches(keyword, &context, entry.case_sensitive))
        };

        if should_activate {
            active_entries.push(entry);
        }
    }

    active_entries.sort_by(|a, b| {
        a.display_order
            .cmp(&b.display_order)
            .then_with(|| a.created_at.cmp(&b.created_at))
    });

    active_entries
}

pub fn get_active_lorebook_entries(
    conn: &DbConnection,
    character_id: &str,
    recent_messages: &[String],
) -> Result<Vec<LorebookEntry>, String> {
    let entries = get_enabled_character_lorebook_entries(conn, character_id)?;
    Ok(activate_lorebook_entries(entries, recent_messages))
}

pub fn format_lorebook_for_prompt(entries: &[LorebookEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }

    entries
        .iter()
        .map(|entry| entry.content.trim())
        .filter(|content| !content.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}
