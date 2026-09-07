use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::{Component, Path, PathBuf},
};

use lettuce_transfer::{
    LEGACY_MEDIA_OBJECT_BYTES_LIMIT, LEGACY_MEDIA_REFERENCE_LIMIT, LEGACY_MEDIA_TOTAL_BYTES_LIMIT,
    LegacyDatabasePreflightError, LegacyLorebookPlan, LegacyMediaCandidate, LegacyMediaPlan,
    LegacyMediaUse, LegacyPersonaPlan,
};
use lettuce_types::ContentHash;

struct PendingMedia {
    path: PathBuf,
    locator: String,
    uses: Vec<LegacyMediaUse>,
}

pub fn plan_legacy_media(
    storage_root: impl AsRef<Path>,
    personas: &LegacyPersonaPlan,
    lorebooks: &LegacyLorebookPlan,
) -> Result<LegacyMediaPlan, LegacyDatabasePreflightError> {
    let storage_root = std::fs::canonicalize(storage_root)
        .map_err(|_| LegacyDatabasePreflightError::Unavailable)?;
    if !storage_root.is_dir() {
        return Err(LegacyDatabasePreflightError::Unavailable);
    }
    require_reference_count(personas, lorebooks)?;
    let mut pending = BTreeMap::new();
    for persona in &personas.personas {
        if let Some(avatar) = &persona.avatar {
            let filename = avatar_filename(&avatar.locator)?;
            let relative = PathBuf::from("avatars")
                .join(format!("persona-{}", persona.id))
                .join(filename);
            add_pending(
                &storage_root,
                relative,
                avatar.locator.clone(),
                LegacyMediaUse::PersonaAvatar {
                    persona_id: persona.id,
                },
                &mut pending,
            )?;
        }
        for (ordinal, reference) in persona.design_references.iter().enumerate() {
            let ordinal = u32::try_from(ordinal).map_err(|_| {
                LegacyDatabasePreflightError::MediaReferenceLimitExceeded {
                    limit: LEGACY_MEDIA_REFERENCE_LIMIT,
                }
            })?;
            let relative = image_reference_path(&storage_root, &reference.locator)?;
            add_pending(
                &storage_root,
                relative,
                reference.locator.clone(),
                LegacyMediaUse::PersonaDesignReference {
                    persona_id: persona.id,
                    ordinal,
                },
                &mut pending,
            )?;
        }
    }
    for lorebook in &lorebooks.lorebooks {
        if let Some(avatar) = &lorebook.avatar {
            let relative = image_reference_path(&storage_root, &avatar.locator)?;
            add_pending(
                &storage_root,
                relative,
                avatar.locator.clone(),
                LegacyMediaUse::LorebookAvatar {
                    lorebook_id: lorebook.id,
                },
                &mut pending,
            )?;
        }
    }

    let mut total_bytes = 0_u64;
    for pending in pending.values() {
        let metadata = std::fs::symlink_metadata(&pending.path).map_err(|_| {
            LegacyDatabasePreflightError::MissingMedia {
                locator: pending.locator.clone(),
            }
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(LegacyDatabasePreflightError::UnsafeMediaReference {
                locator: pending.locator.clone(),
            });
        }
        if metadata.len() > LEGACY_MEDIA_OBJECT_BYTES_LIMIT {
            return Err(LegacyDatabasePreflightError::MediaObjectTooLarge {
                locator: pending.locator.clone(),
                limit: LEGACY_MEDIA_OBJECT_BYTES_LIMIT,
            });
        }
        total_bytes = total_bytes.checked_add(metadata.len()).ok_or(
            LegacyDatabasePreflightError::MediaTotalTooLarge {
                limit: LEGACY_MEDIA_TOTAL_BYTES_LIMIT,
            },
        )?;
        if total_bytes > LEGACY_MEDIA_TOTAL_BYTES_LIMIT {
            return Err(LegacyDatabasePreflightError::MediaTotalTooLarge {
                limit: LEGACY_MEDIA_TOTAL_BYTES_LIMIT,
            });
        }
    }
    let mut media = Vec::with_capacity(pending.len());
    for (relative_path, pending) in pending {
        let metadata = std::fs::symlink_metadata(&pending.path).map_err(|_| {
            LegacyDatabasePreflightError::MissingMedia {
                locator: pending.locator.clone(),
            }
        })?;
        let content_hash = hash_file(&pending.path, metadata.len(), &pending.locator)?;
        media.push(LegacyMediaCandidate {
            relative_path,
            byte_len: metadata.len(),
            content_hash,
            uses: pending.uses,
        });
    }
    Ok(LegacyMediaPlan { media, total_bytes })
}

fn require_reference_count(
    personas: &LegacyPersonaPlan,
    lorebooks: &LegacyLorebookPlan,
) -> Result<(), LegacyDatabasePreflightError> {
    let count = personas
        .personas
        .iter()
        .try_fold(0_u64, |count, persona| {
            count
                .checked_add(u64::from(persona.avatar.is_some()))
                .and_then(|count| count.checked_add(persona.design_references.len() as u64))
        })
        .and_then(|count| {
            lorebooks
                .lorebooks
                .iter()
                .try_fold(count, |count, lorebook| {
                    count.checked_add(u64::from(lorebook.avatar.is_some()))
                })
        })
        .ok_or(LegacyDatabasePreflightError::MediaReferenceLimitExceeded {
            limit: LEGACY_MEDIA_REFERENCE_LIMIT,
        })?;
    if count > u64::from(LEGACY_MEDIA_REFERENCE_LIMIT) {
        Err(LegacyDatabasePreflightError::MediaReferenceLimitExceeded {
            limit: LEGACY_MEDIA_REFERENCE_LIMIT,
        })
    } else {
        Ok(())
    }
}

fn avatar_filename(locator: &str) -> Result<&str, LegacyDatabasePreflightError> {
    if !single_component(locator, true)
        || !matches!(
            Path::new(locator)
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("jpg" | "jpeg" | "png" | "gif" | "webp")
        )
    {
        return Err(LegacyDatabasePreflightError::UnsafeMediaReference {
            locator: locator.into(),
        });
    }
    Ok(locator)
}

fn image_reference_path(
    storage_root: &Path,
    locator: &str,
) -> Result<PathBuf, LegacyDatabasePreflightError> {
    if !single_component(locator, false) {
        return Err(LegacyDatabasePreflightError::UnsafeMediaReference {
            locator: locator.into(),
        });
    }
    let matches = ["jpg", "jpeg", "png", "gif", "webp"]
        .into_iter()
        .map(|extension| PathBuf::from("images").join(format!("{locator}.{extension}")))
        .filter(|relative| storage_root.join(relative).exists())
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [relative] => Ok(relative.clone()),
        [] => Err(LegacyDatabasePreflightError::MissingMedia {
            locator: locator.into(),
        }),
        _ => Err(LegacyDatabasePreflightError::ConflictingMediaReference {
            locator: locator.into(),
        }),
    }
}

