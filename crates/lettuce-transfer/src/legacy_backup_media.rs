use std::collections::BTreeMap;

use crate::{
    LEGACY_MEDIA_OBJECT_BYTES_LIMIT, LEGACY_MEDIA_REFERENCE_LIMIT, LEGACY_MEDIA_TOTAL_BYTES_LIMIT,
    LegacyBackupAuthoredPlan, LegacyBackupMedia, LegacyBackupMediaRoot, LegacyImportSkip,
    LegacyImportSkipKind, LegacyImportSkipReason, LegacyMediaCandidate, LegacyMediaPlan,
    LegacyMediaReference, LegacyMediaUse,
};

#[derive(Debug)]
pub struct LegacyBackupAuthoredMediaPlan {
    pub media: LegacyMediaPlan,
    pub authored: LegacyBackupAuthoredPlan,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyBackupMediaPlanError {
    #[error("legacy backup media reference is unsafe")]
    Unsafe { locator: String },
    #[error("legacy backup media reference is missing")]
    Missing { locator: String },
    #[error("legacy backup media reference is ambiguous")]
    Ambiguous { locator: String },
    #[error("legacy backup media reference limit exceeded")]
    ReferenceLimit,
    #[error("legacy backup media object limit exceeded")]
    ObjectLimit { locator: String },
    #[error("legacy backup media total limit exceeded")]
    TotalLimit,
}

struct PlannedMedia<'a> {
    media: &'a LegacyBackupMedia,
    uses: Vec<LegacyMediaUse>,
}

pub fn plan_legacy_backup_authored_media(
    mut authored: LegacyBackupAuthoredPlan,
) -> Result<LegacyBackupAuthoredMediaPlan, LegacyBackupMediaPlanError> {
    let skipped = prune_missing_media(&mut authored)?;
    let mut planned = BTreeMap::<String, PlannedMedia<'_>>::new();
    let mut references = 0_u32;
    for persona in &authored.personas.personas {
        if let Some(avatar) = &persona.avatar {
            add_avatar(
                &authored,
                &mut planned,
                avatar,
                &format!("persona-{}", persona.id),
                LegacyMediaUse::PersonaAvatar {
                    persona_id: persona.id,
                },
                &mut references,
            )?;
        }
        for (ordinal, reference) in persona.design_references.iter().enumerate() {
            add_image(
                &authored,
                &mut planned,
                reference,
                LegacyMediaUse::PersonaDesignReference {
                    persona_id: persona.id,
                    ordinal: bounded_ordinal(ordinal)?,
                },
                &mut references,
            )?;
        }
    }
    for lorebook in &authored.lorebooks.lorebooks {
        if let Some(avatar) = &lorebook.avatar {
            add_image(
                &authored,
                &mut planned,
                avatar,
                LegacyMediaUse::LorebookAvatar {
                    lorebook_id: lorebook.id,
                },
                &mut references,
            )?;
        }
    }
    for character in &authored.characters {
        if let Some(avatar) = &character.media.avatar {
            add_avatar(
                &authored,
                &mut planned,
                avatar,
                &format!("character-{}", character.id),
                LegacyMediaUse::CharacterAvatar {
                    character_id: character.id,
                },
                &mut references,
            )?;
        }
        if let Some(background) = &character.media.background {
            add_image(
                &authored,
                &mut planned,
                background,
                LegacyMediaUse::CharacterBackground {
                    character_id: character.id,
                },
                &mut references,
            )?;
        }
        for (ordinal, reference) in character.media.design_references.iter().enumerate() {
            add_image(
                &authored,
                &mut planned,
                reference,
                LegacyMediaUse::CharacterDesignReference {
                    character_id: character.id,
                    ordinal: bounded_ordinal(ordinal)?,
                },
                &mut references,
            )?;
        }
        for scene in &character.scenes {
            if let Some(background) = &scene.background {
                add_image(
                    &authored,
                    &mut planned,
                    background,
                    LegacyMediaUse::CharacterSceneBackground {
                        character_id: character.id,
                        scene_id: scene.id,
                    },
                    &mut references,
                )?;
            }
        }
    }
    for group in &authored.groups {
        if let Some(background) = &group.background {
            add_image(
                &authored,
                &mut planned,
                background,
                LegacyMediaUse::GroupBackground { group_id: group.id },
                &mut references,
            )?;
        }
        if let Some(scene) = &group.starting_scene
            && let Some(background) = &scene.background
        {
            add_image(
                &authored,
                &mut planned,
                background,
                LegacyMediaUse::GroupSceneBackground {
                    group_id: group.id,
                    scene_id: scene.id,
                },
                &mut references,
            )?;
        }
    }

    let mut total_bytes = 0_u64;
    let mut media = Vec::with_capacity(planned.len());
    for (relative_path, item) in planned {
        let byte_len = item.media.byte_len;
        if byte_len > LEGACY_MEDIA_OBJECT_BYTES_LIMIT {
            return Err(LegacyBackupMediaPlanError::ObjectLimit {
                locator: relative_path,
            });
        }
        total_bytes = total_bytes
            .checked_add(byte_len)
            .ok_or(LegacyBackupMediaPlanError::TotalLimit)?;
        if total_bytes > LEGACY_MEDIA_TOTAL_BYTES_LIMIT {
            return Err(LegacyBackupMediaPlanError::TotalLimit);
        }
        let content_hash = item.media.content_hash.clone();
        media.push(LegacyMediaCandidate {
            source_locator: relative_path.clone(),
            relative_path,
            byte_len,
            content_hash,
            uses: item.uses,
        });
    }
    Ok(LegacyBackupAuthoredMediaPlan {
        media: LegacyMediaPlan {
            media,
            total_bytes,
            skipped,
        },
        authored,
    })
}

