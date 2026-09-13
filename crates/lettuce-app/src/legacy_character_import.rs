use lettuce_transfer::{
    BackupLorebookBindings, LegacyBackupCharacterCandidate, LegacyCharacterMaterializationRequest,
    LegacyImportAdmission, LegacyImportPlan, LegacyImportRepository, LegacyImportRepositoryError,
    LegacyImportStageReceipt,
};
use lettuce_types::{CharacterId, TimestampMillis};

#[derive(Debug)]
pub struct LegacyCharacterImportCoordinator<'a, R: ?Sized> {
    repository: &'a R,
}

impl<'a, R: LegacyImportRepository + ?Sized> LegacyCharacterImportCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    /// Writes the planned legacy characters of an admitted run once its
    /// persona, lorebook, media, provider and prompt stages completed.
    pub fn execute(
        &self,
        admission: &LegacyImportAdmission,
        plan: &LegacyImportPlan,
        characters: &[LegacyBackupCharacterCandidate],
        character_lorebooks: &[BackupLorebookBindings<CharacterId>],
        completed_at: TimestampMillis,
    ) -> Result<LegacyImportStageReceipt, LegacyImportRepositoryError> {
        let plan_fingerprint = crate::legacy_import::plan_fingerprint(plan);
        if plan_fingerprint != admission.plan_fingerprint {
            return Err(LegacyImportRepositoryError::Conflict);
        }
        let source_fingerprint = plan
            .source_fingerprint
            .clone()
            .ok_or(LegacyImportRepositoryError::InvalidInput)?;
        self.repository
            .materialize_characters(LegacyCharacterMaterializationRequest {
                run_id: admission.run_id,
                plan_fingerprint,
                source_fingerprint,
                media: plan.media.clone(),
                characters: characters.to_vec(),
                character_lorebooks: character_lorebooks.to_vec(),
                completed_at,
            })
    }

    pub fn execute_database_import(
        &self,
        admission: &LegacyImportAdmission,
        import: &crate::LegacyDatabaseImportPlan,
        completed_at: TimestampMillis,
    ) -> Result<LegacyImportStageReceipt, LegacyImportRepositoryError> {
        let authored = import.compatibility.authored_plan();
        self.execute(
            admission,
            &import.plan,
            &authored.characters,
            &authored.character_lorebooks,
            completed_at,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use lettuce_characters::{
        CardStyle, CharacterProfile, CharacterProvenance, CharacterRepository, ChatAppearanceV1,
        ChatMode, GradientSource, GroupMember, GroupRepository, InteractionMode, LifecycleStatus,
        MemoryPolicy, SceneDocumentV1, ScenePart, Selection, SpeakerSelection, StarterMessage,
        StarterRole,
    };
    use lettuce_companions::CompanionSoulConfig;
    use lettuce_context::{
        CharacterLorebookBindingRepository, GroupLorebookBindingRepository, LorebookBinding,
        PromptEntryDraft, PromptEntryPosition, PromptEntryRole, PromptPurpose,
    };
    use lettuce_settings::InMemorySecretStore;
    use lettuce_transfer::LegacyBackupGroupCandidate;
    use lettuce_transfer::{
        LegacyAsrPlan, LegacyBackupCharacterDefaults, LegacyBackupCharacterMedia,
        LegacyBackupCharacterPresentation, LegacyBackupSceneCandidate,
        LegacyBackupSceneVariantCandidate, LegacyBackupStarterCandidate, LegacyDatabaseInventory,
        LegacyImportAssignment, LegacyLorebookCandidate, LegacyLorebookDetectionPolicy,
        LegacyLorebookPlan, LegacyMediaPlan, LegacyPersonaPlan, LegacyPromptCandidate,
        LegacyPromptEntryCandidate, LegacyPromptPlan, LegacyProviderModelPlan,
    };
    use lettuce_types::{
        ContentHash, ConversationStarterId, GroupId, LegacyImportRunId, LorebookId, Revision,
        SceneId, SceneVariantId, StarterMessageId,
    };

    use super::*;
    use crate::AppBackend;

    fn scene_text(text: &str) -> SceneDocumentV1 {
        SceneDocumentV1::new(vec![ScenePart::Text { text: text.into() }]).expect("scene text")
    }

    #[tokio::test]
    async fn legacy_characters_and_groups_materialize_with_remapped_references_and_replay() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-app-legacy-characters-{}.sqlite3",
            LegacyImportRunId::new()
        ));
        let prompt_source = "legacy-direct-prompt".to_owned();
        let legacy_lorebook = LorebookId::new();
        let plan = LegacyImportPlan {
            provider_models: LegacyProviderModelPlan {
                skipped: Vec::new(),
                provider_accounts: Vec::new(),
                model_profiles: Vec::new(),
                default_provider_account_id: None,
                default_model_profile_id: None,
            },
            prompts: LegacyPromptPlan {
                skipped: Vec::new(),
                prompts: vec![LegacyPromptCandidate {
                    source_id: prompt_source.clone(),
                    name: "Imported Direct Prompt".to_owned(),
                    purpose: PromptPurpose::DirectChat,
                    entries: vec![LegacyPromptEntryCandidate {
                        source_id: "system-entry".to_owned(),
                        draft: PromptEntryDraft {
                            built_in_entry_key: None,
                            name: "System".to_owned(),
                            role: PromptEntryRole::System,
                            content: "Stay in character".to_owned(),
                            enabled: true,
                            injection_position: PromptEntryPosition::Relative,
                            depth: 0,
                            conditional_min_messages: None,
                            interval_turns: None,
                            system_prompt: true,
                            conditions: None,
                            payload: None,
                        },
                    }],
                    condense: false,
                    created_at: TimestampMillis::new(2),
                    updated_at: TimestampMillis::new(2),
                }],
                default_prompt_source_id: None,
                deprecated_system_prompt: None,
            },
            personas: LegacyPersonaPlan {
                skipped: Vec::new(),
                personas: Vec::new(),
                default_persona_id: None,
            },
            lorebooks: LegacyLorebookPlan {
                skipped: Vec::new(),
                lorebooks: vec![LegacyLorebookCandidate {
                    id: legacy_lorebook,
                    name: "World".to_owned(),
                    avatar: None,
                    detection_policy: LegacyLorebookDetectionPolicy::RecentMessageWindow,
                    entries: Vec::new(),
                    created_at: TimestampMillis::new(2),
                    updated_at: TimestampMillis::new(2),
                }],
            },
            asr: LegacyAsrPlan {
                vocabulary: Vec::new(),
                corrections: Vec::new(),
                ignored_suggestions: Vec::new(),
                voice_examples: Vec::new(),
            },
            media: LegacyMediaPlan {
                media: Vec::new(),
                total_bytes: 0,
                skipped: Vec::new(),
            },
            source_fingerprint: Some(ContentHash::parse("55".repeat(32)).expect("source hash")),
            later_skips: Vec::new(),
        };
        let inventory = LegacyDatabaseInventory {
            schema_version: 92,
            provider_accounts: 0,
            models: 0,
            prompts: 1,
            personas: 0,
            characters: 2,
            lorebooks: 1,
            chat_templates: 1,
            direct_conversations: 0,
            group_profiles: 1,
            group_conversations: 0,
        };
        let character_id = CharacterId::new();
        let scene_id = SceneId::new();
        let variant_id = SceneVariantId::new();
        let starter_id = ConversationStarterId::new();
        let character = LegacyBackupCharacterCandidate {
            id: character_id,
            profile: CharacterProfile {
                name: "Mira".to_owned(),
                nickname: None,
                description: Some("A harbor pilot".to_owned()),
                definition: Some("Navigator".to_owned()),
                design_description: None,
                scenario: Some("At the harbor".to_owned()),
                rules: vec!["Stay kind".to_owned()],
            },
            provenance: CharacterProvenance::default(),
            defaults: LegacyBackupCharacterDefaults {
                interaction_mode: InteractionMode::Companion,
                memory_policy: MemoryPolicy::Manual,
                model_profile_id: None,
                default_scene_id: Some(scene_id),
                default_starter_source_id: Some("template-1".to_owned()),
                direct_prompt_source_id: Some(prompt_source.clone()),
                group_conversation_prompt_source_id: Some("deleted-prompt".to_owned()),
                group_roleplay_prompt_source_id: None,
                system_prompt: Some("Retained in the legacy source".to_owned()),
                companion_soul: Some(CompanionSoulConfig::default()),
                companion_prompt_source_id: Some(prompt_source.clone()),
                voice: None,
                voice_autoplay: true,
            },
            presentation: LegacyBackupCharacterPresentation {
                card_style: CardStyle::Circle,
                avatar_crop: None,
                banner_crop: None,
                disable_gradient: false,
                gradient_source: GradientSource::Base,
                custom_gradient_enabled: false,
                custom_gradient_colors: Vec::new(),
                primary_text_color: None,
                secondary_text_color: None,
                chat_appearance: ChatAppearanceV1::default(),
            },
            media: LegacyBackupCharacterMedia::default(),
            image_recommendation: None,
            active_lorebook_ids: vec![legacy_lorebook],
            scenes: vec![LegacyBackupSceneCandidate {
                id: scene_id,
                ordinal: 0,
                content: scene_text("A quiet harbor"),
                direction: None,
                background: None,
                selected_variant_id: Some(variant_id),
                variants: vec![LegacyBackupSceneVariantCandidate {
                    id: variant_id,
                    ordinal: 0,
                    content: scene_text("A rainy harbor"),
                    direction: None,
                    created_at: TimestampMillis::new(5),
                }],
                created_at: TimestampMillis::new(5),
            }],
            starters: vec![LegacyBackupStarterCandidate {
                id: starter_id,
                source_id: "template-1".to_owned(),
                name: "Opening".to_owned(),
                ordinal: 0,
                messages: vec![StarterMessage {
                    id: StarterMessageId::new(),
                    role: StarterRole::Assistant,
                    content: "Welcome aboard".to_owned(),
                }],
                scene_id: Some(scene_id),
                prompt_source_id: Some(prompt_source.clone()),
                lorebook_ids: Some(vec![legacy_lorebook]),
                created_at: TimestampMillis::new(5),
            }],
            created_at: TimestampMillis::new(5),
            updated_at: TimestampMillis::new(6),
        };
        let bindings = vec![BackupLorebookBindings {
            owner_id: character_id,
            bindings: vec![LorebookBinding {
                lorebook_id: legacy_lorebook,
                enabled: true,
                ordinal: 0,
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(5),
                updated_at: TimestampMillis::new(6),
            }],
        }];
        let second_id = CharacterId::new();
        let second = LegacyBackupCharacterCandidate {
            id: second_id,
            defaults: LegacyBackupCharacterDefaults {
                interaction_mode: InteractionMode::Roleplay,
                default_scene_id: None,
                default_starter_source_id: None,
                companion_soul: None,
                companion_prompt_source_id: None,
                ..character.defaults.clone()
            },
            active_lorebook_ids: Vec::new(),
            scenes: Vec::new(),
            starters: Vec::new(),
            ..character.clone()
        };
        let characters = vec![character.clone(), second];
        let group_id = GroupId::new();
        let group_scene = SceneId::new();
        let group_variant = SceneVariantId::new();
        let group = LegacyBackupGroupCandidate {
            id: group_id,
            status: LifecycleStatus::Active,
            name: "Harbor crew".to_owned(),
            chat_mode: ChatMode::Roleplay,
            persona: Selection::Inherit,
            speaker_selection: SpeakerSelection::Director,
            memory_policy: MemoryPolicy::Manual,
            disable_character_lorebooks: false,
            group_conversation_prompt_source_id: Some("deleted-group-prompt".to_owned()),
            group_roleplay_prompt_source_id: None,
            chat_appearance: ChatAppearanceV1::default(),
            members: vec![
                GroupMember {
                    character_id,
                    ordinal: 0,
                    muted: false,
                    model_profile_override: None,
                },
                GroupMember {
                    character_id: second_id,
                    ordinal: 1,
                    muted: true,
                    model_profile_override: None,
                },
            ],
            starting_scene: Some(LegacyBackupSceneCandidate {
                id: group_scene,
                ordinal: 0,
                content: scene_text("The crew gathers"),
                direction: None,
                background: None,
                selected_variant_id: Some(group_variant),
                variants: vec![LegacyBackupSceneVariantCandidate {
                    id: group_variant,
                    ordinal: 0,
                    content: scene_text("The crew argues"),
                    direction: None,
                    created_at: TimestampMillis::new(7),
                }],
                created_at: TimestampMillis::new(7),
            }),
            background: None,
            lorebook_ids: vec![legacy_lorebook],
            created_at: TimestampMillis::new(7),
            updated_at: TimestampMillis::new(8),
        };
        let group_bindings = vec![BackupLorebookBindings {
            owner_id: group_id,
            bindings: vec![LorebookBinding {
                lorebook_id: legacy_lorebook,
                enabled: true,
                ordinal: 0,
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(7),
                updated_at: TimestampMillis::new(8),
            }],
        }];
        let run_id = LegacyImportRunId::new();
        let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("open backend");
        let admission = backend
            .legacy_import_admission()
            .admit(run_id, &inventory, &plan, TimestampMillis::new(20))
            .expect("admit import");
        let prompt_id = admission
            .assignments
            .iter()
            .find_map(|assignment| match assignment {
                LegacyImportAssignment::Prompt {
                    legacy_id,
                    destination_id,
                } if legacy_id == &prompt_source => Some(*destination_id),
                _ => None,
            })
            .expect("prompt assignment");
        let lorebook_id = admission
            .assignments
            .iter()
            .find_map(|assignment| match assignment {
                LegacyImportAssignment::Lorebook {
                    legacy_id,
                    destination_id,
                } if *legacy_id == legacy_lorebook => Some(*destination_id),
                _ => None,
            })
            .expect("lorebook assignment");
        backend
            .legacy_import_executor()
            .execute(&admission, &plan, TimestampMillis::new(30))
            .expect("materialize authored graph");
        assert_eq!(
            backend
                .legacy_character_importer()
                .execute(
                    &admission,
                    &plan,
                    &characters,
                    &bindings,
                    TimestampMillis::new(35),
                )
                .expect_err("characters wait for the prompt stage"),
            LegacyImportRepositoryError::Conflict
        );
        backend
            .legacy_provider_model_importer(&InMemorySecretStore::new())
            .execute(&admission, &plan, TimestampMillis::new(40))
            .await
            .expect("materialize prompts");
        assert_eq!(
            backend
                .legacy_group_importer()
                .execute(
                    &admission,
                    &plan,
                    std::slice::from_ref(&group),
                    &group_bindings,
                    TimestampMillis::new(45),
                )
                .expect_err("groups wait for the characters stage"),
            LegacyImportRepositoryError::Conflict
        );

        let receipt = backend
            .legacy_character_importer()
            .execute(
                &admission,
                &plan,
                &characters,
                &bindings,
                TimestampMillis::new(50),
            )
            .expect("materialize characters");

        assert_eq!(receipt.record_count, 2);
        assert!(!receipt.replayed);
        let details = CharacterRepository::get(backend.database(), character_id)
            .expect("read character")
            .expect("character exists");
        assert_eq!(
            details.character.profile.scenario.as_deref(),
            Some("At the harbor")
        );
        assert_eq!(details.character.profile.rules, ["Stay kind"]);
        let defaults = &details.character.defaults;
        assert_eq!(defaults.direct_prompt_id, Some(prompt_id));
        assert_eq!(defaults.group_conversation_prompt_id, None);
        assert_eq!(defaults.default_scene_id, Some(scene_id));
        assert_eq!(defaults.default_starter_id, Some(starter_id));
        assert_eq!(
            defaults
                .companion_soul
                .as_ref()
                .and_then(|soul| soul.prompting.prompt_template_id),
            Some(prompt_id)
        );
        assert_eq!(details.scenes.len(), 1);
        assert_eq!(details.variants.len(), 1);
        assert_eq!(
            details.starters[0].lorebooks,
            Selection::Explicit(vec![lorebook_id])
        );
        let bound = CharacterLorebookBindingRepository::list_character_bindings(
            backend.database(),
            character_id,
        )
        .expect("character bindings");
        assert_eq!(bound.len(), 1);
        assert_eq!(bound[0].lorebook_id, lorebook_id);
        let group_receipt = backend
            .legacy_group_importer()
            .execute(
                &admission,
                &plan,
                std::slice::from_ref(&group),
                &group_bindings,
                TimestampMillis::new(55),
            )
            .expect("materialize groups");
        assert_eq!(group_receipt.record_count, 1);
        let group_details = GroupRepository::get(backend.database(), group_id)
            .expect("read group")
            .expect("group exists");
        assert_eq!(group_details.group.members.len(), 2);
        assert!(group_details.group.members[1].muted);
        assert_eq!(group_details.group.group_conversation_prompt_id, None);
        assert_eq!(
            group_details
                .starting_scene
                .as_ref()
                .map(|scene| scene.variants.len()),
            Some(1)
        );
        let group_bound =
            GroupLorebookBindingRepository::list_group_bindings(backend.database(), group_id)
                .expect("group bindings");
        assert_eq!(group_bound.len(), 1);
        assert_eq!(group_bound[0].lorebook_id, lorebook_id);
        drop(backend);

        let reopened = AppBackend::open(&path, TimestampMillis::new(60)).expect("reopen backend");
        let replayed_admission = reopened
            .legacy_import_admission()
            .admit(run_id, &inventory, &plan, TimestampMillis::new(70))
            .expect("replay admission");
        let replay = reopened
            .legacy_character_importer()
            .execute(
                &replayed_admission,
                &plan,
                &characters,
                &bindings,
                TimestampMillis::new(80),
            )
            .expect("replay characters");
        assert!(replay.replayed);
        assert_eq!(replay.completed_at, TimestampMillis::new(50));
        let group_replay = reopened
            .legacy_group_importer()
            .execute(
                &replayed_admission,
                &plan,
                std::slice::from_ref(&group),
                &group_bindings,
                TimestampMillis::new(90),
            )
            .expect("replay groups");
        assert!(group_replay.replayed);
        assert_eq!(group_replay.completed_at, TimestampMillis::new(55));
        drop(reopened);
        fs::remove_file(path).expect("remove database");
    }
}
