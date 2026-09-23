//! Pure mode: scoring model output against the content dictionaries and
//! blocking it above the level's threshold, for whole texts and for streams.
//! The English dictionary always applies; the lexicon of the language the
//! text is detected in is added to it.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{LazyLock, Mutex};

use serde::Deserialize;

use crate::content_lexicons;

/// How strictly model output is filtered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PureModeLevel {
    Off = 0,
    Low = 1,
    Standard = 2,
    Strict = 3,
}

impl PureModeLevel {
    #[must_use]
    pub fn try_from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "off" => Some(Self::Off),
            "low" => Some(Self::Low),
            "standard" => Some(Self::Standard),
            "strict" => Some(Self::Strict),
            _ => None,
        }
    }

    const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Off,
            1 => Self::Low,
            3 => Self::Strict,
            _ => Self::Standard,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Low => "low",
            Self::Standard => "standard",
            Self::Strict => "strict",
        }
    }

    pub(crate) const fn min_weight(self) -> f32 {
        match self {
            Self::Low => 0.8,
            _ => 0.0,
        }
    }

    const fn threshold(self) -> f32 {
        match self {
            Self::Off => f32::MAX,
            Self::Low => 2.0,
            Self::Standard => 1.5,
            Self::Strict => 1.0,
        }
    }
}

#[derive(Deserialize)]
struct Dictionary {
    #[serde(rename = "EXPLICIT_SEXUAL")]
    explicit_sexual: Vec<(String, f32)>,
    #[serde(rename = "VIOLENCE_GRAPHIC")]
    violence_graphic: Vec<(String, f32)>,
    #[serde(rename = "SLURS")]
    slurs: Vec<(String, f32)>,
    #[serde(rename = "CONTEXT_ALLOWLIST")]
    allowlist: Vec<String>,
}

static DICTIONARY: LazyLock<Dictionary> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../resources/content-filter-dictionary.json"))
        .expect("the bundled content filter dictionary parses")
});

fn dictionary() -> &'static Dictionary {
    &DICTIONARY
}

/// The outcome of one check.
#[derive(Debug, Clone, PartialEq)]
pub struct FilterResult {
    pub blocked: bool,
    pub score: f32,
    pub matched_terms: Vec<String>,
}

impl FilterResult {
    const fn clean() -> Self {
        Self {
            blocked: false,
            score: 0.0,
            matched_terms: Vec::new(),
        }
    }
}

/// The last 500 bytes of a stream, rescanned on every delta.
#[derive(Debug, Clone, Default)]
pub struct StreamFilterContext {
    accumulated: String,
}

pub(crate) const STREAM_WINDOW_BYTES: usize = 500;

/// A redacted record of a check that scored above zero.
#[derive(Debug, Clone, PartialEq)]
pub struct FilterLogEntry {
    pub timestamp_ms: u64,
    pub text_snippet: String,
    pub score: f32,
    pub blocked: bool,
    pub matched_terms: Vec<String>,
    pub level: PureModeLevel,
}

const FILTER_LOG_MAX: usize = 200;

/// The filter engine; its level can change while it is shared.
#[derive(Debug)]
pub struct ContentFilter {
    level: AtomicU8,
    log: Mutex<Vec<FilterLogEntry>>,
}

impl ContentFilter {
    #[must_use]
    pub fn new(level: PureModeLevel) -> Self {
        Self {
            level: AtomicU8::new(level as u8),
            log: Mutex::new(Vec::new()),
        }
    }

    #[must_use]
    pub fn level(&self) -> PureModeLevel {
        PureModeLevel::from_u8(self.level.load(Ordering::Relaxed))
    }

    pub fn set_level(&self, level: PureModeLevel) {
        self.level.store(level as u8, Ordering::Relaxed);
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.level() != PureModeLevel::Off
    }

    /// The recorded hits, oldest first.
    #[must_use]
    pub fn hit_log(&self) -> Vec<FilterLogEntry> {
        self.log.lock().map(|log| log.clone()).unwrap_or_default()
    }

    pub fn clear_hit_log(&self) {
        if let Ok(mut log) = self.log.lock() {
            log.clear();
        }
    }

