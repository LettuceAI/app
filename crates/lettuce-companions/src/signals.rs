use crate::{EmotionVector, RelationshipDelta};

#[derive(Debug, Clone, PartialEq)]
pub struct EmotionLabelScore {
    pub label: String,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EmotionClassification {
    pub labels: Vec<EmotionLabelScore>,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionSignalBundle {
    pub signals: Vec<String>,
    pub emotion_delta: EmotionVector,
    pub relationship_delta: RelationshipDelta,
    pub confidence: f64,
}

#[must_use]
pub fn unavailable_signal_bundle() -> CompanionSignalBundle {
    CompanionSignalBundle {
        signals: Vec::new(),
        emotion_delta: EmotionVector::default(),
        relationship_delta: RelationshipDelta {
            stability: 0.01,
            ..RelationshipDelta::default()
        },
        confidence: 0.2,
    }
}

#[must_use]
pub fn signals_from_classification(
    classification: &EmotionClassification,
) -> CompanionSignalBundle {
    let mut signals = Vec::new();
    let mut delta = EmotionVector::default();
    let mut rel = RelationshipDelta::default();
    let mut applied_score = 0.0_f64;

    for item in classification.labels.iter().take(8) {
        if item.score < label_threshold(item.label.as_str()) {
            continue;
        }
        applied_score = applied_score.max(item.score as f64);
        apply_emotion_label(item, &mut signals, &mut delta, &mut rel);
    }

    if signals.is_empty() {
        rel.stability += 0.01;
    }

    let confidence = if signals.is_empty() {
        0.25
    } else {
        clamp01((classification.confidence * 0.75) + (applied_score * 0.25))
    };

    CompanionSignalBundle {
        signals,
        emotion_delta: delta.clamp_signed(),
        relationship_delta: rel,
        confidence,
    }
}

fn apply_emotion_label(
    item: &EmotionLabelScore,
    signals: &mut Vec<String>,
    delta: &mut EmotionVector,
    rel: &mut RelationshipDelta,
) {
    let score = item.score as f64;
    let label = item.label.as_str();

    match label {
        "love" => {
            push_signal(signals, "emotion:love");
            delta.warmth += 0.10 * score;
            delta.affection_intensity += 0.15 * score;
            delta.longing += 0.06 * score;
            delta.trust += 0.04 * score;
            rel.closeness += 0.035 * score;
            rel.affection += 0.055 * score;
        }
        "caring" => {
            push_signal(signals, "emotion:caring");
            delta.warmth += 0.11 * score;
            delta.trust += 0.05 * score;
            delta.calm += 0.04 * score;
            rel.closeness += 0.025 * score;
            rel.trust += 0.025 * score;
        }
        "gratitude" | "admiration" | "approval" => {
            push_signal(signals, "emotion:appreciation");
            delta.warmth += 0.08 * score;
            delta.trust += 0.07 * score;
            delta.calm += 0.035 * score;
            rel.trust += 0.03 * score;
            rel.stability += 0.025 * score;
        }
        "joy" | "amusement" | "excitement" | "optimism" => {
            push_signal(signals, "emotion:positive");
            delta.warmth += 0.07 * score;
            delta.calm += 0.035 * score;
            delta.affection_intensity += 0.04 * score;
            rel.closeness += 0.018 * score;
            rel.stability += 0.015 * score;
        }
        "desire" => {
            push_signal(signals, "emotion:desire");
            delta.longing += 0.12 * score;
            delta.affection_intensity += 0.08 * score;
            delta.vulnerability += 0.035 * score;
            rel.closeness += 0.025 * score;
            rel.affection += 0.03 * score;
        }
        "relief" => {
            push_signal(signals, "emotion:relief");
            delta.calm += 0.08 * score;
            delta.trust += 0.04 * score;
            delta.tension -= 0.05 * score;
            delta.hurt -= 0.035 * score;
            rel.stability += 0.03 * score;
            rel.tension -= 0.025 * score;
        }
        "remorse" => {
            push_signal(signals, "emotion:remorse");
            delta.warmth += 0.04 * score;
            delta.trust += 0.035 * score;
            delta.hurt -= 0.06 * score;
            delta.tension -= 0.05 * score;
            rel.trust += 0.025 * score;
            rel.tension -= 0.025 * score;
            rel.stability += 0.02 * score;
        }
        "sadness" | "grief" | "disappointment" => {
            push_signal(signals, "emotion:distress");
            delta.warmth += 0.035 * score;
            delta.vulnerability += 0.10 * score;
            delta.reassurance_need += 0.09 * score;
            delta.hurt += 0.045 * score;
            delta.calm -= 0.035 * score;
            rel.closeness += 0.012 * score;
        }
        "fear" | "nervousness" => {
            push_signal(signals, "emotion:anxiety");
            delta.vulnerability += 0.09 * score;
            delta.reassurance_need += 0.10 * score;
            delta.tension += 0.04 * score;
            delta.calm -= 0.06 * score;
            rel.stability -= 0.015 * score;
        }
        "anger" | "annoyance" | "disapproval" | "disgust" => {
            push_signal(signals, "emotion:conflict");
            delta.hurt += 0.08 * score;
            delta.irritation += 0.10 * score;
            delta.tension += 0.12 * score;
            delta.calm -= 0.08 * score;
            delta.warmth -= 0.06 * score;
            delta.trust -= 0.045 * score;
            rel.tension += 0.07 * score;
            rel.trust -= 0.045 * score;
            rel.affection -= 0.06 * score;
            rel.closeness -= 0.03 * score;
            rel.stability -= 0.035 * score;
        }
        "embarrassment" => {
            push_signal(signals, "emotion:embarrassment");
            delta.vulnerability += 0.07 * score;
            delta.reassurance_need += 0.045 * score;
            delta.tension += 0.02 * score;
            delta.warmth += 0.02 * score;
        }
        "confusion" => {
            push_signal(signals, "emotion:uncertainty");
            delta.tension += 0.025 * score;
            delta.reassurance_need += 0.035 * score;
            delta.calm -= 0.025 * score;
        }
        "curiosity" | "realization" | "surprise" => {
            push_signal(signals, "emotion:engagement");
            delta.warmth += 0.025 * score;
            delta.vulnerability += 0.02 * score;
            rel.closeness += 0.01 * score;
        }
        "pride" => {
            push_signal(signals, "emotion:pride");
            delta.calm += 0.035 * score;
            delta.warmth += 0.025 * score;
            rel.stability += 0.015 * score;
        }
        "neutral" => {
            push_signal(signals, "emotion:neutral");
            rel.stability += 0.01 * score;
        }
        _ => {}
    }
}

fn label_threshold(label: &str) -> f32 {
    match label {
        "neutral" => 0.55,
        "love" | "caring" | "gratitude" | "remorse" | "anger" | "sadness" | "fear" => 0.18,
        _ => 0.22,
    }
}

fn push_signal(signals: &mut Vec<String>, label: &str) {
    if !signals.iter().any(|existing| existing == label) {
        signals.push(label.to_string());
    }
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(label: &str, score: f32) -> EmotionLabelScore {
        EmotionLabelScore {
            label: label.into(),
            score,
        }
    }

    #[test]
    fn all_legacy_label_groups_copy_exact_deltas() {
        let cases = [
            ("love", "emotion:love", 0.10, 0.055),
            ("caring", "emotion:caring", 0.11, 0.0),
            ("gratitude", "emotion:appreciation", 0.08, 0.0),
            ("joy", "emotion:positive", 0.07, 0.0),
            ("desire", "emotion:desire", 0.0, 0.03),
            ("relief", "emotion:relief", 0.0, 0.0),
            ("remorse", "emotion:remorse", 0.04, 0.0),
            ("sadness", "emotion:distress", 0.035, 0.0),
            ("fear", "emotion:anxiety", 0.0, 0.0),
            ("anger", "emotion:conflict", -0.06, -0.06),
            ("embarrassment", "emotion:embarrassment", 0.02, 0.0),
            ("confusion", "emotion:uncertainty", 0.0, 0.0),
            ("curiosity", "emotion:engagement", 0.025, 0.0),
            ("pride", "emotion:pride", 0.025, 0.0),
            ("neutral", "emotion:neutral", 0.0, 0.0),
        ];
        for (label, signal, warmth, affection) in cases {
            let bundle = signals_from_classification(&EmotionClassification {
                labels: vec![score(label, 1.0)],
                confidence: 1.0,
            });
            assert_eq!(bundle.signals, [signal], "label {label}");
            assert_eq!(bundle.emotion_delta.warmth, warmth, "label {label}");
            assert_eq!(
                bundle.relationship_delta.affection, affection,
                "label {label}"
            );
            assert_eq!(bundle.confidence, 1.0, "label {label}");
        }
    }

    #[test]
    fn thresholds_top_eight_and_confidence_copy_legacy_order() {
        let bundle = signals_from_classification(&EmotionClassification {
            labels: vec![
                score("neutral", 0.54),
                score("love", 0.18),
                score("caring", 0.17),
                score("joy", 0.22),
                score("confusion", 0.21),
                score("gratitude", 0.19),
                score("fear", 0.18),
                score("pride", 0.22),
                score("anger", 1.0),
            ],
            confidence: 0.8,
        });
        assert_eq!(
            bundle.signals,
            [
                "emotion:love",
                "emotion:positive",
                "emotion:appreciation",
                "emotion:anxiety",
                "emotion:pride"
            ]
        );
        assert!((bundle.confidence - 0.655).abs() < 1e-8);
        assert_eq!(bundle.relationship_delta.tension, 0.0);
    }

    #[test]
    fn duplicate_group_labels_accumulate_but_signal_is_deduplicated() {
        let bundle = signals_from_classification(&EmotionClassification {
            labels: vec![score("joy", 0.5), score("optimism", 0.4)],
            confidence: 0.5,
        });
        assert_eq!(bundle.signals, ["emotion:positive"]);
        assert!((bundle.emotion_delta.warmth - 0.063).abs() < 1e-8);
    }

    #[test]
    fn no_applied_label_and_unavailable_model_keep_distinct_legacy_fallbacks() {
        let empty = signals_from_classification(&EmotionClassification {
            labels: vec![score("neutral", 0.54), score("unknown", 1.0)],
            confidence: 0.9,
        });
        assert!(empty.signals.is_empty());
        assert_eq!(empty.relationship_delta.stability, 0.01);
        assert_eq!(empty.confidence, 0.25);

        let unavailable = unavailable_signal_bundle();
        assert!(unavailable.signals.is_empty());
        assert_eq!(unavailable.relationship_delta.stability, 0.01);
        assert_eq!(unavailable.confidence, 0.2);
    }

    #[test]
    fn accumulated_emotion_delta_clamps_only_after_all_labels() {
        let bundle = signals_from_classification(&EmotionClassification {
            labels: vec![
                score("love", 10.0),
                score("caring", 10.0),
                score("gratitude", 10.0),
                score("joy", 10.0),
            ],
            confidence: 1.0,
        });
        assert_eq!(bundle.emotion_delta.warmth, 1.0);
        assert!(bundle.relationship_delta.closeness > 0.0);
    }
}