fn add_avatar<'a>(
    authored: &'a LegacyBackupAuthoredPlan,
    planned: &mut BTreeMap<String, PlannedMedia<'a>>,
    reference: &LegacyMediaReference,
    owner_directory: &str,
    media_use: LegacyMediaUse,
    references: &mut u32,
) -> Result<(), LegacyBackupMediaPlanError> {
    let media = avatar_media(
        &authored.configuration.source.media,
        reference,
        owner_directory,
    )?
    .ok_or_else(|| missing(reference))?;
    add_planned(planned, media, media_use, references)
}

fn add_image<'a>(
    authored: &'a LegacyBackupAuthoredPlan,
    planned: &mut BTreeMap<String, PlannedMedia<'a>>,
    reference: &LegacyMediaReference,
    media_use: LegacyMediaUse,
    references: &mut u32,
) -> Result<(), LegacyBackupMediaPlanError> {
    let media = image_media(&authored.configuration.source.media, reference)?
        .ok_or_else(|| missing(reference))?;
    add_planned(planned, media, media_use, references)
}

fn missing(reference: &LegacyMediaReference) -> LegacyBackupMediaPlanError {
    LegacyBackupMediaPlanError::Missing {
        locator: reference.locator.clone(),
    }
}

fn avatar_media<'a>(
    source: &'a [LegacyBackupMedia],
    reference: &LegacyMediaReference,
    owner_directory: &str,
) -> Result<Option<&'a LegacyBackupMedia>, LegacyBackupMediaPlanError> {
    if !safe_component(&reference.locator, true) || !image_extension(&reference.locator) {
        return Err(LegacyBackupMediaPlanError::Unsafe {
            locator: reference.locator.clone(),
        });
    }
    Ok(exact_media(
        source,
        LegacyBackupMediaRoot::Avatars,
        &[owner_directory, reference.locator.as_str()],
    ))
}