    fn redact_snippet(text: &str, max_chars: usize) -> String {
        let mut out = String::new();
        for (count, character) in text.chars().enumerate() {
            if count >= max_chars {
                out.push_str("...");
                break;
            }
            if character.is_whitespace() || character.is_ascii_punctuation() {
                out.push(character);
            } else {
                out.push('*');
            }
        }
        out
    }

    fn record_hit(&self, text: &str, result: &FilterResult, level: PureModeLevel, now_ms: u64) {
        if result.score <= 0.0 {
            return;
        }
        let entry = FilterLogEntry {
            timestamp_ms: now_ms,
            text_snippet: Self::redact_snippet(text, 200),
            score: result.score,
            blocked: result.blocked,
            matched_terms: result.matched_terms.clone(),
            level,
        };
        if let Ok(mut log) = self.log.lock() {
            if log.len() >= FILTER_LOG_MAX {
                log.remove(0);
            }
            log.push(entry);
        }
    }

    fn evaluate(text: &str, level: PureModeLevel) -> FilterResult {
        let cleaned = Self::strip_formatting(text);
        let lower = cleaned.to_lowercase();
        let unicode_norm = Self::normalize_unicode(&lower);
        let normalized = Self::normalize_leet(&unicode_norm);
        let has_context = Self::has_allowlist_context(&normalized);
        let words = Self::tokenize(&normalized);
        let (mut score, mut matched_terms) =
            Self::score_text(&words, &normalized, has_context, level);
        content_lexicons::score(
            &lower,
            level,
            if has_context { 0.5 } else { 1.0 },
            &mut score,
            &mut matched_terms,
        );
        FilterResult {
            blocked: score >= level.threshold(),
            score,
            matched_terms,
        }
    }

    /// Checks a whole text.
    pub fn check_text(&self, text: &str, now_ms: u64) -> FilterResult {
        let level = self.level();
        if level == PureModeLevel::Off {
            return FilterResult::clean();
        }
        let result = Self::evaluate(text, level);
        self.record_hit(text, &result, level, now_ms);
        result
    }

    /// Checks a stream delta against the stream's sliding window.
    pub fn check_delta(
        &self,
        context: &mut StreamFilterContext,
        delta: &str,
        now_ms: u64,
    ) -> FilterResult {
        let level = self.level();
        if level == PureModeLevel::Off {
            return FilterResult::clean();
        }
        context.accumulated.push_str(delta);
        if context.accumulated.len() > STREAM_WINDOW_BYTES {
            let mut trim_at = context.accumulated.len() - STREAM_WINDOW_BYTES;
            while trim_at < context.accumulated.len()
                && !context.accumulated.is_char_boundary(trim_at)
            {
                trim_at += 1;
            }
            context.accumulated.drain(..trim_at);
        }
        let result = Self::evaluate(&context.accumulated, level);
        self.record_hit(&context.accumulated, &result, level, now_ms);
        result
    }

