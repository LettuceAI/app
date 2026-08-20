use lettuce_characters::{
    Character, CharacterDefaults, ConversationStarter, InteractionMode, MemoryPolicy, Scene,
    ScenePart, SceneVariant, Selection,
};
use lettuce_context::LorebookBinding;
use lettuce_conversations::{
    MemoryModeSnapshot, ModelProviderKind, SnapshotSelection, SnapshotSource,
};
use lettuce_models::ProviderProtocol;
use lettuce_types::{
    CharacterId, ConversationStarterId, LorebookId, ModelProfileId, PersonaId, Revision, SceneId,
};

use super::request::LaunchSelection;

const MAX_SCENE_TITLE_CHARS: usize = 96;

pub(crate) const MAX_LAUNCH_LOREBOOKS: usize = 128;
pub(crate) const MAX_LAUNCH_TIMELINE_ENTRIES: usize = 512;

pub(crate) const fn lorebook_bound_exceeded(count: usize) -> bool {
    count > MAX_LAUNCH_LOREBOOKS
}

pub(crate) const fn timeline_bound_exceeded(count: usize) -> bool {
    count > MAX_LAUNCH_TIMELINE_ENTRIES
}

/// A resolved slot together with how it was chosen, before it is turned into a
/// launch snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Selected<T> {
    Inherited(T),
    Explicit(T),
    Disabled,
}

impl<T> Selected<T> {
    pub(crate) const fn value(&self) -> Option<&T> {
        match self {
            Self::Inherited(value) | Self::Explicit(value) => Some(value),
            Self::Disabled => None,
        }
    }

    pub(crate) const fn as_ref(&self) -> Selected<&T> {
        match self {
            Self::Inherited(value) => Selected::Inherited(value),
            Self::Explicit(value) => Selected::Explicit(value),
            Self::Disabled => Selected::Disabled,
        }
    }

    pub(crate) fn map<U>(self, transform: impl FnOnce(T) -> U) -> Selected<U> {
        match self {
            Self::Inherited(value) => Selected::Inherited(transform(value)),
            Self::Explicit(value) => Selected::Explicit(transform(value)),
            Self::Disabled => Selected::Disabled,
        }
    }

    pub(crate) fn with<U>(&self, value: U) -> Selected<U> {
        match self {
            Self::Inherited(_) => Selected::Inherited(value),
            Self::Explicit(_) => Selected::Explicit(value),
            Self::Disabled => Selected::Disabled,
        }
    }

    pub(crate) fn into_snapshot(self) -> SnapshotSelection<T> {
        match self {
            Self::Inherited(value) => SnapshotSelection::Inherited(value),
            Self::Explicit(value) => SnapshotSelection::Explicit(value),
            Self::Disabled => SnapshotSelection::Disabled,
        }
    }
}

pub(crate) fn character_display_name(character: &Character) -> String {
    character
        .profile
        .nickname
        .clone()
        .unwrap_or_else(|| character.profile.name.clone())
}

pub(crate) const fn is_companion(defaults: &CharacterDefaults) -> bool {
    matches!(defaults.interaction_mode, InteractionMode::Companion)
}

pub(crate) const fn memory_mode(defaults: &CharacterDefaults) -> MemoryModeSnapshot {
    match defaults.memory_policy {
        MemoryPolicy::Manual => MemoryModeSnapshot::Manual,
        MemoryPolicy::Dynamic => MemoryModeSnapshot::Dynamic,
    }
}

