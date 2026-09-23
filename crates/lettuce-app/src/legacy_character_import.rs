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
        let legacy_provider = lettuce_types::ProviderAccountId::new();
        let legacy_model = lettuce_types::ModelProfileId::new();
        let plan = LegacyImportPlan {
            provider_models: LegacyProviderModelPlan {
                skipped: Vec::new(),
                provider_accounts: vec![lettuce_transfer::LegacyProviderAccountCandidate {
                    id: legacy_provider,
                    origin: lettuce_transfer::LegacyProviderAccountOrigin::Stored,
                    secret_owner_id: lettuce_settings::SecretOwnerId::from_uuid(
                        legacy_provider.as_uuid(),
                    ),
                    provider_kind: "openai".to_owned(),
                    protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
                    label: "Primary".to_owned(),
                    endpoint: None,
                    enabled: true,
                    streaming_enabled: true,
                    allow_invalid_tls: false,
                    default_model: None,
                    config: lettuce_models::ProviderConfig::Standard,
                    pending_secrets: Vec::new(),
                    deferred_config_fields: Vec::new(),
                    created_at: TimestampMillis::new(1),
                    updated_at: TimestampMillis::new(1),
                }],
                model_profiles: vec![lettuce_transfer::LegacyModelProfileCandidate {
                    id: legacy_model,
                    provider_account_id: legacy_provider,
                    source_provider_kind: "openai".to_owned(),
                    source_provider_label: "Primary".to_owned(),
                    external_model_id: "gpt-example".to_owned(),
                    display_name: "Example Chat".to_owned(),
                    kind: lettuce_models::ModelKind::Chat,
                    config: lettuce_models::ModelProfileConfig {
                        llama_cpp: Default::default(),
                        stable_diffusion: Default::default(),
                        chat_parameters: Default::default(),
                        feature_parameters: Default::default(),
                        capabilities: lettuce_models::ModelCapabilities {
                            input_modalities: lettuce_models::ModalityCapabilities {
                                text: lettuce_models::CapabilityStatus::Supported,
                                ..Default::default()
                            },
                            output_modalities: lettuce_models::ModalityCapabilities {
                                text: lettuce_models::CapabilityStatus::Supported,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    },
                    prompt_template_id: None,
                    deprecated_system_prompt: None,
                    deferred_advanced_fields: Vec::new(),
                    created_at: TimestampMillis::new(1),
                }],
                default_provider_account_id: Some(legacy_provider),
                default_model_profile_id: Some(legacy_model),
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
            provider_accounts: 1,
            models: 1,
            prompts: 1,
            personas: 0,
            characters: 4,
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
        let second_scene_id = SceneId::new();
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
            scenes: vec![LegacyBackupSceneCandidate {
                id: second_scene_id,
                ordinal: 0,
                content: scene_text("A night office"),
                direction: None,
                background: None,
                selected_variant_id: None,
                variants: Vec::new(),
                created_at: TimestampMillis::new(5),
            }],
            starters: Vec::new(),
            ..character.clone()
        };
        let third_id = CharacterId::new();
        let third = LegacyBackupCharacterCandidate {
            id: third_id,
            ..second.clone()
        };
        let companion_id = CharacterId::new();
        let companion_character = LegacyBackupCharacterCandidate {
            id: companion_id,
            defaults: LegacyBackupCharacterDefaults {
                interaction_mode: InteractionMode::Companion,
                companion_soul: Some(CompanionSoulConfig::default()),
                companion_prompt_source_id: None,
                ..second.defaults.clone()
            },
            ..second.clone()
        };
        let characters = vec![character.clone(), second, third, companion_character];
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
                    character_id: second_id,
                    ordinal: 0,
                    muted: false,
                    model_profile_override: None,
                },
                GroupMember {
                    character_id: third_id,
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
        let scope = lettuce_transfer::LegacyIdScope::new(
            plan.source_fingerprint
                .as_ref()
                .expect("source fingerprint"),
        );
        let conv = |id: lettuce_types::ConversationId| {
            lettuce_types::ConversationId::from_uuid(scope.uuid(id.as_uuid()))
        };
        let character = |id: CharacterId| CharacterId::from_uuid(scope.uuid(id.as_uuid()));

        assert_eq!(receipt.record_count, 4);
        assert!(!receipt.replayed);
        let details = CharacterRepository::get(backend.database(), character(character_id))
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
        assert_eq!(
            defaults.default_scene_id,
            Some(SceneId::from_uuid(scope.uuid(scene_id.as_uuid())))
        );
        assert_eq!(
            defaults.default_starter_id,
            Some(ConversationStarterId::from_uuid(
                scope.uuid(starter_id.as_uuid())
            ))
        );
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
            character(character_id),
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
        let group_details = GroupRepository::get(
            backend.database(),
            GroupId::from_uuid(scope.uuid(group_id.as_uuid())),
        )
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
        let group_bound = GroupLorebookBindingRepository::list_group_bindings(
            backend.database(),
            GroupId::from_uuid(scope.uuid(group_id.as_uuid())),
        )
        .expect("group bindings");
        assert_eq!(group_bound.len(), 1);
        assert_eq!(group_bound[0].lorebook_id, lorebook_id);

        let session_id = lettuce_types::ConversationId::new();
        let first_variant = lettuce_types::MessageCandidateId::new().to_string();
        let second_variant = lettuce_types::MessageCandidateId::new().to_string();
        let usage = |prompt_tokens, completion_tokens| lettuce_transfer::LegacyBackupMessageUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens: None,
            first_token_ms: None,
            tokens_per_second: None,
            mtp_stats_json: None,
        };
        let message = |role: &str, ordinal, content: &str, created_at, variants, selected| {
            lettuce_transfer::LegacyBackupDirectMessage {
                source_id: lettuce_types::MessageId::new().to_string(),
                ordinal,
                role: role.to_owned(),
                content: content.to_owned(),
                created_at,
                effective_at: None,
                visible_in_chat: false,
                scene_edited: false,
                usage: usage(None, None),
                model_source_id: None,
                selected_variant_source_id: selected,
                pinned: false,
                memory_refs_json: "[]".to_owned(),
                used_lorebook_entries_json: "[]".to_owned(),
                attachments_json: "[]".to_owned(),
                reasoning: None,
                parent_message_source_id: None,
                variants,
            }
        };
        let session = lettuce_transfer::LegacyBackupDirectSession {
            source_id: session_id.to_string(),
            character_source_id: second_id.to_string(),
            title: "Night shift".to_owned(),
            parent_session_source_id: None,
            branched_from_message_source_id: None,
            root_session_source_id: session_id.to_string(),
            background_image_locator: None,
            deprecated_system_prompt: None,
            mode: "roleplay".to_owned(),
            selected_scene_source_id: None,
            author_note: Some("Keep replies short".to_owned()),
            persona_source_id: None,
            persona_disabled: false,
            voice_autoplay: None,
            prompt_source_id: None,
            lorebook_source_ids_override: Some(vec![legacy_lorebook.to_string()]),
            generation_settings: lettuce_transfer::LegacyBackupSessionGenerationSettings {
                model_settings: Default::default(),
            },
            companion_state_json: None,
            memories_json: "[]".to_owned(),
            memory_embeddings_json: "[]".to_owned(),
            memory_summary: None,
            memory_summary_token_count: 0,
            memory_tool_events_json: "[]".to_owned(),
            memory_status: None,
            memory_error: None,
            memory_progress_step: None,
            archived: false,
            created_at: 100,
            updated_at: 130,
            messages: vec![
                message("user", 0, "Hello", 110, Vec::new(), None),
                message(
                    "assistant",
                    1,
                    "Second take",
                    120,
                    vec![
                        lettuce_transfer::LegacyBackupDirectMessageVariant {
                            source_id: first_variant.clone(),
                            ordinal: 0,
                            content: "First take".to_owned(),
                            created_at: 120,
                            usage: usage(Some(12), Some(4)),
                            reasoning: None,
                        },
                        lettuce_transfer::LegacyBackupDirectMessageVariant {
                            source_id: second_variant.clone(),
                            ordinal: 1,
                            content: "Second take".to_owned(),
                            created_at: 125,
                            usage: usage(None, None),
                            reasoning: None,
                        },
                    ],
                    Some(second_variant.clone()),
                ),
            ],
        };
        let memory_id = lettuce_types::MemoryId::new();
        let session_memory = lettuce_transfer::LegacyBackupMemoryEmbeddingOwner {
            ordinal: 0,
            source_id: session_id.to_string(),
            kind: lettuce_transfer::LegacyBackupMemoryOwnerKind::DirectConversation,
            memory_embeddings_json: "[]".to_owned(),
            memories: vec![lettuce_transfer::LegacyBackupMemoryEmbedding {
                ordinal: 0,
                id: memory_id.to_string(),
                text: "The user likes night shifts".to_owned(),
                embedding: vec![0.25; 64],
                created_at: 115,
                token_count: 6,
                is_cold: false,
                last_accessed_at: 118,
                importance_score: 0.8,
                persistence_importance: 0.6,
                prompt_importance: 0.7,
                volatility: 0.4,
                is_pinned: true,
                access_count: 2,
                embedding_source_version: Some("v4".to_owned()),
                embedding_dimensions: Some(64),
                match_score: None,
                category: Some("preference".to_owned()),
                observed_at: None,
                observed_time_precision: None,
                canonical_entities: Vec::new(),
                fact_signature: None,
                fact_polarity: None,
                source_role: None,
                source_message_id: None,
                superseded_by: None,
                superseded_at: None,
                supersedes: Vec::new(),
                materialization:
                    lettuce_transfer::LegacyBackupMemoryMaterialization::InitialItemAndProjection,
            }],
        };
        let companion_first_id = lettuce_types::ConversationId::new();
        let companion_second_id = lettuce_types::ConversationId::new();
        let companion_session =
            |id: lettuce_types::ConversationId, created_at: u64, state: Option<String>| {
                lettuce_transfer::LegacyBackupDirectSession {
                    source_id: id.to_string(),
                    character_source_id: companion_id.to_string(),
                    title: "Evening walk".to_owned(),
                    root_session_source_id: id.to_string(),
                    author_note: None,
                    lorebook_source_ids_override: None,
                    mode: "companion".to_owned(),
                    companion_state_json: state,
                    created_at,
                    updated_at: created_at + 10,
                    messages: vec![message(
                        "user",
                        0,
                        "Good evening",
                        created_at + 1,
                        Vec::new(),
                        None,
                    )],
                    ..session.clone()
                }
            };
        let companion_memory = lettuce_transfer::LegacyBackupMemoryEmbeddingOwner {
            source_id: companion_second_id.to_string(),
            memories: vec![lettuce_transfer::LegacyBackupMemoryEmbedding {
                id: lettuce_types::MemoryId::new().to_string(),
                text: "Nia remembers the lighthouse".to_owned(),
                ..session_memory.memories[0].clone()
            }],
            ..session_memory.clone()
        };
        let legacy_state = r#"{"emotionalState":{"felt":{"warmth":0.4},"confidence":0.7,"updatedAt":100},"relationshipState":{"closeness":0.6,"trust":0.5,"affection":0.3,"tension":0.1,"stability":0.6,"interactionCount":3,"lastInteractionAt":90},"activeSignals":["curious"],"updatedAt":120}"#;
        let companion_third_id = lettuce_types::ConversationId::new();
        let scene_session_id = lettuce_types::ConversationId::new();
        let scene_session = lettuce_transfer::LegacyBackupDirectSession {
            source_id: scene_session_id.to_string(),
            character_source_id: second_id.to_string(),
            root_session_source_id: scene_session_id.to_string(),
            selected_scene_source_id: Some(second_scene_id.to_string()),
            lorebook_source_ids_override: None,
            created_at: 600,
            updated_at: 610,
            messages: vec![message(
                "user",
                0,
                "Back at the dock",
                601,
                Vec::new(),
                None,
            )],
            ..session.clone()
        };
        let direct_sessions = vec![
            session.clone(),
            scene_session,
            companion_session(companion_first_id, 300, Some(legacy_state.to_owned())),
            companion_session(companion_second_id, 400, None),
            companion_session(companion_third_id, 500, None),
        ];
        let direct_memories = vec![session_memory.clone(), companion_memory];
        let companion_shared = lettuce_transfer::LegacyBackupCompanionSharedMemory {
            ordinal: 0,
            character_id: companion_id,
            memories_json: "[]".to_owned(),
            memory_embeddings_json: "[]".to_owned(),
            memory_summary: None,
            memory_summary_token_count: 0,
            memory_tool_events_json: "[]".to_owned(),
            memory_status: None,
            memory_error: None,
            memory_progress_step: None,
            soul_growth_json: "[]".to_owned(),
            soul_facts: None,
            relationship_states_json: "[]".to_owned(),
            relationship_states: Vec::new(),
            episodes: vec![lettuce_transfer::LegacyBackupCompanionEpisode {
                ordinal: 0,
                conversation_source_id: companion_second_id.to_string(),
                persona_id: None,
                episode_index: 1,
                previous_conversation_source_id: None,
                started_at: 400,
                ended_at: None,
                updated_at: 405,
            }],
            created_at: 300,
            updated_at: 405,
            memory_materialization:
                lettuce_transfer::LegacyBackupCompanionMaterialization::RetainedEvidence,
            soul_materialization:
                lettuce_transfer::LegacyBackupCompanionMaterialization::RetainedEvidence,
            relationship_materialization:
                lettuce_transfer::LegacyBackupCompanionMaterialization::RetainedEvidence,
        };
        let conversation_receipt = backend
            .legacy_direct_conversation_importer()
            .execute(
                &admission,
                &plan,
                &direct_sessions,
                &direct_memories,
                &[companion_shared],
                &[],
                TimestampMillis::new(57),
            )
            .expect("materialize direct conversations");
        assert_eq!(conversation_receipt.record_count, 5);
        let graph =
            lettuce_transfer::ProviderBackupSource::read_provider_backup_graph(backend.database())
                .expect("backup graph");
        let history = graph
            .conversation_history
            .conversations
            .iter()
            .find(|history| history.aggregate.conversation.id == conv(session_id))
            .expect("direct conversation history");
        assert_eq!(history.aggregate.conversation.id, conv(session_id));
        assert_eq!(history.messages.len(), 2);
        let settings = history
            .aggregate
            .conversation
            .current_settings
            .as_ref()
            .expect("session settings");
        assert_eq!(settings.author_note.as_deref(), Some("Keep replies short"));
        assert_eq!(
            settings
                .lorebooks
                .as_ref()
                .map(|books| books.iter().map(|book| book.source_id).collect::<Vec<_>>()),
            Some(vec![lorebook_id])
        );
        let reply = &history.messages[1];
        assert_eq!(reply.candidates.len(), 2);
        assert_eq!(
            reply.message.active_render_source,
            lettuce_conversations::MessageRenderSource::Candidate(
                lettuce_types::MessageCandidateId::from_uuid(scope.source(&second_variant))
            )
        );
        assert_eq!(
            graph
                .conversation_runtime
                .conversations
                .iter()
                .find(|runtime| runtime.conversation_id == conv(session_id))
                .expect("direct runtime")
                .turns
                .len(),
            2
        );
        let space = graph
            .memory
            .spaces
            .iter()
            .find(|space| space.conversation_id == conv(session_id))
            .expect("imported memory space");
        assert_eq!(space.snapshot.items.len(), 1);
        assert_eq!(space.snapshot.items[0].text, "The user likes night shifts");
        assert!(space.snapshot.items[0].is_pinned);
        assert_eq!(graph.memory_projections.projections.len(), 3);
        let scene_conversation = lettuce_conversations::ConversationReader::get(
            backend.database(),
            conv(scene_session_id),
        )
        .expect("scene conversation")
        .conversation;
        assert_eq!(
            lettuce_conversations::resolve_effective_settings(&scene_conversation, None)
                .expect("effective settings")
                .scene
                .map(|scene| scene.source_id),
            Some(SceneId::from_uuid(scope.uuid(second_scene_id.as_uuid()))),
            "a selected scene survives when the chat does not open with it"
        );
        let episodes = [companion_first_id, companion_second_id, companion_third_id]
            .map(conv)
            .map(|id| {
                graph
                    .companion_state
                    .episodes
                    .iter()
                    .find(|episode| episode.conversation_id == id)
                    .map(|episode| {
                        (
                            episode.episode_index,
                            episode.previous_conversation_id,
                            episode.ended_at.map(TimestampMillis::get),
                        )
                    })
                    .expect("companion episode")
            });
        assert_eq!(
            episodes,
            [
                (1, None, Some(400)),
                (2, Some(conv(companion_first_id)), None),
                (3, Some(conv(companion_second_id)), None),
            ]
        );
        let pool = graph
            .memory
            .spaces
            .iter()
            .find(|space| {
                graph
                    .memory
                    .pools
                    .iter()
                    .any(|entry| entry.space_id == space.snapshot.id)
                    && space
                        .snapshot
                        .items
                        .iter()
                        .any(|item| item.text == "Nia remembers the lighthouse")
            })
            .expect("companion memory pool");
        let mut bound = pool.shared_conversation_ids.clone();
        bound.push(pool.conversation_id);
        bound.sort();
        let mut expected_bound = vec![
            conv(companion_first_id),
            conv(companion_second_id),
            conv(companion_third_id),
        ];
        expected_bound.sort();
        assert_eq!(bound, expected_bound);
        for conversation in &expected_bound {
            assert!(
                graph.memory.spaces.iter().any(|space| {
                    space.conversation_id == *conversation
                        && space.shared_conversation_ids.is_empty()
                        && space.snapshot.id != pool.snapshot.id
                }),
                "each companion conversation keeps its own memory beside the pool"
            );
        }
        assert!(
            lettuce_companions::CompanionStateRepository::get(
                backend.database(),
                lettuce_companions::CompanionStateOwner {
                    conversation_id: conv(companion_first_id),
                    character_id: character(companion_id),
                    persona_id: None,
                },
            )
            .expect("companion state")
            .is_some()
        );

        let group_session_id = lettuce_types::ConversationId::new();
        let deleted_speaker = CharacterId::new();
        let group_first = lettuce_types::MessageCandidateId::new().to_string();
        let group_second = lettuce_types::MessageCandidateId::new().to_string();
        let group_usage = || lettuce_transfer::LegacyBackupGroupMessageUsage {
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
            first_token_ms: None,
            tokens_per_second: None,
            mtp_stats_json: None,
        };
        let group_variant = |source_id: &str, content: &str, created_at| {
            lettuce_transfer::LegacyBackupGroupMessageVariant {
                source_id: source_id.to_owned(),
                ordinal: 0,
                content: content.to_owned(),
                speaker_character_source_id: Some(second_id.to_string()),
                created_at,
                usage: group_usage(),
                reasoning: None,
                selection_reasoning: None,
                model_source_id: None,
                attachments_json: "[]".to_owned(),
                gemini_content_json: None,
                usage_json: None,
            }
        };
        let group_message = |role: &str,
                             ordinal,
                             content: &str,
                             speaker: Option<CharacterId>,
                             variants,
                             selected| {
            lettuce_transfer::LegacyBackupGroupMessage {
                source_id: lettuce_types::MessageId::new().to_string(),
                ordinal,
                role: role.to_owned(),
                content: content.to_owned(),
                speaker_character_source_id: speaker.map(|id| id.to_string()),
                turn_number: ordinal,
                created_at: 200 + ordinal,
                usage: group_usage(),
                selected_variant_source_id: selected,
                pinned: false,
                attachments_json: "[]".to_owned(),
                used_lorebook_entries_json: "[]".to_owned(),
                memory_refs_json: "[]".to_owned(),
                reasoning: None,
                selection_reasoning: None,
                model_source_id: None,
                gemini_content_json: None,
                usage_json: None,
                parent_message_source_id: None,
                variants,
            }
        };
        let group_session = lettuce_transfer::LegacyBackupGroupSession {
            source_id: group_session_id.to_string(),
            group_source_id: Some(group_id.to_string()),
            name: String::new(),
            member_source_ids: vec![
                second_id.to_string(),
                third_id.to_string(),
                deleted_speaker.to_string(),
            ],
            muted_member_source_ids: vec![third_id.to_string()],
            persona_source_id: None,
            parent_session_source_id: None,
            branched_from_message_source_id: None,
            root_session_source_id: group_session_id.to_string(),
            chat_mode: "conversation".to_owned(),
            speaker_selection: "director".to_owned(),
            memory_policy: "dynamic".to_owned(),
            character_model_overrides: std::collections::BTreeMap::new(),
            group_conversation_prompt_source_id: None,
            group_roleplay_prompt_source_id: None,
            starting_scene_json: None,
            background_image_locator: None,
            lorebook_source_ids: Vec::new(),
            lorebooks_overridden: false,
            disable_character_lorebooks: true,
            author_note: None,
            config_overrides_json: "{}".to_owned(),
            memories_json: "[]".to_owned(),
            memory_embeddings_json: "[]".to_owned(),
            memory_summary: String::new(),
            memory_summary_token_count: 0,
            memory_tool_events_json: "[]".to_owned(),
            memory_status: None,
            memory_error: None,
            memory_progress_step: None,
            archived: false,
            created_at: 190,
            updated_at: 230,
            participation: Vec::new(),
            messages: vec![
                group_message("user", 0, "Hi crew", None, Vec::new(), None),
                group_message(
                    "assistant",
                    1,
                    "Aye",
                    Some(second_id),
                    vec![
                        group_variant(&group_first, "Aye", 201),
                        group_variant(&group_second, "Nay", 202),
                    ],
                    Some(group_first.clone()),
                ),
                group_message(
                    "assistant",
                    2,
                    "A voice from the past",
                    Some(deleted_speaker),
                    Vec::new(),
                    None,
                ),
            ],
        };
        let group_conversation_receipt = backend
            .legacy_group_conversation_importer()
            .execute(
                &admission,
                &plan,
                std::slice::from_ref(&group_session),
                &[],
                TimestampMillis::new(58),
            )
            .expect("materialize group conversations");
        assert_eq!(group_conversation_receipt.record_count, 1);
        let graph =
            lettuce_transfer::ProviderBackupSource::read_provider_backup_graph(backend.database())
                .expect("backup graph with group conversation");
        let group_history = graph
            .conversation_history
            .conversations
            .iter()
            .find(|history| history.aggregate.conversation.id == conv(group_session_id))
            .inspect(|history| {
                let lettuce_conversations::ConversationKind::Group(details) =
                    &history.aggregate.conversation.kind
                else {
                    panic!("group conversation");
                };
                assert_eq!(
                    details.group.chat_mode,
                    lettuce_conversations::GroupChatModeSnapshot::Conversation,
                    "the session's own chat type wins over the group's"
                );
                assert!(details.group.disable_character_lorebook);
                assert!(!matches!(
                    details.group.memory,
                    lettuce_conversations::SnapshotSelection::Disabled
                ));
            })
            .expect("group conversation");
        let cast = &group_history.aggregate.conversation.participants;
        assert_eq!(cast.len(), 4);
        let unknown = cast
            .iter()
            .find(|participant| {
                participant.source
                    == lettuce_conversations::ParticipantSource::Character(character(
                        deleted_speaker,
                    ))
            })
            .expect("unknown participant");
        assert_eq!(unknown.display_name, "Unknown");
        assert!(!unknown.enabled && unknown.muted);
        assert_eq!(group_history.messages.len(), 3);
        assert_eq!(group_history.messages[1].candidates.len(), 2);
        assert_eq!(
            group_history.messages[1].message.active_render_source,
            lettuce_conversations::MessageRenderSource::Candidate(
                lettuce_types::MessageCandidateId::from_uuid(scope.source(&group_first))
            )
        );
        assert_eq!(
            group_history.messages[2].message.author_participant_id,
            Some(unknown.id)
        );
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
        let conversation_replay = reopened
            .legacy_direct_conversation_importer()
            .execute(
                &replayed_admission,
                &plan,
                &direct_sessions,
                &direct_memories,
                &[],
                &[],
                TimestampMillis::new(95),
            )
            .expect("replay direct conversations");
        assert!(conversation_replay.replayed);
        assert_eq!(conversation_replay.completed_at, TimestampMillis::new(57));
        let group_conversation_replay = reopened
            .legacy_group_conversation_importer()
            .execute(
                &replayed_admission,
                &plan,
                std::slice::from_ref(&group_session),
                &[],
                TimestampMillis::new(96),
            )
            .expect("replay group conversations");
        assert!(group_conversation_replay.replayed);
        assert_eq!(
            reopened.complete_legacy_import(run_id, TimestampMillis::new(97)),
            Err(LegacyImportRepositoryError::Conflict)
        );
        let second_plan = LegacyImportPlan {
            source_fingerprint: Some(
                ContentHash::parse("56".repeat(32)).expect("second source hash"),
            ),
            ..plan.clone()
        };
        let second = reopened
            .legacy_import_admission()
            .admit(
                LegacyImportRunId::new(),
                &inventory,
                &second_plan,
                TimestampMillis::new(100),
            )
            .expect("admit a second legacy source");
        assert!(!second.replayed);
        reopened
            .legacy_import_executor()
            .execute(&second, &second_plan, TimestampMillis::new(101))
            .expect("materialize the second authored graph");
        reopened
            .legacy_provider_model_importer(&InMemorySecretStore::new())
            .execute(&second, &second_plan, TimestampMillis::new(102))
            .await
            .expect("materialize the second providers next to the first defaults");
        reopened
            .legacy_character_importer()
            .execute(
                &second,
                &second_plan,
                &characters,
                &bindings,
                TimestampMillis::new(103),
            )
            .expect("characters of a second source import alongside");
        reopened
            .legacy_group_importer()
            .execute(
                &second,
                &second_plan,
                std::slice::from_ref(&group),
                &group_bindings,
                TimestampMillis::new(104),
            )
            .expect("groups of a second source import alongside");
        reopened
            .legacy_direct_conversation_importer()
            .execute(
                &second,
                &second_plan,
                &direct_sessions,
                &direct_memories,
                &[],
                &[],
                TimestampMillis::new(105),
            )
            .expect("direct conversations of a second source import alongside");
        reopened
            .legacy_group_conversation_importer()
            .execute(
                &second,
                &second_plan,
                std::slice::from_ref(&group_session),
                &[],
                TimestampMillis::new(106),
            )
            .expect("group conversations of a second source import alongside");
        let second_scope = lettuce_transfer::LegacyIdScope::new(
            second_plan
                .source_fingerprint
                .as_ref()
                .expect("second source fingerprint"),
        );
        for id in [
            character(character_id),
            CharacterId::from_uuid(second_scope.uuid(character_id.as_uuid())),
        ] {
            assert!(
                CharacterRepository::get(reopened.database(), id)
                    .expect("read imported character")
                    .is_some()
            );
        }
        drop(reopened);
        fs::remove_file(path).expect("remove database");
    }
}
