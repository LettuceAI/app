//! The LoRAs a local scene image uses: the character's and persona's image
//! recommendations, with trigger keywords from the LoRA library.

use lettuce_characters::ImageRecommendation;
use lettuce_conversations::SceneLoraBinding;
use lettuce_image_generation::sd_runtime::lora_library::{
    LoraLibraryRepository, hydrate_lora_keywords,
};
use lettuce_models::StableDiffusionLora;

/// A subject's LoRA. Only an `unresolved_legacy_name` is a
/// stable-diffusion.cpp path; an artifact recommendation has no resolvable
/// file yet.
pub(crate) fn recommendation_lora<R: LoraLibraryRepository + ?Sized>(
    library: &R,
    recommendation: Option<&ImageRecommendation>,
) -> Option<StableDiffusionLora> {
    let recommendation = recommendation?;
    let path = recommendation
        .unresolved_legacy_name
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())?;
    let mut lora = StableDiffusionLora {
        path: path.to_owned(),
        multiplier: recommendation
            .strength
            .to_string()
            .parse()
            .unwrap_or_else(|_| f64::from(recommendation.strength)),
        is_high_noise: false,
        keywords: Vec::new(),
    };
    if let Err(error) = hydrate_lora_keywords(library, std::slice::from_mut(&mut lora)) {
        tracing::warn!(?error, "Failed to hydrate scene LoRA keywords");
    }
    Some(lora)
}

/// The character's LoRA and, when the chat's persona still exists, the
/// persona's (`Some(None)` for a persona without one). A subject that cannot
/// be read has no LoRA; the binding never fails the turn.
pub(crate) fn subject_loras<R>(
    repository: &R,
    character_id: lettuce_types::CharacterId,
    persona_id: Option<lettuce_types::PersonaId>,
) -> (
    Option<StableDiffusionLora>,
    Option<Option<StableDiffusionLora>>,
)
where
    R: lettuce_characters::CharacterRepository
        + lettuce_characters::PersonaRepository
        + LoraLibraryRepository
        + ?Sized,
{
    let character = lettuce_characters::CharacterRepository::get(repository, character_id)
        .inspect_err(|error| tracing::warn!(?error, "Failed to load the scene character"))
        .ok()
        .flatten();
    let persona = persona_id.and_then(|persona_id| {
        lettuce_characters::PersonaRepository::get(repository, persona_id)
            .inspect_err(|error| tracing::warn!(?error, "Failed to load the scene persona"))
            .ok()
            .flatten()
    });
    (
        recommendation_lora(
            repository,
            character
                .as_ref()
                .and_then(|details| details.character.image_recommendation.as_ref()),
        ),
        persona
            .map(|persona| recommendation_lora(repository, persona.image_recommendation.as_ref())),
    )
}

/// How a subject is named in the scene protocol, from its LoRA.
pub(crate) fn subject_binding(lora: Option<&StableDiffusionLora>) -> SceneLoraBinding {
    lora.map_or(SceneLoraBinding::NoLora, |lora| {
        SceneLoraBinding::Keywords(lora.keywords.join(", "))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lora(keywords: &[&str]) -> StableDiffusionLora {
        StableDiffusionLora {
            path: "mira.safetensors".into(),
            multiplier: 0.8,
            is_high_noise: false,
            keywords: keywords
                .iter()
                .map(|keyword| (*keyword).to_owned())
                .collect(),
        }
    }

    #[test]
    fn subject_bindings_follow_legacy_scene_loras() {
        assert_eq!(subject_binding(None), SceneLoraBinding::NoLora);
        assert_eq!(
            subject_binding(Some(&lora(&[]))),
            SceneLoraBinding::Keywords(String::new())
        );
        assert_eq!(
            subject_binding(Some(&lora(&["mira", "red coat"]))),
            SceneLoraBinding::Keywords("mira, red coat".into())
        );
    }
}