fn image_media<'a>(
    source: &'a [LegacyBackupMedia],
    reference: &LegacyMediaReference,
) -> Result<Option<&'a LegacyBackupMedia>, LegacyBackupMediaPlanError> {
    if !safe_component(&reference.locator, false) {
        return Err(LegacyBackupMediaPlanError::Unsafe {
            locator: reference.locator.clone(),
        });
    }
    let matches = source
        .iter()
        .filter(|media| media.root == LegacyBackupMediaRoot::Images)
        .filter(|media| {
            matches!(media.relative_segments.as_slice(), [filename] if image_stem(filename) == Some(reference.locator.as_str()))
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [media] => Ok(Some(*media)),
        [] => Ok(None),
        _ => Err(LegacyBackupMediaPlanError::Ambiguous {
            locator: reference.locator.clone(),
        }),
    }
}

fn prune_missing_media(
    authored: &mut LegacyBackupAuthoredPlan,
) -> Result<Vec<LegacyImportSkip>, LegacyBackupMediaPlanError> {
    let source = &authored.configuration.source.media;
    let mut skipped = Vec::new();
    for persona in &mut authored.personas.personas {
        let persona_id = persona.id;
        if let Some(avatar) = &persona.avatar
            && avatar_media(source, avatar, &format!("persona-{persona_id}"))?.is_none()
        {
            skipped.push(missing_skip(
                LegacyImportSkipKind::PersonaAvatar,
                persona_id.to_string(),
            ));
            persona.avatar = None;
            persona.avatar_crop = None;
        }
        retain_present(
            source,
            &mut persona.design_references,
            &mut skipped,
            |locator| {
                missing_skip(
                    LegacyImportSkipKind::PersonaDesignReference,
                    format!("{persona_id}:{locator}"),
                )
            },
        )?;
    }
    for lorebook in &mut authored.lorebooks.lorebooks {
        if let Some(avatar) = &lorebook.avatar
            && image_media(source, avatar)?.is_none()
        {
            skipped.push(missing_skip(
                LegacyImportSkipKind::LorebookAvatar,
                lorebook.id.to_string(),
            ));
            lorebook.avatar = None;
        }
    }
    for character in &mut authored.characters {
        let character_id = character.id;
        let skip = |slot: String| {
            missing_skip(
                LegacyImportSkipKind::CharacterMedia,
                format!("{character_id}:{slot}"),
            )
        };
        if let Some(avatar) = &character.media.avatar
            && avatar_media(source, avatar, &format!("character-{character_id}"))?.is_none()
        {
            skipped.push(skip("avatar".into()));
            character.media.avatar = None;
            character.presentation.avatar_crop = None;
        }
        if let Some(background) = &character.media.background
            && image_media(source, background)?.is_none()
        {
            skipped.push(skip("background".into()));
            character.media.background = None;
        }
        retain_present(
            source,
            &mut character.media.design_references,
            &mut skipped,
            |locator| skip(format!("design:{locator}")),
        )?;
        for scene in &mut character.scenes {
            if let Some(background) = &scene.background
                && image_media(source, background)?.is_none()
            {
                skipped.push(skip(format!("scene:{}", scene.id)));
                scene.background = None;
            }
        }
    }
    for group in &mut authored.groups {
        let group_id = group.id;
        if let Some(background) = &group.background
            && image_media(source, background)?.is_none()
        {
            skipped.push(missing_skip(
                LegacyImportSkipKind::GroupMedia,
                format!("{group_id}:background"),
            ));
            group.background = None;
        }
        if let Some(scene) = &mut group.starting_scene
            && let Some(background) = &scene.background
            && image_media(source, background)?.is_none()
        {
            skipped.push(missing_skip(
                LegacyImportSkipKind::GroupMedia,
                format!("{group_id}:scene:{}", scene.id),
            ));
            scene.background = None;
        }
    }
    skipped.sort();
    skipped.dedup();
    Ok(skipped)
}

