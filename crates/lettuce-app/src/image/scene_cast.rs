//! Who a scene image of a message depicts. A direct chat's cast is its
//! character and persona; a group chat's is the message's speaker, the
//! members who wrote or are named in the messages around it, and the persona.
//! The user may pick the cast instead.

use lettuce_characters::{
    CharacterDetails, CharacterMediaSlot, CharacterRepository, GroupRepository,
    ImageRecommendation, Persona, PersonaMediaSlot, PersonaRepository,
};
use lettuce_conversations::{
    Conversation, ConversationBackground, ConversationKind, ConversationParticipant,
    ParticipantRole, ParticipantSource, TimelineItem,
};
use lettuce_types::{AssetId, CharacterId, MessageId, PageLimit, PageRequest, PersonaId};

pub use lettuce_conversations::SceneCastSelection;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SceneCastError {
    #[error("Session character not found")]
    CharacterNotFound,
    #[error("the cast names a character that is not in the conversation")]
    ForeignCharacter,
    #[error("the cast is empty")]
    Empty,
    #[error("Message not found in loaded session window")]
    MessageNotFound,
    #[error("scene cast storage failed: {reason}")]
    Storage { reason: String },
}

fn storage(error: impl std::fmt::Display) -> SceneCastError {
    SceneCastError::Storage {
        reason: error.to_string(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CastSubjectKind {
    Character(CharacterId),
    Persona(PersonaId),
}

/// One depicted subject, read live from its character or persona.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CastSubject {
    pub(crate) kind: CastSubjectKind,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) design_notes: Option<String>,
    pub(crate) design: Vec<AssetId>,
    pub(crate) avatars: Vec<AssetId>,
    pub(crate) recommendation: Option<ImageRecommendation>,
}

impl CastSubject {
    pub(crate) const fn is_persona(&self) -> bool {
        matches!(self.kind, CastSubjectKind::Persona(_))
    }

    fn character(details: &CharacterDetails) -> Self {
        let profile = &details.character.profile;
        Self {
            kind: CastSubjectKind::Character(details.character.id),
            name: profile.name.clone(),
            description: profile
                .definition
                .as_deref()
                .or(profile.description.as_deref())
                .map(str::trim)
                .unwrap_or_default()
                .to_owned(),
            design_notes: trimmed(profile.design_description.as_deref()),
            design: character_media(details, CharacterMediaSlot::DesignReference),
            avatars: character_media(details, CharacterMediaSlot::AvatarOriginal),
            recommendation: details.character.image_recommendation.clone(),
        }
    }

    fn persona(persona: &Persona) -> Self {
        let mut design = persona
            .media
            .links
            .iter()
            .filter(|link| link.slot == PersonaMediaSlot::DesignReference)
            .collect::<Vec<_>>();
        design.sort_by_key(|link| link.ordinal);
        Self {
            kind: CastSubjectKind::Persona(persona.id),
            name: persona_scene_name(persona),
            description: persona.description.trim().to_owned(),
            design_notes: trimmed(persona.design_description.as_deref()),
            design: design.into_iter().map(|link| link.asset_id).collect(),
            avatars: persona
                .media
                .links
                .iter()
                .filter(|link| link.slot == PersonaMediaSlot::Avatar)
                .map(|link| link.asset_id)
                .collect(),
            recommendation: persona.image_recommendation.clone(),
        }
    }
}

fn trimmed(text: Option<&str>) -> Option<String> {
    text.map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

pub(crate) fn character_media(details: &CharacterDetails, slot: CharacterMediaSlot) -> Vec<AssetId> {
    let mut links = details
        .character
        .media
        .links
        .iter()
        .filter(|link| link.slot == slot)
        .collect::<Vec<_>>();
    links.sort_by_key(|link| link.ordinal);
    links.into_iter().map(|link| link.asset_id).collect()
}

/// The persona's nickname, else its title.
pub(crate) fn persona_scene_name(persona: &Persona) -> String {
    persona
        .nickname
        .as_deref()
        .filter(|nickname| !nickname.trim().is_empty())
        .unwrap_or(&persona.title)
        .to_owned()
}

/// The subjects of one scene image, in prompt order (characters first, the
/// persona last), with the chat background and the character the message
/// speaks as.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SceneCast {
    pub(crate) subjects: Vec<CastSubject>,
    pub(crate) speaker: Option<CharacterId>,
    pub(crate) background: Option<AssetId>,
    pub(crate) group: bool,
}

impl SceneCast {
    pub(crate) fn characters(&self) -> impl Iterator<Item = &CastSubject> {
        self.subjects.iter().filter(|subject| !subject.is_persona())
    }

    pub(crate) fn persona(&self) -> Option<&CastSubject> {
        self.subjects.iter().find(|subject| subject.is_persona())
    }

    /// The subject old single-character prompts mean by "the character": the
    /// speaker, else the first character.
    pub(crate) fn primary(&self) -> Option<&CastSubject> {
        self.characters()
            .find(|subject| {
                self.speaker
                    .is_some_and(|speaker| subject.kind == CastSubjectKind::Character(speaker))
            })
            .or_else(|| self.characters().next())
    }
}

/// The ports a scene cast reads.
pub trait SceneCastSources:
    lettuce_conversations::ConversationReader
    + CharacterRepository
    + PersonaRepository
    + GroupRepository
{
}

impl<T> SceneCastSources for T where
    T: lettuce_conversations::ConversationReader
        + CharacterRepository
        + PersonaRepository
        + GroupRepository
        + ?Sized
{
}

/// The message and the two messages before it on the selected branch, oldest
/// first: the moment a scene image shows.
pub(crate) fn scene_moment<R>(
    repository: &R,
    conversation: &Conversation,
    message_id: MessageId,
) -> Result<Vec<TimelineItem>, SceneCastError>
where
    R: lettuce_conversations::ConversationReader + ?Sized,
{
    let mut moment = Vec::new();
    let mut cursor = None;
    loop {
        let page = repository
            .timeline_page(
                conversation.id,
                conversation.active_branch_id,
                &PageRequest {
                    cursor,
                    limit: PageLimit::new(200),
                },
            )
            .map_err(storage)?;
        for item in page.items {
            if moment.is_empty() && item.message.id != message_id {
                continue;
            }
            moment.push(item);
            if moment.len() == 3 {
                moment.reverse();
                return Ok(moment);
            }
        }
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None if moment.is_empty() => return Err(SceneCastError::MessageNotFound),
            None => {
                moment.reverse();
                return Ok(moment);
            }
        }
    }
}

