//! The scene tag a model writes into a direct chat reply, `<img>…</img>`.

const SCENE_TAG_OPEN: &str = "<img>";
const SCENE_CLOSE_TOKENS: [&str; 3] = ["</img>", "[continue]", "[/continue]"];

fn find_ignore_ascii_case(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

/// Removes every scene tag and returns the first non-blank prompt. A tag
/// closes at `</img>`, `[continue]` or `[/continue]` (any case); an unclosed
/// tag drops the rest of the text. The result is trimmed.
#[must_use]
pub fn extract_scene_prompt(content: &str) -> (String, Option<String>) {
    let mut visible = String::with_capacity(content.len());
    let mut prompt = None;
    let mut rest = content;
    loop {
        let Some(open) = find_ignore_ascii_case(rest, SCENE_TAG_OPEN) else {
            visible.push_str(rest);
            break;
        };
        visible.push_str(&rest[..open]);
        rest = &rest[open + SCENE_TAG_OPEN.len()..];
        let Some((close, token)) = SCENE_CLOSE_TOKENS
            .iter()
            .filter_map(|token| find_ignore_ascii_case(rest, token).map(|index| (index, *token)))
            .min_by_key(|(index, _)| *index)
        else {
            break;
        };
        let candidate = rest[..close].trim();
        if prompt.is_none() && !candidate.is_empty() {
            prompt = Some(candidate.to_owned());
        }
        rest = &rest[close + token.len()..];
    }
    (visible.trim().to_owned(), prompt)
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
}
