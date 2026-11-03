//! The scene tag a model writes into a chat reply, `<img>…</img>`, which may
//! name who the image shows: `<img subjects="Mira, Sol">…</img>`.

const SCENE_TAG_OPEN: &str = "<img";
const SCENE_CLOSE_TOKENS: [&str; 3] = ["</img>", "[continue]", "[/continue]"];

fn find_ignore_ascii_case(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

/// The prompt of a scene tag and the subjects it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneTag {
    pub prompt: String,
    pub subjects: Vec<String>,
}

/// The next opening tag: where it starts, where its body starts and its
/// attribute text. `<img>` and `<img …>` open a tag, `<images` does not.
fn find_open(text: &str) -> Option<(usize, usize, &str)> {
    let mut from = 0;
    loop {
        let start = from + find_ignore_ascii_case(&text[from..], SCENE_TAG_OPEN)?;
        let after = &text[start + SCENE_TAG_OPEN.len()..];
        match after.chars().next() {
            Some('>') => return Some((start, start + SCENE_TAG_OPEN.len() + 1, "")),
            Some(next) if next.is_ascii_whitespace() => {
                let close = after.find('>')?;
                return Some((
                    start,
                    start + SCENE_TAG_OPEN.len() + close + 1,
                    &after[..close],
                ));
            }
            _ => from = start + SCENE_TAG_OPEN.len(),
        }
    }
}

/// The names in a `subjects="…"` attribute, comma separated.
fn subjects_of(attributes: &str) -> Vec<String> {
    let Some(key) = find_ignore_ascii_case(attributes, "subjects") else {
        return Vec::new();
    };
    let rest = attributes[key + "subjects".len()..].trim_start();
    let Some(rest) = rest.strip_prefix('=') else {
        return Vec::new();
    };
    let rest = rest.trim_start();
    let Some(quote) = rest.chars().next().filter(|quote| matches!(quote, '"' | '\'')) else {
        return Vec::new();
    };
    let value = &rest[1..];
    let value = value.find(quote).map_or(value, |end| &value[..end]);
    value
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Removes every scene tag and returns the first one with a non-blank
/// prompt. A tag closes at `</img>`, `[continue]` or `[/continue]` (any
/// case); an unclosed tag drops the rest of the text. The result is trimmed.
#[must_use]
pub fn extract_scene_tag(content: &str) -> (String, Option<SceneTag>) {
    let mut visible = String::with_capacity(content.len());
    let mut tag = None;
    let mut rest = content;
    loop {
        let Some((open, body, attributes)) = find_open(rest) else {
            visible.push_str(rest);
            break;
        };
        visible.push_str(&rest[..open]);
        let subjects = subjects_of(attributes);
        rest = &rest[body..];
        let Some((close, token)) = SCENE_CLOSE_TOKENS
            .iter()
            .filter_map(|token| find_ignore_ascii_case(rest, token).map(|index| (index, *token)))
            .min_by_key(|(index, _)| *index)
        else {
            break;
        };
        let candidate = rest[..close].trim();
        if tag.is_none() && !candidate.is_empty() {
            tag = Some(SceneTag {
                prompt: candidate.to_owned(),
                subjects,
            });
        }
        rest = &rest[close + token.len()..];
    }
    (visible.trim().to_owned(), tag)
}

/// [`extract_scene_tag`] without the subjects.
#[must_use]
pub fn extract_scene_prompt(content: &str) -> (String, Option<String>) {
    let (visible, tag) = extract_scene_tag(content);
    (visible, tag.map(|tag| tag.prompt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_tags_are_extracted_like_legacy() {
        assert_eq!(
            extract_scene_prompt("  She waves.\n<IMG> harbor at dusk </img> Then leaves. "),
            (
                "She waves.\n Then leaves.".to_owned(),
                Some("harbor at dusk".to_owned())
            )
        );
        assert_eq!(
            extract_scene_prompt("a<img> </img>b<img>second[CONTINUE]c<img>third[/continue]"),
            ("abc".to_owned(), Some("second".to_owned()))
        );
        assert_eq!(
            extract_scene_prompt("Hello <img>never closed"),
            ("Hello".to_owned(), None)
        );
        assert_eq!(
            extract_scene_prompt("<img>x</img><im"),
            ("<im".to_owned(), Some("x".to_owned()))
        );
        assert_eq!(extract_scene_prompt(" plain "), ("plain".to_owned(), None));
    }

    #[test]
    fn scene_tags_name_their_subjects() {
        assert_eq!(
            extract_scene_tag("Hi <IMG subjects=\"Mira, Sol ,\">dock</img> bye"),
            (
                "Hi  bye".to_owned(),
                Some(SceneTag {
                    prompt: "dock".to_owned(),
                    subjects: vec!["Mira".to_owned(), "Sol".to_owned()],
                })
            )
        );
        assert_eq!(
            extract_scene_tag("<img subjects = 'Ada'>rain</img>").1,
            Some(SceneTag {
                prompt: "rain".to_owned(),
                subjects: vec!["Ada".to_owned()],
            })
        );
        assert_eq!(
            extract_scene_tag("<img alt=x>rain</img>").1,
            Some(SceneTag {
                prompt: "rain".to_owned(),
                subjects: Vec::new(),
            })
        );
        assert_eq!(
            extract_scene_prompt("<images>kept</img>"),
            ("<images>kept</img>".to_owned(), None)
        );
        assert_eq!(
            extract_scene_prompt("text <img subjects=\"A\" never closed"),
            ("text <img subjects=\"A\" never closed".to_owned(), None)
        );
    }
}