/// The participant's character when it is an enabled character member.
fn member_character(participant: &ConversationParticipant) -> Option<CharacterId> {
    match (participant.enabled, participant.role, participant.source) {
        (true, ParticipantRole::Character, ParticipantSource::Character(id)) => Some(id),
        _ => None,
    }
}

/// Whether `name` appears in `text` as a whole word, ignoring case.
fn names(text: &str, name: &str) -> bool {
    let name = name.trim().to_lowercase();
    if name.is_empty() {
        return false;
    }
    let text = text.to_lowercase();
    text.match_indices(&name).any(|(start, _)| {
        let before = text[..start].chars().next_back();
        let after = text[start + name.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

pub(crate) fn item_text(item: &TimelineItem) -> String {
    let parts = item
        .active_candidate
        .as_ref()
        .map(|candidate| candidate.parts.as_slice())
        .or_else(|| {
            item.active_revision
                .as_ref()
                .map(|revision| revision.parts.as_slice())
        })
        .unwrap_or_default();
    parts
        .iter()
        .filter_map(|part| match part {
            lettuce_conversations::MessagePart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The character that wrote `item`, when a member did.
pub(crate) fn author_character(
    conversation: &Conversation,
    item: &TimelineItem,
) -> Option<CharacterId> {
    match &conversation.kind {
        ConversationKind::Direct(details) => (item.message.role
            == lettuce_conversations::MessageRole::Assistant)
            .then_some(details.character.source_id),
        ConversationKind::Group(_) => {
            let author = item.message.author_participant_id?;
            conversation
                .participants
                .iter()
                .find(|participant| participant.id == author)
                .and_then(|participant| match participant.source {
                    ParticipantSource::Character(id) => Some(id),
                    _ => None,
                })
        }
    }
}

/// The cast of a scene image of `message_id`: `selection` when the user
/// picked one, else the rule.
pub(crate) fn scene_cast<R: SceneCastSources + ?Sized>(
    repository: &R,
    conversation: &Conversation,
    message_id: MessageId,
    selection: Option<&SceneCastSelection>,
) -> Result<SceneCast, SceneCastError> {
    let moment = scene_moment(repository, conversation, message_id)?;
    let target = moment.last().ok_or(SceneCastError::MessageNotFound)?;
    let speaker = author_character(conversation, target);
    let live_group =
        crate::generation::live_sources::live_group(repository, conversation).map_err(storage)?;
    let profile = live_group.as_ref().and_then(|group| group.profile.as_ref());
    let persona = crate::generation::live_sources::live_persona(repository, conversation, profile)
        .map_err(storage)?;
    let load = |id: CharacterId| CharacterRepository::get(repository, id).map_err(storage);
    let (characters, background) = match &conversation.kind {
        ConversationKind::Direct(details) => {
            let character = load(details.character.source_id)?
                .ok_or(SceneCastError::CharacterNotFound)?;
            if let Some(selection) = selection
                && selection
                    .character_ids
                    .iter()
                    .any(|id| *id != details.character.source_id)
            {
                return Err(SceneCastError::ForeignCharacter);
            }
            let include = selection.is_none_or(|selection| !selection.character_ids.is_empty());
            let background = direct_background(conversation, &character);
            (
                if include { vec![character] } else { Vec::new() },
                background,
            )
        }
        ConversationKind::Group(_) => {
            let mut members = conversation
                .participants
                .iter()
                .filter_map(|participant| {
                    member_character(participant).map(|id| (participant, id))
                })
                .collect::<Vec<_>>();
            members.sort_by_key(|(participant, _)| participant.ordinal);
            let mut chosen = Vec::new();
            match selection {
                Some(selection) => {
                    if selection
                        .character_ids
                        .iter()
                        .any(|id| !members.iter().any(|(_, member)| member == id))
                    {
                        return Err(SceneCastError::ForeignCharacter);
                    }
                    for (_, id) in &members {
                        if selection.character_ids.contains(id) {
                            if let Some(character) = load(*id)? {
                                chosen.push(character);
                            }
                        }
                    }
                }
                None => {
                    let authors = moment
                        .iter()
                        .filter_map(|item| author_character(conversation, item))
                        .collect::<Vec<_>>();
                    let text = moment.iter().map(item_text).collect::<Vec<_>>().join("\n");
                    for (participant, id) in &members {
                        let Some(character) = load(*id)? else {
                            continue;
                        };
                        if authors.contains(id)
                            || names(&text, &character.character.profile.name)
                            || names(&text, &participant.display_name)
                        {
                            chosen.push(character);
                        }
                    }
                }
            }
            if let Some(speaker) = speaker
                && let Some(index) = chosen
                    .iter()
                    .position(|character| character.character.id == speaker)
            {
                let first = chosen.remove(index);
                chosen.insert(0, first);
            }
            let background = match conversation
                .current_settings
                .as_ref()
                .and_then(|settings| settings.background)
            {
                Some(ConversationBackground::Image { asset_id }) => Some(asset_id),
                Some(ConversationBackground::Hidden) => None,
                None => profile.and_then(|profile| profile.background_asset_id),
            };
            (chosen, background)
        }
    };
    let include_persona = selection.is_none_or(|selection| selection.include_persona);
    let subjects = characters
        .iter()
        .map(CastSubject::character)
        .chain(
            persona
                .as_ref()
                .filter(|_| include_persona)
                .map(CastSubject::persona),
        )
        .collect::<Vec<_>>();
    if subjects.is_empty() {
        return Err(SceneCastError::Empty);
    }
    Ok(SceneCast {
        subjects,
        speaker,
        background,
        group: matches!(conversation.kind, ConversationKind::Group(_)),
    })
}

/// Every subject a reply of the conversation may depict, for the chat
/// prompt's scene protocol: the direct chat's character or every enabled
/// group member, then the persona. A subject that cannot be read is left out.
pub(crate) fn chat_cast<R: SceneCastSources + ?Sized>(
    repository: &R,
    conversation: &Conversation,
) -> Result<Vec<CastSubject>, SceneCastError> {
    let live_group =
        crate::generation::live_sources::live_group(repository, conversation).map_err(storage)?;
    let profile = live_group.as_ref().and_then(|group| group.profile.as_ref());
    let ids = match &conversation.kind {
        ConversationKind::Direct(details) => vec![details.character.source_id],
        ConversationKind::Group(_) => {
            let mut members = conversation
                .participants
                .iter()
                .filter_map(|participant| {
                    member_character(participant).map(|id| (participant.ordinal, id))
                })
                .collect::<Vec<_>>();
            members.sort_by_key(|(ordinal, _)| *ordinal);
            members.into_iter().map(|(_, id)| id).collect()
        }
    };
    let mut subjects = Vec::new();
    for id in ids {
        if let Some(character) = CharacterRepository::get(repository, id).map_err(storage)? {
            subjects.push(CastSubject::character(&character));
        }
    }
    if let Some(persona) =
        crate::generation::live_sources::live_persona(repository, conversation, profile)
            .map_err(storage)?
    {
        subjects.push(CastSubject::persona(&persona));
    }
    Ok(subjects)
}

/// The effective background of a one-to-one chat with `character`: the
/// conversation's own (none when it hides it), else the selected or default
/// scene's, else the character's.
pub(crate) fn direct_background(
    conversation: &Conversation,
    character: &CharacterDetails,
) -> Option<AssetId> {
    match conversation
        .current_settings
        .as_ref()
        .and_then(|settings| settings.background)
    {
        Some(ConversationBackground::Image { asset_id }) => return Some(asset_id),
        Some(ConversationBackground::Hidden) => return None,
        None => {}
    }
    let scene_id = lettuce_conversations::resolve_effective_settings(conversation, None)
        .ok()
        .and_then(|settings| settings.scene.map(|scene| scene.source_id))
        .or(character.character.defaults.default_scene_id);
    scene_id
        .and_then(|scene_id| character.scenes.iter().find(|scene| scene.id == scene_id))
        .and_then(|scene| {
            scene
                .assets
                .iter()
                .find(|link| link.slot == lettuce_characters::SceneAssetSlot::Background)
                .map(|link| link.asset_id)
        })
        .or_else(|| {
            character_media(character, CharacterMediaSlot::Background)
                .first()
                .copied()
        })
}

#[cfg(test)]
mod tests {
    use super::names;

    #[test]
    fn names_match_whole_words_only() {
        assert!(names("Then Maya smiled.", "maya"));
        assert!(names("maya", "Maya"));
        assert!(!names("Mayans", "Maya"));
        assert!(!names("Also", "Al"));
        assert!(names("Al, wait!", "Al"));
        assert!(!names("anything", " "));
    }
}