fn retain_present(
    source: &[LegacyBackupMedia],
    references: &mut Vec<LegacyMediaReference>,
    skipped: &mut Vec<LegacyImportSkip>,
    skip: impl Fn(&str) -> LegacyImportSkip,
) -> Result<(), LegacyBackupMediaPlanError> {
    let mut kept = Vec::with_capacity(references.len());
    for reference in std::mem::take(references) {
        if image_media(source, &reference)?.is_some() {
            kept.push(reference);
        } else {
            skipped.push(skip(&reference.locator));
        }
    }
    *references = kept;
    Ok(())
}

fn missing_skip(kind: LegacyImportSkipKind, source_key: String) -> LegacyImportSkip {
    LegacyImportSkip {
        kind,
        source_key,
        reason: LegacyImportSkipReason::MissingMediaFile,
    }
}

fn add_planned<'a>(
    planned: &mut BTreeMap<String, PlannedMedia<'a>>,
    media: &'a LegacyBackupMedia,
    media_use: LegacyMediaUse,
    references: &mut u32,
) -> Result<(), LegacyBackupMediaPlanError> {
    *references = references
        .checked_add(1)
        .ok_or(LegacyBackupMediaPlanError::ReferenceLimit)?;
    if *references > LEGACY_MEDIA_REFERENCE_LIMIT {
        return Err(LegacyBackupMediaPlanError::ReferenceLimit);
    }
    let path = archive_path(media);
    planned
        .entry(path)
        .or_insert_with(|| PlannedMedia {
            media,
            uses: Vec::new(),
        })
        .uses
        .push(media_use);
    Ok(())
}

fn exact_media<'a>(
    media: &'a [LegacyBackupMedia],
    root: LegacyBackupMediaRoot,
    expected: &[&str],
) -> Option<&'a LegacyBackupMedia> {
    media.iter().find(|media| {
        media.root == root
            && media
                .relative_segments
                .iter()
                .map(String::as_str)
                .eq(expected.iter().copied())
    })
}

pub(crate) fn archive_path(media: &LegacyBackupMedia) -> String {
    let root = match media.root {
        LegacyBackupMediaRoot::Images => "images",
        LegacyBackupMediaRoot::Avatars => "avatars",
        LegacyBackupMediaRoot::Attachments => "attachments",
        LegacyBackupMediaRoot::Sessions => "sessions",
        LegacyBackupMediaRoot::GeneratedImages => "generated_images",
    };
    format!("{root}/{}", media.relative_segments.join("/"))
}

fn safe_component(value: &str, allow_dot: bool) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_')
                || allow_dot && character == '.'
        })
}

fn image_extension(filename: &str) -> bool {
    filename.rsplit_once('.').is_some_and(|(_, extension)| {
        matches!(
            extension.to_ascii_lowercase().as_str(),
            "jpg" | "jpeg" | "png" | "gif" | "webp"
        )
    })
}

fn image_stem(filename: &str) -> Option<&str> {
    let (stem, _) = filename.rsplit_once('.')?;
    image_extension(filename).then_some(stem)
}

fn bounded_ordinal(value: usize) -> Result<u32, LegacyBackupMediaPlanError> {
    u32::try_from(value).map_err(|_| LegacyBackupMediaPlanError::ReferenceLimit)
}

#[cfg(test)]
mod tests {
    use lettuce_types::ContentHash;
    use serde_json::{Value, json};
    use zeroize::Zeroizing;

    use super::*;
    use crate::{
        LegacyBackupDocument, LegacyBackupDocumentKind, LegacyBackupInventory,
        plan_legacy_backup_authored, plan_legacy_backup_configuration,
    };

    fn id(value: u128) -> String {
        uuid::Uuid::from_u128(value).to_string()
    }

    fn document(kind: LegacyBackupDocumentKind, value: Value) -> LegacyBackupDocument {
        LegacyBackupDocument {
            kind,
            bytes: Zeroizing::new(serde_json::to_vec(&value).expect("fixture document")),
        }
    }