fn single_component(value: &str, allow_dots: bool) -> bool {
    let path = Path::new(value);
    !value.is_empty()
        && !path.is_absolute()
        && matches!(
            path.components().collect::<Vec<_>>().as_slice(),
            [Component::Normal(_)]
        )
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_')
                || (allow_dots && character == '.')
        })
}

fn add_pending(
    storage_root: &Path,
    relative: PathBuf,
    locator: String,
    usage: LegacyMediaUse,
    pending: &mut BTreeMap<String, PendingMedia>,
) -> Result<(), LegacyDatabasePreflightError> {
    let path = storage_root.join(&relative);
    let canonical =
        std::fs::canonicalize(&path).map_err(|_| LegacyDatabasePreflightError::MissingMedia {
            locator: locator.clone(),
        })?;
    if !canonical.starts_with(storage_root) {
        return Err(LegacyDatabasePreflightError::UnsafeMediaReference { locator });
    }
    let relative_path = relative
        .to_str()
        .ok_or_else(|| LegacyDatabasePreflightError::UnsafeMediaReference {
            locator: locator.clone(),
        })?
        .replace('\\', "/");
    let item = pending
        .entry(relative_path)
        .or_insert_with(|| PendingMedia {
            path,
            locator,
            uses: Vec::new(),
        });
    item.uses.push(usage);
    Ok(())
}

