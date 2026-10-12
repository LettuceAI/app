use std::collections::BTreeMap;

use lettuce_characters::*;
use lettuce_context::{CharacterLorebookBindingRepository, LorebookBinding};
use lettuce_transfer::uec3 as wire;
use lettuce_types::*;

use super::*;

fn ordinal(index: usize) -> Result<u32, ApiError> {
    u32::try_from(index).map_err(format_error)
}

fn reference<T: Copy>(values: &BTreeMap<String, T>, key: &str) -> Result<T, ApiError> {
    values
        .get(key)
        .copied()
        .ok_or_else(|| format_error(format!("missing reference: {key}")))
}

pub(super) fn logical_asset(
    assets: &BTreeMap<String, AssetId>,
    uploads: &mut Vec<lettuce_media::IngestedMedia>,
    value: &str,
    preview: bool,
) -> Result<AssetId, ApiError> {
    let id = uec3_asset(assets, value)?;
    let fresh = AssetId::new();
    if !preview {
        let mut upload = uploads
            .iter()
            .find(|upload| upload.asset.id == id)
            .cloned()
            .ok_or_else(|| internal("missing staged asset"))?;
        upload.asset.id = fresh;
        uploads.push(upload);
    }
    Ok(fresh)
}

fn matching_character(context: &ApiContext, name: &str) -> Result<Option<CharacterId>, ApiError> {
    let mut cursor = None;
    let mut matches = Vec::new();
    loop {
        let page = CharacterRepository::search(
            context.backend().database(),
            CharacterSearch {
                text: name.into(),
                include_archived: true,
                ..Default::default()
            },
            PageRequest {
                cursor,
                limit: PageLimit::new(200),
            },
        )
        .map_err(IntoApiError::into_api_error)?;
        matches.extend(
            page.items
                .into_iter()
                .filter(|character| character.profile.name == name)
                .map(|character| character.id),
        );
        let Some(next) = page.next_cursor else {
            break;
        };
        cursor = Some(next);
    }
    Ok(if matches.len() == 1 {
        matches.first().copied()
    } else {
        None
    })
}

pub(in crate::api) struct ImportAssets<'a> {
    pub ids: &'a BTreeMap<String, AssetId>,
    pub uploads: &'a mut Vec<lettuce_media::IngestedMedia>,
    pub preview: bool,
}

