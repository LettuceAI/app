use std::collections::BTreeMap;

use lettuce_types::ContentHash;

use crate::{
    LEGACY_MEDIA_OBJECT_BYTES_LIMIT, LEGACY_MEDIA_REFERENCE_LIMIT, LEGACY_MEDIA_TOTAL_BYTES_LIMIT,
    LegacyBackupAuthoredPlan, LegacyBackupMedia, LegacyBackupMediaRoot, LegacyMediaCandidate,
    LegacyMediaPlan, LegacyMediaReference, LegacyMediaUse,
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
    authored: LegacyBackupAuthoredPlan,
) -> Result<LegacyBackupAuthoredMediaPlan, LegacyBackupMediaPlanError> {
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
        let byte_len = u64::try_from(item.media.bytes.len()).map_err(|_| {
            LegacyBackupMediaPlanError::ObjectLimit {
                locator: relative_path.clone(),
            }
        })?;
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
        let content_hash = ContentHash::parse(blake3::hash(&item.media.bytes).to_hex().to_string())
            .expect("BLAKE3 produces a valid content hash");
        media.push(LegacyMediaCandidate {
            source_locator: relative_path.clone(),
            relative_path,
            byte_len,
            content_hash,
            uses: item.uses,
        });
    }
    Ok(LegacyBackupAuthoredMediaPlan {
        media: LegacyMediaPlan { media, total_bytes },
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
    if !safe_component(&reference.locator, true) || !image_extension(&reference.locator) {
        return Err(LegacyBackupMediaPlanError::Unsafe {
            locator: reference.locator.clone(),
        });
    }
    let expected = [owner_directory, reference.locator.as_str()];
    let media = exact_media(
        &authored.configuration.source.media,
        LegacyBackupMediaRoot::Avatars,
        &expected,
    )
    .ok_or_else(|| LegacyBackupMediaPlanError::Missing {
        locator: reference.locator.clone(),
    })?;
    add_planned(planned, media, media_use, references)
}

fn add_image<'a>(
    authored: &'a LegacyBackupAuthoredPlan,
    planned: &mut BTreeMap<String, PlannedMedia<'a>>,
    reference: &LegacyMediaReference,
    media_use: LegacyMediaUse,
    references: &mut u32,
) -> Result<(), LegacyBackupMediaPlanError> {
    if !safe_component(&reference.locator, false) {
        return Err(LegacyBackupMediaPlanError::Unsafe {
            locator: reference.locator.clone(),
        });
    }
    let matches = authored
        .configuration
        .source
        .media
        .iter()
        .filter(|media| media.root == LegacyBackupMediaRoot::Images)
        .filter(|media| {
            matches!(media.relative_segments.as_slice(), [filename] if image_stem(filename) == Some(reference.locator.as_str()))
        })
        .collect::<Vec<_>>();
    let media = match matches.as_slice() {
        [media] => *media,
        [] => {
            return Err(LegacyBackupMediaPlanError::Missing {
                locator: reference.locator.clone(),
            });
        }
        _ => {
            return Err(LegacyBackupMediaPlanError::Ambiguous {
                locator: reference.locator.clone(),
            });
        }
    };
    add_planned(planned, media, media_use, references)
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

fn archive_path(media: &LegacyBackupMedia) -> String {
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
        LegacyBackupMedia {
            root,
            relative_segments: segments.iter().map(|value| (*value).to_owned()).collect(),
            bytes: Zeroizing::new(bytes.to_vec()),
        }
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
    fn missing_and_ambiguous_authored_images_fail_closed() {
        let persona = id(20);
        let documents = vec![document(
            LegacyBackupDocumentKind::Personas,
            json!([{
                "id": persona,
                "title": "User",
                "description": "User profile",
                "design_reference_image_ids": "[\"shared\"]",
                "created_at": 1,
                "updated_at": 1
            }]),
        )];
        let missing = plan_legacy_backup_authored_media(authored(documents.clone(), Vec::new()))
            .expect_err("missing image should fail");
        assert!(matches!(
            missing,
            LegacyBackupMediaPlanError::Missing { .. }
        ));
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