    pub fn normalize_unicode(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        for ch in text.chars() {
            match ch {
                ch if is_invisible(ch) => {}

                '\u{0430}' | '\u{0410}' => result.push('a'),
                '\u{0435}' | '\u{0415}' => result.push('e'),
                '\u{043E}' | '\u{041E}' => result.push('o'),
                '\u{0441}' | '\u{0421}' => result.push('c'),
                '\u{0440}' | '\u{0420}' => result.push('p'),
                '\u{0443}' | '\u{0423}' => result.push('y'),
                '\u{0445}' | '\u{0425}' => result.push('x'),
                '\u{0456}' => result.push('i'),
                '\u{0458}' => result.push('j'),
                '\u{0455}' => result.push('s'),

                '\u{03BF}' => result.push('o'),
                '\u{03B1}' => result.push('a'),
                '\u{03B5}' => result.push('e'),

                '\u{00E0}'..='\u{00E5}' | '\u{0101}' | '\u{0103}' | '\u{0105}' => result.push('a'),
                '\u{00E8}'..='\u{00EB}'
                | '\u{0113}'
                | '\u{0115}'
                | '\u{0117}'
                | '\u{0119}'
                | '\u{011B}' => result.push('e'),
                '\u{00EC}'..='\u{00EF}' | '\u{012B}' | '\u{012D}' | '\u{012F}' | '\u{0131}' => {
                    result.push('i')
                }
                '\u{00F2}'..='\u{00F6}' | '\u{00F8}' | '\u{014D}' | '\u{014F}' | '\u{0151}' => {
                    result.push('o')
                }
                '\u{00F9}'..='\u{00FC}'
                | '\u{016B}'
                | '\u{016D}'
                | '\u{016F}'
                | '\u{0171}'
                | '\u{0173}' => result.push('u'),
                '\u{00F1}' | '\u{0144}' | '\u{0146}' | '\u{0148}' => result.push('n'),
                '\u{00E7}' | '\u{0107}' | '\u{010D}' => result.push('c'),
                '\u{015B}' | '\u{015D}' | '\u{015F}' | '\u{0161}' => result.push('s'),
                '\u{017A}' | '\u{017C}' | '\u{017E}' => result.push('z'),
                '\u{00FD}' | '\u{00FF}' => result.push('y'),
                '\u{00F0}' | '\u{010F}' | '\u{0111}' => result.push('d'),
                '\u{013A}' | '\u{013E}' | '\u{0142}' => result.push('l'),
                '\u{0155}' | '\u{0159}' => result.push('r'),
                '\u{0165}' | '\u{0167}' => result.push('t'),
                '\u{011F}' | '\u{0121}' => result.push('g'),

                '\u{00DF}' => result.push_str("ss"),
                '\u{00E6}' => result.push_str("ae"),

                other => result.push(other),
            }
        }
        result
    }