pub(in crate::api) fn prepare(
    context: &ApiContext,
    document: &wire::Document,
    staged: ImportAssets<'_>,
    warnings: &mut Vec<dto::FileWarning>,
    now: TimestampMillis,
    books: &BTreeMap<String, lettuce_context::LorebookDetails>,
    prompts: &BTreeMap<String, lettuce_context::PromptDocument>,
) -> Result<lettuce_transfer::CharacterFileImport, ApiError> {
    let ImportAssets {
        ids: assets,
        uploads,
        preview,
    } = staged;
    let wire::Entity::Character(card) = &document.entity else {
        return Err(format_error("expected character entity"));
    };
    let id = CharacterId::new();
    let mut scene_ids = BTreeMap::new();
    for scene in &card.scenes {
        if scene.key.is_empty()
            || scene_ids
                .insert(scene.key.clone(), SceneId::new())
                .is_some()
        {
            return Err(format_error("duplicate or empty scene key"));
        }
    }
    let mut starter_ids = BTreeMap::new();
    for starter in &card.starters {
        if starter.template.key.is_empty()
            || starter_ids
                .insert(starter.template.key.clone(), ConversationStarterId::new())
                .is_some()
        {
            return Err(format_error("duplicate or empty starter key"));
        }
    }
    let mut scenes = Vec::new();
    let mut variants = Vec::new();
    for (index, source) in card.scenes.iter().enumerate() {
        let scene_id = reference(&scene_ids, &source.key)?;
        let mut links = Vec::new();
        let mut parts = |parts: &[wire::ScenePart], version| -> Result<SceneDocumentV1, ApiError> {
            let mut output = Vec::new();
            for part in parts {
                output.push(match part {
                    wire::ScenePart::Text { text } => ScenePart::Text { text: text.clone() },
                    wire::ScenePart::Image { image } => {
                        let link_id = SceneAssetLinkId::new();
                        links.push(SceneAssetLink {
                            id: link_id,
                            asset_id: logical_asset(assets, uploads, image, preview)?,
                            slot: SceneAssetSlot::Inline,
                            ordinal: ordinal(links.len())?,
                        });
                        ScenePart::InlineAsset { link_id }
                    }
                });
            }
            let value = SceneDocumentV1 {
                format_version: version,
                parts: output,
            };
            value.validate().map_err(format_error)?;
            Ok(value)
        };
        let content = parts(&source.parts, source.format_version)?;
        let mut variant_ids = BTreeMap::new();
        for (index, variant) in source.variants.iter().enumerate() {
            let variant_id = SceneVariantId::new();
            if variant.key.is_empty()
                || variant_ids
                    .insert(variant.key.clone(), variant_id)
                    .is_some()
            {
                return Err(format_error("duplicate or empty variant key"));
            }
            variants.push(SceneVariant {
                id: variant_id,
                scene_id,
                ordinal: ordinal(index)?,
                content: parts(&variant.parts, variant.format_version)?,
                direction: variant.direction.clone(),
                revision: Revision::INITIAL,
                created_at: now,
                updated_at: now,
            });
        }
        if let Some(background) = &source.background {
            links.push(SceneAssetLink {
                id: SceneAssetLinkId::new(),
                asset_id: logical_asset(assets, uploads, background, preview)?,
                slot: SceneAssetSlot::Background,
                ordinal: 0,
            });
        }
        scenes.push(Scene {
            id: scene_id,
            owner: SceneOwner::Character(id),
            status: LifecycleStatus::Active,
            ordinal: ordinal(index)?,
            content,
            direction: source.direction.clone(),
            selected_variant_id: source
                .selected_variant
                .as_deref()
                .map(|key| reference(&variant_ids, key))
                .transpose()?,
            assets: links,
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        });
    }
    let mut media = CharacterMedia::default();
    for (source, slot) in [
        (&card.media.avatar, CharacterMediaSlot::AvatarOriginal),
        (&card.media.banner, CharacterMediaSlot::Banner),
        (&card.media.background, CharacterMediaSlot::Background),
    ] {
        if let Some(source) = source {
            media.links.push(CharacterMediaLink {
                asset_id: logical_asset(assets, uploads, source, preview)?,
                slot,
                ordinal: 0,
            });
        }
    }
    for (index, source) in card.design.references.iter().enumerate() {
        media.links.push(CharacterMediaLink {
            asset_id: logical_asset(assets, uploads, source, preview)?,
            slot: CharacterMediaSlot::DesignReference,
            ordinal: ordinal(index)?,
        });
    }
    let chat = &card.appearance.chat;
    let mut asset =
        |value: &str| uec3_asset(assets, value).map_err(|error| wire::Error::Format(error.message));
    let mut character = |value: &str| {
        if value == "self" {
            return Ok(Some(id));
        }
        let matched = matching_character(context, value)
            .map_err(|error| wire::Error::Format(error.message))?;
        if matched.is_none() {
            warnings.push(warning(
                dto::FileWarningCode::FieldSkipped,
                format!("appearance.chat.character:{value}"),
            ));
        }
        Ok(matched)
    };
    let slots = ChatWidgetSlots {
        left: chat
            .chat_widget_slots
            .left
            .clone()
            .into_iter()
            .map(|node| node.into_native(&mut asset, &mut character))
            .collect::<Result<_, _>>()
            .map_err(uec3_error)?,
        right: chat
            .chat_widget_slots
            .right
            .clone()
            .into_iter()
            .map(|node| node.into_native(&mut asset, &mut character))
            .collect::<Result<_, _>>()
            .map_err(uec3_error)?,
    };
    let presentation = CharacterPresentationV1 {
        format_version: card.appearance.format_version,
        card_style: card.appearance.card_style,
        avatar_crop: card.media.avatar_crop.map(Into::into),
        banner_crop: card.media.banner_crop.map(Into::into),
        disable_gradient: card.appearance.disable_gradient,
        gradient_source: card.appearance.gradient_source,
        custom_gradient_enabled: card.appearance.custom_gradient.enabled,
        custom_gradient_colors: card.appearance.custom_gradient.colors.clone(),
        primary_text_color: card.appearance.text_colors.primary.clone(),
        secondary_text_color: card.appearance.text_colors.secondary.clone(),
        chat_appearance: chat.clone().into_native(slots),
    };
    let prompt = |value: &Option<String>| -> Result<Option<PromptDocumentId>, ApiError> {
        value
            .as_deref()
            .map(|value| Ok(uec3_prompt_reference(context, value, prompts)?.id))
            .transpose()
    };
    let (companion_soul, scheduled_notes) =
        prepare_companion(card.companion.as_ref(), &prompt, id, now)?;
    let defaults = CharacterDefaults {
        interaction_mode: card.behavior.interaction_mode,
        memory_policy: card.behavior.memory,
        default_scene_id: card
            .default_scene
            .as_deref()
            .map(|key| reference(&scene_ids, key))
            .transpose()?,
        default_starter_id: card
            .default_starter
            .as_deref()
            .map(|key| reference(&starter_ids, key))
            .transpose()?,
        direct_prompt_id: prompt(&card.behavior.prompts.direct)?,
        group_conversation_prompt_id: prompt(&card.behavior.prompts.group_conversation)?,
        group_roleplay_prompt_id: prompt(&card.behavior.prompts.group_roleplay)?,
        model_profile_id: import_model_hint(context, card.behavior.model.as_ref(), warnings)?,
        voice: import_voice(context, card.behavior.voice.provider.as_ref(), warnings)?,
        voice_autoplay: card.behavior.voice.autoplay,
        companion_soul,
    };
    let character = Character::new(
        id,
        CharacterProfile {
            name: card.name.clone(),
            nickname: card.nickname.clone(),
            description: card.description.clone(),
            definition: card.definition.clone(),
            design_description: card.design.description.clone(),
            scenario: card.scenario.clone(),
        },
        CharacterProvenance {
            creator: card.creator.clone(),
            creator_notes: card.creator_notes.clone(),
            localized_creator_notes: card.creator_notes_localized.clone(),
            sources: card.sources.clone(),
            tags: card.tags.clone(),
        },
        defaults,
        presentation,
        import_uec3_image(context, card.behavior.image.as_ref(), warnings)?,
        media,
        now,
    )
    .map_err(format_error)?;
    let mut details = CharacterDetails {
        character,
        scenes,
        variants,
        starters: Vec::new(),
    };
    for (index, source) in card.starters.iter().enumerate() {
        let mut starter =
            prepare_uec3_template(context, &source.template, &details, books, prompts, now)?;
        starter.id = reference(&starter_ids, &source.template.key)?;
        starter.ordinal = ordinal(index)?;
        starter.scene_id = source
            .scene
            .as_deref()
            .map(|key| reference(&scene_ids, key))
            .transpose()?;
        details.starters.push(starter);
    }
    details.validate().map_err(format_error)?;
    let lorebook_bindings = card
        .behavior
        .lorebooks
        .iter()
        .enumerate()
        .map(|(index, binding)| {
            Ok(LorebookBinding {
                lorebook_id: books
                    .get(&binding.book)
                    .ok_or_else(|| format_error(format!("missing lorebook: {}", binding.book)))?
                    .book
                    .id,
                enabled: binding.enabled,
                ordinal: ordinal(index)?,
                revision: Revision::INITIAL,
                created_at: now,
                updated_at: now,
            })
        })
        .collect::<Result<_, ApiError>>()?;
    let companion_memory = import_state(
        document.state.as_ref(),
        card,
        details.character.defaults.companion_soul.as_ref(),
        now,
    )?;
    let character = CreateCharacterPlan {
        character: details.character,
        scenes: details.scenes,
        variants: details.variants,
        starters: details.starters,
    };
    Ok(lettuce_transfer::CharacterFileImport {
        lorebooks: Vec::new(),
        character,
        lorebook_bindings,
        scheduled_notes,
        companion_memory,
        skipped: Vec::new(),
        notices: Vec::new(),
    })
}

