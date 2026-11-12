//! Phrase patterns inside literal keywords: `(a|b)` picks one alternative and
//! a standalone `*` stands for any words in between, so
//! `(went|go) to the (bar|pub)` or `Doc * medic` match without regex.

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Word { text: String, prefix: bool },
    Gap,
}

enum Segment {
    Text(String),
    Choice(Vec<String>),
}

fn segments(keyword: &str) -> Option<Vec<Segment>> {
    let mut segments = Vec::new();
    let mut text = String::new();
    let mut has_choice = false;
    let mut rest = keyword;
    while let Some(open) = rest.find('(') {
        let after = &rest[open + 1..];
        let Some(close) = after.find(')') else { break };
        let inner = &after[..close];
        if inner.contains('|') && !inner.contains('(') {
            text.push_str(&rest[..open]);
            segments.push(Segment::Text(std::mem::take(&mut text)));
            segments.push(Segment::Choice(
                inner
                    .split('|')
                    .map(|choice| choice.trim().to_owned())
                    .collect(),
            ));
            has_choice = true;
        } else {
            text.push_str(&rest[..open + 1 + close + 1]);
        }
        rest = &after[close + 1..];
    }
    text.push_str(rest);
    segments.push(Segment::Text(text));
    let has_gap = segments.iter().any(|segment| match segment {
        Segment::Text(text) => text.split_whitespace().any(|word| word == "*"),
        Segment::Choice(_) => false,
    });
    (has_choice || has_gap).then_some(segments)
}

fn tokens(phrase: &str, normalize: impl Fn(&str) -> String) -> Vec<Token> {
    let mut tokens = Vec::new();
    for raw in phrase.split_whitespace() {
        if raw == "*" {
            if tokens.last() != Some(&Token::Gap) {
                tokens.push(Token::Gap);
            }
            continue;
        }
        let prefix = raw.len() > 1 && raw.ends_with('*');
        let words = normalize(raw.strip_suffix('*').unwrap_or(raw));
        let mut words = words.split_whitespace().peekable();
        while let Some(word) = words.next() {
            tokens.push(Token::Word {
                text: word.to_owned(),
                prefix: prefix && words.peek().is_none(),
            });
        }
    }
    while tokens.first() == Some(&Token::Gap) {
        tokens.remove(0);
    }
    while tokens.last() == Some(&Token::Gap) {
        tokens.pop();
    }
    tokens
}

/// Every phrase a pattern keyword stands for, as word tokens; `None` when the
/// keyword has no `(a|b)` group or standalone `*` and keeps the plain rules.
fn phrases(keyword: &str, normalize: impl Fn(&str) -> String + Copy) -> Option<Vec<Vec<Token>>> {
    let segments = segments(keyword)?;
    let mut expanded = vec![String::new()];
    for segment in &segments {
        expanded = match segment {
            Segment::Text(text) => expanded.into_iter().map(|start| start + text).collect(),
            Segment::Choice(choices) => expanded
                .iter()
                .flat_map(|start| choices.iter().map(move |choice| format!("{start}{choice}")))
                .collect(),
        };
    }
    let mut phrases = Vec::new();
    for phrase in expanded {
        let tokens = tokens(&phrase, normalize);
        if tokens
            .iter()
            .any(|token| matches!(token, Token::Word { .. }))
            && !phrases.contains(&tokens)
        {
            phrases.push(tokens);
        }
    }
    Some(phrases)
}

/// Whether `keyword` uses the phrase pattern syntax.
#[must_use]
pub fn is_phrase_pattern(keyword: &str) -> bool {
    segments(keyword.trim()).is_some()
}

fn word_matches(token: &Token, word: &str) -> bool {
    match token {
        Token::Word { text, prefix: true } => word.starts_with(text.as_str()),
        Token::Word {
            text,
            prefix: false,
        } => word == text,
        Token::Gap => false,
    }
}

fn matches_from(tokens: &[Token], words: &[&str]) -> bool {
    match tokens.split_first() {
        None => true,
        Some((Token::Gap, rest)) => {
            (0..=words.len()).any(|skip| matches_from(rest, &words[skip..]))
        }
        Some((token, rest)) => words
            .split_first()
            .is_some_and(|(word, after)| word_matches(token, word) && matches_from(rest, after)),
    }
}

fn unsegmented_matches(tokens: &[Token], text: &str) -> bool {
    let mut from = 0;
    for piece in tokens.split(|token| *token == Token::Gap) {
        let needle = piece
            .iter()
            .filter_map(|token| match token {
                Token::Word { text, .. } => Some(text.as_str()),
                Token::Gap => None,
            })
            .collect::<Vec<_>>()
            .join(" ");
        if needle.is_empty() {
            continue;
        }
        let Some(found) = text[from..].find(&needle) else {
            return false;
        };
        from += found + needle.len();
    }
    true
}

/// `Some(matched)` for a phrase pattern against already normalized `text`;
/// `None` when the keyword is not a pattern.
pub(crate) fn phrase_pattern_matches(
    keyword: &str,
    text: &str,
    normalize: impl Fn(&str) -> String + Copy,
    unsegmented: impl Fn(&str) -> bool,
) -> Option<bool> {
    let phrases = phrases(keyword, normalize)?;
    let words = text.split_whitespace().collect::<Vec<_>>();
    Some(phrases.iter().any(|tokens| {
        let segmented = tokens.iter().all(|token| match token {
            Token::Word { text, .. } => !unsegmented(text),
            Token::Gap => true,
        }) && !unsegmented(text);
        if segmented {
            (0..words.len()).any(|start| matches_from(tokens, &words[start..]))
        } else {
            unsegmented_matches(tokens, text)
        }
    }))
}