/// A starter only ever enters a launch when the caller names one. The
/// character's default starter is a picker suggestion, not an inherited slot.
pub(crate) fn starter_choice(
    request: LaunchSelection<ConversationStarterId>,
) -> Selected<ConversationStarterId> {
    match request {
        LaunchSelection::Explicit(id) => Selected::Explicit(id),
        LaunchSelection::Inherit | LaunchSelection::Disabled => Selected::Disabled,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SceneChoice {
    Explicit(SceneId),
    Inherited(SceneId),
    Inherit,
    None,
}

/// A resolved starter always wins over the requested and default scene: it
/// either points at its own scene or clears the slot entirely. The scene keeps
/// the starter's own provenance.
pub(crate) fn scene_choice(
    request: LaunchSelection<SceneId>,
    starter_scene: Selected<Option<SceneId>>,
) -> SceneChoice {
    match starter_scene {
        Selected::Explicit(Some(id)) => return SceneChoice::Explicit(id),
        Selected::Inherited(Some(id)) => return SceneChoice::Inherited(id),
        Selected::Explicit(None) | Selected::Inherited(None) => return SceneChoice::None,
        Selected::Disabled => {}
    }
    match request {
        LaunchSelection::Explicit(id) => SceneChoice::Explicit(id),
        LaunchSelection::Inherit => SceneChoice::Inherit,
        LaunchSelection::Disabled => SceneChoice::None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InheritedScene<'a> {
    Resolved(&'a Scene),
    Dangling(SceneId),
    None,
}

/// Two inheritance tiers: the character's chosen default, then its
/// lowest-ordinal active scene. A default that has since been archived leaves
/// the launch without a scene rather than falling through to another one.
pub(crate) fn inherited_scene(
    scenes: &[Scene],
    default_scene_id: Option<SceneId>,
) -> InheritedScene<'_> {
    if let Some(id) = default_scene_id {
        return match find_any_scene(scenes, id) {
            None => InheritedScene::Dangling(id),
            Some(scene) if scene.status == lettuce_characters::LifecycleStatus::Active => {
                InheritedScene::Resolved(scene)
            }
            Some(_) => InheritedScene::None,
        };
    }
    scenes
        .iter()
        .filter(|scene| scene.status == lettuce_characters::LifecycleStatus::Active)
        .min_by_key(|scene| scene.ordinal)
        .map_or(InheritedScene::None, InheritedScene::Resolved)
}

pub(crate) fn model_choice(
    defaults: &CharacterDefaults,
    application_default: Option<ModelProfileId>,
) -> Selected<ModelProfileId> {
    match defaults.model_profile_id.or(application_default) {
        Some(id) => Selected::Inherited(id),
        None => Selected::Disabled,
    }
}

pub(crate) fn lorebook_choice(
    starter: Selected<&Selection<Vec<LorebookId>>>,
    character_bindings: &[LorebookBinding],
    persona_bindings: &[LorebookBinding],
) -> Selected<Vec<LorebookId>> {
    match starter {
        Selected::Inherited(Selection::Explicit(ids))
        | Selected::Explicit(Selection::Explicit(ids)) => Selected::Explicit(dedup(ids.clone())),
        Selected::Inherited(Selection::Disabled) | Selected::Explicit(Selection::Disabled) => {
            Selected::Disabled
        }
        _ => Selected::Inherited(inherited_lorebooks(character_bindings, persona_bindings)),
    }
}

pub(crate) fn inherited_lorebooks(
    character_bindings: &[LorebookBinding],
    persona_bindings: &[LorebookBinding],
) -> Vec<LorebookId> {
    dedup(
        enabled_lorebooks(character_bindings)
            .into_iter()
            .chain(enabled_lorebooks(persona_bindings))
            .collect(),
    )
}

pub(crate) fn enabled_lorebooks(bindings: &[LorebookBinding]) -> Vec<LorebookId> {
    let mut ordered: Vec<&LorebookBinding> =
        bindings.iter().filter(|binding| binding.enabled).collect();
    ordered.sort_by_key(|binding| binding.ordinal);
    ordered.iter().map(|binding| binding.lorebook_id).collect()
}

fn dedup(ids: Vec<LorebookId>) -> Vec<LorebookId> {
    let mut seen = std::collections::HashSet::new();
    ids.into_iter().filter(|id| seen.insert(*id)).collect()
}

pub(crate) fn project_scene_text(parts: &[ScenePart]) -> String {
    parts
        .iter()
        .filter_map(|part| match part {
            ScenePart::Text { text } => Some(text.as_str()),
            ScenePart::InlineAsset { .. } => None,
        })
        .collect()
}

/// Legacy scene text precedence: selected variant, then the base document,
/// then the direction. A selected variant that no longer belongs to this scene
/// falls back to the base document rather than blanking the scene.
pub(crate) fn resolve_scene_text(scene: &Scene, variants: &[SceneVariant]) -> Option<String> {
    let variant_text = scene
        .selected_variant_id
        .and_then(|id| {
            variants
                .iter()
                .find(|variant| variant.id == id && variant.scene_id == scene.id)
        })
        .map(|variant| project_scene_text(&variant.content.parts));
    let base_text = project_scene_text(&scene.content.parts);
    [
        variant_text.as_deref(),
        Some(base_text.as_str()),
        scene.direction.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .find(|value| !value.is_empty())
    .map(str::to_owned)
}

pub(crate) fn scene_title(text: Option<&str>, ordinal: u32) -> String {
    let derived: String = text
        .unwrap_or_default()
        .trim()
        .chars()
        .take(MAX_SCENE_TITLE_CHARS)
        .collect();
    if derived.trim().is_empty() {
        format!("Scene {}", ordinal.saturating_add(1))
    } else {
        derived
    }
}

pub(crate) const fn provider_kind(protocol: ProviderProtocol) -> ModelProviderKind {
    match protocol {
        ProviderProtocol::OpenAiCompatible => ModelProviderKind::OpenAiCompatible,
        ProviderProtocol::Anthropic => ModelProviderKind::Anthropic,
        ProviderProtocol::Gemini => ModelProviderKind::Gemini,
        ProviderProtocol::Ollama => ModelProviderKind::Ollama,
        ProviderProtocol::LlamaCpp => ModelProviderKind::LlamaCpp,
        ProviderProtocol::StableDiffusion => ModelProviderKind::Other,
    }
}

pub(crate) fn find_scene(scenes: &[Scene], id: SceneId) -> Option<&Scene> {
    scenes
        .iter()
        .find(|scene| scene.id == id && scene.status == lettuce_characters::LifecycleStatus::Active)
}

pub(crate) fn find_any_scene(scenes: &[Scene], id: SceneId) -> Option<&Scene> {
    scenes.iter().find(|scene| scene.id == id)
}

/// Detects an authored source that moved between the first read and the
/// re-read taken after the binding queries, so a torn launch is retried
/// instead of snapshotting two different revisions of the same graph.
pub(crate) fn detect_source_drift(
    character: (CharacterId, Revision, Revision),
    persona: Option<(PersonaId, Revision, Revision)>,
) -> Option<SnapshotSource> {
    let (character_id, before, after) = character;
    if before != after {
        return Some(SnapshotSource::Character(character_id));
    }
    if let Some((persona_id, before, after)) = persona
        && before != after
    {
        return Some(SnapshotSource::Persona(persona_id));
    }
    None
}

pub(crate) fn find_starter(
    starters: &[ConversationStarter],
    id: ConversationStarterId,
) -> Option<&ConversationStarter> {
    starters.iter().find(|starter| starter.id == id)
}

/// Registers every lorebook a launch needs exactly once so a book reachable
/// from both the conversation and its persona still produces one artifact.
#[derive(Debug, Default)]
pub(crate) struct LorebookRegistry {
    ordered: Vec<LorebookId>,
}

impl LorebookRegistry {
    pub(crate) fn register(&mut self, id: LorebookId) -> usize {
        match self.ordered.iter().position(|value| *value == id) {
            Some(index) => index,
            None => {
                self.ordered.push(id);
                self.ordered.len() - 1
            }
        }
    }

    pub(crate) fn ordered(&self) -> &[LorebookId] {
        &self.ordered
    }
}