pub(super) fn export(
    context: &ApiContext,
    id: CharacterId,
    include_learned: bool,
    warnings: &mut Vec<dto::FileWarning>,
) -> Result<Uec3Export, ApiError> {
    let record = lettuce_transfer::CharacterFileRepository::character_export_record(
        context.backend().database(),
        id,
    )
    .map_err(internal)?
    .ok_or_else(|| api_error(ApiErrorCode::NotFound, "character was not found"))?;
    let details = &record.details;
    let character = &details.character;
    let mut assets = BTreeMap::new();
    let mut ids = BTreeMap::new();
    let mut dependencies = Vec::new();
    let mut book_refs: BTreeMap<LorebookId, String> = BTreeMap::new();
    let bindings = context
        .backend()
        .database()
        .list_character_bindings(id)
        .map_err(internal)?;
    let mut book = |book_id| -> Result<String, ApiError> {
        if let Some(value) = book_refs.get(&book_id) {
            return Ok(value.clone());
        }
        let details = LorebookRepository::get(context.backend().database(), book_id)
            .map_err(IntoApiError::into_api_error)?
            .ok_or_else(|| api_error(ApiErrorCode::NotFound, "bound lorebook is missing"))?;
        let icon = details
            .book
            .icon_asset_id
            .map(|id| add_uec3_asset(context, id, &mut assets, &mut ids))
            .transpose()?;
        let key = format!("l{}", book_refs.len() + 1);
        let value = format!("dep:{key}");
        dependencies.push(wire::Dependency {
            key,
            entity: wire::Entity::Lorebook(wire::Lorebook::from_details(&details, icon)),
        });
        book_refs.insert(book_id, value.clone());
        Ok(value)
    };
    let lorebooks = bindings
        .iter()
        .map(|binding| {
            Ok(wire::LorebookBinding {
                book: book(binding.lorebook_id)?,
                enabled: binding.enabled,
            })
        })
        .collect::<Result<_, ApiError>>()?;
    let starter_books = details
        .starters
        .iter()
        .map(|starter| {
            Ok(match &starter.lorebooks {
                Selection::Inherit => {
                    wire::LorebookSelection::Keyword(wire::SelectionKeyword::Inherit)
                }
                Selection::Disabled => {
                    wire::LorebookSelection::Keyword(wire::SelectionKeyword::Disabled)
                }
                Selection::Explicit(values) => wire::LorebookSelection::Explicit(
                    values
                        .iter()
                        .map(|id| book(*id))
                        .collect::<Result<_, _>>()?,
                ),
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let mut prompt_refs = BTreeMap::new();
    let mut prompt = |value: Option<PromptDocumentId>| {
        value
            .map(|id| bundle_uec3_prompt(context, id, &mut dependencies, &mut prompt_refs))
            .transpose()
    };
    let prompts = wire::CharacterPrompts {
        direct: prompt(character.defaults.direct_prompt_id)?,
        group_conversation: prompt(character.defaults.group_conversation_prompt_id)?,
        group_roleplay: prompt(character.defaults.group_roleplay_prompt_id)?,
    };
    let companion = export_companion(
        character.defaults.companion_soul.as_ref(),
        &record.scheduled_notes,
        &mut prompt,
    )?;
    let active_scenes = details
        .scenes
        .iter()
        .filter(|scene| scene.status == LifecycleStatus::Active)
        .collect::<Vec<_>>();
    let scene_ids = active_scenes
        .iter()
        .enumerate()
        .map(|(index, scene)| (scene.id, format!("s{}", index + 1)))
        .collect::<BTreeMap<_, _>>();
    let mut scenes = Vec::new();
    for scene in active_scenes {
        let mut parts = |document: &SceneDocumentV1| {
            document
                .parts
                .iter()
                .map(|part| match part {
                    ScenePart::Text { text } => Ok(wire::ScenePart::Text { text: text.clone() }),
                    ScenePart::InlineAsset { link_id } => {
                        let link = scene
                            .assets
                            .iter()
                            .find(|link| link.id == *link_id)
                            .ok_or_else(|| internal("missing scene inline link"))?;
                        Ok(wire::ScenePart::Image {
                            image: add_uec3_asset(context, link.asset_id, &mut assets, &mut ids)?,
                        })
                    }
                })
                .collect::<Result<Vec<_>, ApiError>>()
        };
        let source_variants = details
            .variants
            .iter()
            .filter(|variant| variant.scene_id == scene.id)
            .collect::<Vec<_>>();
        let variant_ids = source_variants
            .iter()
            .enumerate()
            .map(|(index, variant)| (variant.id, format!("v{}", index + 1)))
            .collect::<BTreeMap<_, _>>();
        let content = parts(&scene.content)?;
        let variants = source_variants
            .iter()
            .map(|variant| {
                Ok(wire::SceneVariant {
                    key: variant_ids[&variant.id].clone(),
                    format_version: variant.content.format_version,
                    parts: parts(&variant.content)?,
                    direction: variant.direction.clone(),
                })
            })
            .collect::<Result<_, ApiError>>()?;
        let background = scene
            .assets
            .iter()
            .find(|link| link.slot == SceneAssetSlot::Background)
            .map(|link| add_uec3_asset(context, link.asset_id, &mut assets, &mut ids))
            .transpose()?;
        scenes.push(wire::Scene {
            key: scene_ids[&scene.id].clone(),
            format_version: scene.content.format_version,
            parts: content,
            direction: scene.direction.clone(),
            background,
            variants,
            selected_variant: scene.selected_variant_id.map(|id| variant_ids[&id].clone()),
        });
    }
    let starter_ids = details
        .starters
        .iter()
        .enumerate()
        .map(|(index, starter)| (starter.id, format!("t{}", index + 1)))
        .collect::<BTreeMap<_, _>>();
    let mut scene_reference = |id: Option<SceneId>, field: String| {
        id.and_then(|id| match scene_ids.get(&id) {
            Some(key) => Some(key.clone()),
            None => {
                warnings.push(warning(dto::FileWarningCode::FieldSkipped, field));
                None
            }
        })
    };
    let starters = details
        .starters
        .iter()
        .zip(starter_books)
        .map(|(starter, lorebooks)| {
            Ok(wire::Starter {
                template: wire::ChatTemplate {
                    key: starter_ids[&starter.id].clone(),
                    name: starter.name.clone(),
                    messages: starter
                        .messages
                        .iter()
                        .map(|message| wire::StarterMessage {
                            role: message.role,
                            text: message.content.clone(),
                        })
                        .collect(),
                    prompt: prompt(starter.prompt_id)?,
                    lorebooks,
                },
                scene: scene_reference(
                    starter.scene_id,
                    format!("starters.{}.scene", starter_ids[&starter.id]),
                ),
            })
        })
        .collect::<Result<_, ApiError>>()?;
    let default_scene = scene_reference(character.defaults.default_scene_id, "defaultScene".into());
    let mut asset = |id| {
        add_uec3_asset(context, id, &mut assets, &mut ids)
            .map_err(|error| wire::Error::Format(error.message))
    };
    let mut widget_character = |other| {
        if other == id {
            return Ok("self".into());
        }
        CharacterRepository::get(context.backend().database(), other)
            .map_err(|error| wire::Error::Format(error.to_string()))?
            .map(|details| details.character.profile.name)
            .ok_or_else(|| wire::Error::Format("widget character is missing".into()))
    };
    let native_chat = &character.presentation.chat_appearance;
    let slots = wire::ChatWidgetSlots {
        left: native_chat
            .chat_widget_slots
            .left
            .iter()
            .map(|node| wire::WidgetNode::from_native(node, &mut asset, &mut widget_character))
            .collect::<Result<_, _>>()
            .map_err(uec3_error)?,
        right: native_chat
            .chat_widget_slots
            .right
            .iter()
            .map(|node| wire::WidgetNode::from_native(node, &mut asset, &mut widget_character))
            .collect::<Result<_, _>>()
            .map_err(uec3_error)?,
    };
    let chat = wire::ChatAppearance::from_native(native_chat, slots);
    let mut media = |slot| {
        character
            .media
            .links
            .iter()
            .find(|link| link.slot == slot)
            .map(|link| add_uec3_asset(context, link.asset_id, &mut assets, &mut ids))
            .transpose()
    };
    let avatar = media(CharacterMediaSlot::AvatarOriginal)?;
    let banner = media(CharacterMediaSlot::Banner)?;
    let background = media(CharacterMediaSlot::Background)?;
    let references = character
        .media
        .links
        .iter()
        .filter(|link| link.slot == CharacterMediaSlot::DesignReference)
        .map(|link| add_uec3_asset(context, link.asset_id, &mut assets, &mut ids))
        .collect::<Result<_, _>>()?;
    let presentation = &character.presentation;
    let card = wire::Character {
        name: character.profile.name.clone(),
        nickname: character.profile.nickname.clone(),
        description: character.profile.description.clone(),
        definition: character.profile.definition.clone(),
        scenario: character.profile.scenario.clone(),
        creator: character.provenance.creator.clone(),
        creator_notes: character.provenance.creator_notes.clone(),
        creator_notes_localized: character.provenance.localized_creator_notes.clone(),
        sources: character.provenance.sources.clone(),
        tags: character.provenance.tags.clone(),
        design: wire::Design {
            description: character.profile.design_description.clone(),
            references,
        },
        media: wire::CharacterMedia {
            avatar,
            banner,
            background,
            avatar_crop: presentation.avatar_crop.map(Into::into),
            banner_crop: presentation.banner_crop.map(Into::into),
        },
        appearance: wire::Appearance {
            format_version: presentation.format_version,
            card_style: presentation.card_style,
            disable_gradient: presentation.disable_gradient,
            gradient_source: presentation.gradient_source,
            custom_gradient: wire::CustomGradient {
                enabled: presentation.custom_gradient_enabled,
                colors: presentation.custom_gradient_colors.clone(),
            },
            text_colors: wire::TextColors {
                primary: presentation.primary_text_color.clone(),
                secondary: presentation.secondary_text_color.clone(),
            },
            chat,
        },
        scenes,
        default_scene,
        starters,
        default_starter: character
            .defaults
            .default_starter_id
            .map(|id| starter_ids[&id].clone()),
        behavior: wire::CharacterBehavior {
            interaction_mode: character.defaults.interaction_mode,
            memory: character.defaults.memory_policy,
            prompts,
            model: export_model_hint(context, character.defaults.model_profile_id)?,
            voice: wire::Voice {
                provider: export_voice(context, character.defaults.voice.as_ref(), warnings)?,
                autoplay: character.defaults.voice_autoplay,
            },
            lorebooks,
            image: export_uec3_image(context, character.image_recommendation.as_ref(), warnings)?,
        },
        companion,
    };
    let state = if include_learned {
        export_state(&record)?
    } else {
        None
    };
    let mut export = Uec3Export::new(
        context,
        wire::Entity::Character(Box::new(card)),
        (character.created_at, character.updated_at),
        dependencies,
        assets,
        ids,
    );
    export.document.state = state;
    Ok(export)
}

fn kind_name(kind: lettuce_speech::AudioProviderKind) -> Result<String, ApiError> {
    serde_json::to_value(kind)
        .map_err(internal)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| internal("invalid audio provider kind"))
}

fn export_voice(
    context: &ApiContext,
    voice: Option<&VoicePreference>,
    warnings: &mut Vec<dto::FileWarning>,
) -> Result<Option<wire::VoicePreference>, ApiError> {
    use lettuce_speech::TtsConfigurationRepository;
    let database = context.backend().database();
    Ok(match voice {
        None => None,
        Some(VoicePreference::UnresolvedLegacy(_)) => {
            warnings.push(warning(
                dto::FileWarningCode::FieldSkipped,
                "behavior.voice.unresolvedLegacy",
            ));
            None
        }
        Some(VoicePreference::Provider {
            provider_id,
            voice_id,
            model_id,
            voice_name,
        }) => {
            let provider = database
                .get_audio_provider(*provider_id)
                .map_err(internal)?
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "voice provider is missing"))?;
            let kind = kind_name(provider.config.provider_kind())?;
            Some(wire::VoicePreference::Provider(wire::ProviderVoice {
                kind: kind.clone(),
                account: wire::ProviderAccountHint {
                    provider: kind,
                    label: provider.label,
                },
                voice_id: voice_id.clone(),
                model_id: model_id.clone(),
                voice_name: voice_name.clone(),
            }))
        }
        Some(VoicePreference::VoiceProfile(id)) => {
            let voice = database
                .get_user_voice(*id)
                .map_err(internal)?
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "voice profile is missing"))?;
            let provider = database
                .get_audio_provider(voice.provider_id)
                .map_err(internal)?
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "voice provider is missing"))?;
            Some(wire::VoicePreference::Profile(wire::VoiceProfile {
                voice_profile: wire::Hint {
                    name: voice.name,
                    provider: kind_name(provider.config.provider_kind())?,
                    external_id: voice.voice_id,
                },
            }))
        }
    })
}