const WORD: &str = r"[\p{L}\p{N}]";
const SEPARATOR: &str = r"[^\p{L}\p{N}]+";

fn phrase_regex(tokens: &[Token]) -> String {
    let mut out = String::new();
    let mut previous_word = false;
    for token in tokens {
        match token {
            Token::Word { text, prefix } => {
                if previous_word {
                    out.push_str(SEPARATOR);
                }
                out.push_str(text);
                if *prefix {
                    out.push_str(WORD);
                    out.push('*');
                }
                previous_word = true;
            }
            Token::Gap => {
                out.push_str(&format!("(?:{SEPARATOR}{WORD}+)*"));
            }
        }
    }
    out
}

pub(crate) fn literal_regex(
    keyword: &str,
    normalize: impl Fn(&str) -> String + Copy,
    unsegmented: impl Fn(&str) -> bool,
) -> String {
    let keyword = keyword.trim();
    let phrases = phrases(keyword, normalize).unwrap_or_else(|| vec![tokens(keyword, normalize)]);
    let alternatives = phrases
        .iter()
        .map(|tokens| phrase_regex(tokens))
        .collect::<Vec<_>>()
        .join("|");
    if unsegmented(keyword) {
        format!("(?:{alternatives})")
    } else {
        format!(r"(?:^|[^\p{{L}}\p{{N}}])(?:{alternatives})(?:[^\p{{L}}\p{{N}}]|$)")
    }
}

#[cfg(test)]
mod tests {
    use crate::{KeywordMatchMode, keyword_matches_with_mode, literal_keyword_regex};

    fn hits(keyword: &str, text: &str) -> bool {
        keyword_matches_with_mode(keyword, text, false, KeywordMatchMode::Literal)
            .expect("literal keyword")
    }

    fn regex_hits(keyword: &str, text: &str) -> bool {
        keyword_matches_with_mode(
            &literal_keyword_regex(keyword, false),
            text,
            false,
            KeywordMatchMode::Regex,
        )
        .expect("exported regex")
    }

    #[test]
    fn choices_expand_into_every_phrase() {
        assert!(hits(
            "(went|go) to the (bar|pub)",
            "Then we went to the pub."
        ));
        assert!(hits("(went|go) to the (bar|pub)", "Let's GO to the bar!"));
        assert!(!hits("(went|go) to the (bar|pub)", "We went to the shop."));
        assert!(hits("run(s|ning|)", "She keeps running."));
        assert!(hits("run(s|ning|)", "Run!"));
        assert!(!hits("run(s|ning|)", "The runner left."));
    }

    #[test]
    fn a_standalone_star_spans_any_words_in_order() {
        assert!(hits("Doc * medic", "Doc is the team's medic"));
        assert!(hits("Doc * medic", "doc, medic"));
        assert!(!hits("Doc * medic", "The medic thanked Doc"));
        assert!(hits("Doc * med*", "Doc fixed it with meds"));
    }

    #[test]
    fn keywords_without_pattern_syntax_keep_the_plain_rules() {
        assert!(hits("Eleanor (Nora)", "Eleanor Nora Whitmore"));
        assert!(hits("dragon*", "dragonstone"));
        assert!(!hits("art", "party"));
    }

    #[test]
    fn unsegmented_scripts_match_in_order() {
        assert!(hits("(東京|大阪) * 行く", "明日は大阪へ行く"));
        assert!(!hits("(東京|大阪) * 行く", "行くのは大阪"));
    }

    #[test]
    fn the_exported_regex_matches_like_the_keyword() {
        for (keyword, text, expected) in [
            (
                "(went|go) to the (bar|pub)",
                "Then we went to the pub.",
                true,
            ),
            ("(went|go) to the (bar|pub)", "We went to the shop.", false),
            ("Doc * medic", "Doc is the team's medic", true),
            ("Doc * medic", "The medic thanked Doc", false),
            ("dragon*", "dragonstone", true),
            ("art", "party", false),
            ("art", "made art.", true),
            ("(東京|大阪)", "明日は大阪へ", true),
        ] {
            assert_eq!(regex_hits(keyword, text), expected, "{keyword} on {text}");
            assert_eq!(hits(keyword, text), expected, "{keyword} on {text}");
        }
    }

    #[test]
    fn every_script_matches_and_turkish_i_folds_together() {
        for (keyword, text) in [
            ("(Москва|Киев) * город", "Москва очень большой город"),
            ("Αθήνα", "Πάμε στην Αθήνα αύριο"),
            ("istanbul", "İstanbul'a gidiyoruz"),
            ("İstanbul", "istanbul çok güzel"),
            ("ılık", "ILIK bir gün"),
            ("(हिंदी|उर्दू) * हूँ", "मैं हिंदी बोलता हूँ"),
            ("القاهرة", "سافرت إلى القاهرة أمس"),
            ("서울", "나는 서울에 산다"),
            ("Ελένη*", "Η Ελένης φίλη"),
        ] {
            assert!(hits(keyword, text), "{keyword} on {text}");
            assert!(regex_hits(keyword, text), "{keyword} exported on {text}");
        }
        assert!(
            keyword_matches_with_mode("İstanbul", "istanbul", false, KeywordMatchMode::Regex)
                .expect("regex"),
            "regex keywords fold the Turkish i too"
        );
    }
}
