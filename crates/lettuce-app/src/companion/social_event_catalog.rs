use std::sync::OnceLock;

use lettuce_companions::GroupSocialEventCatalog;

const SOCIAL_EVENT_CATALOG_JSON: &str =
    include_str!("../../resources/companion/social-events/v1.json");

pub(crate) fn social_event_catalog() -> &'static GroupSocialEventCatalog {
    static CATALOG: OnceLock<GroupSocialEventCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        GroupSocialEventCatalog::from_json(SOCIAL_EVENT_CATALOG_JSON)
            .expect("the bundled social event catalog is valid")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_companions::{EmotionVector, RelationshipDelta};

    use lettuce_memory::SOCIAL_EVENT_KINDS as EXPECTED_KINDS;

    #[test]
    fn bundled_catalog_defines_the_tool_kinds_with_moderate_values() {
        let resource: serde_json::Value =
            serde_json::from_str(SOCIAL_EVENT_CATALOG_JSON).expect("JSON");
        assert_eq!(resource["version"], 1);
        let kinds = resource["kinds"].as_array().expect("kinds");
        let names: Vec<&str> = kinds
            .iter()
            .filter_map(|kind| kind["kind"].as_str())
            .collect();
        assert_eq!(names, EXPECTED_KINDS);

        let catalog = social_event_catalog();
        for kind in EXPECTED_KINDS {
            let event = catalog.kind(kind).expect("bundled kind");
            assert!(
                emotion_values(&event.emotion)
                    .into_iter()
                    .all(|value| value.abs() <= 0.15)
            );
            assert!(
                relationship_values(&event.relationship)
                    .into_iter()
                    .all(|value| value.abs() <= 0.15)
            );
            if let Some(witness) = &event.witness_emotion {
                assert!(
                    emotion_values(witness)
                        .into_iter()
                        .all(|value| value.abs() <= 0.15)
                );
            }
        }
    }

    fn emotion_values(value: &EmotionVector) -> [f64; 10] {
        [
            value.warmth,
            value.trust,
            value.calm,
            value.vulnerability,
            value.longing,
            value.hurt,
            value.tension,
            value.irritation,
            value.affection_intensity,
            value.reassurance_need,
        ]
    }

    fn relationship_values(value: &RelationshipDelta) -> [f64; 5] {
        [
            value.closeness,
            value.trust,
            value.affection,
            value.tension,
            value.stability,
        ]
    }
}