fn import_voice(
    context: &ApiContext,
    voice: Option<&wire::VoicePreference>,
    warnings: &mut Vec<dto::FileWarning>,
) -> Result<Option<VoicePreference>, ApiError> {
    use lettuce_speech::TtsConfigurationRepository;
    let database = context.backend().database();
    let providers = database.list_audio_providers().map_err(internal)?;
    let Some(voice) = voice else {
        return Ok(None);
    };
    match voice {
        wire::VoicePreference::Provider(voice) => {
            if voice.kind != voice.account.provider {
                return Err(format_error("voice kind and account provider disagree"));
            }
            let candidates = providers
                .iter()
                .filter_map(
                    |provider| match kind_name(provider.config.provider_kind()) {
                        Ok(kind) if kind == voice.account.provider => Some(Ok(provider)),
                        Ok(_) => None,
                        Err(error) => Some(Err(error)),
                    },
                )
                .collect::<Result<Vec<_>, _>>()?;
            let exact = candidates
                .iter()
                .filter(|provider| provider.label == voice.account.label)
                .copied()
                .collect::<Vec<_>>();
            let matched = if exact.len() == 1 {
                exact.first().copied()
            } else if exact.is_empty() && candidates.len() == 1 {
                candidates.first().copied()
            } else {
                None
            };
            if let Some(provider) = matched {
                return Ok(Some(VoicePreference::Provider {
                    provider_id: provider.id,
                    voice_id: voice.voice_id.clone(),
                    model_id: voice.model_id.clone(),
                    voice_name: voice.voice_name.clone(),
                }));
            }
            warnings.push(warning(
                dto::FileWarningCode::FieldSkipped,
                format!(
                    "behavior.voice.account:{}:{}",
                    voice.account.provider, voice.account.label
                ),
            ));
        }
        wire::VoicePreference::Profile(profile) => {
            let hint = &profile.voice_profile;
            let mut matches = Vec::new();
            for voice in database.list_user_voices().map_err(internal)? {
                let provider = providers
                    .iter()
                    .find(|provider| provider.id == voice.provider_id)
                    .ok_or_else(|| internal("voice provider is missing"))?;
                if voice.name == hint.name
                    && voice.voice_id == hint.external_id
                    && kind_name(provider.config.provider_kind())? == hint.provider
                {
                    matches.push(voice.id);
                }
            }
            if matches.len() == 1 {
                return Ok(matches.first().copied().map(VoicePreference::VoiceProfile));
            }
            warnings.push(warning(
                dto::FileWarningCode::FieldSkipped,
                format!("behavior.voice.profile:{}", hint.name),
            ));
        }
    }
    Ok(None)
}