    /// Collapse runs of 3+ identical characters to a single character.
    /// Runs of exactly 2 are preserved (common in English: "all", "too", "see").
    /// Used as a secondary matching pass to catch evasion like "fuuuck" → "fuck".
    pub fn collapse_repeated_chars(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            result.push(ch);
            let mut run = 1u32;
            while chars.peek() == Some(&ch) {
                chars.next();
                run += 1;
            }
            if run == 2 {
                result.push(ch);
            }
        }
        result
    }

    /// Strip markdown formatting characters that LLMs use for roleplay emphasis.
    pub fn strip_formatting(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        for ch in text.chars() {
            match ch {
                '*' | '_' | '~' => {}
                _ => result.push(ch),
            }
        }
        result
    }

    /// Normalize common leet-speak substitutions so "p0rn", "$lut", "n!gg3r"
    /// are mapped back to their dictionary forms. Digit mappings always apply;
    /// symbol mappings (@, $, !, +) only apply when followed by a word character
    /// to avoid mangling trailing punctuation (e.g. "hello!" stays "hello!").
    pub fn normalize_leet(text: &str) -> String {
        let chars: Vec<char> = text.chars().collect();
        let mut result = String::with_capacity(text.len());
        for (i, &ch) in chars.iter().enumerate() {
            let next_is_word = i + 1 < chars.len()
                && (chars[i + 1].is_alphanumeric() || "$@!+".contains(chars[i + 1]));
            result.push(match ch {
                '0' => 'o',
                '1' => 'i',
                '3' => 'e',
                '4' => 'a',
                '5' => 's',
                '7' => 't',
                '8' => 'b',
                '9' => 'g',
                '@' if next_is_word => 'a',
                '$' if next_is_word => 's',
                '!' if next_is_word => 'i',
                '+' if next_is_word => 't',
                other => other,
            });
        }
        result
    }

    /// Split normalized text into word tokens on non-alphanumeric boundaries.
    /// Apostrophes within words are preserved for contractions.
    pub fn tokenize(text: &str) -> Vec<&str> {
        text.split(|c: char| !c.is_alphanumeric() && c != '\'')
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Check if a text word matches a single-word dictionary term with
    /// morphological tolerance. The word must start with the term, and
    /// the remaining suffix must be short (≤3 chars, covers -s, -ed, -ing,
    /// -er, -ly) or be a recognized longer suffix (-tion, -ation, etc.).
    /// This prevents false positives like "cocktail" matching "cock" (suffix
    /// "tail" = 4 chars, not a morphological suffix).
    pub fn word_matches_term(word: &str, term: &str) -> bool {
        if !word.starts_with(term) {
            return false;
        }
        let suffix_len = word.len() - term.len();
        if suffix_len <= 3 {
            return true;
        }
        let suffix = &word[term.len()..];
        matches!(
            suffix,
            "tion"
                | "ation"
                | "ness"
                | "able"
                | "ible"
                | "ized"
                | "ised"
                | "ious"
                | "eous"
                | "ally"
                | "ical"
                | "ling"
                | "ated"
                | "ates"
                | "ings"
                | "ment"
                | "sion"
        )
    }

    /// Check if any N-gram (sliding window) of consecutive words in `text_words`
    /// matches the `term_words` sequence, using morphological tolerance per word.
    /// This replaces substring matching for multi-word terms to enforce word
    /// boundaries — e.g. "scum on her" no longer false-positives on "cum on her".
    pub fn ngram_matches_term(text_words: &[&str], term_words: &[&str]) -> bool {
        let n = term_words.len();
        if n == 0 || text_words.len() < n {
            return false;
        }
        'outer: for window in text_words.windows(n) {
            for (text_word, term_word) in window.iter().zip(term_words.iter()) {
                if !Self::word_matches_term(text_word, term_word) {
                    continue 'outer;
                }
            }
            return true;
        }
        false
    }

    /// Check whether allowlist context terms are present, which halves match weights.
    pub fn has_allowlist_context(text: &str) -> bool {
        dictionary()
            .allowlist
            .iter()
            .any(|term| text.contains(term.as_str()))
    }

    /// Score tokenized + normalized text against dictionaries based on level.
    pub fn score_text(
        words: &[&str],
        normalized_text: &str,
        has_context: bool,
        level: PureModeLevel,
    ) -> (f32, Vec<String>) {
        let context_factor = if has_context { 0.5 } else { 1.0 };
        let mut total_score: f32 = 0.0;
        let mut matched = Vec::new();

        let collapsed_text = Self::collapse_repeated_chars(normalized_text);
        let collapsed_words_vec;
        let collapsed_words: &[&str] = if collapsed_text != normalized_text {
            collapsed_words_vec = Self::tokenize(&collapsed_text);
            &collapsed_words_vec
        } else {
            &[]
        };

        match level {
            PureModeLevel::Off => {}
            PureModeLevel::Low => {
                Self::score_dictionary(
                    words,
                    normalized_text,
                    collapsed_words,
                    &dictionary().explicit_sexual,
                    Some(PureModeLevel::Low.min_weight()),
                    context_factor,
                    &mut total_score,
                    &mut matched,
                );
                Self::score_dictionary(
                    words,
                    normalized_text,
                    collapsed_words,
                    &dictionary().slurs,
                    None,
                    context_factor,
                    &mut total_score,
                    &mut matched,
                );
            }
            PureModeLevel::Standard | PureModeLevel::Strict => {
                for dict in &[
                    &dictionary().explicit_sexual,
                    &dictionary().violence_graphic,
                    &dictionary().slurs,
                ] {
                    Self::score_dictionary(
                        words,
                        normalized_text,
                        collapsed_words,
                        dict,
                        None,
                        context_factor,
                        &mut total_score,
                        &mut matched,
                    );
                }
            }
        }

        (total_score, matched)
    }

    /// Score one dictionary against the text.
    ///
    /// Strategy:
    /// - **Single-word terms**: word-boundary matching with morphological suffix
    ///   tolerance. Eliminates false positives like "cocktail"→"cock".
    ///   Terms with a trailing space in the dictionary (e.g. "porn ") require
    ///   an exact word match — the space was an intentional boundary marker.
    /// - **Phrasal terms** (contain spaces): word N-gram matching with
    ///   morphological tolerance per word. Prevents "scum on her" → "cum on her".
    /// - **Compound terms** (hyphen/slash only, no spaces): substring matching
    ///   on `normalized_text` so the punctuation must be present. Prevents
    ///   "hard on to the floor" → "hard-on".
    ///
    /// A secondary pass on `collapsed_words` (runs of 3+ chars reduced to 1) catches
    /// repeated-character evasion like "fuuuck me" → "fuck me".
    #[expect(
        clippy::too_many_arguments,
        reason = "the scoring pass keeps the old engine's shape so its matching stays verifiable"
    )]
    fn score_dictionary(
        words: &[&str],
        normalized_text: &str,
        collapsed_words: &[&str],
        dict: &[(String, f32)],
        min_weight: Option<f32>,
        context_factor: f32,
        total_score: &mut f32,
        matched: &mut Vec<String>,
    ) {
        let min_w = min_weight.unwrap_or(0.0);
        for (term, weight) in dict {
            let (term, weight) = (term.as_str(), *weight);
            if weight < min_w {
                continue;
            }
            let exact_only = term.ends_with(' ');
            let trimmed = term.trim();
            let has_space = trimmed.contains(' ');
            let has_punct = trimmed.contains('-') || trimmed.contains('/');
            let is_phrasal = has_space;
            let is_compound = !has_space && has_punct;

            let found = if is_phrasal {
                let term_words = Self::tokenize(trimmed);
                Self::ngram_matches_term(words, &term_words)
                    || (!collapsed_words.is_empty()
                        && Self::ngram_matches_term(collapsed_words, &term_words))
            } else if is_compound {
                normalized_text.contains(trimmed)
            } else if exact_only {
                words.contains(&trimmed)
                    || (!collapsed_words.is_empty() && collapsed_words.contains(&trimmed))
            } else {
                words.iter().any(|w| Self::word_matches_term(w, trimmed))
                    || (!collapsed_words.is_empty()
                        && collapsed_words
                            .iter()
                            .any(|w| Self::word_matches_term(w, trimmed)))
            };

            if found {
                let effective_weight = weight * context_factor;
                *total_score += effective_weight;
                matched.push(trimmed.to_string());
            }
        }
    }
}