    fn media(root: LegacyBackupMediaRoot, segments: &[&str], bytes: &[u8]) -> LegacyBackupMedia {
        LegacyBackupMedia::from_bytes(
            root,
            segments.iter().map(|value| (*value).to_owned()).collect(),
            Zeroizing::new(bytes.to_vec()),
        )
    }

    fn authored(
        documents: Vec<LegacyBackupDocument>,
        media: Vec<LegacyBackupMedia>,
    ) -> LegacyBackupAuthoredPlan {
        let inventory = LegacyBackupInventory {
            version: 1,
            created_at: 1,
            app_version: "legacy".into(),
            source_hash: ContentHash::parse("33".repeat(32)).expect("source hash"),
            documents,
            media,
        };
        let configuration =
            plan_legacy_backup_configuration(inventory).expect("configuration plan");
        plan_legacy_backup_authored(configuration).expect("authored plan")
    }

    #[test]
    fn authored_media_resolves_every_slot_and_deduplicates_shared_images() {
        let persona = id(1);
        let first_character = id(2);
        let second_character = id(3);
        let lorebook = id(4);
        let character_scene = id(5);
        let group = id(6);
        let group_scene = id(7);
        let starting_scene = serde_json::to_string(&json!({
            "id": group_scene,
            "content": "Group scene",
            "backgroundImagePath": "shared",
            "createdAt": 1
        }))
        .expect("group scene");
        let documents = vec![
            document(
                LegacyBackupDocumentKind::Personas,
                json!([{
                    "id": persona,
                    "title": "User",
                    "description": "User profile",
                    "avatar_path": "persona.webp",
                    "design_reference_image_ids": "[\"shared\"]",
                    "created_at": 1,
                    "updated_at": 1
                }]),
            ),
            document(
                LegacyBackupDocumentKind::Lorebooks,
                json!([{
                    "id": lorebook,
                    "name": "World",
                    "avatar_path": "lorebook",
                    "created_at": 1,
                    "updated_at": 1
                }]),
            ),
            document(
                LegacyBackupDocumentKind::Characters,
                json!([
                    {
                        "id": first_character,
                        "name": "Mira",
                        "avatar_path": "mira.png",
                        "background_image_path": "shared",
                        "design_reference_image_ids": "[\"design\"]",
                        "scenes": [{
                            "id": character_scene,
                            "content": "Scene",
                            "background_image_path": "scene",
                            "created_at": 1
                        }],
                        "created_at": 1,
                        "updated_at": 1
                    },
                    {
                        "id": second_character,
                        "name": "Sol",
                        "created_at": 1,
                        "updated_at": 1
                    }
                ]),
            ),
            document(
                LegacyBackupDocumentKind::GroupCharacters,
                json!([{
                    "id": group,
                    "name": "Crew",
                    "character_ids": format!("[\"{first_character}\",\"{second_character}\"]"),
                    "starting_scene": starting_scene,
                    "background_image_path": "group",
                    "created_at": 1,
                    "updated_at": 1
                }]),
            ),
        ];
        let source_media = vec![
            media(
                LegacyBackupMediaRoot::Avatars,
                &[&format!("persona-{persona}"), "persona.webp"],
                b"persona avatar",
            ),
            media(
                LegacyBackupMediaRoot::Avatars,
                &[&format!("character-{first_character}"), "mira.png"],
                b"character avatar",
            ),
            media(LegacyBackupMediaRoot::Images, &["shared.webp"], b"shared"),
            media(LegacyBackupMediaRoot::Images, &["lorebook.png"], b"book"),
            media(LegacyBackupMediaRoot::Images, &["design.jpg"], b"design"),
            media(LegacyBackupMediaRoot::Images, &["scene.webp"], b"scene"),
            media(LegacyBackupMediaRoot::Images, &["group.png"], b"group"),
        ];

        let plan = plan_legacy_backup_authored_media(authored(documents, source_media))
            .expect("media plan");
        assert_eq!(plan.media.media.len(), 7);
        let shared = plan
            .media
            .media
            .iter()
            .find(|candidate| candidate.relative_path == "images/shared.webp")
            .expect("shared image");
        assert_eq!(shared.uses.len(), 3);
        assert!(
            shared
                .uses
                .iter()
                .any(|usage| matches!(usage, LegacyMediaUse::GroupSceneBackground { .. }))
        );
        assert_eq!(
            shared.content_hash,
            ContentHash::parse(blake3::hash(b"shared").to_hex().to_string()).expect("hash")
        );
    }