fn export_model_hint(
    context: &ApiContext,
    id: Option<ModelProfileId>,
) -> Result<Option<wire::Hint>, ApiError> {
    use lettuce_models::ModelCatalog;
    let Some(id) = id else {
        return Ok(None);
    };
    let database = context.backend().database();
    let profile = database
        .model_profiles()
        .map_err(internal)?
        .into_iter()
        .find(|profile| profile.id == id)
        .ok_or_else(|| api_error(ApiErrorCode::NotFound, "model profile is missing"))?;
    let account = database
        .provider_accounts()
        .map_err(internal)?
        .into_iter()
        .find(|account| account.id == profile.provider_account_id)
        .ok_or_else(|| api_error(ApiErrorCode::NotFound, "model account is missing"))?;
    let model = wire::ModelProfile::from_stored(&profile, &account);
    Ok(Some(wire::Hint {
        name: profile.display_name,
        provider: account.provider_kind,
        external_id: model.external_id,
    }))
}

fn import_model_hint(
    context: &ApiContext,
    hint: Option<&wire::Hint>,
    warnings: &mut Vec<dto::FileWarning>,
) -> Result<Option<ModelProfileId>, ApiError> {
    use lettuce_models::ModelCatalog;
    let Some(hint) = hint else {
        return Ok(None);
    };
    let mut matches = Vec::new();
    for profile in context
        .backend()
        .database()
        .model_profiles()
        .map_err(internal)?
    {
        let Some(candidate) = export_model_hint(context, Some(profile.id))? else {
            return Err(internal("model hint is missing"));
        };
        if candidate == *hint {
            matches.push(profile.id);
        }
    }
    if matches.len() == 1 {
        return Ok(matches.first().copied());
    }
    warnings.push(warning(
        dto::FileWarningCode::FieldSkipped,
        format!("behavior.model:{}", hint.name),
    ));
    Ok(None)
}

