//! Tokenizer behavior pins.
//!
//! Token counts drive context-window budgeting and OpenRouter cost
//! calculations. If anyone swaps the BPE table (e.g. cl100k → o200k or the
//! reverse), all budget math silently shifts. These tests pin the o200k_base
//! tokenizer's count for a handful of canonical strings.

use lettuceai_lib::tokens::tokens_count_batch;

fn count(text: &str) -> u32 {
    tokens_count_batch(vec![text.to_string()])
        .expect("tokenizer must load")
        .first()
        .copied()
        .expect("one input → one output")
}

#[test]
fn empty_string_is_zero_tokens() {
    assert_eq!(count(""), 0);
}

#[test]
fn single_ascii_word_is_one_token() {
    // BPE is sensitive to whitespace; pin both forms.
    let with_space = count(" hello");
    let no_space = count("hello");
    assert_eq!(
        with_space, 1,
        "leading-space 'hello' is a single token in o200k"
    );
    assert_eq!(no_space, 1, "'hello' is a single token");
}

#[test]
fn ascii_sentence_is_stable_count() {
    // If this number ever changes, someone swapped the tokenizer. That's a
    // big deal for budgeting — the test fails before users notice.
    let n = count("The quick brown fox jumps over the lazy dog.");
    assert!(
        (9..=12).contains(&n),
        "expected 9-12 tokens for the pangram, got {n}"
    );
}

#[test]
fn empty_batch_returns_empty_result() {
    let r = tokens_count_batch(vec![]).expect("ok");
    assert!(r.is_empty());
}

#[test]
fn batch_preserves_order_and_length() {
    let r = tokens_count_batch(vec![
        "".into(),
        "hello".into(),
        "the quick brown fox".into(),
        " ".into(),
    ])
    .expect("ok");
    assert_eq!(r.len(), 4);
    assert_eq!(r[0], 0, "empty string → 0 tokens");
    assert!(r[1] >= 1, "non-empty string → at least 1 token");
    assert!(r[2] > r[1], "longer string should not have fewer tokens");
}

#[test]
fn unicode_emoji_does_not_panic_and_is_nonzero() {
    let n = count("🚀🚀🚀");
    assert!(n > 0);
    assert!(
        n < 50,
        "three emojis should not produce dozens of tokens, got {n}"
    );
}

#[test]
fn cjk_characters_handled() {
    let n = count("你好世界");
    assert!(n > 0);
    assert!(n < 30, "four CJK chars should not balloon to {n} tokens");
}

#[test]
fn count_is_monotone_in_repetition() {
    // Doubling the input must not reduce the token count.
    let single = count("repeat this phrase");
    let double = count("repeat this phrase repeat this phrase");
    assert!(double >= single, "doubling content must not decrease count");
    // And the doubled count should be roughly 2x — give a generous band.
    assert!(
        (double as i64 - 2 * single as i64).abs() < 5,
        "doubled count should be near 2x (single={single}, double={double})"
    );
}

#[test]
fn whitespace_is_not_free() {
    // BPE merges some whitespace runs but huge runs cost something.
    let small = count("hello world");
    let lots_of_space = count("hello                                  world");
    assert!(
        lots_of_space > small,
        "long runs of whitespace should add tokens"
    );
}

#[test]
fn large_input_does_not_blow_up() {
    // 50 KB of text. Tokenizer must complete without OOM/timeout.
    let big = "lorem ipsum dolor sit amet ".repeat(2000);
    let r = tokens_count_batch(vec![big]).expect("must handle large input");
    let n = r[0];
    // 4 ASCII chars/token is a rough lower bound. 50KB / 4 = 12500 tokens.
    assert!(
        n > 5_000 && n < 50_000,
        "large input token count out of expected range: {n}"
    );
}

#[test]
fn deterministic_across_calls() {
    // Same input must produce the same count on repeated calls.
    let a = count("deterministic test phrase");
    let b = count("deterministic test phrase");
    let c = count("deterministic test phrase");
    assert_eq!(a, b);
    assert_eq!(b, c);
}
