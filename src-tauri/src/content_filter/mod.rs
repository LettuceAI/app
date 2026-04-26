#[cfg(test)]
mod adversarial_corpus;
mod dictionary;

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

/// Pure Mode filtering level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PureModeLevel {
    Off = 0,
    Low = 1,
    Standard = 2,
    Strict = 3,
}

impl PureModeLevel {
    pub fn try_from_str(s: &str) -> Option<Self> {
        let normalized = s.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "off" => Some(Self::Off),
            "low" => Some(Self::Low),
            "standard" => Some(Self::Standard),
            "strict" => Some(Self::Strict),
            _ => None,
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Off,
            1 => Self::Low,
            2 => Self::Standard,
            3 => Self::Strict,
            _ => Self::Standard,
        }
    }

    pub fn from_str(s: &str) -> Self {
        Self::try_from_str(s).unwrap_or(Self::Standard)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Low => "low",
            Self::Standard => "standard",
            Self::Strict => "strict",
        }
    }

    fn threshold(&self) -> f32 {
        match self {
            Self::Off => f32::MAX,
            Self::Low => 2.0,
            Self::Standard => 1.5,
            Self::Strict => 1.0,
        }
    }
}

pub fn level_from_app_state(app_state: Option<&serde_json::Value>) -> PureModeLevel {
    if let Some(level) = app_state
        .and_then(|v| v.get("pureModeLevel"))
        .and_then(|v| v.as_str())
        .and_then(PureModeLevel::try_from_str)
    {
        return level;
    }
    let enabled = app_state
        .and_then(|v| v.get("pureModeEnabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    if enabled {
        PureModeLevel::Standard
    } else {
        PureModeLevel::Off
    }
}

/// Result of a content filter check.
pub struct FilterResult {
    pub blocked: bool,
    pub score: f32,
    pub matched_terms: Vec<String>,
}

/// Sliding window context for incremental streaming checks.
pub struct StreamFilterContext {
    accumulated: String,
    window_size: usize,
}

impl StreamFilterContext {
    pub fn new() -> Self {
        Self {
            accumulated: String::new(),
            window_size: 500,
        }
    }
}

/// A recorded filter hit for the security log.
#[derive(Clone, serde::Serialize)]
pub struct FilterLogEntry {
    pub timestamp_ms: u64,
    pub text_snippet: String,
    pub score: f32,
    pub blocked: bool,
    pub matched_terms: Vec<String>,
    pub level: String,
}

const FILTER_LOG_MAX: usize = 200;

/// The content filter engine. Holds an atomic level for zero-cost when disabled.
pub struct ContentFilter {
    level: AtomicU8,
    log: Mutex<Vec<FilterLogEntry>>,
}

impl ContentFilter {
    pub fn new(level: PureModeLevel) -> Self {
        Self {
            level: AtomicU8::new(level as u8),
            log: Mutex::new(Vec::new()),
        }
    }

    fn redact_snippet(text: &str, max_chars: usize) -> String {
        let mut out = String::new();
        for (count, ch) in text.chars().enumerate() {
            if count >= max_chars {
                out.push_str("...");
                break;
            }
            if ch.is_whitespace() || ch.is_ascii_punctuation() {
                out.push(ch);
            } else {
                out.push('*');
            }
        }
        out
    }

    /// Record a filter hit to the internal log (only when score > 0).
    fn record_hit(&self, text: &str, result: &FilterResult, level: PureModeLevel) {
        if result.score <= 0.0 {
            return;
        }
        let snippet = Self::redact_snippet(text, 200);
        let entry = FilterLogEntry {
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            text_snippet: snippet,
            score: result.score,
            blocked: result.blocked,
            matched_terms: result.matched_terms.clone(),
            level: level.as_str().to_string(),
        };
        if let Ok(mut log) = self.log.lock() {
            if log.len() >= FILTER_LOG_MAX {
                log.remove(0);
            }
            log.push(entry);
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.level() != PureModeLevel::Off
    }

    pub fn level(&self) -> PureModeLevel {
        PureModeLevel::from_u8(self.level.load(Ordering::Relaxed))
    }

    pub fn set_level(&self, level: PureModeLevel) {
        self.level.store(level as u8, Ordering::Relaxed);
    }

    /// Check a full text block against the content filter dictionary.
    pub fn check_text(&self, text: &str) -> FilterResult {
        let level = self.level();
        if level == PureModeLevel::Off {
            return FilterResult {
                blocked: false,
                score: 0.0,
                matched_terms: vec![],
            };
        }
        let cleaned = Self::strip_formatting(text);
        let lower = cleaned.to_lowercase();
        let unicode_norm = Self::normalize_unicode(&lower);
        let normalized = Self::normalize_leet(&unicode_norm);
        let has_context = Self::has_allowlist_context(&normalized);
        let words = Self::tokenize(&normalized);
        let (score, matched) = Self::score_text(&words, &normalized, has_context, level);

        let result = FilterResult {
            blocked: score >= level.threshold(),
            score,
            matched_terms: matched,
        };
        self.record_hit(text, &result, level);
        result
    }

    /// Incremental check for streaming deltas. Accumulates text in a sliding window
    /// and rescans the bounded window to avoid score double-counting.
    pub fn check_delta(&self, ctx: &mut StreamFilterContext, delta: &str) -> FilterResult {
        let level = self.level();
        if level == PureModeLevel::Off {
            return FilterResult {
                blocked: false,
                score: 0.0,
                matched_terms: vec![],
            };
        }
        ctx.accumulated.push_str(delta);

        // Trim to sliding window size
        if ctx.accumulated.len() > ctx.window_size {
            let mut trim_at = ctx.accumulated.len() - ctx.window_size;
            while trim_at < ctx.accumulated.len() && !ctx.accumulated.is_char_boundary(trim_at) {
                trim_at += 1;
            }
            ctx.accumulated.drain(..trim_at);
        }

        let cleaned = Self::strip_formatting(&ctx.accumulated);
        let lower = cleaned.to_lowercase();
        let unicode_norm = Self::normalize_unicode(&lower);
        let normalized = Self::normalize_leet(&unicode_norm);
        let has_context = Self::has_allowlist_context(&normalized);
        let words = Self::tokenize(&normalized);
        let (score, matched) = Self::score_text(&words, &normalized, has_context, level);

        let result = FilterResult {
            blocked: score >= level.threshold(),
            score,
            matched_terms: matched,
        };
        self.record_hit(&ctx.accumulated, &result, level);
        result
    }

    // ── Text preprocessing ───────────────────────────────────────────

    /// Normalize Unicode: strip invisible characters, map homoglyphs to ASCII,
    /// and strip diacritical marks. Runs BEFORE leet-speak normalization so that
    /// homoglyph evasion (Cyrillic а for Latin a, etc.) is neutralized early.
    fn normalize_unicode(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        for ch in text.chars() {
            match ch {
                // ── Invisible / zero-width characters: drop ──────────
                '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' | '\u{00AD}' | '\u{200E}'
                | '\u{200F}' | '\u{2060}' | '\u{2061}' | '\u{2062}' | '\u{2063}' | '\u{2064}' => {}
                '\u{FE00}'..='\u{FE0F}' => {}

                // ── Cyrillic homoglyphs (text already lowercased) ────
                '\u{0430}' | '\u{0410}' => result.push('a'), // а / А
                '\u{0435}' | '\u{0415}' => result.push('e'), // е / Е
                '\u{043E}' | '\u{041E}' => result.push('o'), // о / О
                '\u{0441}' | '\u{0421}' => result.push('c'), // с / С
                '\u{0440}' | '\u{0420}' => result.push('p'), // р / Р
                '\u{0443}' | '\u{0423}' => result.push('y'), // у / У
                '\u{0445}' | '\u{0425}' => result.push('x'), // х / Х
                '\u{0456}' => result.push('i'),              // і
                '\u{0458}' => result.push('j'),              // ј
                '\u{0455}' => result.push('s'),              // ѕ

                // ── Greek homoglyphs ─────────────────────────────────
                '\u{03BF}' => result.push('o'), // ο
                '\u{03B1}' => result.push('a'), // α
                '\u{03B5}' => result.push('e'), // ε

                // ── Accented Latin → ASCII base ──────────────────────
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

                // ── Multi-char mappings ──────────────────────────────
                '\u{00DF}' => result.push_str("ss"), // ß
                '\u{00E6}' => result.push_str("ae"), // æ

                other => result.push(other),
            }
        }
        result
    }

    /// Collapse runs of 3+ identical characters to a single character.
    /// Runs of exactly 2 are preserved (common in English: "all", "too", "see").
    /// Used as a secondary matching pass to catch evasion like "fuuuck" → "fuck".
    fn collapse_repeated_chars(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            result.push(ch);
            let mut run = 1u32;
            while chars.peek() == Some(&ch) {
                chars.next();
                run += 1;
            }
            // Preserve runs of exactly 2 (common English double letters)
            if run == 2 {
                result.push(ch);
            }
            // Runs of 3+ collapse to the single char already pushed
        }
        result
    }

    /// Strip markdown formatting characters that LLMs use for roleplay emphasis.
    fn strip_formatting(text: &str) -> String {
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
    fn normalize_leet(text: &str) -> String {
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
    fn tokenize(text: &str) -> Vec<&str> {
        text.split(|c: char| !c.is_alphanumeric() && c != '\'')
            .filter(|s| !s.is_empty())
            .collect()
    }

    // ── Matching helpers ─────────────────────────────────────────────

    /// Check if a text word matches a single-word dictionary term with
    /// morphological tolerance. The word must start with the term, and
    /// the remaining suffix must be short (≤3 chars, covers -s, -ed, -ing,
    /// -er, -ly) or be a recognized longer suffix (-tion, -ation, etc.).
    /// This prevents false positives like "cocktail" matching "cock" (suffix
    /// "tail" = 4 chars, not a morphological suffix).
    fn word_matches_term(word: &str, term: &str) -> bool {
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
    fn ngram_matches_term(text_words: &[&str], term_words: &[&str]) -> bool {
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

    // ── Context ──────────────────────────────────────────────────────

    /// Check whether allowlist context terms are present, which halves match weights.
    fn has_allowlist_context(text: &str) -> bool {
        dictionary::CONTEXT_ALLOWLIST
            .iter()
            .any(|term| text.contains(term))
    }

    // ── Scoring engine ───────────────────────────────────────────────

    /// Score tokenized + normalized text against dictionaries based on level.
    fn score_text(
        words: &[&str],
        normalized_text: &str,
        has_context: bool,
        level: PureModeLevel,
    ) -> (f32, Vec<String>) {
        let context_factor = if has_context { 0.5 } else { 1.0 };
        let mut total_score: f32 = 0.0;
        let mut matched = Vec::new();

        // Pre-compute collapsed text for secondary matching (catches "fuuuck" → "fuck")
        let collapsed_text = Self::collapse_repeated_chars(normalized_text);
        let collapsed_words_vec;
        let collapsed_words: &[&str] = if collapsed_text != normalized_text {
            collapsed_words_vec = Self::tokenize(&collapsed_text);
            &collapsed_words_vec
        } else {
            // Text had no runs of 3+; skip redundant secondary pass
            &[]
        };

        match level {
            PureModeLevel::Off => {}
            PureModeLevel::Low => {
                Self::score_dictionary(
                    words,
                    normalized_text,
                    collapsed_words,
                    dictionary::EXPLICIT_SEXUAL,
                    Some(0.8),
                    context_factor,
                    &mut total_score,
                    &mut matched,
                );
                Self::score_dictionary(
                    words,
                    normalized_text,
                    collapsed_words,
                    dictionary::SLURS,
                    None,
                    context_factor,
                    &mut total_score,
                    &mut matched,
                );
            }
            PureModeLevel::Standard | PureModeLevel::Strict => {
                for dict in &[
                    dictionary::EXPLICIT_SEXUAL,
                    dictionary::VIOLENCE_GRAPHIC,
                    dictionary::SLURS,
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
    fn score_dictionary(
        words: &[&str],
        normalized_text: &str,
        collapsed_words: &[&str],
        dict: &[(&str, f32)],
        min_weight: Option<f32>,
        context_factor: f32,
        total_score: &mut f32,
        matched: &mut Vec<String>,
    ) {
        let min_w = min_weight.unwrap_or(0.0);
        for &(term, weight) in dict {
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
                // Phrasal (space-separated): word N-gram matching
                let term_words = Self::tokenize(trimmed);
                Self::ngram_matches_term(words, &term_words)
                    || (!collapsed_words.is_empty()
                        && Self::ngram_matches_term(collapsed_words, &term_words))
            } else if is_compound {
                // Compound (hyphen/slash only): require punctuation in text
                normalized_text.contains(trimmed)
            } else if exact_only {
                // Trailing-space terms: exact word match only
                words.contains(&trimmed)
                    || (!collapsed_words.is_empty() && collapsed_words.contains(&trimmed))
            } else {
                // Single-word: word-boundary + morphological matching
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

#[tauri::command]
pub fn set_content_filter_level(app: tauri::AppHandle, level: String) -> Result<(), String> {
    use tauri::Manager;
    if let Some(filter) = app.try_state::<ContentFilter>() {
        filter.set_level(PureModeLevel::from_str(&level));
        Ok(())
    } else {
        Err("Content filter not initialized".to_string())
    }
}

/// Debug command: show how the content filter pipeline processes a sentence.
#[tauri::command]
pub fn debug_content_filter(
    app: tauri::AppHandle,
    text: String,
) -> Result<serde_json::Value, String> {
    if !cfg!(debug_assertions) {
        return Err("debug_content_filter is only available in debug builds".to_string());
    }
    use tauri::Manager;
    let filter = app
        .try_state::<ContentFilter>()
        .ok_or("Content filter not initialized")?;

    let level = filter.level();
    let stripped = ContentFilter::strip_formatting(&text);
    let lower = stripped.to_lowercase();
    let unicode_norm = ContentFilter::normalize_unicode(&lower);
    let normalized = ContentFilter::normalize_leet(&unicode_norm);
    let tokens: Vec<&str> = ContentFilter::tokenize(&normalized);
    let collapsed = ContentFilter::collapse_repeated_chars(&normalized);
    let collapsed_tokens: Vec<&str> = ContentFilter::tokenize(&collapsed);
    let has_context = ContentFilter::has_allowlist_context(&normalized);
    // Run scoring directly to avoid polluting the filter log
    let (score, matched_terms) =
        ContentFilter::score_text(&tokens, &normalized, has_context, level);
    let blocked = score >= level.threshold();

    Ok(serde_json::json!({
        "level": level.as_str(),
        "pipeline": {
            "original": text,
            "stripped": stripped,
            "lowercased": lower,
            "unicode_normalized": unicode_norm,
            "leet_normalized": normalized,
            "tokens": tokens,
            "collapsed": collapsed,
            "collapsed_tokens": collapsed_tokens,
        },
        "context_allowlist_hit": has_context,
        "result": {
            "blocked": blocked,
            "score": score,
            "matched_terms": matched_terms,
        }
    }))
}

#[tauri::command]
pub fn get_filter_log(app: tauri::AppHandle) -> Result<Vec<FilterLogEntry>, String> {
    if !cfg!(debug_assertions) {
        return Err("get_filter_log is only available in debug builds".to_string());
    }
    use tauri::Manager;
    let filter = app
        .try_state::<ContentFilter>()
        .ok_or("Content filter not initialized")?;
    let entries = filter.log.lock().map_err(|e| e.to_string())?.clone();
    Ok(entries)
}

#[tauri::command]
pub fn clear_filter_log(app: tauri::AppHandle) -> Result<(), String> {
    if !cfg!(debug_assertions) {
        return Err("clear_filter_log is only available in debug builds".to_string());
    }
    use tauri::Manager;
    let filter = app
        .try_state::<ContentFilter>()
        .ok_or("Content filter not initialized")?;
    filter.log.lock().map_err(|e| e.to_string())?.clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::adversarial_corpus::{
        ALLOWED_STANDARD_CASES, BLOCKED_STANDARD_CASES, LEVEL_EXPECTATION_CASES,
    };
    use super::*;

    #[test]
    fn test_clean_text_passes() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("Hello, how are you today?");
        assert!(!result.blocked);
        assert!(result.score < 0.1);
    }

    #[test]
    fn test_explicit_content_blocked() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("She gave him a blowjob and then they had anal sex");
        assert!(result.blocked);
        assert!(!result.matched_terms.is_empty());
    }

    #[test]
    fn test_medical_context_reduces_score() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let without_context = filter.check_text("erection and arousal");
        let with_context =
            filter.check_text("In this medical textbook, erection and arousal are discussed");
        assert!(with_context.score < without_context.score);
    }

    #[test]
    fn test_streaming_accumulation() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let mut ctx = StreamFilterContext::new();

        let r1 = filter.check_delta(&mut ctx, "She gave him a ");
        assert!(!r1.blocked);

        let _r2 = filter.check_delta(&mut ctx, "blowjob and then ");

        let r3 = filter.check_delta(&mut ctx, "they had anal sex in an orgy");
        assert!(r3.blocked);
    }

    #[test]
    fn test_markdown_formatting_stripped() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter
            .check_text(r#""You want to *fuck* me? You'll have to *work* for it. Beg for it.""#);
        assert!(
            result.matched_terms.iter().any(|t| t == "fuck me"),
            "should match 'fuck me' through asterisks, got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_roleplay_response_blocked() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let text = r#"*She leans in close.* "You want to *fuck* me? You'll have to *work* for it." *She drags her gaze down your body.* "I don't take just *anyone* to my bed.""#;
        let result = filter.check_text(text);
        assert!(
            result.score > 0.0,
            "roleplay with explicit terms should score > 0, got {}",
            result.score
        );
    }

    #[test]
    fn test_off_level_passes_everything() {
        let filter = ContentFilter::new(PureModeLevel::Off);
        assert!(!filter.is_enabled());
        let result = filter.check_text("She gave him a blowjob and then they had anal sex");
        assert!(!result.blocked);
        assert_eq!(result.score, 0.0);
    }

    #[test]
    fn test_low_level_skips_low_weight_sexual() {
        let filter = ContentFilter::new(PureModeLevel::Low);
        let result = filter.check_text("cock");
        assert_eq!(result.score, 0.0);
    }

    #[test]
    fn test_low_level_catches_high_weight_sexual() {
        let filter = ContentFilter::new(PureModeLevel::Low);
        let result = filter.check_text("blowjob");
        assert!(result.score > 0.0);
    }

    #[test]
    fn test_low_level_skips_violence() {
        let filter = ContentFilter::new(PureModeLevel::Low);
        let result = filter.check_text("disembowel and decapitate");
        assert_eq!(result.score, 0.0);
    }

    #[test]
    fn test_strict_level_lower_threshold() {
        let filter = ContentFilter::new(PureModeLevel::Strict);
        let result = filter.check_text("She gave him a blowjob");
        assert!(result.blocked);
    }

    #[test]
    fn test_set_level() {
        let filter = ContentFilter::new(PureModeLevel::Off);
        assert!(!filter.is_enabled());
        filter.set_level(PureModeLevel::Standard);
        assert!(filter.is_enabled());
        assert_eq!(filter.level(), PureModeLevel::Standard);
        filter.set_level(PureModeLevel::Low);
        assert_eq!(filter.level(), PureModeLevel::Low);
    }

    #[test]
    fn test_level_from_str() {
        assert_eq!(PureModeLevel::from_str("off"), PureModeLevel::Off);
        assert_eq!(PureModeLevel::from_str("low"), PureModeLevel::Low);
        assert_eq!(PureModeLevel::from_str("standard"), PureModeLevel::Standard);
        assert_eq!(PureModeLevel::from_str("strict"), PureModeLevel::Strict);
        assert_eq!(PureModeLevel::from_str("unknown"), PureModeLevel::Standard);
    }

    // ── N-gram false-positive elimination tests ──────────────────────

    #[test]
    fn test_no_false_positive_cocktail() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("I ordered a cocktail at the bar");
        assert!(
            !result.matched_terms.iter().any(|t| t == "cock"),
            "cocktail should NOT match 'cock', got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_no_false_positive_dictionary() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("I looked it up in the dictionary");
        assert!(
            !result.matched_terms.iter().any(|t| t == "dick"),
            "dictionary should NOT match 'dick', got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_no_false_positive_cumulative() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("The cumulative effect was significant");
        assert!(
            !result.matched_terms.iter().any(|t| t == "cum"),
            "cumulative should NOT match 'cum', got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_morphological_suffix_still_matches() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        // "ejaculat" should still match "ejaculation" (suffix "ion" = 3 chars)
        let result = filter.check_text("premature ejaculation");
        assert!(
            result.matched_terms.iter().any(|t| t == "ejaculat"),
            "ejaculation should match 'ejaculat', got: {:?}",
            result.matched_terms
        );
    }

    // ── Leet-speak normalization tests ────────────────────────────────

    #[test]
    fn test_leet_speak_porn() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("looking at p0rn");
        assert!(
            result.matched_terms.iter().any(|t| t == "porn"),
            "p0rn should match 'porn' via normalization, got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_leet_speak_slur() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("he called him a n!gg3r");
        assert!(
            result.matched_terms.iter().any(|t| t == "nigger"),
            "n!gg3r should match 'nigger' via normalization, got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_leet_speak_dollar_s() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("she's a $lut");
        assert!(
            result.matched_terms.iter().any(|t| t == "slut"),
            "$lut should match 'slut' via normalization, got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_leet_speak_hentai() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("watching h3nt4i");
        assert!(
            result.matched_terms.iter().any(|t| t == "hentai"),
            "h3nt4i should match 'hentai' via normalization, got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_leet_speak_does_not_mangle_punctuation() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        // "hello!" should NOT become "helloi" — the ! is punctuation, not leet
        let result = filter.check_text("hello! how are you?");
        assert!(!result.blocked);
        assert_eq!(result.score, 0.0);
    }

    // ── N-gram multi-word matching tests ──────────────────────────────

    #[test]
    fn test_no_false_positive_scum_on_her() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("the scum on her shoes");
        assert!(
            !result.matched_terms.iter().any(|t| t == "cum on her"),
            "'scum on her' should NOT match 'cum on her', got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_cum_on_her_still_matches() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("he wanted to cum on her face");
        assert!(
            result.matched_terms.iter().any(|t| t == "cum on her"),
            "actual 'cum on her' should match, got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_no_false_positive_analysis_sex_ed() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("the analysis of sex education programs");
        assert!(
            !result.matched_terms.iter().any(|t| t == "anal sex"),
            "'analysis of sex' should NOT match 'anal sex', got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_no_false_positive_cum_laude() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("she graduated magna cum laude on her resume");
        assert!(
            !result.matched_terms.iter().any(|t| t == "cum on her"),
            "'cum laude on her' should NOT match 'cum on her', got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_multiword_hyphenated_term_matches() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("it was clearly non-con");
        assert!(
            result.matched_terms.iter().any(|t| t == "non-con"),
            "'non-con' should match, got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_no_false_positive_hard_on() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("he banged his head hard on to the floor");
        assert!(
            !result.matched_terms.iter().any(|t| t == "hard-on"),
            "'hard on to' should NOT match 'hard-on', got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_hard_on_with_hyphen_still_matches() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("he had a hard-on");
        assert!(
            result.matched_terms.iter().any(|t| t == "hard-on"),
            "'hard-on' with hyphen should match, got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_multiword_slash_term_matches() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("the dom/sub dynamic was clear");
        assert!(
            result.matched_terms.iter().any(|t| t == "dom/sub"),
            "'dom/sub' should match, got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_ngram_morphological_tolerance() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        // "cum drip" should match "cum dripping" (suffix "ping" = 4 chars,
        // not a recognized suffix, but "drip" + "ping"... actually suffix is 4,
        // wait: "dripping" starts with "drip", suffix is "ping" = 4 chars.
        // Let's use a cleaner example: "anal play" matching "anal playing"
        // "playing" starts with "play", suffix "ing" = 3 chars → matches
        let result = filter.check_text("they were into anal playing");
        assert!(
            result.matched_terms.iter().any(|t| t == "anal play"),
            "'anal playing' should match 'anal play', got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_ngram_matches_term_basic() {
        let text_words = vec!["the", "scum", "on", "her", "shoes"];
        let term_words = vec!["cum", "on", "her"];
        assert!(!ContentFilter::ngram_matches_term(&text_words, &term_words));
    }

    #[test]
    fn test_ngram_matches_term_positive() {
        let text_words = vec!["he", "cum", "on", "her", "face"];
        let term_words = vec!["cum", "on", "her"];
        assert!(ContentFilter::ngram_matches_term(&text_words, &term_words));
    }

    #[test]
    fn test_ngram_matches_term_too_short() {
        let text_words = vec!["cum", "on"];
        let term_words = vec!["cum", "on", "her"];
        assert!(!ContentFilter::ngram_matches_term(&text_words, &term_words));
    }

    // ── Unicode normalization tests ───────────────────────────────────

    #[test]
    fn test_normalize_unicode_invisible_chars() {
        let input = "h\u{200B}e\u{200C}l\u{200D}l\u{FEFF}o";
        assert_eq!(ContentFilter::normalize_unicode(input), "hello");
    }

    #[test]
    fn test_normalize_unicode_cyrillic() {
        let input = "\u{0430}\u{0435}\u{043E}\u{0441}";
        assert_eq!(ContentFilter::normalize_unicode(input), "aeoc");
    }

    #[test]
    fn test_normalize_unicode_diacritics() {
        let input = "\u{00E9}\u{00F1}\u{00FC}\u{00E0}";
        assert_eq!(ContentFilter::normalize_unicode(input), "enua");
    }

    #[test]
    fn test_normalize_unicode_eszett() {
        let input = "stra\u{00DF}e";
        assert_eq!(ContentFilter::normalize_unicode(input), "strasse");
    }

    #[test]
    fn test_zero_width_chars_caught() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("f\u{200B}u\u{200B}c\u{200B}k me");
        assert!(
            result.matched_terms.iter().any(|t| t == "fuck me"),
            "zero-width chars should be stripped, got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_cyrillic_homoglyphs_caught() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        // "fu\u{0441}k" = "fuсk" with Cyrillic с
        let result = filter.check_text("fu\u{0441}k me");
        assert!(
            result.matched_terms.iter().any(|t| t == "fuck me"),
            "Cyrillic homoglyphs should be normalized, got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_diacritical_evasion_caught() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("she's a sl\u{00FC}t");
        assert!(
            result.matched_terms.iter().any(|t| t == "slut"),
            "diacritical evasion should be caught, got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_soft_hyphen_caught() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("a blow\u{00AD}job");
        assert!(
            result.matched_terms.iter().any(|t| t == "blowjob"),
            "soft hyphens should be stripped, got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_clean_unicode_not_affected() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text(
            "The caf\u{00E9} was lovely and the cr\u{00E8}me br\u{00FB}l\u{00E9}e was excellent",
        );
        assert!(!result.blocked);
    }

    // ── Repeated character collapse tests ─────────────────────────────

    #[test]
    fn test_collapse_preserves_double_letters() {
        assert_eq!(
            ContentFilter::collapse_repeated_chars("all good balls"),
            "all good balls"
        );
    }

    #[test]
    fn test_collapse_triple_plus() {
        assert_eq!(
            ContentFilter::collapse_repeated_chars("fuuuck yooou"),
            "fuck you"
        );
    }

    #[test]
    fn test_collapse_various_runs() {
        assert_eq!(ContentFilter::collapse_repeated_chars("aaa"), "a");
        assert_eq!(ContentFilter::collapse_repeated_chars("aa"), "aa");
        assert_eq!(ContentFilter::collapse_repeated_chars("aaaa"), "a");
        assert_eq!(ContentFilter::collapse_repeated_chars("aabbb"), "aab");
        assert_eq!(ContentFilter::collapse_repeated_chars("abc"), "abc");
    }

    #[test]
    fn test_repeated_chars_fuuuck_me() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("fuuuck me");
        assert!(
            result.matched_terms.iter().any(|t| t == "fuck me"),
            "'fuuuck me' should match 'fuck me' via collapse, got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_repeated_chars_sluuut() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("she's a sluuut");
        assert!(
            result.matched_terms.iter().any(|t| t == "slut"),
            "'sluuut' should match 'slut' via collapse, got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_repeated_chars_multiword_evasion() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let result = filter.check_text("cuuum on her face");
        assert!(
            result.matched_terms.iter().any(|t| t == "cum on her"),
            "'cuuum on her' should match via collapse + ngram, got: {:?}",
            result.matched_terms
        );
    }

    #[test]
    fn test_adversarial_corpus_blocked_standard() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        for case in BLOCKED_STANDARD_CASES {
            let result = filter.check_text(case.input);
            assert!(
                result.blocked,
                "case '{}' should be blocked at standard, score={}, matched={:?}, input={:?}",
                case.name, result.score, result.matched_terms, case.input
            );
        }
    }

    #[test]
    fn test_adversarial_corpus_allowed_standard() {
        let filter = ContentFilter::new(PureModeLevel::Standard);
        for case in ALLOWED_STANDARD_CASES {
            let result = filter.check_text(case.input);
            assert!(
                !result.blocked,
                "case '{}' should pass at standard, score={}, matched={:?}, input={:?}",
                case.name, result.score, result.matched_terms, case.input
            );
        }
    }

    #[test]
    fn test_adversarial_level_expectations() {
        for case in LEVEL_EXPECTATION_CASES {
            let off = ContentFilter::new(PureModeLevel::Off).check_text(case.input);
            let low = ContentFilter::new(PureModeLevel::Low).check_text(case.input);
            let standard = ContentFilter::new(PureModeLevel::Standard).check_text(case.input);
            let strict = ContentFilter::new(PureModeLevel::Strict).check_text(case.input);

            assert_eq!(
                off.blocked, case.off_blocked,
                "case '{}' off mismatch: score={}, matched={:?}",
                case.name, off.score, off.matched_terms
            );
            assert_eq!(
                low.blocked, case.low_blocked,
                "case '{}' low mismatch: score={}, matched={:?}",
                case.name, low.score, low.matched_terms
            );
            assert_eq!(
                standard.blocked, case.standard_blocked,
                "case '{}' standard mismatch: score={}, matched={:?}",
                case.name, standard.score, standard.matched_terms
            );
            assert_eq!(
                strict.blocked, case.strict_blocked,
                "case '{}' strict mismatch: score={}, matched={:?}",
                case.name, strict.score, strict.matched_terms
            );
        }
    }
}