fn prepare_companion(
    companion: Option<&wire::Companion>,
    prompt: &impl Fn(&Option<String>) -> Result<Option<PromptDocumentId>, ApiError>,
    id: CharacterId,
    now: TimestampMillis,
) -> Result<
    (
        Option<lettuce_companions::CompanionSoulConfig>,
        Vec<lettuce_companions::CompanionScheduledNote>,
    ),
    ApiError,
> {
    use lettuce_companions::*;
    let Some(source) = companion else {
        return Ok((None, Vec::new()));
    };
    let identity = &source.soul;
    let config = CompanionSoulConfig {
        soul: CompanionSoulIdentity {
            essence: identity.essence.clone(),
            traits: identity.traits.clone(),
            backstory: identity.backstory.clone(),
            appearance: identity.appearance.clone(),
            goals: identity.goals.clone(),
            likes: identity.likes.clone(),
            voice: identity.voice.clone(),
            relational_style: identity.relational_style.clone(),
            vulnerabilities: identity.vulnerabilities.clone(),
            fears: identity.fears.clone(),
            habits: identity.habits.clone(),
            boundaries: identity.boundaries.clone(),
            baseline_affect: source.baseline_affect.clone().into(),
            regulation_style: source.regulation_style.clone().into(),
        },
        authored_facts: source
            .authored_facts
            .iter()
            .map(|fact| SoulFact {
                id: uuid::Uuid::new_v4().to_string(),
                category: fact.category,
                value: fact.value.clone(),
                kind: SoulFactKind::Authored,
                policy: fact.policy,
                slot: fact.slot.clone(),
                confidence: fact.confidence,
                evidence_count: fact.evidence_count,
                weight: fact.weight,
                valid_from: TimestampMillis::new(fact.valid_from),
                valid_until: fact.valid_until.map(TimestampMillis::new),
                locked: fact.locked,
                source_memory_ids: Vec::new(),
                created_at: now,
                supersedes: Vec::new(),
                superseded_by: None,
                superseded_at: None,
            })
            .collect(),
        relationship_defaults: source.relationship_defaults.clone().into(),
        prompting: CompanionPromptingConfig {
            prompt_template_id: prompt(&source.prompting.prompt)?,
            style_notes: source.prompting.style_notes.clone(),
        },
        time_awareness: source.time_awareness,
        share_memory_across_chats: source.share_memory_across_chats,
        share_soul_growth_across_chats: source.share_soul_growth_across_chats,
    };
    let mut note_keys = std::collections::BTreeSet::new();
    let notes = source
        .scheduled_notes
        .iter()
        .map(|note| {
            if note.key.is_empty() || !note_keys.insert(&note.key) {
                return Err(format_error("duplicate or empty note key"));
            }
            let value = CompanionScheduledNote {
                id: uuid::Uuid::new_v4(),
                character_id: id,
                label: note.label.clone(),
                content: note.content.clone(),
                available_at: TimestampMillis::new(note.available_at),
                expires_at: note.expires_at.map(TimestampMillis::new),
                recurrence: note.recurrence,
                recurrence_window_ms: note.recurrence_window_ms,
                enabled: note.enabled,
                created_at: now,
                updated_at: now,
            };
            value.validate().map_err(format_error)?;
            Ok(value)
        })
        .collect::<Result<_, _>>()?;
    Ok((Some(config), notes))
}

fn export_companion(
    source: Option<&lettuce_companions::CompanionSoulConfig>,
    notes: &[lettuce_companions::CompanionScheduledNote],
    prompt: &mut impl FnMut(Option<PromptDocumentId>) -> Result<Option<String>, ApiError>,
) -> Result<Option<wire::Companion>, ApiError> {
    let Some(source) = source else {
        return Ok(None);
    };
    let identity = &source.soul;
    Ok(Some(wire::Companion {
        soul: wire::SoulIdentity {
            essence: identity.essence.clone(),
            traits: identity.traits.clone(),
            backstory: identity.backstory.clone(),
            appearance: identity.appearance.clone(),
            goals: identity.goals.clone(),
            likes: identity.likes.clone(),
            voice: identity.voice.clone(),
            relational_style: identity.relational_style.clone(),
            vulnerabilities: identity.vulnerabilities.clone(),
            fears: identity.fears.clone(),
            habits: identity.habits.clone(),
            boundaries: identity.boundaries.clone(),
        },
        baseline_affect: (&identity.baseline_affect).into(),
        regulation_style: (&identity.regulation_style).into(),
        relationship_defaults: (&source.relationship_defaults).into(),
        authored_facts: source
            .authored_facts
            .iter()
            .enumerate()
            .map(|(index, fact)| wire::AuthoredFact {
                key: format!("f{}", index + 1),
                category: fact.category,
                value: fact.value.clone(),
                policy: fact.policy,
                slot: fact.slot.clone(),
                locked: fact.locked,
                weight: fact.weight,
                confidence: fact.confidence,
                evidence_count: fact.evidence_count,
                valid_from: fact.valid_from.get(),
                valid_until: fact.valid_until.map(TimestampMillis::get),
            })
            .collect(),
        prompting: wire::Prompting {
            prompt: prompt(source.prompting.prompt_template_id)?,
            style_notes: source.prompting.style_notes.clone(),
        },
        time_awareness: source.time_awareness,
        share_memory_across_chats: source.share_memory_across_chats,
        share_soul_growth_across_chats: source.share_soul_growth_across_chats,
        scheduled_notes: notes
            .iter()
            .enumerate()
            .map(|(index, note)| wire::ScheduledNote {
                key: format!("n{}", index + 1),
                label: note.label.clone(),
                content: note.content.clone(),
                available_at: note.available_at.get(),
                expires_at: note.expires_at.map(TimestampMillis::get),
                recurrence: note.recurrence,
                recurrence_window_ms: note.recurrence_window_ms,
                enabled: note.enabled,
            })
            .collect(),
    }))
}

