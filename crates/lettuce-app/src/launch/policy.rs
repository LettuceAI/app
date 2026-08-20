use lettuce_characters::{
    Character, CharacterDefaults, ChatMode, ConversationStarter, GroupMember, InteractionMode,
    MemoryPolicy, Scene, ScenePart, SceneVariant, Selection, SpeakerSelection,
};
use lettuce_context::LorebookBinding;
use lettuce_conversations::{
    GroupChatModeSnapshot, GroupSpeakerSelectionSnapshot, MemoryModeSnapshot, ModelProviderKind,
    SnapshotSelection, SnapshotSource,
};
use lettuce_models::ProviderProtocol;
use lettuce_types::{
    CharacterId, ConversationStarterId, GroupId, LorebookId, ModelProfileId, PersonaId,
    PromptDocumentId, Revision, SceneId,
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
    memory_mode_of(defaults.memory_policy)
}

pub(crate) const fn memory_mode_of(policy: MemoryPolicy) -> MemoryModeSnapshot {
    match policy {
        MemoryPolicy::Manual => MemoryModeSnapshot::Manual,
        MemoryPolicy::Dynamic => MemoryModeSnapshot::Dynamic,
    }
}

pub(crate) const fn group_chat_mode(mode: ChatMode) -> GroupChatModeSnapshot {
    match mode {
        ChatMode::Conversation => GroupChatModeSnapshot::Conversation,
        ChatMode::Roleplay => GroupChatModeSnapshot::Roleplay,
    }
}

pub(crate) const fn group_speaker_selection(
    selection: SpeakerSelection,
) -> GroupSpeakerSelectionSnapshot {
    match selection {
        SpeakerSelection::Llm => GroupSpeakerSelectionSnapshot::Llm,
        SpeakerSelection::Heuristic => GroupSpeakerSelectionSnapshot::Heuristic,
        SpeakerSelection::RoundRobin => GroupSpeakerSelectionSnapshot::RoundRobin,
        SpeakerSelection::Director => GroupSpeakerSelectionSnapshot::Director,
        SpeakerSelection::DirectorAction => GroupSpeakerSelectionSnapshot::DirectorAction,
    }
}

/// Authored member ordinals only have to be an order; the launch document
/// requires positions. Sorting first keeps the authored order, reindexing
/// closes any gap a reorder left behind.
pub(crate) fn ordered_members(members: &[GroupMember]) -> Vec<GroupMember> {
    let mut ordered = members.to_vec();
    ordered.sort_by_key(|member| member.ordinal);
    for (index, member) in ordered.iter_mut().enumerate() {
        member.ordinal = u32::try_from(index).unwrap_or(u32::MAX);
    }
    ordered
}

pub(crate) const MIN_GROUP_MEMBERS: usize = 2;

/// One participant slot always belongs to the user, so a cast can only fill
/// the rest of the conversation's participant bound.
pub(crate) const MAX_GROUP_MEMBERS: usize = 255;

/// The launch document rejects these shapes as well, but a caller deserves a
/// typed reason before the planner starts drafting snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MemberShape {
    Launchable,
    TooFew,
    TooMany,
    AllMuted,
}

pub(crate) fn member_shape(members: &[GroupMember]) -> MemberShape {
    if members.len() < MIN_GROUP_MEMBERS {
        return MemberShape::TooFew;
    }
    if members.len() > MAX_GROUP_MEMBERS {
        return MemberShape::TooMany;
    }
    if members.iter().all(|member| member.muted) {
        return MemberShape::AllMuted;
    }
    MemberShape::Launchable
}

/// Legacy naming for an untitled cast: up to three names in member order,
/// then the first two followed by how many members were left out.
pub(crate) fn derive_group_title(names: &[String], max_bytes: usize) -> String {
    let derived = if names.len() <= 3 {
        names.join(", ")
    } else {
        format!(
            "{}, {} & {} others",
            names[0],
            names[1],
            names.len().saturating_sub(2)
        )
    };
    bound_display(&derived, max_bytes)
}