/// Characters that render as nothing and are dropped before matching.
pub(crate) const fn is_invisible(ch: char) -> bool {
    matches!(
        ch,
        '\u{200B}'
            | '\u{200C}'
            | '\u{200D}'
            | '\u{FEFF}'
            | '\u{00AD}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{2060}'
            | '\u{2061}'
            | '\u{2062}'
            | '\u{2063}'
            | '\u{2064}'
            | '\u{FE00}'..='\u{FE0F}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocked(level: PureModeLevel, text: &str) -> bool {
        ContentFilter::new(level).check_text(text, 0).blocked
    }

    #[test]
    fn levels_block_what_the_old_filter_blocked() {
        let violence = "the villain threatened to decapitate and disembowel everyone";
        assert!(!blocked(PureModeLevel::Off, violence));
        assert!(!blocked(PureModeLevel::Low, violence));
        assert!(blocked(PureModeLevel::Standard, violence));
        assert!(blocked(PureModeLevel::Strict, violence));
        for harmless in [
            "hello there, can we plan a hiking trip this weekend?",
            "I ordered a cocktail and checked the dictionary after dinner.",
            "she graduated magna cum laude and thanked her professor.",
            "he slipped and landed hard on to the floor during practice.",
        ] {
            assert!(!blocked(PureModeLevel::Standard, harmless), "{harmless}");
        }
    }

    #[test]
    fn streams_are_checked_in_a_sliding_window_and_hits_are_redacted() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let mut context = StreamFilterContext::default();
        assert!(
            !filter
                .check_delta(&mut context, "the villain threatened to decap", 1)
                .blocked
        );
        assert!(
            filter
                .check_delta(&mut context, "itate and disembowel everyone", 2)
                .blocked
        );
        let log = filter.hit_log();
        assert!(
            log.last()
                .is_some_and(|entry| entry.blocked && !entry.text_snippet.contains("villain"))
        );
        filter.clear_hit_log();
        assert!(filter.hit_log().is_empty());
        filter.set_level(PureModeLevel::Off);
        assert!(!filter.check_text("decapitate and disembowel", 3).blocked);
        assert_eq!(
            PureModeLevel::try_from_str(" Strict "),
            Some(PureModeLevel::Strict)
        );
    }
}