fn portable_reference(values: &BTreeMap<String, String>, key: &str) -> Result<String, ApiError> {
    values
        .get(key)
        .cloned()
        .ok_or_else(|| format_error(format!("unmapped historical reference: {key}")))
}

fn export_state(
    record: &lettuce_transfer::CharacterExportRecord,
) -> Result<Option<wire::State>, ApiError> {
    use lettuce_companions::SoulFactKind;
    let Some(memory) = &record.companion_memory else {
        return Ok(None);
    };
    let Some(config) = &record.details.character.defaults.companion_soul else {
        return Ok(None);
    };
    let mut fact_keys = config
        .authored_facts
        .iter()
        .enumerate()
        .map(|(index, fact)| (fact.id.clone(), format!("f{}", index + 1)))
        .collect::<BTreeMap<_, _>>();
    let learned = memory
        .soul_facts
        .as_deref()
        .unwrap_or_default()
        .iter()
        .collect::<Vec<_>>();
    for (index, fact) in learned.iter().enumerate() {
        if !fact_keys.contains_key(&fact.id) {
            fact_keys.insert(fact.id.clone(), format!("g{}", index + 1));
        }
    }
    let items = memory
        .pool
        .as_ref()
        .map(|pool| pool.items.as_slice())
        .unwrap_or_default();
    let mut memory_keys = items
        .iter()
        .enumerate()
        .map(|(index, item)| (item.id.to_string(), format!("m{}", index + 1)))
        .collect::<BTreeMap<_, _>>();
    fact_keys.insert("consolidation".into(), "consolidation".into());
    for fact in memory.soul_facts.as_deref().unwrap_or_default() {
        for id in fact.supersedes.iter().chain(fact.superseded_by.iter()) {
            if !fact_keys.contains_key(id) {
                fact_keys.insert(id.clone(), format!("hf{}", fact_keys.len() + 1));
            }
        }
        for id in &fact.source_memory_ids {
            if !memory_keys.contains_key(id) {
                memory_keys.insert(id.clone(), format!("hm{}", memory_keys.len() + 1));
            }
        }
    }
    for item in items {
        for id in item.supersedes.iter().chain(item.superseded_by.iter()) {
            let id = id.to_string();
            if !memory_keys.contains_key(&id) {
                memory_keys.insert(id, format!("hm{}", memory_keys.len() + 1));
            }
        }
    }
    let soul = learned
        .into_iter()
        .map(|fact| {
            Ok(wire::LearnedFact {
                key: portable_reference(&fact_keys, &fact.id)?,
                category: fact.category,
                value: fact.value.clone(),
                kind: match fact.kind {
                    SoulFactKind::Add => wire::LearnedFactKind::Add,
                    SoulFactKind::Adjust => wire::LearnedFactKind::Adjust,
                    SoulFactKind::Consolidated => wire::LearnedFactKind::Consolidated,
                    SoulFactKind::Authored => wire::LearnedFactKind::Authored,
                },
                policy: fact.policy,
                slot: fact.slot.clone(),
                confidence: fact.confidence,
                evidence_count: fact.evidence_count,
                weight: fact.weight,
                valid_from: fact.valid_from.get(),
                valid_until: fact.valid_until.map(TimestampMillis::get),
                locked: fact.locked,
                created_at: fact.created_at.get(),
                supersedes: fact
                    .supersedes
                    .iter()
                    .map(|id| portable_reference(&fact_keys, id))
                    .collect::<Result<_, _>>()?,
                superseded_by: fact
                    .superseded_by
                    .as_deref()
                    .map(|id| portable_reference(&fact_keys, id))
                    .transpose()?,
                superseded_at: fact.superseded_at.map(TimestampMillis::get),
                source_memories: fact
                    .source_memory_ids
                    .iter()
                    .map(|id| portable_reference(&memory_keys, id))
                    .collect::<Result<_, _>>()?,
            })
        })
        .collect::<Result<_, ApiError>>()?;
    let items = items
        .iter()
        .map(|item| {
            Ok(wire::MemoryItem {
                key: portable_reference(&memory_keys, &item.id.to_string())?,
                text: item.text.clone(),
                category: item.category,
                origin: item.origin,
                observed_at: item.observed_at.map(TimestampMillis::get),
                observed_precision: item
                    .observed_time_precision
                    .as_ref()
                    .map(|_| wire::ObservedPrecision::User),
                supersedes: item
                    .supersedes
                    .iter()
                    .map(|id| portable_reference(&memory_keys, &id.to_string()))
                    .collect::<Result<_, _>>()?,
                superseded_by: item
                    .superseded_by
                    .map(|id| portable_reference(&memory_keys, &id.to_string()))
                    .transpose()?,
                superseded_at: item.superseded_at.map(TimestampMillis::get),
                pinned: item.is_pinned,
                cold: item.is_cold,
                importance: item.importance.basis_points(),
                persistence_importance: item.persistence_importance.basis_points(),
                prompt_importance: item.prompt_importance.basis_points(),
                volatility: item.volatility.basis_points(),
                created_at: item.created_at.get(),
            })
        })
        .collect::<Result<_, ApiError>>()?;
    Ok(Some(wire::State {
        companion: wire::CompanionState {
            soul,
            memory: wire::Memory { items },
        },
    }))
}

