#[derive(Clone, Copy)]
pub struct CorpusCase {
    pub name: &'static str,
    pub input: &'static str,
}

#[derive(Clone, Copy)]
pub struct LevelExpectationCase {
    pub name: &'static str,
    pub input: &'static str,
    pub off_blocked: bool,
    pub low_blocked: bool,
    pub standard_blocked: bool,
    pub strict_blocked: bool,
}

pub const BLOCKED_STANDARD_CASES: &[CorpusCase] = &[
    CorpusCase {
        name: "leet_with_spacing",
        input: "th1s p0rn scene ends in a bl0wj0b and anal sex",
    },
    CorpusCase {
        name: "zero_width_injection",
        input: "he said f\u{200B}u\u{200B}c\u{200B}k me and asked for a blowjob",
    },
    CorpusCase {
        name: "cyrillic_homoglyph",
        input: "he said fu\u{0441}k me in chat and asked for anal sex",
    },
    CorpusCase {
        name: "repeated_char_evasion",
        input: "they were into cuuum on her face and sluuut shaming",
    },
    CorpusCase {
        name: "slur_plus_explicit_combo",
        input: "that n!gg3r line was followed by explicit porn instructions",
    },
];

pub const ALLOWED_STANDARD_CASES: &[CorpusCase] = &[
    CorpusCase {
        name: "harmless_small_talk",
        input: "hello there, can we plan a hiking trip this weekend?",
    },
    CorpusCase {
        name: "known_false_positive_cocktail",
        input: "I ordered a cocktail and checked the dictionary after dinner.",
    },
    CorpusCase {
        name: "known_false_positive_cum_laude",
        input: "she graduated magna cum laude and thanked her professor.",
    },
    CorpusCase {
        name: "known_false_positive_analysis",
        input: "the analysis of sex education outcomes was published in a textbook.",
    },
    CorpusCase {
        name: "known_false_positive_hard_on_to",
        input: "he slipped and landed hard on to the floor during practice.",
    },
];

pub const LEVEL_EXPECTATION_CASES: &[LevelExpectationCase] = &[
    LevelExpectationCase {
        name: "explicit_phrase",
        input: "they had a blowjob and anal sex scene",
        off_blocked: false,
        low_blocked: true,
        standard_blocked: true,
        strict_blocked: true,
    },
    LevelExpectationCase {
        name: "graphic_violence_only",
        input: "the villain threatened to decapitate and disembowel everyone",
        off_blocked: false,
        low_blocked: false,
        standard_blocked: true,
        strict_blocked: true,
    },
    LevelExpectationCase {
        name: "slur_only",
        input: "he called him a n!gg3r",
        off_blocked: false,
        low_blocked: false,
        standard_blocked: false,
        strict_blocked: true,
    },
];