fn hash_file(
    path: &Path,
    expected_len: u64,
    locator: &str,
) -> Result<ContentHash, LegacyDatabasePreflightError> {
    let mut file = File::open(path).map_err(|_| LegacyDatabasePreflightError::MediaReadFailed {
        locator: locator.into(),
    })?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut read_len = 0_u64;
    loop {
        let read =
            file.read(&mut buffer)
                .map_err(|_| LegacyDatabasePreflightError::MediaReadFailed {
                    locator: locator.into(),
                })?;
        if read == 0 {
            break;
        }
        read_len = read_len.checked_add(read as u64).ok_or_else(|| {
            LegacyDatabasePreflightError::MediaReadFailed {
                locator: locator.into(),
            }
        })?;
        if read_len > LEGACY_MEDIA_OBJECT_BYTES_LIMIT {
            return Err(LegacyDatabasePreflightError::MediaObjectTooLarge {
                locator: locator.into(),
                limit: LEGACY_MEDIA_OBJECT_BYTES_LIMIT,
            });
        }
        hasher.update(&buffer[..read]);
    }
    if read_len != expected_len {
        return Err(LegacyDatabasePreflightError::MediaReadFailed {
            locator: locator.into(),
        });
    }
    ContentHash::parse(hasher.finalize().to_hex().to_string()).map_err(|_| {
        LegacyDatabasePreflightError::MediaReadFailed {
            locator: locator.into(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_transfer::{
        LegacyLorebookCandidate, LegacyLorebookDetectionPolicy, LegacyMediaReference,
        LegacyPersonaCandidate,
    };
    use lettuce_types::{LorebookId, MediaBlobId, PersonaId, TimestampMillis};

    fn root() -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("lettuce-legacy-media-{}", MediaBlobId::new()));
        std::fs::create_dir_all(root.join("images")).expect("create images directory");
        root
    }

    fn persona(
        id: PersonaId,
        avatar: Option<&str>,
        design_references: Vec<LegacyMediaReference>,
    ) -> LegacyPersonaCandidate {
        LegacyPersonaCandidate {
            id,
            title: "Reader".into(),
            description: "Reads stories".into(),
            nickname: None,
            avatar: avatar.map(|locator| LegacyMediaReference {
                locator: locator.into(),
            }),
            avatar_crop: None,
            design_description: None,
            design_references,
            image_recommendation: None,
            active_lorebook_ids: Vec::new(),
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        }
    }

    fn lorebook(id: LorebookId, avatar: Option<&str>) -> LegacyLorebookCandidate {
        LegacyLorebookCandidate {
            id,
            name: "World".into(),
            avatar: avatar.map(|locator| LegacyMediaReference {
                locator: locator.into(),
            }),
            detection_policy: LegacyLorebookDetectionPolicy::RecentMessageWindow,
            entries: Vec::new(),
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        }
    }

    #[test]
    fn media_plan_hashes_and_deduplicates_shared_legacy_files() {
        let root = root();
        let persona_id = PersonaId::new();
        let lorebook_id = LorebookId::new();
        let avatar_dir = root.join("avatars").join(format!("persona-{persona_id}"));
        std::fs::create_dir_all(&avatar_dir).expect("create avatar directory");
        std::fs::write(avatar_dir.join("avatar_base.webp"), b"avatar").expect("write avatar");
        std::fs::write(root.join("images/shared.png"), b"shared").expect("write shared image");
        let personas = LegacyPersonaPlan {
            personas: vec![persona(
                persona_id,
                Some("avatar_base.webp"),
                vec![LegacyMediaReference {
                    locator: "shared".into(),
                }],
            )],
            default_persona_id: Some(persona_id),
        };
        let lorebooks = LegacyLorebookPlan {
            lorebooks: vec![lorebook(lorebook_id, Some("shared"))],
        };

        let plan = plan_legacy_media(&root, &personas, &lorebooks).expect("plan media");

        assert_eq!(plan.media.len(), 2);
        assert_eq!(plan.total_bytes, 12);
        assert_eq!(
            plan.media[0].relative_path,
            format!("avatars/persona-{persona_id}/avatar_base.webp")
        );
        assert_eq!(plan.media[0].byte_len, 6);
        assert_eq!(
            plan.media[0].content_hash.as_str(),
            blake3::hash(b"avatar").to_hex().as_str()
        );
        assert_eq!(plan.media[1].relative_path, "images/shared.png");
        assert_eq!(
            plan.media[1].uses,
            [
                LegacyMediaUse::PersonaDesignReference {
                    persona_id,
                    ordinal: 0
                },
                LegacyMediaUse::LorebookAvatar { lorebook_id }
            ]
        );
        std::fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn media_plan_rejects_missing_unsafe_and_ambiguous_locators() {
        let root = root();
        let persona_id = PersonaId::new();
        let missing = LegacyPersonaPlan {
            personas: vec![persona(
                persona_id,
                None,
                vec![LegacyMediaReference {
                    locator: "missing".into(),
                }],
            )],
            default_persona_id: None,
        };
        assert_eq!(
            plan_legacy_media(&root, &missing, &LegacyLorebookPlan { lorebooks: vec![] }),
            Err(LegacyDatabasePreflightError::MissingMedia {
                locator: "missing".into()
            })
        );
        let unsafe_plan = LegacyPersonaPlan {
            personas: vec![persona(persona_id, Some("../avatar.webp"), Vec::new())],
            default_persona_id: None,
        };
        assert_eq!(
            plan_legacy_media(
                &root,
                &unsafe_plan,
                &LegacyLorebookPlan { lorebooks: vec![] }
            ),
            Err(LegacyDatabasePreflightError::UnsafeMediaReference {
                locator: "../avatar.webp".into()
            })
        );
        std::fs::write(root.join("images/conflict.jpg"), b"one").expect("write jpg");
        std::fs::write(root.join("images/conflict.png"), b"two").expect("write png");
        let ambiguous = LegacyPersonaPlan {
            personas: vec![persona(
                persona_id,
                None,
                vec![LegacyMediaReference {
                    locator: "conflict".into(),
                }],
            )],
            default_persona_id: None,
        };
        assert_eq!(
            plan_legacy_media(&root, &ambiguous, &LegacyLorebookPlan { lorebooks: vec![] }),
            Err(LegacyDatabasePreflightError::ConflictingMediaReference {
                locator: "conflict".into()
            })
        );
        std::fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn media_plan_enforces_reference_and_object_bounds() {
        let root = root();
        let persona_id = PersonaId::new();
        let references = (0..=LEGACY_MEDIA_REFERENCE_LIMIT)
            .map(|_| LegacyMediaReference {
                locator: "shared".into(),
            })
            .collect();
        let too_many = LegacyPersonaPlan {
            personas: vec![persona(persona_id, None, references)],
            default_persona_id: None,
        };
        assert_eq!(
            plan_legacy_media(&root, &too_many, &LegacyLorebookPlan { lorebooks: vec![] }),
            Err(LegacyDatabasePreflightError::MediaReferenceLimitExceeded {
                limit: LEGACY_MEDIA_REFERENCE_LIMIT
            })
        );
        let large = File::create(root.join("images/large.png")).expect("create large image");
        large
            .set_len(LEGACY_MEDIA_OBJECT_BYTES_LIMIT + 1)
            .expect("size large image");
        let too_large = LegacyPersonaPlan {
            personas: vec![persona(
                persona_id,
                None,
                vec![LegacyMediaReference {
                    locator: "large".into(),
                }],
            )],
            default_persona_id: None,
        };
        assert_eq!(
            plan_legacy_media(&root, &too_large, &LegacyLorebookPlan { lorebooks: vec![] }),
            Err(LegacyDatabasePreflightError::MediaObjectTooLarge {
                locator: "large".into(),
                limit: LEGACY_MEDIA_OBJECT_BYTES_LIMIT
            })
        );
        std::fs::remove_dir_all(root).expect("remove fixture");
    }
}
