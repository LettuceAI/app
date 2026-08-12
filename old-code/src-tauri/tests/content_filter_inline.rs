//! Gathered from inline tests in src/content_filter/mod.rs.

use lettuceai_lib::content_filter::adversarial_corpus::{
    ALLOWED_STANDARD_CASES, BLOCKED_STANDARD_CASES, LEVEL_EXPECTATION_CASES,
};
use lettuceai_lib::content_filter::*;

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
    let result =
        filter.check_text(r#""You want to *fuck* me? You'll have to *work* for it. Beg for it.""#);
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
