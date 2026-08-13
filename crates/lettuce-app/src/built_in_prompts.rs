use lettuce_context::{
    BuiltInPromptSeed, BuiltInReconcileMode, BuiltInReconcileOutcome, BuiltInReconcileRequest,
    PromptBehaviorVersion, PromptBootstrapError, PromptBootstrapPort, PromptEntryCondition,
    PromptEntryPayload, PromptEntryPosition, PromptEntryRole, PromptMetadataDraft, PromptPurpose,
};
use lettuce_types::{PromptDocumentId, TimestampMillis};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const CATALOG_JSON: &str = include_str!("../resources/built-in-prompts/v1.json");
const CATALOG_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltInPromptId {
    AppDefault,
    LocalRoleplay,
    Companion,
    DynamicSummary,
    DynamicMemory,
    DynamicMemoryLocal,
    ReplyHelper,
    ReplyHelperConversational,
    LorebookEntryWriter,
    LorebookKeywordGenerator,
    LorebookGeneratorPlanner,
    LorebookGeneratorWriter,
    LorebookGeneratorRefine,
    LorebookGeneratorCoherence,
    GroupChat,
    GroupChatRoleplay,
    AvatarGeneration,
    AvatarEdit,
    SceneGeneration,
    ScenePromptWriter,
    DesignReference,
    CompanionSoulWriter,
    CompanionGrowthcycle,
    CompanionConsolidation,
}