    #[test]
    fn missing_authored_media_is_cleared_and_recorded_while_ambiguous_images_fail_closed() {
        let persona = id(20);
        let character = id(21);
        let scene = id(22);
        let documents = vec![
            document(
                LegacyBackupDocumentKind::Personas,
                json!([{
                    "id": persona,
                    "title": "User",
                    "description": "User profile",
                    "avatar_path": "gone.png",
                    "avatar_crop_x": 0.1,
                    "avatar_crop_y": 0.1,
                    "avatar_crop_scale": 1.0,
                    "design_reference_image_ids": "[\"shared\"]",
                    "created_at": 1,
                    "updated_at": 1
                }]),
            ),
            document(
                LegacyBackupDocumentKind::Characters,
                json!([{
                    "id": character,
                    "name": "Mira",
                    "avatar_path": "mira.png",
                    "background_image_path": "absent",
                    "design_reference_image_ids": "[\"kept\",\"gone\"]",
                    "scenes": [{
                        "id": scene,
                        "content": "Scene",
                        "background_image_path": "gone",
                        "created_at": 1
                    }],
                    "created_at": 1,
                    "updated_at": 1
                }]),
            ),
        ];
        let plan = plan_legacy_backup_authored_media(authored(
            documents.clone(),
            vec![media(LegacyBackupMediaRoot::Images, &["kept.png"], b"kept")],
        ))
        .expect("missing media is pruned");
        let persona_plan = &plan.authored.personas.personas[0];
        assert!(persona_plan.avatar.is_none() && persona_plan.avatar_crop.is_none());
        assert!(persona_plan.design_references.is_empty());
        let character_plan = &plan.authored.characters[0];
        assert!(character_plan.media.avatar.is_none());
        assert!(character_plan.media.background.is_none());
        assert_eq!(character_plan.media.design_references.len(), 1);
        assert!(character_plan.scenes[0].background.is_none());
        assert_eq!(plan.media.media.len(), 1);
        let keys = plan
            .media
            .skipped
            .iter()
            .map(|skip| (skip.kind, skip.source_key.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            vec![
                (LegacyImportSkipKind::PersonaAvatar, persona.clone()),
                (
                    LegacyImportSkipKind::PersonaDesignReference,
                    format!("{persona}:shared")
                ),
                (
                    LegacyImportSkipKind::CharacterMedia,
                    format!("{character}:avatar")
                ),
                (
                    LegacyImportSkipKind::CharacterMedia,
                    format!("{character}:background")
                ),
                (
                    LegacyImportSkipKind::CharacterMedia,
                    format!("{character}:design:gone")
                ),
                (
                    LegacyImportSkipKind::CharacterMedia,
                    format!("{character}:scene:{scene}")
                ),
            ]
        );
        let ambiguous = plan_legacy_backup_authored_media(authored(
            documents,
            vec![
                media(LegacyBackupMediaRoot::Images, &["shared.png"], b"one"),
                media(LegacyBackupMediaRoot::Images, &["shared.webp"], b"two"),
            ],
        ))
        .expect_err("ambiguous image should fail");
        assert!(matches!(
            ambiguous,
            LegacyBackupMediaPlanError::Ambiguous { .. }
        ));
    }
}
