//! The scene tag a direct chat reply carries: the reply's text is cleaned
//! when it is finalized, and the scene prompt is handed back for the caller
//! to run or ask about.

use lettuce_conversations::{ConversationKind, MessagePart, extract_scene_prompt};
use lettuce_models::ModelCatalog;
use lettuce_settings::SceneGenerationMode;

use crate::{ImageFeature, image_feature_model};

/// The scene mode of a direct chat's replies: set when scene generation is
/// on and its model resolves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReplyImageFacts {
    pub scene_mode: Option<SceneGenerationMode>,
}

/// A scene image a finalized reply asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneImageFollowUp {
    pub prompt: String,
    /// The user approves the prompt before the image is generated.
    pub ask_first: bool,
}

/// The facts for a conversation's replies; group chats had no scene tag.
pub(crate) fn reply_image_facts<C: ModelCatalog + ?Sized>(
    models: &C,
    settings: &lettuce_settings::GlobalSettings,
    conversation: &lettuce_conversations::Conversation,
) -> Option<ReplyImageFacts> {
    if !matches!(conversation.kind, ConversationKind::Direct(_)) {
        return None;
    }
    let images = &settings.image_generation;
    Some(ReplyImageFacts {
        scene_mode: (images.scene_enabled
            && image_feature_model(models, settings, ImageFeature::Scene).is_ok())
        .then_some(images.scene_mode),
    })
}

/// Removes the scene tag from a reply's text, always, and keeps its prompt
/// unless the scene mode is manual or scenes cannot run.
pub(crate) fn take_scene_image(
    parts: &mut [MessagePart],
    facts: ReplyImageFacts,
) -> Option<SceneImageFollowUp> {
    let mut prompt = None;
    for part in parts.iter_mut() {
        if let MessagePart::Text { text } = part {
            let (clean, found) = extract_scene_prompt(text);
            *text = clean;
            if prompt.is_none() {
                prompt = found;
            }
        }
    }
    let ask_first = match facts.scene_mode? {
        SceneGenerationMode::Auto => false,
        SceneGenerationMode::AskFirst => true,
        SceneGenerationMode::Manual => return None,
    };
    Some(SceneImageFollowUp {
        prompt: prompt?,
        ask_first,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_lose_the_scene_tag_and_keep_its_prompt_by_mode() {
        let run = |scene_mode| {
            let mut parts = vec![MessagePart::Text {
                text: " Hi <img>harbor</img> ".into(),
            }];
            let follow_up = take_scene_image(&mut parts, ReplyImageFacts { scene_mode });
            let MessagePart::Text { text } = &parts[0] else {
                unreachable!()
            };
            assert_eq!(text, "Hi");
            follow_up
        };
        assert_eq!(run(None), None);
        assert_eq!(run(Some(SceneGenerationMode::Manual)), None);
        assert_eq!(
            run(Some(SceneGenerationMode::Auto)),
            Some(SceneImageFollowUp {
                prompt: "harbor".into(),
                ask_first: false
            })
        );
        assert!(run(Some(SceneGenerationMode::AskFirst)).is_some_and(|scene| scene.ask_first));
    }
}
