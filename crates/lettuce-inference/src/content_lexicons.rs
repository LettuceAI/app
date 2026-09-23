//! Per-language lexicons scored next to the English dictionary. Languages are
//! detected from the checked text itself, over windows the size of the stream
//! window that overlap by half, so a passage long enough to be recognized in
//! a stream is also recognized in the whole text. In Latin script only a confident detection of a
//! language other than English selects its lexicon; in other scripts, where
//! English is not a candidate, an unsure detection selects every lexicon
//! written in that script.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;
use whatlang::{Lang, Script};

use crate::content_filter::{ContentFilter, PureModeLevel, STREAM_WINDOW_BYTES, is_invisible};

#[derive(Deserialize)]
struct LexiconDocument {
    weight: f32,
    languages: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextForm {
    Folded,
    Native,
    Unspaced,
}

#[derive(Debug, PartialEq, Eq)]
enum Matching {
    LatinWord,
    Words(Vec<String>),
    Substring,
    Isolated,
}

#[derive(Debug)]
struct Term {
    text: String,
    form: TextForm,
    matching: Matching,
}

struct Lexicons {
    weight: f32,
    by_language: HashMap<Lang, Vec<Term>>,
    scripts: HashMap<Lang, Vec<Script>>,
}

static LEXICONS: LazyLock<Lexicons> = LazyLock::new(|| {
    let document: LexiconDocument =
        serde_json::from_str(include_str!("../resources/content-filter-lexicons.json"))
            .expect("the bundled content filter lexicons parse");
    let mut by_language = HashMap::new();
    let mut scripts = HashMap::new();
    for (code, terms) in document.languages {
        let Some(language) = Lang::from_code(code) else {
            continue;
        };
        let mut written: Vec<Script> = Vec::new();
        for script in terms.iter().filter_map(|raw| whatlang::detect_script(raw)) {
            if script != Script::Latin && !written.contains(&script) {
                written.push(script);
            }
        }
        scripts.insert(language, written);
        by_language.insert(language, terms.iter().filter_map(|raw| term(raw)).collect());
    }
    Lexicons {
        weight: document.weight,
        by_language,
        scripts,
    }
});

fn is_unspaced(ch: char) -> bool {
    ch.is_alphabetic()
        && whatlang::detect_script(ch.encode_utf8(&mut [0; 4])).is_none_or(|script| {
            matches!(
                script,
                Script::Mandarin
                    | Script::Hiragana
                    | Script::Katakana
                    | Script::Thai
                    | Script::Khmer
                    | Script::Myanmar
            )
        })
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || is_combining_mark(ch)
}

fn words(text: &str) -> Vec<&str> {
    text.split(|ch: char| !is_word_char(ch))
        .filter(|word| !word.is_empty())
        .collect()
}

fn owned_words(text: &str) -> Vec<String> {
    words(text).into_iter().map(str::to_owned).collect()
}

fn composed(lower: &str) -> String {
    lower
        .nfc()
        .collect::<String>()
        .replace("i\u{307}", "i")
        .replace(['\u{2019}', '\u{2018}', '\u{02BC}'], "'")
}

fn folded(composed: &str) -> String {
    ContentFilter::normalize_leet(&ContentFilter::normalize_unicode(composed))
}

fn native(composed: &str) -> String {
    composed.chars().filter(|ch| !is_invisible(*ch)).collect()
}

fn unspaced(native: &str) -> String {
    native.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn term(raw: &str) -> Option<Term> {
    let lower = composed(&raw.trim().to_lowercase());
    if !lower.chars().any(char::is_alphabetic) {
        return None;
    }
    if lower.chars().any(is_unspaced) {
        let text = unspaced(&native(&lower));
        return Some(if text.chars().count() == 1 {
            Term {
                text,
                form: TextForm::Native,
                matching: Matching::Isolated,
            }
        } else {
            Term {
                text,
                form: TextForm::Unspaced,
                matching: Matching::Substring,
            }
        });
    }
    let (form, text) = if whatlang::detect_script(&lower) == Some(Script::Latin) {
        (TextForm::Folded, folded(&lower))
    } else {
        (TextForm::Native, native(&lower))
    };
    let compound = !text.contains(char::is_whitespace)
        && text.chars().any(|ch| !is_word_char(ch) && ch != '\'');
    let words = owned_words(&text);
    let matching = if compound {
        Matching::Substring
    } else if form == TextForm::Folded && words.len() == 1 && words[0] == text {
        Matching::LatinWord
    } else {
        Matching::Words(words)
    };
    Some(Term {
        text,
        form,
        matching,
    })
}

fn floor_boundary(text: &str, mut index: usize) -> usize {
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn windows(text: &str) -> impl Iterator<Item = &str> {
    let mut start = 0;
    let mut done = text.is_empty();
    std::iter::from_fn(move || {
        if done {
            return None;
        }
        let end = floor_boundary(text, text.len().min(start + STREAM_WINDOW_BYTES));
        let window = &text[start..end];
        if end == text.len() {
            done = true;
        } else {
            start = floor_boundary(text, start + STREAM_WINDOW_BYTES / 2).max(start + 1);
            start = (start..=text.len())
                .find(|index| text.is_char_boundary(*index))
                .unwrap_or(text.len());
        }
        Some(window)
    })
}

fn detected_languages(text: &str) -> Vec<Lang> {
    let mut languages = Vec::new();
    for info in windows(text).filter_map(whatlang::detect) {
        let selected: Vec<Lang> = if info.is_reliable() {
            vec![info.lang()]
        } else if info.script() == Script::Latin {
            Vec::new()
        } else {
            LEXICONS
                .scripts
                .iter()
                .filter(|(_, scripts)| scripts.contains(&info.script()))
                .map(|(language, _)| *language)
                .collect()
        };
        for language in selected {
            if language != Lang::Eng && !languages.contains(&language) {
                languages.push(language);
            }
        }
    }
    languages.sort_by_key(|language| language.code());
    languages
}

struct CheckedText {
    collapsed: String,
    folded: String,
    folded_words: Vec<String>,
    collapsed_words: Vec<String>,
    native: String,
    native_words: Vec<String>,
    unspaced: String,
}

impl CheckedText {
    fn new(lower: &str) -> Self {
        let composed = composed(lower);
        let folded = folded(&composed);
        let collapsed = ContentFilter::collapse_repeated_chars(&folded);
        let native = native(&composed);
        Self {
            folded_words: owned_words(&folded),
            collapsed_words: if collapsed == folded {
                Vec::new()
            } else {
                owned_words(&collapsed)
            },
            collapsed,
            native_words: owned_words(&native),
            unspaced: unspaced(&native),
            folded,
            native,
        }
    }

    fn contains(&self, term: &Term) -> bool {
        let (text, words, collapsed): (&str, &[String], &[String]) = match term.form {
            TextForm::Folded => (&self.folded, &self.folded_words, &self.collapsed_words),
            TextForm::Native => (&self.native, &self.native_words, &[]),
            TextForm::Unspaced => (&self.unspaced, &[], &[]),
        };
        match &term.matching {
            Matching::LatinWord => {
                latin_word(text, &term.text)
                    || (!collapsed.is_empty() && latin_word(&self.collapsed, &term.text))
            }
            Matching::Substring => text.contains(term.text.as_str()),
            Matching::Isolated => isolated(text, &term.text),
            Matching::Words(term_words) => {
                has_sequence(words, term_words) || has_sequence(collapsed, term_words)
            }
        }
    }
}

fn has_sequence(words: &[String], term_words: &[String]) -> bool {
    !term_words.is_empty()
        && words.len() >= term_words.len()
        && words
            .windows(term_words.len())
            .any(|window| window == term_words)
}

fn is_latin_word_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
        || (ch.is_alphanumeric() && ('\u{00C0}'..='\u{024F}').contains(&ch))
        || is_combining_mark(ch)
}

fn latin_word(text: &str, term: &str) -> bool {
    text.match_indices(term).any(|(start, found)| {
        let before = text[..start].chars().next_back();
        let after = text[start + found.len()..].chars().next();
        !before.is_some_and(is_latin_word_char) && !after.is_some_and(is_latin_word_char)
    })
}

fn isolated(text: &str, term: &str) -> bool {
    text.match_indices(term).any(|(start, found)| {
        let before = text[..start].chars().next_back();
        let after = text[start + found.len()..].chars().next();
        !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
    })
}

fn already_counted(matched: &[String], term: &str) -> bool {
    matched.iter().any(|counted| {
        counted == term
            || (!counted.contains(' ')
                && !term.contains(' ')
                && (ContentFilter::word_matches_term(term, counted)
                    || ContentFilter::word_matches_term(counted, term)))
    })
}

/// Adds the hits of the lexicons of the languages detected in a check's
/// lowercased, formatting-stripped text. A term the English dictionary
/// already counted, or a form of it, is not counted again.
pub(crate) fn score(
    lower: &str,
    level: PureModeLevel,
    context_factor: f32,
    total_score: &mut f32,
    matched: &mut Vec<String>,
) {
    let lexicons = &*LEXICONS;
    if level == PureModeLevel::Off || lexicons.weight < level.min_weight() {
        return;
    }
    let selected: Vec<&Vec<Term>> = detected_languages(lower)
        .into_iter()
        .filter_map(|language| lexicons.by_language.get(&language))
        .collect();
    if selected.is_empty() {
        return;
    }
    let text = CheckedText::new(lower);
    for term in selected.into_iter().flatten() {
        if !already_counted(matched, &term.text) && text.contains(term) {
            *total_score += lexicons.weight * context_factor;
            matched.push(term.text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_filter::{PureModeLevel, StreamFilterContext};

    const TURKISH: &str = "Bugün hava çok güzeldi, parkta uzun bir yürüyüş yaptık ve akşam \
                           arkadaşlarımızla birlikte yemek yedik.";

    fn lexicon(code: &str) -> &'static [Term] {
        &LEXICONS.by_language[&Lang::from_code(code).expect("language")]
    }

    fn single_words(code: &str, form: TextForm) -> impl Iterator<Item = &'static str> {
        lexicon(code)
            .iter()
            .filter(move |term| {
                term.form == form
                    && (term.matching == Matching::LatinWord
                        || matches!(&term.matching, Matching::Words(words) if words.len() == 1))
            })
            .map(|term| term.text.as_str())
    }

    fn blocked(level: PureModeLevel, text: &str) -> bool {
        ContentFilter::new(level).check_text(text, 0).blocked
    }

    fn lexicon_hits(text: &str) -> Vec<String> {
        let (mut total, mut matched) = (0.0, Vec::new());
        score(
            &text.to_lowercase(),
            PureModeLevel::Strict,
            1.0,
            &mut total,
            &mut matched,
        );
        matched
    }

    fn english_score(text: &str) -> f32 {
        let normalized = ContentFilter::normalize_leet(&ContentFilter::normalize_unicode(
            &ContentFilter::strip_formatting(text).to_lowercase(),
        ));
        let words = ContentFilter::tokenize(&normalized);
        ContentFilter::score_text(&words, &normalized, false, PureModeLevel::Strict).0
    }

    #[test]
    fn every_bundled_language_and_term_loads() {
        let document: LexiconDocument =
            serde_json::from_str(include_str!("../resources/content-filter-lexicons.json"))
                .expect("lexicons");
        for (code, terms) in &document.languages {
            let language = Lang::from_code(code.as_str()).expect("a detectable language");
            let with_letters = terms
                .iter()
                .filter(|term| term.chars().any(char::is_alphabetic))
                .count();
            assert_eq!(
                LEXICONS.by_language[&language].len(),
                with_letters,
                "{code}"
            );
        }
        assert!((LEXICONS.weight - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn a_confidently_detected_language_adds_its_lexicon() {
        assert!(!blocked(PureModeLevel::Strict, TURKISH));
        let terms: Vec<&str> = single_words("tur", TextForm::Folded).take(2).collect();
        let text = format!("{TURKISH} Sonra {} ve {} dedi.", terms[0], terms[1]);
        assert_eq!(detected_languages(&text.to_lowercase()), [Lang::Tur]);
        assert!(english_score(&text) < 1.0);
        assert!(blocked(PureModeLevel::Standard, &text));
    }

    #[test]
    fn cyrillic_text_is_matched_in_its_own_script() {
        let calm = "Сегодня была прекрасная погода, мы долго гуляли в парке, а вечером \
                    ужинали вместе с друзьями.";
        assert!(!blocked(PureModeLevel::Strict, calm));
        let terms: Vec<&str> = single_words("rus", TextForm::Native).take(2).collect();
        let text = format!("{calm} Потом он сказал {} и {}.", terms[0], terms[1]);
        assert!(blocked(PureModeLevel::Standard, &text));
    }

    #[test]
    fn english_and_unsure_text_take_no_lexicon() {
        for text in [
            "He grabbed her hand.",
            "Kiss me.",
            "What a day!",
            "She smiled and poured the tea.",
            "we walked through the park this afternoon and had dinner with friends later.",
        ] {
            assert!(lexicon_hits(text).is_empty(), "{text}");
        }
        let terms: Vec<&str> = single_words("deu", TextForm::Folded).take(2).collect();
        assert!(lexicon_hits(&format!("What a {}!", terms[0])).is_empty());
    }

    #[test]
    fn a_foreign_passage_is_caught_by_the_stream_and_the_whole_text() {
        let english = "We walked through the quiet park this afternoon, talked about \
                       the books we had read, and had dinner with our friends later. "
            .repeat(30);
        let terms: Vec<&str> = single_words("tur", TextForm::Folded).take(2).collect();
        let text = format!(
            "{english}{TURKISH} Sonra {} ve {} dedi.",
            terms[0], terms[1]
        );
        let filter = ContentFilter::new(PureModeLevel::Standard);
        let mut stream = StreamFilterContext::default();
        let streamed = text
            .split_inclusive(' ')
            .any(|delta| filter.check_delta(&mut stream, delta, 0).blocked);
        assert!(streamed);
        assert!(filter.check_text(&text, 0).blocked);
        assert!(!filter.check_text(&english, 0).blocked);
    }

    #[test]
    fn dotted_capitals_and_curly_apostrophes_match() {
        let term = single_words("tur", TextForm::Folded)
            .find(|term| term.contains('i'))
            .expect("a Turkish term with i");
        let capitals: String = term
            .chars()
            .map(|ch| if ch == 'i' { 'İ' } else { ch })
            .flat_map(char::to_uppercase)
            .collect();
        let lower = format!("{TURKISH} {capitals}").to_lowercase();
        let found = lexicon("tur")
            .iter()
            .find(|candidate| candidate.text == term)
            .expect("term");
        assert!(CheckedText::new(&lower).contains(found));

        let french = lexicon("fra")
            .iter()
            .find(|term| {
                term.form == TextForm::Folded
                    && (term.matching == Matching::LatinWord
                        || matches!(&term.matching, Matching::Words(words) if words.len() == 1))
                    && term.text.starts_with(|ch: char| "aeiou".contains(ch))
            })
            .expect("a French word starting with a vowel");
        for apostrophe in ['\'', '\u{2019}'] {
            let text = format!("quelle espèce d{apostrophe}{} !", french.text);
            assert!(CheckedText::new(&text).contains(french), "{text}");
        }
    }

    #[test]
    fn unspaced_scripts_match_inside_running_text() {
        let single = lexicon("cmn")
            .iter()
            .find(|term| term.matching == Matching::Isolated)
            .expect("a single ideograph");
        let embedded = CheckedText::new(&format!("我们今天{}下午一起去公园散步", single.text));
        assert!(!embedded.contains(single));
        let alone = CheckedText::new(&format!("我们今天下午一起去公园散步。 {} 。", single.text));
        assert!(alone.contains(single));

        let mixed = lexicon("cmn")
            .iter()
            .find(|term| {
                term.form == TextForm::Unspaced
                    && term.text.chars().any(|ch| ch.is_ascii_alphabetic())
            })
            .expect("a Han term with Latin letters");
        let running = CheckedText::new(&format!("我们今天{}下午一起去", mixed.text));
        assert!(running.contains(mixed));

        let raw = serde_json::from_str::<serde_json::Value>(include_str!(
            "../resources/content-filter-lexicons.json"
        ))
        .expect("lexicons");
        let spaced = raw["languages"]["jpn"]
            .as_array()
            .expect("jpn")
            .iter()
            .filter_map(|term| term.as_str())
            .find(|term| term.contains(' ') && term.chars().any(is_unspaced))
            .expect("a Japanese term with a space");
        let spaced_term = term(spaced).expect("term");
        let running = CheckedText::new(&format!(
            "今日は{}でした",
            spaced.replace(' ', "").to_lowercase()
        ));
        assert!(running.contains(&spaced_term));
    }

    #[test]
    fn forms_of_an_english_hit_are_not_counted_twice() {
        let matched = vec!["kiss".to_owned()];
        assert!(already_counted(&matched, "kisses"));
        assert!(already_counted(&matched, "kiss"));
        assert!(!already_counted(&matched, "kismet"));
        let german = "Heute war das Wetter wunderschön, wir sind lange im Park spazieren \
                      gegangen und haben abends mit Freunden gegessen.";
        let shared = single_words("deu", TextForm::Folded)
            .find(|term| english_score(term) > 0.0)
            .expect("a German term the English dictionary also scores");
        let text = format!("{german} {shared}");
        assert_eq!(detected_languages(&text.to_lowercase()), [Lang::Deu]);
        let result = ContentFilter::new(PureModeLevel::Strict).check_text(&text, 0);
        assert!((result.score - english_score(shared)).abs() < f32::EPSILON);
    }

    #[test]
    fn latin_terms_match_inside_unspaced_text() {
        let term = lexicon("jpn")
            .iter()
            .find(|term| term.matching == Matching::LatinWord)
            .expect("a Latin term in the Japanese lexicon");
        let glued = CheckedText::new(&format!("彼女は{}です", term.text));
        assert!(glued.contains(term));
        let inside = CheckedText::new(&format!("x{}x", term.text));
        assert!(!inside.contains(term));
    }

    #[test]
    fn everyday_text_passes_strict_in_every_language() {
        for text in [
            "Heute war das Wetter wunderschön, wir sind lange im Park spazieren gegangen und haben abends mit Freunden gegessen.",
            "Hoy hizo un tiempo precioso, paseamos mucho por el parque y por la noche cenamos con unos amigos.",
            "Aujourd'hui il faisait très beau, nous nous sommes promenés dans le parc et nous avons dîné avec des amis.",
            "Oggi il tempo era bellissimo, abbiamo passeggiato a lungo nel parco e la sera abbiamo cenato con gli amici.",
            "Hoje o tempo estava lindo, passeamos muito pelo parque e à noite jantamos com os amigos.",
            "Vandaag was het prachtig weer, we hebben lang in het park gewandeld en 's avonds met vrienden gegeten.",
            "Idag var vädret underbart, vi promenerade länge i parken och åt middag med vänner på kvällen.",
            "Dzisiaj była piękna pogoda, długo spacerowaliśmy po parku, a wieczorem zjedliśmy kolację z przyjaciółmi.",
            "Dnes bylo krásné počasí, dlouho jsme se procházeli v parku a večer jsme večeřeli s přáteli.",
            "Tänään oli kaunis sää, kävelimme pitkään puistossa ja illalla söimme ystävien kanssa.",
            "Ma gyönyörű idő volt, sokáig sétáltunk a parkban, este pedig a barátainkkal vacsoráztunk.",
            TURKISH,
            "Сегодня была прекрасная погода, мы долго гуляли в парке, а вечером ужинали с друзьями.",
            "كان الطقس جميلا اليوم، تمشينا طويلا في الحديقة وتناولنا العشاء مع الأصدقاء في المساء.",
            "امروز هوا خیلی خوب بود، مدت زیادی در پارک قدم زدیم و شب با دوستان شام خوردیم.",
            "今日はとても良い天気で、公園を長く散歩して、夜は友達と一緒に晩ご飯を食べました。",
            "오늘은 날씨가 정말 좋아서 공원을 오래 산책했고 저녁에는 친구들과 함께 밥을 먹었습니다.",
            "今天天气非常好，我们在公园里散步了很久，晚上和朋友们一起吃了饭，还喝了奶茶。",
            "วันนี้อากาศดีมาก เราเดินเล่นในสวนสาธารณะนานและตอนเย็นกินข้าวกับเพื่อนๆ",
        ] {
            assert!(!blocked(PureModeLevel::Strict, text), "{text}");
        }
    }
}
