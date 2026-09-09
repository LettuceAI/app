use std::collections::HashMap;

use lettuce_platform::{EspeakNgError, EspeakPhonemizer};

const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_TOKENS: usize = 65_536;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroPhonemizationInput {
    pub voice_id: String,
    pub text: String,
    pub lexicon: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroPhonemization {
    pub normalized_text: String,
    pub effective_text: String,
    pub language: String,
    pub used_lexicon_entries: Vec<String>,
    pub segments: Vec<KokoroPhonemizationSegment>,
    pub token_ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroPhonemizationSegment {
    pub kind: String,
    pub source_text: String,
    pub ipa: String,
    pub token_ids: Vec<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KokoroPhonemizationError {
    #[error("Kokoro phonemization input is invalid")]
    InvalidInput,
    #[error("Kokoro phonemizer failed: {0}")]
    Process(EspeakNgError),
    #[error("Kokoro phonemization exceeds its token limit")]
    LimitExceeded,
}

#[must_use]
pub fn kokoro_voice_language(voice: &str) -> &'static str {
    match voice.get(..2).unwrap_or_default() {
        "af" | "am" => "en-US",
        "bf" | "bm" => "en-GB",
        "ef" | "em" => "es",
        "ff" => "fr",
        "hf" | "hm" => "hi",
        "if" | "im" => "it",
        "jf" | "jm" => "ja",
        "pf" | "pm" => "pt-BR",
        "zf" | "zm" => "cmn",
        _ => "en-US",
    }
}

pub fn phonemize_kokoro<P: EspeakPhonemizer + ?Sized>(
    process: &P,
    input: &KokoroPhonemizationInput,
) -> Result<KokoroPhonemization, KokoroPhonemizationError> {
    if input.text.len() > MAX_TEXT_BYTES
        || input.text.contains('\0')
        || !lettuce_model_hub::is_valid_kokoro_voice_id(&input.voice_id)
        || input.lexicon.len() > 4096
        || input.lexicon.iter().any(|(key, value)| {
            key.trim().is_empty()
                || value.trim().is_empty()
                || key.len() > 256
                || value.len() > 4096
        })
    {
        return Err(KokoroPhonemizationError::InvalidInput);
    }
    let language = kokoro_voice_language(&input.voice_id);
    let vocab = kokoro_vocab();
    let normalized_text = normalize_input_text(&input.text);
    let (effective_text, used_lexicon_entries) =
        apply_lexicon_annotations(&normalized_text, &input.lexicon);
    let parts = split_text_parts(&effective_text);
    let text_segments = parts
        .iter()
        .filter_map(|part| match part {
            TextPart::Text(segment) | TextPart::StressText { text: segment, .. } => {
                Some(segment.as_str())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let segment_ipas = phonemize_segments_batch(&text_segments, language, process)?;
    let mut token_ids = Vec::new();
    let mut segments = Vec::new();
    let mut segment_index = 0;
    for part in parts {
        match part {
            TextPart::Text(text) => {
                if let Some(ipa) = segment_ipas.get(segment_index) {
                    let ids = ipa_to_ids(ipa, &vocab);
                    token_ids.extend(ids.iter().copied());
                    segments.push(KokoroPhonemizationSegment {
                        kind: "text".to_owned(),
                        source_text: text,
                        ipa: ipa.clone(),
                        token_ids: ids,
                    });
                }
                segment_index += 1;
            }
            TextPart::StressText { text, delta } => {
                if let Some(ipa) = segment_ipas.get(segment_index) {
                    let ipa = apply_stress_delta(ipa, delta);
                    let ids = ipa_to_ids(&ipa, &vocab);
                    token_ids.extend(ids.iter().copied());
                    segments.push(KokoroPhonemizationSegment {
                        kind: format!("stress:{delta:+}"),
                        source_text: text,
                        ipa,
                        token_ids: ids,
                    });
                }
                segment_index += 1;
            }
            TextPart::Phonemes(ipa) => {
                let ids = ipa_to_ids(&ipa, &vocab);
                token_ids.extend(ids.iter().copied());
                segments.push(KokoroPhonemizationSegment {
                    kind: "phonemes".to_owned(),
                    source_text: ipa.clone(),
                    ipa,
                    token_ids: ids,
                });
            }
            TextPart::Punct(character) => {
                if let Some(&id) = vocab.get(&character) {
                    token_ids.push(id);
                    segments.push(KokoroPhonemizationSegment {
                        kind: "punct".to_owned(),
                        source_text: character.to_string(),
                        ipa: character.to_string(),
                        token_ids: vec![id],
                    });
                }
            }
            TextPart::Space => {
                if let Some(id) = push_space_id(&mut token_ids, &vocab) {
                    segments.push(KokoroPhonemizationSegment {
                        kind: "space".to_owned(),
                        source_text: " ".to_owned(),
                        ipa: " ".to_owned(),
                        token_ids: vec![id],
                    });
                }
            }
        }
    }
    if token_ids.len() > MAX_TOKENS {
        return Err(KokoroPhonemizationError::LimitExceeded);
    }
    Ok(KokoroPhonemization {
        normalized_text,
        effective_text,
        language: language.to_owned(),
        used_lexicon_entries,
        segments,
        token_ids,
    })
}

fn phonemize_segments_batch<P: EspeakPhonemizer + ?Sized>(
    segments: &[&str],
    language: &str,
    process: &P,
) -> Result<Vec<String>, KokoroPhonemizationError> {
    if segments.is_empty() {
        return Ok(Vec::new());
    }
    let batched = segments
        .iter()
        .map(|segment| format!("{segment}."))
        .collect::<Vec<_>>()
        .join("\n");
    let output = process
        .phonemize(&batched, language)
        .map_err(KokoroPhonemizationError::Process)?;
    let lines = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.len() == segments.len() {
        return Ok(lines.into_iter().map(normalize_ipa_output).collect());
    }
    segments
        .iter()
        .map(|segment| {
            process
                .phonemize(segment, language)
                .map(|value| normalize_ipa_output(&value))
                .map_err(KokoroPhonemizationError::Process)
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextPart {
    Text(String),
    StressText { text: String, delta: i8 },
    Phonemes(String),
    Punct(char),
    Space,
}

pub fn split_text_parts(text: &str) -> Vec<TextPart> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut idx = 0usize;

    while idx < text.len() {
        if let Some((next_idx, annotation)) = try_parse_inline_annotation(text, idx) {
            flush_text_part(&mut parts, &mut current);
            parts.push(annotation);
            idx = next_idx;
            continue;
        }

        let ch = text[idx..]
            .chars()
            .next()
            .expect("valid utf-8 slice should produce a char");
        let ch_len = ch.len_utf8();

        if matches!(ch, '\n' | '\r') {
            flush_text_part(&mut parts, &mut current);
            push_pause_punctuation(&mut parts);
            idx += ch_len;
            continue;
        }

        if let Some(punct) = map_boundary_punctuation(ch) {
            if !is_numeric_connector_between_digits(text, idx, ch_len, ch) {
                flush_text_part(&mut parts, &mut current);
                parts.push(TextPart::Punct(punct));
                idx += ch_len;
                continue;
            }
        }

        if ch.is_whitespace() {
            flush_text_part(&mut parts, &mut current);
            push_space_part(&mut parts);
            idx += ch_len;
            continue;
        }

        current.push(ch);
        idx += ch_len;
    }

    flush_text_part(&mut parts, &mut current);
    parts
}

pub fn normalize_input_text(text: &str) -> String {
    let cleaned = strip_inline_markdown(text);
    let mut normalized = String::with_capacity(cleaned.len());
    let mut prev_space = false;
    for ch in cleaned.chars() {
        let mapped = match ch {
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201C}' | '\u{201D}' => '"',
            '\u{2013}' | '\u{2014}' => '—',
            '\u{2026}' => '…',
            '\t' => ' ',
            _ => ch,
        };

        if matches!(mapped, '\r') {
            continue;
        }

        if mapped.is_whitespace() && mapped != '\n' {
            if prev_space {
                continue;
            }
            normalized.push(' ');
            prev_space = true;
            continue;
        }

        prev_space = false;
        normalized.push(mapped);
    }
    normalized.trim().to_string()
}

fn strip_inline_markdown(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut output = String::with_capacity(text.len());
    let mut idx = 0usize;

    while idx < chars.len() {
        let ch = chars[idx];

        if matches!(ch, '*' | '_') {
            let run_len = marker_run_len(&chars, idx, ch);
            if let Some(closing_idx) = find_matching_marker_run(&chars, idx + run_len, ch, run_len)
            {
                let inner_has_content = chars[idx + run_len..closing_idx]
                    .iter()
                    .any(|inner| !inner.is_whitespace());
                let left_ok = idx == 0 || chars[idx.saturating_sub(1)].is_whitespace();
                let right_ok = idx + run_len < chars.len() && !chars[idx + run_len].is_whitespace();
                let closing_left_ok = closing_idx > 0 && !chars[closing_idx - 1].is_whitespace();
                let closing_right_ok = closing_idx + run_len == chars.len()
                    || chars[closing_idx + run_len].is_whitespace()
                    || is_trailing_punctuation(chars[closing_idx + run_len]);

                if inner_has_content && left_ok && right_ok && closing_left_ok && closing_right_ok {
                    for inner in &chars[idx + run_len..closing_idx] {
                        output.push(*inner);
                    }
                    idx = closing_idx + run_len;
                    continue;
                }
            }
        }

        if ch == '`' {
            let run_len = marker_run_len(&chars, idx, ch);
            if let Some(closing_idx) = find_matching_marker_run(&chars, idx + run_len, ch, run_len)
            {
                for inner in &chars[idx + run_len..closing_idx] {
                    output.push(*inner);
                }
                idx = closing_idx + run_len;
                continue;
            }
        }

        output.push(ch);
        idx += 1;
    }

    output
}

fn marker_run_len(chars: &[char], start: usize, marker: char) -> usize {
    let mut len = 0usize;
    while start + len < chars.len() && chars[start + len] == marker {
        len += 1;
    }
    len
}

fn find_matching_marker_run(
    chars: &[char],
    start: usize,
    marker: char,
    run_len: usize,
) -> Option<usize> {
    let mut idx = start;
    while idx + run_len <= chars.len() {
        if chars[idx] == marker && marker_run_len(chars, idx, marker) >= run_len {
            return Some(idx);
        }
        idx += 1;
    }
    None
}

fn is_trailing_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '.' | ',' | '!' | '?' | ';' | ':' | ')' | ']' | '"' | '\''
    )
}

fn apply_lexicon_annotations(
    text: &str,
    lexicon: &HashMap<String, String>,
) -> (String, Vec<String>) {
    if lexicon.is_empty() {
        return (text.to_string(), Vec::new());
    }

    let mut entries = lexicon
        .iter()
        .map(|(key, value)| LexiconEntry {
            label: key.clone(),
            label_lower: key.to_lowercase(),
            ipa: value.clone(),
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.label_lower.len()));

    let mut output = String::with_capacity(text.len());
    let mut used_entries = Vec::new();
    let mut idx = 0usize;
    while idx < text.len() {
        if let Some((next_idx, _)) = try_parse_inline_annotation(text, idx) {
            output.push_str(&text[idx..next_idx]);
            idx = next_idx;
            continue;
        }

        if let Some((next_idx, matched, original)) = match_lexicon_entry(text, idx, &entries) {
            output.push('[');
            output.push_str(original);
            output.push_str("](/");
            output.push_str(&matched.ipa);
            output.push_str("/)");
            used_entries.push(matched.label.clone());
            idx = next_idx;
            continue;
        }

        let ch = text[idx..]
            .chars()
            .next()
            .expect("valid utf-8 slice should produce a char");
        output.push(ch);
        idx += ch.len_utf8();
    }

    used_entries.sort();
    used_entries.dedup();
    (output, used_entries)
}

#[derive(Debug, Clone)]
struct LexiconEntry {
    label: String,
    label_lower: String,
    ipa: String,
}

fn match_lexicon_entry<'a>(
    text: &'a str,
    start: usize,
    entries: &'a [LexiconEntry],
) -> Option<(usize, &'a LexiconEntry, &'a str)> {
    for entry in entries {
        let candidate = text.get(start..start + entry.label.len())?;
        if !candidate.eq_ignore_ascii_case(&entry.label)
            && candidate.to_lowercase() != entry.label_lower
        {
            continue;
        }
        if !has_word_boundary_before(text, start) {
            continue;
        }
        let end = start + candidate.len();
        if !has_word_boundary_after(text, end) {
            continue;
        }
        return Some((end, entry, candidate));
    }
    None
}

fn has_word_boundary_before(text: &str, index: usize) -> bool {
    match text[..index].chars().next_back() {
        None => true,
        Some(ch) => !is_lexical_char(ch),
    }
}

fn has_word_boundary_after(text: &str, index: usize) -> bool {
    match text[index..].chars().next() {
        None => true,
        Some(ch) => !is_lexical_char(ch),
    }
}

fn is_lexical_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '\'' | '’' | '_' | '-')
}

fn flush_text_part(parts: &mut Vec<TextPart>, current: &mut String) {
    let trimmed = current.trim();
    if trimmed.is_empty() {
        current.clear();
        return;
    }
    parts.push(TextPart::Text(trimmed.to_string()));
    current.clear();
}

fn map_boundary_punctuation(ch: char) -> Option<char> {
    match ch {
        '.' | '!' | '?' | ',' | ';' | ':' | '—' | '…' | '"' | '(' | ')' | '\u{201c}'
        | '\u{201d}' => Some(ch),
        _ => None,
    }
}

fn try_parse_inline_annotation(text: &str, start: usize) -> Option<(usize, TextPart)> {
    if text[start..].chars().next()? != '[' {
        return None;
    }

    let closing_bracket_rel = text[start..].find(']')?;
    let closing_bracket = start + closing_bracket_rel;
    let label = text.get(start + 1..closing_bracket)?.trim();
    if label.is_empty() {
        return None;
    }

    let after_bracket = text.get(closing_bracket + 1..)?;
    if !after_bracket.starts_with('(') {
        return None;
    }

    let target_start = closing_bracket + 2;
    let closing_paren_rel = text.get(target_start..)?.find(')')?;
    let target_end = target_start + closing_paren_rel;
    let target = text.get(target_start..target_end)?.trim();

    if let Some(ipa) = target
        .strip_prefix('/')
        .and_then(|value| value.strip_suffix('/'))
    {
        let ipa = ipa.trim();
        if ipa.is_empty() {
            return None;
        }
        return Some((target_end + 1, TextPart::Phonemes(ipa.to_string())));
    }

    if let Ok(delta) = target.parse::<i8>() {
        if matches!(delta, -2 | -1 | 1 | 2) {
            return Some((
                target_end + 1,
                TextPart::StressText {
                    text: label.to_string(),
                    delta,
                },
            ));
        }
    }

    None
}

fn is_numeric_connector_between_digits(text: &str, idx: usize, ch_len: usize, ch: char) -> bool {
    if !matches!(ch, '.' | ',') {
        return false;
    }

    let prev = text[..idx].chars().next_back();
    let next = text[idx + ch_len..].chars().next();

    matches!(
        (prev, next),
        (Some(left), Some(right)) if left.is_ascii_digit() && right.is_ascii_digit()
    )
}

fn ipa_to_ids(ipa: &str, vocab: &HashMap<char, i64>) -> Vec<i64> {
    let mut ids = Vec::new();
    for line in ipa.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        for ch in line.chars() {
            if ch == '_' {
                continue;
            }
            if let Some(&id) = vocab.get(&ch) {
                ids.push(id);
            }
        }
    }
    ids
}