fn bound_display(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut bounded = String::with_capacity(max_bytes);
    for character in value.chars() {
        if bounded.len() + character.len_utf8() > max_bytes {
            break;
        }
        bounded.push(character);
    }
    bounded
}

/// A member's own override wins, then the character's default model. The
/// application default is a group-level fallback, never a member one.
pub(crate) fn member_model_choice(
    member: &GroupMember,
    defaults: &CharacterDefaults,
) -> Selected<ModelProfileId> {
    match member.model_profile_override {
        Some(id) => Selected::Explicit(id),
        None => match defaults.model_profile_id {
            Some(id) => Selected::Inherited(id),
            None => Selected::Disabled,
        },
    }
}

/// The application default only enters a group launch when at least one
/// member would otherwise be left without a model to generate with.
pub(crate) fn group_model_needed(choices: &[Selected<ModelProfileId>]) -> bool {
    choices
        .iter()
        .any(|choice| matches!(choice, Selected::Disabled))
}

pub(crate) fn application_model_choice(
    application_default: Option<ModelProfileId>,
) -> Selected<ModelProfileId> {
    match application_default {
        Some(id) => Selected::Inherited(id),
        None => Selected::Disabled,
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

/// Legacy direct scene text precedence: selected variant, then the base
/// document, then the direction. A selected variant that no longer belongs to
/// this scene falls back to the base document rather than blanking the scene.
pub(crate) fn resolve_scene_text(scene: &Scene, variants: &[SceneVariant]) -> Option<String> {
    scene_text(scene, variants, true)
}

/// A group scene only ever spoke its content. The direction stays authored
/// guidance for the prompt and never becomes the opening message.
pub(crate) fn resolve_group_scene_text(scene: &Scene, variants: &[SceneVariant]) -> Option<String> {
    scene_text(scene, variants, false)
}

fn scene_text(scene: &Scene, variants: &[SceneVariant], with_direction: bool) -> Option<String> {
    let variant_text = scene
        .selected_variant_id
        .and_then(|id| {
            variants
                .iter()
                .find(|variant| variant.id == id && variant.scene_id == scene.id)
        })
        .map(|variant| project_scene_text(&variant.content.parts));
    let base_text = project_scene_text(&scene.content.parts);
    let direction = with_direction
        .then_some(scene.direction.as_deref())
        .flatten();
    [variant_text.as_deref(), Some(base_text.as_str()), direction]
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

/// The group counterpart of [`detect_source_drift`]. Member characters are
/// re-read too: their bodies are copied before the per-member binding queries,
/// so an edit in between would pin a body and a binding list from two
/// different revisions of the same character.
pub(crate) fn detect_group_source_drift(
    group: (GroupId, Revision, Revision),
    members: &[(CharacterId, Revision, Revision)],
    persona: Option<(PersonaId, Revision, Revision)>,
) -> Option<SnapshotSource> {
    let (group_id, before, after) = group;
    if before != after {
        return Some(SnapshotSource::Group(group_id));
    }
    for (character_id, before, after) in members {
        if before != after {
            return Some(SnapshotSource::Character(*character_id));
        }
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

/// Registers every source a launch needs exactly once so a record reachable
/// from more than one slot still produces a single artifact.
#[derive(Debug)]
pub(crate) struct LaunchRegistry<T> {
    ordered: Vec<T>,
}

impl<T> Default for LaunchRegistry<T> {
    fn default() -> Self {
        Self {
            ordered: Vec::new(),
        }
    }
}

impl<T: Copy + PartialEq> LaunchRegistry<T> {
    pub(crate) fn register(&mut self, id: T) -> usize {
        match self.ordered.iter().position(|value| *value == id) {
            Some(index) => index,
            None => {
                self.ordered.push(id);
                self.ordered.len() - 1
            }
        }
    }

    pub(crate) fn ordered(&self) -> &[T] {
        &self.ordered
    }
}

pub(crate) type LorebookRegistry = LaunchRegistry<LorebookId>;
pub(crate) type ModelRegistry = LaunchRegistry<ModelProfileId>;
pub(crate) type PromptRegistry = LaunchRegistry<PromptDocumentId>;