fn import_state(
    source: Option<&wire::State>,
    card: &wire::Character,
    config: Option<&lettuce_companions::CompanionSoulConfig>,
    now: TimestampMillis,
) -> Result<Option<lettuce_transfer::CharacterFileCompanionMemory>, ApiError> {
    use lettuce_companions::{SoulFact, SoulFactKind, SoulState};
    use lettuce_memory::{MemoryItem, MemoryShortId, MemorySpaceSnapshot, Score};
    let Some(source) = source else {
        return Ok(None);
    };
    let config =
        config.ok_or_else(|| format_error("learned state requires companion configuration"))?;
    let companion = card
        .companion
        .as_ref()
        .ok_or_else(|| format_error("learned state requires companion configuration"))?;
    let mut fact_ids = BTreeMap::new();
    for (wire, fact) in companion.authored_facts.iter().zip(&config.authored_facts) {
        if wire.key.is_empty()
            || wire.key == "consolidation"
            || fact_ids.insert(wire.key.clone(), fact.id.clone()).is_some()
        {
            return Err(format_error("duplicate or empty fact key"));
        }
    }
    let mut shared_keys = std::collections::BTreeSet::new();
    for fact in &source.companion.soul {
        if fact.key.is_empty() || fact.key == "consolidation" || !shared_keys.insert(&fact.key) {
            return Err(format_error("duplicate or reserved fact key"));
        }
        if fact_ids.contains_key(&fact.key) && fact.kind != wire::LearnedFactKind::Authored {
            return Err(format_error("config fact key requires authored kind"));
        }
        fact_ids
            .entry(fact.key.clone())
            .or_insert_with(|| uuid::Uuid::new_v4().to_string());
    }
    let mut memory_ids = BTreeMap::new();
    for item in &source.companion.memory.items {
        if item.key.is_empty()
            || memory_ids
                .insert(item.key.clone(), MemoryId::new())
                .is_some()
        {
            return Err(format_error("duplicate or empty memory key"));
        }
    }
    fact_ids.insert("consolidation".into(), "consolidation".into());
    for fact in &source.companion.soul {
        for key in fact.supersedes.iter().chain(fact.superseded_by.iter()) {
            if key.is_empty() {
                return Err(format_error("empty fact reference"));
            }
            fact_ids
                .entry(key.clone())
                .or_insert_with(|| uuid::Uuid::new_v4().to_string());
        }
        for key in &fact.source_memories {
            if key.is_empty() {
                return Err(format_error("empty memory reference"));
            }
            memory_ids.entry(key.clone()).or_insert_with(MemoryId::new);
        }
    }
    for item in &source.companion.memory.items {
        for key in item.supersedes.iter().chain(item.superseded_by.iter()) {
            if key.is_empty() {
                return Err(format_error("empty memory reference"));
            }
            memory_ids.entry(key.clone()).or_insert_with(MemoryId::new);
        }
    }
    let mut facts = Vec::new();
    for fact in &source.companion.soul {
        facts.push(SoulFact {
            id: portable_reference(&fact_ids, &fact.key)?,
            category: fact.category,
            value: fact.value.clone(),
            kind: match fact.kind {
                wire::LearnedFactKind::Authored => SoulFactKind::Authored,
                wire::LearnedFactKind::Add => SoulFactKind::Add,
                wire::LearnedFactKind::Adjust => SoulFactKind::Adjust,
                wire::LearnedFactKind::Consolidated => SoulFactKind::Consolidated,
            },
            policy: fact.policy,
            slot: fact.slot.clone(),
            confidence: fact.confidence,
            evidence_count: fact.evidence_count,
            weight: fact.weight,
            valid_from: TimestampMillis::new(fact.valid_from),
            valid_until: fact.valid_until.map(TimestampMillis::new),
            locked: fact.locked,
            created_at: TimestampMillis::new(fact.created_at),
            supersedes: fact
                .supersedes
                .iter()
                .map(|key| portable_reference(&fact_ids, key))
                .collect::<Result<_, _>>()?,
            superseded_by: fact
                .superseded_by
                .as_deref()
                .map(|key| portable_reference(&fact_ids, key))
                .transpose()?,
            superseded_at: fact.superseded_at.map(TimestampMillis::new),
            source_memory_ids: fact
                .source_memories
                .iter()
                .map(|key| reference(&memory_ids, key).map(|id| id.to_string()))
                .collect::<Result<_, _>>()?,
        });
    }
    lettuce_companions::validate_state(&SoulState {
        revision: Revision::INITIAL,
        facts: facts.clone(),
    })
    .map_err(|error| format_error(format!("invalid Soul state: {error:?}")))?;
    let score = |value| {
        Score::from_basis_points(value).ok_or_else(|| format_error("memory score exceeds 10000"))
    };
    let items = source
        .companion
        .memory
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let mut value = MemoryItem::written(
                reference(&memory_ids, &item.key)?,
                MemoryShortId::new(ordinal(index)?)
                    .ok_or_else(|| format_error("memory short id space exhausted"))?,
                item.text.clone(),
                TimestampMillis::new(item.created_at),
            );
            value.category = item.category;
            value.origin = item.origin;
            value.observed_at = item.observed_at.map(TimestampMillis::new);
            value.observed_time_precision = item.observed_precision.map(|_| "user".into());
            value.supersedes = item
                .supersedes
                .iter()
                .map(|key| reference(&memory_ids, key))
                .collect::<Result<_, _>>()?;
            value.superseded_by = item
                .superseded_by
                .as_deref()
                .map(|key| reference(&memory_ids, key))
                .transpose()?;
            value.superseded_at = item.superseded_at.map(TimestampMillis::new);
            value.is_pinned = item.pinned;
            value.is_cold = item.cold;
            value.importance = score(item.importance)?;
            value.persistence_importance = score(item.persistence_importance)?;
            value.prompt_importance = score(item.prompt_importance)?;
            value.volatility = score(item.volatility)?;
            Ok(value)
        })
        .collect::<Result<_, ApiError>>()?;
    let pool = MemorySpaceSnapshot {
        id: MemorySpaceId::new(),
        revision: Revision::INITIAL,
        items,
    };
    pool.validate().map_err(format_error)?;
    Ok(Some(lettuce_transfer::CharacterFileCompanionMemory {
        pool: Some(pool),
        soul_facts: Some(facts),
        relationships: Vec::new(),
        created_at: now,
        updated_at: now,
    }))
}
