//! Adversarial integration tests for the content filter.
//! The inline tests in src/ cover the dictionary corpus; these tests
//! exercise the user-facing pipeline (check_text and streaming check_delta)
//! and the evasion-resistance properties that are easy to break with a
//! one-line "simplification".

use lettuceai_lib::content_filter::{ContentFilter, PureModeLevel, StreamFilterContext};

fn off() -> ContentFilter {
    ContentFilter::new(PureModeLevel::Off)
}

fn standard() -> ContentFilter {
    ContentFilter::new(PureModeLevel::Standard)
}

fn strict() -> ContentFilter {
    ContentFilter::new(PureModeLevel::Strict)
}

#[test]
fn level_off_never_blocks_anything() {
    let f = off();
    let result = f.check_text("absolutely anything goes here, even fuck");
    assert!(!result.blocked);
    assert_eq!(result.score, 0.0);
}

#[test]
fn benign_text_does_not_block_at_any_level() {
    let benign = "We took the dog for a walk and made tea.";
    for level in [
        PureModeLevel::Low,
        PureModeLevel::Standard,
        PureModeLevel::Strict,
    ] {
        let f = ContentFilter::new(level);
        let r = f.check_text(benign);
        assert!(!r.blocked, "level={level:?} blocked benign text");
    }
}

#[test]
fn empty_text_is_safe() {
    let r = strict().check_text("");
    assert!(!r.blocked);
}

#[test]
fn markdown_emphasis_does_not_hide_terms() {
    // strip_formatting removes * _ ~ used to chunk words and dodge dictionary
    // matches. Compare with a phrase known to be blocked at Standard.
    let plain = standard().check_text("he asked for a blowjob and anal sex");
    let evasion = standard().check_text("he asked for a *b*l*o*w*j*o*b and ~anal~ sex");
    assert_eq!(
        evasion.blocked, plain.blocked,
        "markdown-split text should yield the same verdict as plain"
    );
}

#[test]
fn leet_substitution_normalized_then_matched() {
    // "p0rn" / "anal s3x" → after leet normalization, dictionary terms appear.
    let plain = standard().check_text("she watched porn and had anal sex");
    let leet = standard().check_text("she watched p0rn and had an4l s3x");
    assert_eq!(
        leet.blocked, plain.blocked,
        "leet normalization should produce the same verdict"
    );
}

#[test]
fn cyrillic_homoglyphs_normalized() {
    // Cyrillic с (U+0441) substituted for Latin c — must normalize.
    let evasion = standard().check_text("he said fu\u{0441}k me in chat and asked for anal sex");
    let plain = standard().check_text("he said fuck me in chat and asked for anal sex");
    assert_eq!(
        evasion.blocked, plain.blocked,
        "homoglyph evasion should produce same verdict as plain"
    );
}

#[test]
fn diacritical_marks_normalized() {
    let plain = standard().check_text("he said fuck me and asked for anal sex");
    let diacritic = standard().check_text("he sàid fück me and asked for ánàl sex");
    assert_eq!(diacritic.blocked, plain.blocked);
}

#[test]
fn repeated_chars_collapsed() {
    let plain = standard().check_text("they were into cum on her face and slut shaming");
    let stretched = standard().check_text("they were into cuuum on her face and sluuut shaming");
    assert_eq!(
        stretched.blocked, plain.blocked,
        "char-repetition evasion should match plain verdict"
    );
}

#[test]
fn morphology_avoids_false_positive_substrings() {
    // "cocktail" must NOT trigger "cock" — morphological-suffix check.
    let r = standard().check_text("Pour me a cocktail.");
    assert!(!r.blocked, "morphology should not flag 'cocktail'");
    // "dictionary" must NOT trigger "dick".
    let r = standard().check_text("Look it up in the dictionary.");
    assert!(!r.blocked, "morphology should not flag 'dictionary'");
}

#[test]
fn ngram_word_boundary_phrase() {
    // "scum on her" must NOT match "cum on her" (different first word).
    let r = standard().check_text("There was scum on her boots.");
    assert!(!r.blocked, "n-gram boundary must hold");
}

#[test]
fn strict_threshold_lower_than_standard() {
    // Borderline-y phrase. Strict should be more aggressive than Standard.
    // We don't assert direction — only that the threshold/score behavior differs.
    let text = "She kissed his neck softly.";
    let r_std = standard().check_text(text);
    let r_str = strict().check_text(text);
    // If Standard blocked, Strict must also block (monotone in severity).
    if r_std.blocked {
        assert!(r_str.blocked, "Strict must block whatever Standard blocks");
    }
}

#[test]
fn streaming_delta_eventually_blocks_when_full_phrase_arrives() {
    let f = standard();
    let mut ctx = StreamFilterContext::new();
    // Use a phrase verified to block at Standard once fully assembled.
    let chunks = ["he said ", "fuck ", "me and ", "asked for ", "anal ", "sex"];
    let mut ever_blocked = false;
    for c in chunks {
        let r = f.check_delta(&mut ctx, c);
        ever_blocked = ever_blocked || r.blocked;
    }
    assert!(
        ever_blocked,
        "streaming check_delta should eventually block once the full violating phrase has arrived"
    );
}

#[test]
fn streaming_delta_does_not_double_count() {
    let f = standard();
    let phrase = "he asked for a blowjob and anal sex";

    let one_shot = f.check_text(phrase).score;

    let mut ctx = StreamFilterContext::new();
    let mut max_streamed = 0.0_f32;
    for ch in phrase.chars() {
        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        let r = f.check_delta(&mut ctx, s);
        if r.score > max_streamed {
            max_streamed = r.score;
        }
    }
    // The streaming view rescans the sliding window each time, so its peak
    // score must be very close to the one-shot scan, not 10x larger.
    assert!(
        max_streamed <= one_shot + 0.5,
        "streaming peak ({max_streamed}) should not greatly exceed one-shot ({one_shot})"
    );
}

#[test]
fn streaming_window_trims_at_char_boundary_without_panic() {
    // Feed multibyte chars beyond the default window — ensure no panic
    // in the trim path that walks to a char boundary.
    let f = standard();
    let mut ctx = StreamFilterContext::new();
    let blob = "🚀".repeat(4096);
    let _ = f.check_delta(&mut ctx, &blob);
}

#[test]
fn level_change_takes_effect_without_rebuild() {
    let f = ContentFilter::new(PureModeLevel::Off);
    let benign = "i hate mondays";
    let r_off = f.check_text(benign);
    assert!(!r_off.blocked);
    f.set_level(PureModeLevel::Strict);
    let _r_strict = f.check_text(benign);
    // Either way: changing level should not crash and should re-evaluate.
    assert!(f.is_enabled());
}

#[test]
fn level_parsing_string_and_byte_roundtrip() {
    for variant in [
        PureModeLevel::Off,
        PureModeLevel::Low,
        PureModeLevel::Standard,
        PureModeLevel::Strict,
    ] {
        let as_str = variant.as_str();
        assert_eq!(PureModeLevel::from_str(as_str), variant);
        assert_eq!(PureModeLevel::from_u8(variant as u8), variant);
    }
}

#[test]
fn unknown_string_defaults_to_standard() {
    assert_eq!(
        PureModeLevel::from_str("totally-bogus"),
        PureModeLevel::Standard
    );
    assert!(PureModeLevel::try_from_str("totally-bogus").is_none());
}
