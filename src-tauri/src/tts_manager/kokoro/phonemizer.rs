use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde::Serialize;

use super::lexicon::{load_lexicon_overrides, resolve_lexicon_path};
use super::model::KokoroError;

#[derive(Debug, Clone, Default)]
pub struct EspeakConfig {
    pub bin_path: Option<PathBuf>,
    pub data_path: Option<PathBuf>,
}

pub fn voice_lang(voice: &str) -> &'static str {
    let prefix = &voice[..voice.len().min(2)];
    match prefix {
        "af" | "am" => "en-us",
        "bf" | "bm" => "en-gb",
        "ef" | "em" => "es",
        "ff" => "fr",
        "hf" | "hm" => "hi",
        "if" | "im" => "it",
        "jf" | "jm" => "ja",
        "pf" | "pm" => "pt-br",
        "zf" | "zm" => "cmn",
        _ => "en-us",
    }
}

pub fn phonemize(
    text: &str,
    lang: &str,
    vocab: &HashMap<char, i64>,
    espeak: &EspeakConfig,
    asset_root: &Path,
) -> Result<Vec<i64>, KokoroError> {
    let trace = build_trace(text, lang, vocab, espeak, asset_root)?;
    Ok(trace.token_ids)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhonemizeTrace {
    pub normalized_text: String,
    pub effective_text: String,
    pub language: String,
    pub lexicon_path: String,
    pub lexicon_entry_count: usize,
    pub used_lexicon_entries: Vec<String>,
    pub segments: Vec<PhonemizeTraceSegment>,
    pub token_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhonemizeTraceSegment {
    pub kind: String,
    pub source_text: String,
    pub ipa: String,
    pub token_ids: Vec<i64>,
}

pub fn build_trace(
    text: &str,
    lang: &str,
    vocab: &HashMap<char, i64>,
    espeak: &EspeakConfig,
    asset_root: &Path,
) -> Result<PhonemizeTrace, KokoroError> {
    let normalized_text = normalize_input_text(text);
    let lexicon = load_lexicon_overrides(asset_root, lang)?;
    let (effective_text, used_lexicon_entries) =
        apply_lexicon_annotations(&normalized_text, &lexicon.entries);
    let parts = split_text_parts(&effective_text);
    if parts.is_empty() {
        return Ok(PhonemizeTrace {
            normalized_text,
            effective_text,
            language: lang.to_string(),
            lexicon_path: lexicon.path.to_string_lossy().to_string(),
            lexicon_entry_count: lexicon.entries.len(),
            used_lexicon_entries,
            segments: Vec::new(),
            token_ids: Vec::new(),
        });
    }

    let text_segments: Vec<&str> = parts
        .iter()
        .filter_map(|part| match part {
            TextPart::Text(segment) => Some(segment.as_str()),
            TextPart::StressText { text, .. } => Some(text.as_str()),
            TextPart::Phonemes(_) | TextPart::Punct(_) | TextPart::Space => None,
        })
        .collect();

    let segment_ipas = if text_segments.is_empty() {
        Vec::new()
    } else {
        phonemize_segments_batch(&text_segments, lang, vocab, espeak)?
    };

    let mut ids = Vec::new();
    let mut trace_segments = Vec::new();
    let mut segment_index = 0usize;
    for part in parts {
        match part {
            TextPart::Text(source) => {
                if let Some(chunk) = segment_ipas.get(segment_index) {
                    let token_ids = ipa_to_ids(chunk, vocab);
                    ids.extend(token_ids.iter().copied());
                    trace_segments.push(PhonemizeTraceSegment {
                        kind: "text".to_string(),
                        source_text: source,
                        ipa: chunk.clone(),
                        token_ids,
                    });
                }
                segment_index += 1;
            }
            TextPart::StressText { text, delta } => {
                if let Some(chunk) = segment_ipas.get(segment_index) {
                    let adjusted = apply_stress_delta(chunk, delta);
                    let token_ids = ipa_to_ids(&adjusted, vocab);
                    ids.extend(token_ids.iter().copied());
                    trace_segments.push(PhonemizeTraceSegment {
                        kind: format!("stress:{delta:+}"),
                        source_text: text,
                        ipa: adjusted,
                        token_ids,
                    });
                }
                segment_index += 1;
            }
            TextPart::Phonemes(ipa) => {
                let token_ids = ipa_to_ids(&ipa, vocab);
                ids.extend(token_ids.iter().copied());
                trace_segments.push(PhonemizeTraceSegment {
                    kind: "phonemes".to_string(),
                    source_text: ipa.clone(),
                    ipa,
                    token_ids,
                });
            }
            TextPart::Punct(ch) => {
                if let Some(&id) = vocab.get(&ch) {
                    ids.push(id);
                    trace_segments.push(PhonemizeTraceSegment {
                        kind: "punct".to_string(),
                        source_text: ch.to_string(),
                        ipa: ch.to_string(),
                        token_ids: vec![id],
                    });
                }
            }
            TextPart::Space => {
                if let Some(space_id) = push_space_id(&mut ids, vocab) {
                    trace_segments.push(PhonemizeTraceSegment {
                        kind: "space".to_string(),
                        source_text: " ".to_string(),
                        ipa: " ".to_string(),
                        token_ids: vec![space_id],
                    });
                }
            }
        }
    }

    Ok(PhonemizeTrace {
        normalized_text,
        effective_text,
        language: lang.to_string(),
        lexicon_path: resolve_lexicon_path(asset_root)
            .to_string_lossy()
            .to_string(),
        lexicon_entry_count: lexicon.entries.len(),
        used_lexicon_entries,
        segments: trace_segments,
        token_ids: ids,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TextPart {
    Text(String),
    StressText { text: String, delta: i8 },
    Phonemes(String),
    Punct(char),
    Space,
}

fn split_text_parts(text: &str) -> Vec<TextPart> {
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

fn normalize_input_text(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut prev_space = false;
    for ch in text.chars() {
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
    entries.sort_by(|left, right| right.label_lower.len().cmp(&left.label_lower.len()));

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

fn phonemize_segments_batch(
    segments: &[&str],
    lang: &str,
    vocab: &HashMap<char, i64>,
    espeak: &EspeakConfig,
) -> Result<Vec<String>, KokoroError> {
    let batched_input = segments.join("\n");
    let output = run_espeak(&batched_input, lang, espeak)?;
    let lines: Vec<&str> = output.lines().collect();

    if lines.len() != segments.len() {
        return segments
            .iter()
            .map(|segment| {
                let output = run_espeak(segment, lang, espeak)?;
                Ok(normalize_ipa_output(&output))
            })
            .collect();
    }

    let _ = vocab;
    Ok(lines
        .iter()
        .map(|line| normalize_ipa_output(line))
        .collect())
}

fn run_espeak(input: &str, lang: &str, espeak: &EspeakConfig) -> Result<String, KokoroError> {
    let bin = espeak
        .bin_path
        .as_deref()
        .map(|p| p.as_os_str().to_owned())
        .unwrap_or_else(|| std::ffi::OsString::from("espeak-ng"));
    let mut cmd = Command::new(&bin);
    cmd.args(["--ipa", "--stdin", "-q", "-v", lang]);
    if let Some(data_path) = espeak.data_path.as_deref() {
        cmd.arg("--path").arg(data_path);
    }

    #[cfg(target_os = "linux")]
    if let Some(bin_dir) = espeak.bin_path.as_deref().and_then(|p| p.parent()) {
        cmd.env("LD_LIBRARY_PATH", bin_dir);
    }

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                KokoroError::EspeakNotFound
            } else {
                KokoroError::Io(e)
            }
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        let stdin_payload = canonicalize_espeak_stdin_payload(input);
        stdin
            .write_all(stdin_payload.as_bytes())
            .map_err(KokoroError::Io)?;
    }

    let output = child.wait_with_output().map_err(KokoroError::Io)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(KokoroError::PhonemizerFailed(format!(
            "espeak-ng exited with code {:?}: {stderr}",
            output.status.code()
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn canonicalize_espeak_stdin_payload(input: &str) -> Cow<'_, str> {
    if input.ends_with('\n') {
        Cow::Borrowed(input)
    } else {
        Cow::Owned(format!("{input}\n"))
    }
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

fn apply_stress_delta(ipa: &str, delta: i8) -> String {
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
    let Some(&space_id) = vocab.get(&' ') else {
        return None;
    };
    if ids.last().copied() == Some(space_id) {
        return None;
    }
    if !ids.is_empty() {
        ids.push(space_id);
        return Some(space_id);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{apply_stress_delta, split_text_parts, TextPart};

    #[test]
    fn parses_inline_phoneme_annotations() {
        let parts = split_text_parts("[Kokoro](/kˈOkəɹO/) is here.");
        assert_eq!(
            parts,
            vec![
                TextPart::Phonemes("kˈOkəɹO".to_string()),
                TextPart::Space,
                TextPart::Text("is".to_string()),
                TextPart::Space,
                TextPart::Text("here".to_string()),
                TextPart::Punct('.'),
            ]
        );
    }

    #[test]
    fn parses_inline_stress_annotations() {
        let parts = split_text_parts("Try [or](+2) now.");
        assert_eq!(
            parts,
            vec![
                TextPart::Text("Try".to_string()),
                TextPart::Space,
                TextPart::StressText {
                    text: "or".to_string(),
                    delta: 2,
                },
                TextPart::Space,
                TextPart::Text("now".to_string()),
                TextPart::Punct('.'),
            ]
        );
    }

    #[test]
    fn preserves_numeric_connectors() {
        let parts = split_text_parts("82 million parameters.");
        assert_eq!(
            parts,
            vec![
                TextPart::Text("82".to_string()),
                TextPart::Space,
                TextPart::Text("million".to_string()),
                TextPart::Space,
                TextPart::Text("parameters".to_string()),
                TextPart::Punct('.'),
            ]
        );
    }

    #[test]
    fn raises_unstressed_segment() {
        assert_eq!(apply_stress_delta("ɔɹ", 2), "ˈɔɹ");
    }

    #[test]
    fn lowers_primary_stress() {
        assert_eq!(apply_stress_delta("kˈOkəɹO", -1), "kˌOkəɹO");
        assert_eq!(apply_stress_delta("kˈOkəɹO", -2), "kOkəɹO");
    }
}