fn normalize_ipa_output(ipa: &str) -> String {
    ipa.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn apply_stress_delta(ipa: &str, delta: i8) -> String {
    if delta == 0 {
        return ipa.to_string();
    }

    let mut chars = ipa.chars().collect::<Vec<_>>();
    let primary_index = chars.iter().position(|&ch| ch == 'ˈ');
    let secondary_index = chars.iter().position(|&ch| ch == 'ˌ');

    match delta {
        -2 => {
            if let Some(index) = primary_index.or(secondary_index) {
                chars.remove(index);
            }
        }
        -1 => {
            if let Some(index) = primary_index {
                chars[index] = 'ˌ';
            } else if let Some(index) = secondary_index {
                chars.remove(index);
            }
        }
        1 => {
            if let Some(index) = secondary_index {
                chars[index] = 'ˈ';
            } else if primary_index.is_none() {
                insert_stress_marker(&mut chars, 'ˌ');
            }
        }
        2 => {
            if let Some(index) = secondary_index {
                chars[index] = 'ˈ';
            } else if primary_index.is_none() {
                insert_stress_marker(&mut chars, 'ˈ');
            }
        }
        _ => {}
    }

    chars.into_iter().collect()
}

fn insert_stress_marker(chars: &mut Vec<char>, marker: char) {
    let insert_at = chars.iter().position(|ch| is_vowel_like(*ch)).unwrap_or(0);
    chars.insert(insert_at, marker);
}

fn is_vowel_like(ch: char) -> bool {
    matches!(
        ch,
        'A' | 'I'
            | 'O'
            | 'Q'
            | 'W'
            | 'Y'
            | 'a'
            | 'e'
            | 'i'
            | 'o'
            | 'u'
            | 'y'
            | 'ɑ'
            | 'ɐ'
            | 'ɒ'
            | 'æ'
            | 'ɔ'
            | 'ə'
            | 'ɚ'
            | 'ɛ'
            | 'ɜ'
            | 'ɨ'
            | 'ɪ'
            | 'ɯ'
            | 'ø'
            | 'œ'
            | 'ʊ'
            | 'ʌ'
            | 'ɤ'
            | 'ᵊ'
            | 'ᵻ'
    )
}

fn push_space_part(parts: &mut Vec<TextPart>) {
    if matches!(
        parts.last(),
        None | Some(TextPart::Space) | Some(TextPart::Punct(_))
    ) {
        return;
    }
    parts.push(TextPart::Space);
}

fn push_pause_punctuation(parts: &mut Vec<TextPart>) {
    if matches!(parts.last(), Some(TextPart::Punct('.'))) {
        return;
    }
    parts.push(TextPart::Punct('.'));
}

fn push_space_id(ids: &mut Vec<i64>, vocab: &HashMap<char, i64>) -> Option<i64> {
    let &space_id = vocab.get(&' ')?;
    if ids.last().copied() == Some(space_id) {
        return None;
    }
    if !ids.is_empty() {
        ids.push(space_id);
        return Some(space_id);
    }
    None
}

fn kokoro_vocab() -> HashMap<char, i64> {
    let entries: &[(char, i64)] = &[
        (';', 1),
        (':', 2),
        (',', 3),
        ('.', 4),
        ('!', 5),
        ('?', 6),
        ('—', 9),
        ('…', 10),
        ('"', 11),
        ('(', 12),
        (')', 13),
        ('\u{201c}', 14),
        ('\u{201d}', 15),
        (' ', 16),
        ('\u{0303}', 17),
        ('ʣ', 18),
        ('ʥ', 19),
        ('ʦ', 20),
        ('ʨ', 21),
        ('ᵝ', 22),
        ('ꭧ', 23),
        ('A', 24),
        ('I', 25),
        ('O', 31),
        ('Q', 33),
        ('S', 35),
        ('T', 36),
        ('W', 39),
        ('Y', 41),
        ('ᵊ', 42),
        ('a', 43),
        ('b', 44),
        ('c', 45),
        ('d', 46),
        ('e', 47),
        ('f', 48),
        ('h', 50),
        ('i', 51),
        ('j', 52),
        ('k', 53),
        ('l', 54),
        ('m', 55),
        ('n', 56),
        ('o', 57),
        ('p', 58),
        ('q', 59),
        ('r', 60),
        ('s', 61),
        ('t', 62),
        ('u', 63),
        ('v', 64),
        ('w', 65),
        ('x', 66),
        ('y', 67),
        ('z', 68),
        ('ɑ', 69),
        ('ɐ', 70),
        ('ɒ', 71),
        ('æ', 72),
        ('β', 75),
        ('ɔ', 76),
        ('ɕ', 77),
        ('ç', 78),
        ('ɖ', 80),
        ('ð', 81),
        ('ʤ', 82),
        ('ə', 83),
        ('ɚ', 85),
        ('ɛ', 86),
        ('ɜ', 87),
        ('ɟ', 90),
        ('ɡ', 92),
        ('ɥ', 99),
        ('ɨ', 101),
        ('ɪ', 102),
        ('ʝ', 103),
        ('ɯ', 110),
        ('ɰ', 111),
        ('ŋ', 112),
        ('ɳ', 113),
        ('ɲ', 114),
        ('ɴ', 115),
        ('ø', 116),
        ('ɸ', 118),
        ('θ', 119),
        ('œ', 120),
        ('ɹ', 123),
        ('ɾ', 125),
        ('ɻ', 126),
        ('ʁ', 128),
        ('ɽ', 129),
        ('ʂ', 130),
        ('ʃ', 131),
        ('ʈ', 132),
        ('ʧ', 133),
        ('ʊ', 135),
        ('ʋ', 136),
        ('ʌ', 138),
        ('ɣ', 139),
        ('ɤ', 140),
        ('χ', 142),
        ('ʎ', 143),
        ('ʒ', 147),
        ('ʔ', 148),
        ('ˈ', 156),
        ('ˌ', 157),
        ('ː', 158),
        ('ʰ', 162),
        ('ʲ', 164),
        ('↓', 169),
        ('→', 171),
        ('↗', 172),
        ('↘', 173),
        ('ᵻ', 177),
    ];
    entries.iter().copied().collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Debug)]
    struct FixturePhonemizer {
        calls: Mutex<Vec<(String, String)>>,
    }

    impl EspeakPhonemizer for FixturePhonemizer {
        fn phonemize(&self, input: &str, language: &str) -> Result<String, EspeakNgError> {
            self.calls
                .lock()
                .expect("calls")
                .push((input.to_owned(), language.to_owned()));
            Ok("həlˈoʊ.".to_owned())
        }
    }

    #[test]
    fn preserves_legacy_language_normalization_annotations_and_vocab() {
        let process = FixturePhonemizer {
            calls: Mutex::new(Vec::new()),
        };
        let result = phonemize_kokoro(
            &process,
            &KokoroPhonemizationInput {
                voice_id: "bm_george".to_owned(),
                text: "**Hello**   Lettuce!".to_owned(),
                lexicon: HashMap::from([("Lettuce".to_owned(), "wɜːld".to_owned())]),
            },
        )
        .expect("phonemization");
        assert_eq!(result.language, "en-GB");
        assert_eq!(result.normalized_text, "Hello Lettuce!");
        assert_eq!(result.effective_text, "Hello [Lettuce](/wɜːld/)!");
        assert_eq!(result.used_lexicon_entries, ["Lettuce"]);
        assert_eq!(result.segments.len(), 4);
        assert_eq!(result.segments[1].kind, "space");
        assert_eq!(result.segments[2].kind, "phonemes");
        assert_eq!(result.segments[2].token_ids, [65, 87, 158, 54, 46]);
        assert_eq!(result.token_ids.last(), Some(&5));
        assert_eq!(
            process.calls.lock().expect("calls").as_slice(),
            &[("Hello.".to_owned(), "en-GB".to_owned())]
        );
        assert_eq!(kokoro_voice_language("zf_xiaobei"), "cmn");
        assert_eq!(kokoro_voice_language("unknown"), "en-US");
    }
}