impl BuiltInPromptId {
    pub const ALL: [Self; 24] = [
        Self::AppDefault,
        Self::LocalRoleplay,
        Self::Companion,
        Self::DynamicSummary,
        Self::DynamicMemory,
        Self::DynamicMemoryLocal,
        Self::ReplyHelper,
        Self::ReplyHelperConversational,
        Self::LorebookEntryWriter,
        Self::LorebookKeywordGenerator,
        Self::LorebookGeneratorPlanner,
        Self::LorebookGeneratorWriter,
        Self::LorebookGeneratorRefine,
        Self::LorebookGeneratorCoherence,
        Self::GroupChat,
        Self::GroupChatRoleplay,
        Self::AvatarGeneration,
        Self::AvatarEdit,
        Self::SceneGeneration,
        Self::ScenePromptWriter,
        Self::DesignReference,
        Self::CompanionSoulWriter,
        Self::CompanionGrowthcycle,
        Self::CompanionConsolidation,
    ];

    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::AppDefault => "prompt_app_default",
            Self::LocalRoleplay => "prompt_app_local_roleplay",
            Self::Companion => "prompt_app_companion",
            Self::DynamicSummary => "prompt_app_dynamic_summary",
            Self::DynamicMemory => "prompt_app_dynamic_memory",
            Self::DynamicMemoryLocal => "prompt_app_dynamic_memory_local",
            Self::ReplyHelper => "prompt_app_help_me_reply",
            Self::ReplyHelperConversational => "prompt_app_help_me_reply_conversational",
            Self::LorebookEntryWriter => "prompt_app_lorebook_entry_writer",
            Self::LorebookKeywordGenerator => "prompt_app_lorebook_keyword_generator",
            Self::LorebookGeneratorPlanner => "prompt_app_lorebook_generator_planner",
            Self::LorebookGeneratorWriter => "prompt_app_lorebook_generator_writer",
            Self::LorebookGeneratorRefine => "prompt_app_lorebook_generator_refine",
            Self::LorebookGeneratorCoherence => "prompt_app_lorebook_generator_coherence",
            Self::GroupChat => "prompt_app_group_chat",
            Self::GroupChatRoleplay => "prompt_app_group_chat_roleplay",
            Self::AvatarGeneration => "prompt_app_avatar_generation",
            Self::AvatarEdit => "prompt_app_avatar_edit",
            Self::SceneGeneration => "prompt_app_scene_generation",
            Self::ScenePromptWriter => "prompt_app_scene_prompt_writer",
            Self::DesignReference => "prompt_app_design_reference",
            Self::CompanionSoulWriter => "prompt_app_companion_soul_writer",
            Self::CompanionGrowthcycle => "prompt_app_companion_growthcycle",
            Self::CompanionConsolidation => "prompt_app_companion_consolidation",
        }
    }

    #[must_use]
    pub const fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::LorebookEntryWriter => &["prompt_app_lorebook_entry_generator"],
            _ => &[],
        }
    }

    #[must_use]
    pub const fn purpose(self) -> PromptPurpose {
        match self {
            Self::AppDefault | Self::LocalRoleplay => PromptPurpose::DirectChat,
            Self::Companion => PromptPurpose::CompanionChat,
            Self::DynamicSummary => PromptPurpose::DynamicMemorySummarizer,
            Self::DynamicMemory | Self::DynamicMemoryLocal => PromptPurpose::DynamicMemoryManager,
            Self::ReplyHelper => PromptPurpose::ReplyHelperRoleplay,
            Self::ReplyHelperConversational => PromptPurpose::ReplyHelperConversational,
            Self::LorebookEntryWriter => PromptPurpose::LorebookEntryWriter,
            Self::LorebookKeywordGenerator => PromptPurpose::LorebookKeywordGenerator,
            Self::LorebookGeneratorPlanner => PromptPurpose::LorebookGeneratorPlanner,
            Self::LorebookGeneratorWriter => PromptPurpose::LorebookGeneratorWriter,
            Self::LorebookGeneratorRefine => PromptPurpose::LorebookGeneratorRefine,
            Self::LorebookGeneratorCoherence => PromptPurpose::LorebookGeneratorCoherence,
            Self::GroupChat => PromptPurpose::GroupChatConversational,
            Self::GroupChatRoleplay => PromptPurpose::GroupChatRoleplay,
            Self::AvatarGeneration => PromptPurpose::AvatarGeneration,
            Self::AvatarEdit => PromptPurpose::AvatarEditRequest,
            Self::SceneGeneration => PromptPurpose::SceneGeneration,
            Self::ScenePromptWriter => PromptPurpose::ScenePromptWriter,
            Self::DesignReference => PromptPurpose::DesignReferenceWriter,
            Self::CompanionSoulWriter => PromptPurpose::CompanionSoulWriter,
            Self::CompanionGrowthcycle => PromptPurpose::CompanionGrowthcycle,
            Self::CompanionConsolidation => PromptPurpose::CompanionConsolidation,
        }
    }

    #[must_use]
    pub fn from_key_or_alias(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|id| id.key() == value || id.aliases().contains(&value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltInPromptIds {
    pub app_default: PromptDocumentId,
    pub local_roleplay: PromptDocumentId,
    pub companion: PromptDocumentId,
    pub dynamic_summary: PromptDocumentId,
    pub dynamic_memory: PromptDocumentId,
    pub dynamic_memory_local: PromptDocumentId,
    pub reply_helper: PromptDocumentId,
    pub reply_helper_conversational: PromptDocumentId,
    pub lorebook_entry_writer: PromptDocumentId,
    pub lorebook_keyword_generator: PromptDocumentId,
    pub lorebook_generator_planner: PromptDocumentId,
    pub lorebook_generator_writer: PromptDocumentId,
    pub lorebook_generator_refine: PromptDocumentId,
    pub lorebook_generator_coherence: PromptDocumentId,
    pub group_chat: PromptDocumentId,
    pub group_chat_roleplay: PromptDocumentId,
    pub avatar_generation: PromptDocumentId,
    pub avatar_edit: PromptDocumentId,
    pub scene_generation: PromptDocumentId,
    pub scene_prompt_writer: PromptDocumentId,
    pub design_reference: PromptDocumentId,
    pub companion_soul_writer: PromptDocumentId,
    pub companion_growthcycle: PromptDocumentId,
    pub companion_consolidation: PromptDocumentId,
}

impl BuiltInPromptIds {
    fn from_outcomes(
        outcomes: &[BuiltInReconcileOutcome],
    ) -> Result<Self, BuiltInPromptServiceError> {
        let mut ids = HashMap::with_capacity(BuiltInPromptId::ALL.len());
        for outcome in outcomes {
            let id = BuiltInPromptId::from_key_or_alias(&outcome.key)
                .ok_or(BuiltInPromptServiceError::InvalidStoredBuiltIn)?;
            if ids.insert(id, outcome.document.id).is_some() {
                return Err(BuiltInPromptServiceError::InvalidStoredBuiltIn);
            }
        }
        for id in BuiltInPromptId::ALL {
            if !ids.contains_key(&id) {
                return Err(BuiltInPromptServiceError::MissingOutcome(id));
            }
        }
        let required = |id| ids[&id];
        Ok(Self {
            app_default: required(BuiltInPromptId::AppDefault),
            local_roleplay: required(BuiltInPromptId::LocalRoleplay),
            companion: required(BuiltInPromptId::Companion),
            dynamic_summary: required(BuiltInPromptId::DynamicSummary),
            dynamic_memory: required(BuiltInPromptId::DynamicMemory),
            dynamic_memory_local: required(BuiltInPromptId::DynamicMemoryLocal),
            reply_helper: required(BuiltInPromptId::ReplyHelper),
            reply_helper_conversational: required(BuiltInPromptId::ReplyHelperConversational),
            lorebook_entry_writer: required(BuiltInPromptId::LorebookEntryWriter),
            lorebook_keyword_generator: required(BuiltInPromptId::LorebookKeywordGenerator),
            lorebook_generator_planner: required(BuiltInPromptId::LorebookGeneratorPlanner),
            lorebook_generator_writer: required(BuiltInPromptId::LorebookGeneratorWriter),
            lorebook_generator_refine: required(BuiltInPromptId::LorebookGeneratorRefine),
            lorebook_generator_coherence: required(BuiltInPromptId::LorebookGeneratorCoherence),
            group_chat: required(BuiltInPromptId::GroupChat),
            group_chat_roleplay: required(BuiltInPromptId::GroupChatRoleplay),
            avatar_generation: required(BuiltInPromptId::AvatarGeneration),
            avatar_edit: required(BuiltInPromptId::AvatarEdit),
            scene_generation: required(BuiltInPromptId::SceneGeneration),
            scene_prompt_writer: required(BuiltInPromptId::ScenePromptWriter),
            design_reference: required(BuiltInPromptId::DesignReference),
            companion_soul_writer: required(BuiltInPromptId::CompanionSoulWriter),
            companion_growthcycle: required(BuiltInPromptId::CompanionGrowthcycle),
            companion_consolidation: required(BuiltInPromptId::CompanionConsolidation),
        })
    }

    #[must_use]
    pub fn get(&self, id: BuiltInPromptId) -> PromptDocumentId {
        match id {
            BuiltInPromptId::AppDefault => self.app_default,
            BuiltInPromptId::LocalRoleplay => self.local_roleplay,
            BuiltInPromptId::Companion => self.companion,
            BuiltInPromptId::DynamicSummary => self.dynamic_summary,
            BuiltInPromptId::DynamicMemory => self.dynamic_memory,
            BuiltInPromptId::DynamicMemoryLocal => self.dynamic_memory_local,
            BuiltInPromptId::ReplyHelper => self.reply_helper,
            BuiltInPromptId::ReplyHelperConversational => self.reply_helper_conversational,
            BuiltInPromptId::LorebookEntryWriter => self.lorebook_entry_writer,
            BuiltInPromptId::LorebookKeywordGenerator => self.lorebook_keyword_generator,
            BuiltInPromptId::LorebookGeneratorPlanner => self.lorebook_generator_planner,
            BuiltInPromptId::LorebookGeneratorWriter => self.lorebook_generator_writer,
            BuiltInPromptId::LorebookGeneratorRefine => self.lorebook_generator_refine,
            BuiltInPromptId::LorebookGeneratorCoherence => self.lorebook_generator_coherence,
            BuiltInPromptId::GroupChat => self.group_chat,
            BuiltInPromptId::GroupChatRoleplay => self.group_chat_roleplay,
            BuiltInPromptId::AvatarGeneration => self.avatar_generation,
            BuiltInPromptId::AvatarEdit => self.avatar_edit,
            BuiltInPromptId::SceneGeneration => self.scene_generation,
            BuiltInPromptId::ScenePromptWriter => self.scene_prompt_writer,
            BuiltInPromptId::DesignReference => self.design_reference,
            BuiltInPromptId::CompanionSoulWriter => self.companion_soul_writer,
            BuiltInPromptId::CompanionGrowthcycle => self.companion_growthcycle,
            BuiltInPromptId::CompanionConsolidation => self.companion_consolidation,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BuiltInPromptCatalog {
    seeds: Vec<BuiltInPromptSeed>,
}

impl BuiltInPromptCatalog {
    pub fn bundled() -> Result<Self, BuiltInPromptCatalogError> {
        let resource: CatalogResource = serde_json::from_str(CATALOG_JSON)?;
        Self::from_resource(resource)
    }

    fn from_resource(resource: CatalogResource) -> Result<Self, BuiltInPromptCatalogError> {
        if resource.version != CATALOG_VERSION {
            return Err(BuiltInPromptCatalogError::UnsupportedVersion(
                resource.version,
            ));
        }
        let mut keys_and_aliases = HashSet::new();
        let mut present_ids = HashSet::new();
        let mut seeds = Vec::with_capacity(resource.prompts.len());
        for prompt in resource.prompts {
            let id = BuiltInPromptId::from_key_or_alias(&prompt.key)
                .ok_or_else(|| BuiltInPromptCatalogError::UnknownPromptKey(prompt.key.clone()))?;
            if prompt.key != id.key() {
                return Err(BuiltInPromptCatalogError::AliasUsedAsCanonical(prompt.key));
            }
            if !present_ids.insert(id) || !keys_and_aliases.insert(prompt.key.clone()) {
                return Err(BuiltInPromptCatalogError::DuplicatePromptKey(prompt.key));
            }
            if prompt.aliases.as_slice() != id.aliases() {
                return Err(BuiltInPromptCatalogError::AliasMismatch(id));
            }
            for alias in &prompt.aliases {
                if !keys_and_aliases.insert(alias.clone()) {
                    return Err(BuiltInPromptCatalogError::DuplicatePromptKey(alias.clone()));
                }
            }
            if prompt.purpose != id.purpose() {
                return Err(BuiltInPromptCatalogError::PurposeMismatch(id));
            }
            if !prompt.required || !prompt.protected {
                return Err(BuiltInPromptCatalogError::PolicyMismatch(id));
            }
            let mut entry_keys = HashSet::with_capacity(prompt.entries.len());
            let entries = prompt
                .entries
                .into_iter()
                .map(|entry| {
                    if !entry_keys.insert(entry.key.clone()) {
                        return Err(BuiltInPromptCatalogError::DuplicateEntryKey {
                            prompt: id,
                            key: entry.key,
                        });
                    }
                    validate_legacy_template_syntax(id, &entry.content)?;
                    Ok(entry.into_draft())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let seed = BuiltInPromptSeed {
                key: prompt.key,
                aliases: prompt.aliases,
                seed_version: CATALOG_VERSION,
                metadata: PromptMetadataDraft {
                    name: prompt.name,
                    purpose: prompt.purpose,
                    condense: prompt.condense,
                    behavior_version: PromptBehaviorVersion::LegacyV1,
                },
                entries,
                required: true,
                protected: true,
            };
            seed.validate()?;
            seeds.push(seed);
        }
        if present_ids.len() != BuiltInPromptId::ALL.len() {
            let missing = BuiltInPromptId::ALL
                .into_iter()
                .find(|id| !present_ids.contains(id))
                .expect("catalog count differs only when an id is missing");
            return Err(BuiltInPromptCatalogError::MissingPrompt(missing));
        }
        Ok(Self { seeds })
    }

    #[must_use]
    pub fn seeds(&self) -> &[BuiltInPromptSeed] {
        &self.seeds
    }

    #[must_use]
    pub const fn version(&self) -> u32 {
        CATALOG_VERSION
    }

    #[must_use]
    pub fn seed(&self, id: BuiltInPromptId) -> &BuiltInPromptSeed {
        self.seeds
            .iter()
            .find(|seed| seed.key == id.key())
            .expect("validated catalog contains every built-in id")
    }
}

fn validate_legacy_template_syntax(
    prompt: BuiltInPromptId,
    content: &str,
) -> Result<(), BuiltInPromptCatalogError> {
    validate_current_draft_conditional(prompt, content)?;
    let mut cursor = 0;
    while let Some(start) = content[cursor..].find("{{") {
        let start = cursor + start;
        let Some(relative_end) = content[start + 2..].find("}}") else {
            return Err(BuiltInPromptCatalogError::UnclosedVariable(prompt));
        };
        let end = start + 2 + relative_end;
        let variable = &content[start + 2..end];
        if !is_registered_legacy_variable(variable) {
            return Err(BuiltInPromptCatalogError::UnknownVariable {
                prompt,
                variable: variable.to_owned(),
            });
        }
        if !is_legacy_variable_allowed_for(prompt.purpose(), variable) {
            return Err(BuiltInPromptCatalogError::IncompatibleVariable {
                prompt,
                variable: variable.to_owned(),
            });
        }
        cursor = end + 2;
    }
    Ok(())
}

fn validate_current_draft_conditional(
    prompt: BuiltInPromptId,
    content: &str,
) -> Result<(), BuiltInPromptCatalogError> {
    const OPEN: &str = "{{#if current_draft}}";
    const ELSE: &str = "{{else}}";
    const CLOSE: &str = "{{/if}}";
    let has_directive =
        content.contains("{{#if") || content.contains(ELSE) || content.contains(CLOSE);
    if !has_directive {
        return Ok(());
    }
    let Some(open) = content.find(OPEN) else {
        return Err(BuiltInPromptCatalogError::MalformedConditional(prompt));
    };
    let Some(alternative) = content.find(ELSE) else {
        return Err(BuiltInPromptCatalogError::MalformedConditional(prompt));
    };
    let Some(close) = content.find(CLOSE) else {
        return Err(BuiltInPromptCatalogError::MalformedConditional(prompt));
    };
    if !(open < alternative && alternative < close)
        || content.matches(OPEN).count() != 1
        || content.matches(ELSE).count() != 1
        || content.matches(CLOSE).count() != 1
        || content.matches("{{#if").count() != 1
        || !matches!(
            prompt.purpose(),
            PromptPurpose::ReplyHelperRoleplay | PromptPurpose::ReplyHelperConversational
        )
    {
        return Err(BuiltInPromptCatalogError::MalformedConditional(prompt));
    }
    Ok(())
}

fn is_legacy_variable_allowed_for(purpose: PromptPurpose, value: &str) -> bool {
    if matches!(value, "#if current_draft" | "else" | "/if") {
        return matches!(
            purpose,
            PromptPurpose::ReplyHelperRoleplay | PromptPurpose::ReplyHelperConversational
        );
    }
    if matches!(
        value,
        "char.name"
            | "char.desc"
            | "persona.name"
            | "persona.desc"
            | "scene"
            | "scene_direction"
            | "lorebook"
            | "author_note"
            | "context_summary"
            | "key_memories"
            | "content_rules"
            | "date"
            | "date_full"
            | "time_12hour_format"
            | "time_timezone_name"
            | "datetime_iso"
    ) {
        return true;
    }
    match purpose {
        PromptPurpose::DirectChat | PromptPurpose::CompanionChat => {
            matches!(
                value,
                "companion_state"
                    | "scheduled_notes"
                    | "lora_keywords[character]"
                    | "lora_keywords[persona]"
            )
        }
        PromptPurpose::GroupChatRoleplay => value == "group_characters",
        PromptPurpose::GroupChatConversational => value == "group_characters",
        PromptPurpose::DynamicMemorySummarizer => {
            matches!(value, "prev_summary" | "character")
        }
        PromptPurpose::DynamicMemoryManager => matches!(
            value,
            "max_entries" | "current_memory_tokens" | "hot_token_budget"
        ),
        PromptPurpose::ReplyHelperRoleplay | PromptPurpose::ReplyHelperConversational => {
            value == "current_draft"
        }
        PromptPurpose::LorebookEntryWriter => matches!(
            value,
            "lorebook_name"
                | "character_name"
                | "session_title"
                | "selected_messages"
                | "memory_summary"
                | "selected_memories"
                | "direction_prompt"
                | "existing_entries"
        ),
        PromptPurpose::LorebookKeywordGenerator => matches!(
            value,
            "entry_title" | "entry_content" | "existing_keywords" | "direction_prompt"
        ),
        PromptPurpose::LorebookGeneratorPlanner => {
            matches!(value, "brief" | "target_count" | "source_excerpts")
        }
        PromptPurpose::LorebookGeneratorWriter => matches!(
            value,
            "brief"
                | "outline"
                | "entry_title"
                | "entry_category"
                | "entry_proposed_keys"
                | "entry_rationale"
                | "relevant_excerpts"
        ),
        PromptPurpose::LorebookGeneratorRefine => matches!(
            value,
            "brief"
                | "outline"
                | "entry_title"
                | "entry_keywords"
                | "entry_always_active"
                | "entry_content"
                | "user_feedback"
                | "relevant_excerpts"
        ),
        PromptPurpose::LorebookGeneratorCoherence => value == "drafted_entries",
        PromptPurpose::AvatarGeneration => matches!(
            value,
            "avatar_subject_name" | "avatar_subject_description" | "avatar_request"
        ),
        PromptPurpose::AvatarEditRequest => matches!(
            value,
            "avatar_subject_name"
                | "avatar_subject_description"
                | "current_avatar_prompt"
                | "edit_request"
        ),
        PromptPurpose::SceneGeneration | PromptPurpose::ScenePromptWriter => matches!(
            value,
            "recent_messages"
                | "scene_request"
                | "reference[character]"
                | "reference[persona]"
                | "lora_keywords[character]"
                | "lora_keywords[persona]"
                | "image_model_instructions"
        ),
        PromptPurpose::DesignReferenceWriter => matches!(
            value,
            "subject_name" | "subject_description" | "current_description"
        ),
        PromptPurpose::CompanionSoulWriter => matches!(
            value,
            "char.definition"
                | "char.description"
                | "opening_context"
                | "current_soul"
                | "user_notes"
        ),
        PromptPurpose::CompanionGrowthcycle => matches!(
            value,
            "companion.name" | "changeable_categories" | "current_growth" | "new_memories"
        ),
        PromptPurpose::CompanionConsolidation => matches!(
            value,
            "companion.name" | "authored_core" | "current_core" | "accumulated_growth"
        ),
        PromptPurpose::Undefined => false,
    }
}

fn is_registered_legacy_variable(value: &str) -> bool {
    matches!(value, "#if current_draft" | "else" | "/if")
        || matches!(
            value,
            "char.name"
                | "char.desc"
                | "char.definition"
                | "char.description"
                | "persona.name"
                | "persona.desc"
                | "scene"
                | "scene_direction"
                | "lorebook"
                | "author_note"
                | "context_summary"
                | "key_memories"
                | "content_rules"
                | "companion_state"
                | "scheduled_notes"
                | "group_characters"
                | "prev_summary"
                | "character"
                | "max_entries"
                | "current_memory_tokens"
                | "hot_token_budget"
                | "current_draft"
                | "lorebook_name"
                | "character_name"
                | "session_title"
                | "selected_messages"
                | "memory_summary"
                | "selected_memories"
                | "direction_prompt"
                | "existing_entries"
                | "entry_title"
                | "entry_content"
                | "existing_keywords"
                | "brief"
                | "target_count"
                | "source_excerpts"
                | "outline"
                | "entry_category"
                | "entry_proposed_keys"
                | "entry_rationale"
                | "relevant_excerpts"
                | "entry_keywords"
                | "entry_always_active"
                | "user_feedback"
                | "drafted_entries"
                | "avatar_subject_name"
                | "avatar_subject_description"
                | "avatar_request"
                | "current_avatar_prompt"
                | "edit_request"
                | "recent_messages"
                | "scene_request"
                | "subject_name"
                | "subject_description"
                | "current_description"
                | "opening_context"
                | "current_soul"
                | "user_notes"
                | "companion.name"
                | "changeable_categories"
                | "current_growth"
                | "new_memories"
                | "authored_core"
                | "current_core"
                | "accumulated_growth"
                | "date"
                | "date_full"
                | "time_12hour_format"
                | "time_timezone_name"
                | "datetime_iso"
                | "reference[character]"
                | "reference[persona]"
                | "lora_keywords[character]"
                | "lora_keywords[persona]"
                | "image_model_instructions"
        )
}

#[derive(Debug)]
pub struct BuiltInPromptService<'a, P> {
    port: &'a P,
    catalog: BuiltInPromptCatalog,
}

impl<'a, P: PromptBootstrapPort> BuiltInPromptService<'a, P> {
    pub fn new(port: &'a P) -> Result<Self, BuiltInPromptServiceError> {
        let catalog = BuiltInPromptCatalog::bundled().map_err(|error| match error {
            BuiltInPromptCatalogError::Json(source) => {
                BuiltInPromptServiceError::CorruptCatalog(source)
            }
            other => BuiltInPromptServiceError::CatalogValidation(other),
        })?;
        Ok(Self { port, catalog })
    }

    pub fn bootstrap(
        &self,
        now: TimestampMillis,
    ) -> Result<BuiltInPromptIds, BuiltInPromptServiceError> {
        let outcomes = self
            .port
            .reconcile_built_ins(
                BuiltInReconcileRequest {
                    seeds: self.catalog.seeds.clone(),
                    mode: BuiltInReconcileMode::RefreshUnedited,
                },
                now,
            )
            .map_err(BuiltInPromptServiceError::from_bootstrap)?;
        BuiltInPromptIds::from_outcomes(&outcomes)
    }

    pub fn reset_one(
        &self,
        id: BuiltInPromptId,
        now: TimestampMillis,
    ) -> Result<BuiltInReconcileOutcome, BuiltInPromptServiceError> {
        let mut outcomes = self
            .port
            .reconcile_built_ins(
                BuiltInReconcileRequest {
                    seeds: vec![self.catalog.seed(id).clone()],
                    mode: BuiltInReconcileMode::ResetToSeed,
                },
                now,
            )
            .map_err(BuiltInPromptServiceError::from_bootstrap)?;
        if outcomes.len() != 1 {
            return Err(BuiltInPromptServiceError::UnexpectedOutcomeCount(
                outcomes.len(),
            ));
        }
        Ok(outcomes.remove(0))
    }

    pub fn reset_all(
        &self,
        now: TimestampMillis,
    ) -> Result<BuiltInPromptIds, BuiltInPromptServiceError> {
        let outcomes = self
            .port
            .reconcile_built_ins(
                BuiltInReconcileRequest {
                    seeds: self.catalog.seeds.clone(),
                    mode: BuiltInReconcileMode::ResetToSeed,
                },
                now,
            )
            .map_err(BuiltInPromptServiceError::from_bootstrap)?;
        BuiltInPromptIds::from_outcomes(&outcomes)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BuiltInPromptCatalogError {
    #[error("invalid bundled prompt JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid bundled prompt: {0}")]
    Validation(#[from] lettuce_context::PromptValidationError),
    #[error("unsupported built-in prompt catalog version {0}")]
    UnsupportedVersion(u32),
    #[error("unknown built-in prompt key {0}")]
    UnknownPromptKey(String),
    #[error("duplicate built-in prompt key or alias {0}")]
    DuplicatePromptKey(String),
    #[error("built-in prompt alias used as canonical key {0}")]
    AliasUsedAsCanonical(String),
    #[error("aliases do not match the compatibility contract for {0:?}")]
    AliasMismatch(BuiltInPromptId),
    #[error("purpose does not match the compatibility contract for {0:?}")]
    PurposeMismatch(BuiltInPromptId),
    #[error("required/protected policy does not match for {0:?}")]
    PolicyMismatch(BuiltInPromptId),
    #[error("missing built-in prompt {0:?}")]
    MissingPrompt(BuiltInPromptId),
    #[error("duplicate entry key {key} in {prompt:?}")]
    DuplicateEntryKey {
        prompt: BuiltInPromptId,
        key: String,
    },
    #[error("unclosed template variable in {0:?}")]
    UnclosedVariable(BuiltInPromptId),
    #[error("unknown template variable {variable} in {prompt:?}")]
    UnknownVariable {
        prompt: BuiltInPromptId,
        variable: String,
    },
    #[error("template variable {variable} is incompatible with {prompt:?}")]
    IncompatibleVariable {
        prompt: BuiltInPromptId,
        variable: String,
    },
    #[error("malformed legacy conditional block in {0:?}")]
    MalformedConditional(BuiltInPromptId),
}

#[derive(Debug, thiserror::Error)]
pub enum BuiltInPromptServiceError {
    #[error("bundled prompt catalog is corrupt: {0}")]
    CorruptCatalog(serde_json::Error),
    #[error("bundled prompt catalog failed validation: {0}")]
    CatalogValidation(BuiltInPromptCatalogError),
    #[error("stored built-in prompt identity is invalid")]
    InvalidStoredBuiltIn,
    #[error("built-in prompt alias conflicts with another catalog identity")]
    AliasConflict,
    #[error("prompt bootstrap omitted {0:?}")]
    MissingOutcome(BuiltInPromptId),
    #[error("prompt bootstrap returned {0} outcomes when one was required")]
    UnexpectedOutcomeCount(usize),
    #[error("prompt storage bootstrap failed: {0}")]
    Bootstrap(PromptBootstrapError),
}

impl BuiltInPromptServiceError {
    fn from_bootstrap(error: PromptBootstrapError) -> Self {
        match error {
            PromptBootstrapError::AliasCollision | PromptBootstrapError::AliasConflict => {
                Self::AliasConflict
            }
            PromptBootstrapError::InvalidStoredBuiltIn => Self::InvalidStoredBuiltIn,
            other => Self::Bootstrap(other),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogResource {
    version: u32,
    prompts: Vec<PromptResource>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PromptResource {
    key: String,
    aliases: Vec<String>,
    name: String,
    purpose: PromptPurpose,
    condense: bool,
    required: bool,
    protected: bool,
    entries: Vec<EntryResource>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EntryResource {
    key: String,
    name: String,
    role: PromptEntryRole,
    content: String,
    enabled: bool,
    injection_position: PromptEntryPosition,
    injection_depth: u32,
    conditional_min_messages: Option<u32>,
    interval_turns: Option<u32>,
    system_prompt: bool,
    conditions: Option<PromptEntryCondition>,
    prompt_entry_payload: Option<PromptEntryPayload>,
}

impl EntryResource {
    fn into_draft(self) -> lettuce_context::PromptEntryDraft {
        lettuce_context::PromptEntryDraft {
            built_in_entry_key: Some(self.key),
            name: self.name,
            role: self.role,
            content: self.content,
            enabled: self.enabled,
            injection_position: self.injection_position,
            depth: self.injection_depth,
            conditional_min_messages: self.conditional_min_messages,
            interval_turns: self.interval_turns,
            system_prompt: self.system_prompt,
            conditions: self.conditions,
            payload: self.prompt_entry_payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_context::{
        BuiltInReconcileAction, LifecycleStatus, PromptDocument, PromptEntry, PromptEntryMutation,
        PromptProvenance, PromptRenderContext, PromptRenderValues, PromptRepository,
        PromptVariable, render_prompt,
    };
    use lettuce_types::{PromptEntryId, Revision};
    use std::sync::Mutex;

    #[test]
    fn catalog_is_the_exact_closed_legacy_set() {
        let catalog = BuiltInPromptCatalog::bundled().expect("valid embedded catalog");
        assert_eq!(catalog.seeds().len(), 24);

        let actual = catalog
            .seeds()
            .iter()
            .map(|seed| seed.key.as_str())
            .collect::<Vec<_>>();
        let expected = BuiltInPromptId::ALL
            .iter()
            .map(|id| id.key())
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        for (seed, id) in catalog.seeds().iter().zip(BuiltInPromptId::ALL) {
            assert_eq!(seed.metadata.purpose, id.purpose());
            assert!(seed.required);
            assert!(seed.protected);
            assert_eq!(seed.seed_version, 1);
            assert!(
                seed.entries
                    .iter()
                    .all(|entry| entry.built_in_entry_key.is_some())
            );
        }
    }

    #[test]
    fn per_entry_digests_pin_every_canonical_shipped_field() {
        let resource: CatalogResource = serde_json::from_str(CATALOG_JSON).expect("resource");
        let snapshots = resource
            .prompts
            .iter()
            .flat_map(|prompt| {
                prompt.entries.iter().map(|entry| {
                    let canonical = serde_json::to_vec(entry).expect("canonical entry");
                    format!(
                        "{} {} {}",
                        prompt.key,
                        entry.key,
                        blake3::hash(&canonical).to_hex()
                    )
                })
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            snapshots,
            include_str!("../resources/built-in-prompts/v1.snap").trim()
        );
    }

    #[test]
    fn legacy_alias_resolves_only_to_the_writer() {
        assert_eq!(
            BuiltInPromptId::from_key_or_alias("prompt_app_lorebook_entry_generator"),
            Some(BuiltInPromptId::LorebookEntryWriter)
        );
        assert_eq!(
            BuiltInPromptId::LorebookEntryWriter.aliases(),
            &["prompt_app_lorebook_entry_generator"]
        );
        assert!(
            BuiltInPromptId::ALL
                .iter()
                .filter(|id| !id.aliases().is_empty())
                .eq([&BuiltInPromptId::LorebookEntryWriter])
        );
    }

    #[test]
    fn bootstrap_and_resets_use_the_intended_modes_and_complete_typed_ids() {
        let fake = FakeBootstrap::default();
        let service = BuiltInPromptService::new(&fake).expect("service");
        let ids = service
            .bootstrap(TimestampMillis::new(10))
            .expect("bootstrap");
        assert!(
            BuiltInPromptId::ALL
                .into_iter()
                .all(|id| ids.get(id) == deterministic_id(id))
        );
        assert_eq!(
            fake.calls.lock().expect("calls")[0].mode,
            BuiltInReconcileMode::RefreshUnedited
        );

        let reset = service
            .reset_one(BuiltInPromptId::SceneGeneration, TimestampMillis::new(11))
            .expect("reset one");
        assert_eq!(reset.key, BuiltInPromptId::SceneGeneration.key());
        let all = service
            .reset_all(TimestampMillis::new(12))
            .expect("reset all");
        assert_eq!(
            all.get(BuiltInPromptId::CompanionConsolidation),
            deterministic_id(BuiltInPromptId::CompanionConsolidation)
        );
        let calls = fake.calls.lock().expect("calls");
        assert_eq!(calls[1].mode, BuiltInReconcileMode::ResetToSeed);
        assert_eq!(calls[1].seeds.len(), 1);
        assert_eq!(calls[2].mode, BuiltInReconcileMode::ResetToSeed);
        assert_eq!(calls[2].seeds.len(), 24);
    }

    #[test]
    fn service_preserves_typed_alias_and_storage_failures() {
        let alias = ErrorBootstrap(PromptBootstrapError::AliasConflict);
        assert!(matches!(
            BuiltInPromptService::new(&alias)
                .expect("service")
                .bootstrap(TimestampMillis::new(1)),
            Err(BuiltInPromptServiceError::AliasConflict)
        ));

        let storage = ErrorBootstrap(PromptBootstrapError::Failure("offline".into()));
        assert!(matches!(
            BuiltInPromptService::new(&storage)
                .expect("service")
                .reset_all(TimestampMillis::new(1)),
            Err(BuiltInPromptServiceError::Bootstrap(
                PromptBootstrapError::Failure(message)
            )) if message == "offline"
        ));
    }

    #[test]
    fn real_database_bootstrap_is_idempotent_preserves_edits_and_reset_restores_seed() {
        let database = lettuce_database::Database::open_in_memory().expect("database");
        let service = BuiltInPromptService::new(&database).expect("service");
        let ids = service
            .bootstrap(TimestampMillis::new(1))
            .expect("first bootstrap");
        let first = PromptRepository::get(&database, ids.app_default)
            .expect("get")
            .expect("default");
        let second_ids = service
            .bootstrap(TimestampMillis::new(2))
            .expect("second bootstrap");
        let second = PromptRepository::get(&database, second_ids.app_default)
            .expect("get")
            .expect("default");
        assert_eq!(second.id, first.id);
        assert_eq!(second.revision, first.revision);
        assert_eq!(second.updated_at, first.updated_at);

        let edited_text = "user-owned built-in edit";
        let mut draft = lettuce_context::PromptEntryDraft::from(second.entries[0].clone());
        draft.built_in_entry_key = None;
        draft.content = edited_text.into();
        let edited = PromptRepository::mutate_entries(
            &database,
            second.id,
            second.revision,
            PromptEntryMutation::Update {
                entry_id: second.entries[0].id,
                draft,
            },
            TimestampMillis::new(3),
        )
        .expect("edit")
        .document;

        service
            .bootstrap(TimestampMillis::new(4))
            .expect("preserve edited");
        let preserved = PromptRepository::get(&database, edited.id)
            .expect("get")
            .expect("edited");
        assert_eq!(preserved.entries[0].content, edited_text);
        assert_eq!(preserved.revision, edited.revision);

        let reset = service
            .reset_one(BuiltInPromptId::AppDefault, TimestampMillis::new(5))
            .expect("reset");
        assert_eq!(reset.action, BuiltInReconcileAction::ResetEdited);
        assert_ne!(reset.document.entries[0].content, edited_text);
        assert_eq!(
            reset.document.entries[0].content,
            service.catalog.seed(BuiltInPromptId::AppDefault).entries[0].content
        );
    }

    #[test]
    fn real_database_bootstrap_canonicalizes_the_legacy_lorebook_alias_in_place() {
        let database = lettuce_database::Database::open_in_memory().expect("database");
        let catalog = BuiltInPromptCatalog::bundled().expect("catalog");
        let mut legacy = catalog.seed(BuiltInPromptId::LorebookEntryWriter).clone();
        legacy.key = "prompt_app_lorebook_entry_generator".into();
        legacy.aliases.clear();
        let created = PromptBootstrapPort::reconcile_built_ins(
            &database,
            BuiltInReconcileRequest {
                seeds: vec![legacy],
                mode: BuiltInReconcileMode::RefreshUnedited,
            },
            TimestampMillis::new(1),
        )
        .expect("legacy bootstrap");
        let legacy_id = created[0].document.id;

        let ids = BuiltInPromptService::new(&database)
            .expect("service")
            .bootstrap(TimestampMillis::new(2))
            .expect("canonical bootstrap");
        assert_eq!(ids.lorebook_entry_writer, legacy_id);
        let canonical = PromptRepository::get(&database, legacy_id)
            .expect("get")
            .expect("writer");
        assert!(matches!(
            canonical.provenance,
            PromptProvenance::BuiltIn { ref key, .. }
                if key == BuiltInPromptId::LorebookEntryWriter.key()
        ));
    }

    #[test]
    fn shipped_reply_conditionals_are_accepted_and_render_both_exact_branches() {
        let catalog = BuiltInPromptCatalog::bundled().expect("catalog");
        for id in [
            BuiltInPromptId::ReplyHelper,
            BuiltInPromptId::ReplyHelperConversational,
        ] {
            let seed = catalog.seed(id);
            let document = outcome(seed, TimestampMillis::new(1)).document;
            let draft_entry = document
                .entries
                .iter()
                .find(|entry| {
                    entry.built_in_entry_key.as_deref()
                        == Some(if id == BuiltInPromptId::ReplyHelper {
                            "reply_draft"
                        } else {
                            "reply_conv_draft"
                        })
                })
                .expect("conditional entry");

            let empty = render_prompt(&document, &PromptRenderContext::default())
                .expect("empty-draft branch");
            let empty_entry = empty
                .relative
                .iter()
                .find(|entry| entry.entry_id == draft_entry.id)
                .expect("rendered conditional");
            assert!(empty_entry.content.contains("Generate a fresh"));
            assert!(!empty_entry.content.contains("{{#if"));

            let mut values = PromptRenderValues::default();
            values
                .purpose_values
                .insert(PromptVariable::CurrentDraft, "I was thinking".into());
            let populated = render_prompt(
                &document,
                &PromptRenderContext {
                    values,
                    ..PromptRenderContext::default()
                },
            )
            .expect("populated-draft branch");
            let populated_entry = populated
                .relative
                .iter()
                .find(|entry| entry.entry_id == draft_entry.id)
                .expect("rendered conditional");
            assert!(populated_entry.content.contains("I was thinking"));
            assert!(!populated_entry.content.contains("Generate a fresh"));
            assert!(!populated_entry.content.contains("{{#if"));
        }
    }

    #[derive(Default)]
    struct FakeBootstrap {
        calls: Mutex<Vec<BuiltInReconcileRequest>>,
    }

    struct ErrorBootstrap(PromptBootstrapError);

    impl PromptBootstrapPort for ErrorBootstrap {
        fn reconcile_built_ins(
            &self,
            _request: BuiltInReconcileRequest,
            _now: TimestampMillis,
        ) -> Result<Vec<BuiltInReconcileOutcome>, PromptBootstrapError> {
            Err(self.0.clone())
        }
    }

    impl PromptBootstrapPort for FakeBootstrap {
        fn reconcile_built_ins(
            &self,
            request: BuiltInReconcileRequest,
            now: TimestampMillis,
        ) -> Result<Vec<BuiltInReconcileOutcome>, PromptBootstrapError> {
            request.validate()?;
            let outcomes = request
                .seeds
                .iter()
                .map(|seed| outcome(seed, now))
                .collect();
            self.calls.lock().expect("calls").push(request);
            Ok(outcomes)
        }
    }

    fn deterministic_id(id: BuiltInPromptId) -> PromptDocumentId {
        let index = BuiltInPromptId::ALL
            .iter()
            .position(|candidate| *candidate == id)
            .expect("known id")
            + 1;
        format!("00000000-0000-0000-0000-{index:012}")
            .parse()
            .expect("deterministic UUID")
    }

    fn outcome(seed: &BuiltInPromptSeed, now: TimestampMillis) -> BuiltInReconcileOutcome {
        let catalog_id = BuiltInPromptId::from_key_or_alias(&seed.key).expect("known key");
        let digest = seed.computed_seed_digest().expect("digest");
        BuiltInReconcileOutcome {
            key: seed.key.clone(),
            action: BuiltInReconcileAction::Created,
            document: PromptDocument {
                id: deterministic_id(catalog_id),
                status: LifecycleStatus::Active,
                name: seed.metadata.name.clone(),
                purpose: seed.metadata.purpose,
                entries: seed
                    .entries
                    .iter()
                    .cloned()
                    .map(|draft| PromptEntry {
                        id: PromptEntryId::new(),
                        built_in_entry_key: draft.built_in_entry_key,
                        name: draft.name,
                        role: draft.role,
                        content: draft.content,
                        enabled: draft.enabled,
                        injection_position: draft.injection_position,
                        depth: draft.depth,
                        conditional_min_messages: draft.conditional_min_messages,
                        interval_turns: draft.interval_turns,
                        system_prompt: draft.system_prompt,
                        conditions: draft.conditions,
                        payload: draft.payload,
                    })
                    .collect(),
                condense: seed.metadata.condense,
                behavior_version: seed.metadata.behavior_version,
                provenance: PromptProvenance::BuiltIn {
                    key: seed.key.clone(),
                    seed_version: seed.seed_version,
                    seed_digest: digest.clone(),
                    authored_digest: digest,
                    required: seed.required,
                    protected: seed.protected,
                },
                revision: Revision::INITIAL,
                created_at: now,
                updated_at: now,
            },
        }
    }

    #[test]
    fn resource_is_compile_time_embedded_and_parseable_without_a_filesystem() {
        assert!(CATALOG_JSON.starts_with('{'));
        let resource: serde_json::Value = serde_json::from_str(CATALOG_JSON).expect("JSON");
        assert_eq!(resource["version"], 1);
    }

    #[test]
    fn every_entry_key_is_nonempty_and_unique_within_its_prompt() {
        let resource: CatalogResource = serde_json::from_str(CATALOG_JSON).expect("resource");
        for prompt in resource.prompts {
            let mut seen = HashSet::new();
            for entry in prompt.entries {
                assert!(!entry.key.trim().is_empty(), "{}", prompt.key);
                assert!(
                    seen.insert(entry.key.clone()),
                    "{}: {}",
                    prompt.key,
                    entry.key
                );
            }
        }
    }

    #[test]
    fn catalog_rejects_unknown_and_duplicate_prompt_keys() {
        let mut unknown: CatalogResource = serde_json::from_str(CATALOG_JSON).expect("resource");
        unknown.prompts[0].key = "prompt_app_unknown".into();
        assert!(matches!(
            BuiltInPromptCatalog::from_resource(unknown),
            Err(BuiltInPromptCatalogError::UnknownPromptKey(key)) if key == "prompt_app_unknown"
        ));

        let mut duplicate: CatalogResource = serde_json::from_str(CATALOG_JSON).expect("resource");
        duplicate.prompts[1].key = BuiltInPromptId::AppDefault.key().into();
        assert!(matches!(
            BuiltInPromptCatalog::from_resource(duplicate),
            Err(BuiltInPromptCatalogError::DuplicatePromptKey(key)) if key == BuiltInPromptId::AppDefault.key()
        ));
    }

    #[test]
    fn catalog_rejects_duplicate_entry_keys_and_unknown_variables() {
        let mut duplicate: CatalogResource = serde_json::from_str(CATALOG_JSON).expect("resource");
        duplicate.prompts[0].entries[1].key = duplicate.prompts[0].entries[0].key.clone();
        assert!(matches!(
            BuiltInPromptCatalog::from_resource(duplicate),
            Err(BuiltInPromptCatalogError::DuplicateEntryKey {
                prompt: BuiltInPromptId::AppDefault,
                ..
            })
        ));

        let mut unknown: CatalogResource = serde_json::from_str(CATALOG_JSON).expect("resource");
        unknown.prompts[0].entries[0].content = "{{not_a_registered_variable}}".into();
        assert!(matches!(
            BuiltInPromptCatalog::from_resource(unknown),
            Err(BuiltInPromptCatalogError::UnknownVariable {
                prompt: BuiltInPromptId::AppDefault,
                variable,
            }) if variable == "not_a_registered_variable"
        ));
    }
}
