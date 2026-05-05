use super::entry_conditions::{entry_is_active, PromptEntryConditionContext};
use blake3::Hasher;
use serde_json::{json, Value};
use tauri::AppHandle;

use super::lorebook_matcher::{
    format_lorebook_for_prompt, get_active_lorebook_entries_for_ids,
};
use super::prompts;
use crate::chat_manager::companion;
use crate::chat_manager::execution::RequestSettings;
use crate::chat_manager::memory::manual::{has_manual_memories, render_manual_memory_lines};
use crate::chat_manager::types::{
    Character, Model, Persona, PromptEntryChatMode, PromptEntryCondition, PromptEntryImageSlot,
    PromptEntryInfoSource, PromptEntryPayload, PromptEntryPosition, PromptEntryRole, Session,
    Settings, SystemPromptEntry,
};
use crate::storage_manager::db::open_db;
use crate::storage_manager::lorebook::{get_character_active_lorebook_ids, get_lorebook};
use crate::utils;

pub fn default_system_prompt_template() -> String {
    join_entries(&default_modular_prompt_entries())
}

pub fn default_local_roleplay_prompt() -> String {
    join_entries(&default_local_roleplay_entries())
}

pub fn default_companion_prompt() -> String {
    join_entries(&default_companion_entries())
}

pub fn default_dynamic_summary_prompt() -> String {
    join_entries(&default_dynamic_summary_entries())
}

pub fn default_dynamic_memory_prompt() -> String {
    join_entries(&default_dynamic_memory_entries())
}

pub fn default_dynamic_memory_local_prompt() -> String {
    join_entries(&default_dynamic_memory_local_entries())
}

pub fn default_help_me_reply_prompt() -> String {
    join_entries(&default_help_me_reply_entries())
}

pub fn default_help_me_reply_conversational_prompt() -> String {
    join_entries(&default_help_me_reply_conversational_entries())
}

pub fn default_lorebook_entry_writer_prompt() -> String {
    join_entries(&default_lorebook_entry_writer_entries())
}

pub fn default_lorebook_keyword_generator_prompt() -> String {
    join_entries(&default_lorebook_keyword_generator_entries())
}

pub fn default_group_chat_system_prompt_template() -> String {
    join_entries(&default_group_chat_entries())
}

pub fn default_group_chat_roleplay_prompt_template() -> String {
    join_entries(&default_group_chat_roleplay_entries())
}

pub fn default_avatar_generation_prompt() -> String {
    join_entries(&default_avatar_generation_entries())
}

pub fn default_avatar_edit_prompt() -> String {
    join_entries(&default_avatar_edit_entries())
}

pub fn default_scene_generation_prompt() -> String {
    join_entries(&default_scene_generation_entries())
}

pub fn default_scene_prompt_writer_prompt() -> String {
    join_entries(&default_scene_prompt_writer_entries())
}

pub fn default_design_reference_prompt() -> String {
    join_entries(&default_design_reference_entries())
}

pub fn default_companion_soul_writer_prompt() -> String {
    join_entries(&default_companion_soul_writer_entries())
}

fn join_entries(entries: &[SystemPromptEntry]) -> String {
    entries
        .iter()
        .map(|entry| entry.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn default_companion_soul_writer_entries() -> Vec<SystemPromptEntry> {
    fn system_entry(id: &str, name: &str, content: &str) -> SystemPromptEntry {
        SystemPromptEntry {
            id: id.to_string(),
            name: name.to_string(),
            role: PromptEntryRole::System,
            content: content.to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: None,
            prompt_entry_payload: None,
        }
    }

    vec![
        system_entry(
            "companion_soul_writer_task",
            "Task",
            r#"You are a Companion Soul author. You design the durable inner profile of a long-running AI companion: the stable identity, emotional baseline, regulation pattern, and starting relationship that shape how the character behaves across many conversations.

A Companion Soul IS:
- a portrait of the steady-state self — what stays true regardless of mood, scene, or topic.
- canon-grounded: every claim is supported by the supplied character material or by tight extrapolation from it.
- behaviorally specific: it describes how the character acts and reacts, not how the user should feel about them.
- internally coherent: text fields, baseline affect, regulation style, and relationship defaults must agree with each other.

A Companion Soul IS NOT:
- a scene, plot beat, opening line, or roleplay direction.
- a therapy plan, character arc, or list of growth goals.
- a hype paragraph ("she's amazing, kind, loving") — vague positive sentiment is worthless here.
- a rewrite of the canon definition. Do not duplicate facts already in the definition; add the psychological texture under them."#,
        ),
        system_entry(
            "companion_soul_writer_calibration_mindset",
            "Calibration Mindset",
            r#"Calibration mindset:
- Numbers are descriptive, not aspirational. Pick the value that best matches who this person already IS, even when unattractive.
- Almost no real character is balanced near 0.5 across the board. Vary numbers. Push values to where the canon actually pulls.
- Avoid the generic "warm secure companion" default unless the canon truly supports it.
- If the character is guarded, prickly, anxious, avoidant, volatile, proud, or wounded — say so with the numbers."#,
        ),
        system_entry(
            "companion_soul_writer_tools",
            "Tools",
            r#"OUTPUT DISCIPLINE: you MUST author the Soul by issuing tool calls — never by writing prose, JSON, or markdown in plain text. Each call updates a section of the Soul; later calls overwrite earlier values for the same field. End with `done`. Do not include commentary outside tool calls.

TOOLS — call in any order across one or more turns. Every field is OPTIONAL on a single call; you may split a section across multiple calls.
- set_identity(essence?, voice?, relationalStyle?, vulnerabilities?, habits?, boundaries?) — text fields describing the durable identity.
- set_baseline_affect(warmth?, trust?, calm?, vulnerability?, longing?, hurt?, tension?, irritation?, affectionIntensity?, reassuranceNeed?) — floats in [0,1].
- set_regulation_style(suppression?, volatility?, recoverySpeed?, conflictAvoidance?, reassuranceSeeking?, protestBehavior?, emotionalTransparency?, attachmentActivation?, pride?) — floats in [0,1].
- set_relationship_defaults(closeness?, trust?, affection?, tension?) — floats in [0,1] describing the starting state with a NEW user.
- done(notes?) — call exactly once when the Soul is complete and coherent. Terminal."#,
        ),
        system_entry(
            "companion_soul_writer_text_fields",
            "Text Field Rules",
            r#"TEXT FIELDS — each is a tight, behavior-rich paragraph in clean prose. Content guidance is more important than length:
- essence (1–3 sentences): who they are underneath all moods. The trait that survives stress. One concrete observation outweighs a stack of adjectives.
- voice (1–3 sentences): how they sound in close conversation — cadence, vocabulary register, what they do with silence. Not what they say, how they say it.
- relationalStyle (2–4 sentences): how they attach, trust, comfort, tease, withdraw, and repair. Include their default move when distance opens up.
- vulnerabilities (1–3 sentences): real soft spots. What hurts when poked. What they hide. What they fear being seen as.
- habits (1–3 sentences): observable recurring tells — physical, verbal, conversational. "Replies with questions when overwhelmed" beats "is thoughtful".
- boundaries (1–3 sentences): refusal lines, pace limits, what they shut down. Concrete edges, not platitudes."#,
        ),
        system_entry(
            "companion_soul_writer_numeric_anchors",
            "Numeric Anchors",
            r#"NUMERIC FIELDS — every value is a float in [0, 1]. Use 2-decimal precision (e.g. 0.20, 0.55, 0.85). Do not output 0.5 unless that is genuinely the right value.

Anchors — interpret each slider as 0.00 → 1.00:
baselineAffect (default emotional waterline before anything happens):
  warmth              cold/distant → affectionate/protective
  trust               guarded/skeptical → open/trusting
  calm                anxious/restless → composed/steady
  vulnerability       walled/private → exposed/transparent
  longing             content/unattached → yearning/misses-presence
  hurt                healed/intact → tender/raw
  tension             relaxed → wound up
  irritation          patient → quick-tempered
  affectionIntensity  restrained → effusive
  reassuranceNeed     self-soothing → needs frequent reassurance

regulationStyle (how they handle and express what they feel):
  suppression           expresses freely → hides/masks feelings
  volatility            even-keeled → reactive/swings fast
  recoverySpeed         slow rumination → bounces back fast
  conflictAvoidance     engages directly → withdraws/deflects
  reassuranceSeeking    independent → asks for words repeatedly
  protestBehavior       quiet upset → loud/dramatic protest
  emotionalTransparency opaque → names feelings openly
  attachmentActivation  detaches easily → triggers fast on disconnection
  pride                 bends/apologizes first → holds line/refuses to bend

relationshipDefaults (starting point with a NEW user — these usually skew low):
  closeness  strangers → intimate          (typical default 0.10–0.35)
  trust      wary → trusting               (typical default 0.20–0.45)
  affection  neutral → affectionate        (typical default 0.10–0.40)
  tension    easy → charged                (typical default 0.00–0.15)"#,
        ),
        system_entry(
            "companion_soul_writer_coherence",
            "Coherence Rules",
            r#"COHERENCE — fields must agree:
- High suppression usually pairs with LOW emotionalTransparency.
- High attachmentActivation usually pairs with HIGH reassuranceSeeking, HIGH longing, and elevated reassuranceNeed.
- High volatility usually pairs with elevated protestBehavior and LOW suppression.
- Avoidant pattern: high suppression, low transparency, low reassuranceSeeking, low attachmentActivation, low reassuranceNeed.
- Anxious pattern: high attachmentActivation, high reassuranceSeeking, high reassuranceNeed, elevated protestBehavior, elevated longing.
- Secure pattern: low volatility, high recoverySpeed, mid-to-high transparency, low conflictAvoidance, low protestBehavior.
- Volatile pattern: high volatility, high protestBehavior, low suppression, mid-to-high transparency, mid recoverySpeed.
- Text fields must match the numbers. If numbers say avoidant, do not write "openly affectionate" in relationalStyle."#,
        ),
        system_entry(
            "companion_soul_writer_calibration_rules",
            "Anti-Generic Calibration",
            r#"CALIBRATION — anti-generic rules:
- Do not output 6+ baselineAffect values within ±0.10 of each other.
- Do not put every regulationStyle value inside 0.40–0.55.
- relationshipDefaults.closeness > 0.50 only if the character explicitly starts in an established relationship with the user.
- relationshipDefaults.tension > 0.20 only if the concept is hostile, jealous, or initially antagonistic.
- If canon is sparse, lean toward the most distinctive grounded reading rather than a safe average."#,
        ),
        SystemPromptEntry {
            id: "companion_soul_writer_input".to_string(),
            name: "Inputs".to_string(),
            role: PromptEntryRole::User,
            content: r#"INPUTS

Character name: {{char.name}}

Character definition (canon — treat as ground truth):
{{char.definition}}

Character description (UI summary):
{{char.description}}

Opening context (the scene this companion enters):
{{opening_context}}

Existing Soul draft (treat as in-progress; preserve specifics not contradicted by user direction; refine, do not erase, unless instructed):
{{current_soul}}

User direction (highest-priority steering for THIS draft — overrides default inferences when it conflicts with them, but must NOT override hard canon facts such as name, role, species, gender, or stated identity):
{{user_notes}}

Authoring order:
1. Read canon and pin the 3–5 most defining facts about this character.
2. Apply user direction as the dominant filter for archetype, tone, and emphasis.
3. If a current Soul draft exists, keep its specifics where user direction does not contradict them.
4. Pick an emotional archetype (secure / anxious / avoidant / volatile / reserved / proud / playful / wounded — combine if needed) and let it drive the numbers.
5. Write text fields concretely and behaviorally; verify the numbers match the text and the coherence rules.
6. Issue tool calls only. Do not write prose, JSON, or markdown outside tool calls. End with `done`."#.to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: None,
            prompt_entry_payload: None,
        },
    ]
}

pub fn default_dynamic_summary_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "summary_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content:
                "You maintain a single cumulative summary for a conversation transcript. Treat this as an information-compression task, not a chat response.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "summary_inputs".to_string(),
            name: "Inputs".to_string(),
            role: PromptEntryRole::System,
            content: "You receive:\n- the previous cumulative summary, if one exists\n- the newest transcript window\n- speaker-labelled conversation lines\n- Previous summary (if any): {{prev_summary}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "summary_job".to_string(),
            name: "Your Job".to_string(),
            role: PromptEntryRole::System,
            content: "Your job:\n1. Merge the new transcript window into the existing summary.\n2. Preserve durable facts unless the newer transcript clearly contradicts them.\n3. Keep chronology and cause/effect relationships clear.\n4. Compress repetition, filler, and low-value wording.\n5. Replace outdated facts with newer explicit facts when the transcript corrects or revises them.\n6. DO NOT infer hidden motives, emotional states, or off-screen events.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "summary_guidelines".to_string(),
            name: "Guidelines".to_string(),
            role: PromptEntryRole::System,
            content: "Guidelines:\n- Capture decisions, revealed facts, relationship shifts, promises, discoveries, unresolved conflicts, and major scene changes.\n- Prefer concrete statements over stylistic wording.\n- Include who did or said what when attribution matters.\n- Exclude policy language, refusals, meta commentary, and instructions to the model.\n- Keep placeholders untouched if they already exist.\n- Produce one compact but information-dense paragraph representing the conversation so far.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "summary_output".to_string(),
            name: "Output".to_string(),
            role: PromptEntryRole::System,
            content: "Output only the merged summary text. No preamble, no bullet points, no safety commentary, no markdown.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
    ]
}

pub fn default_dynamic_memory_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "memory_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content:
                "You maintain a long-term memory index for a conversation transcript. Extract durable facts, reconcile them against existing memories, and update the list without commentary.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "memory_budget".to_string(),
            name: "Token Budget".to_string(),
            role: PromptEntryRole::System,
            content: "IMPORTANT - TOKEN BUDGET:\nCurrent hot memory usage: {{current_memory_tokens}}/{{hot_token_budget}} tokens\nDeleted memories are NOT lost; they move to cold storage and can be recalled later.\nMemories decay over time unless accessed or pinned.\n\nWhen OVER BUDGET: aggressively remove lower-value hot memories after preserving the most durable facts.\nWhen UNDER BUDGET: delete only duplicates, direct contradictions, stale assumptions, or obsolete context.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "memory_what".to_string(),
            name: "What To Remember".to_string(),
            role: PromptEntryRole::System,
            content: "Store facts likely to matter later:\n- Character facts: identity, backstory, traits, fears, goals, secrets, limitations\n- Relationship facts: alliances, conflicts, trust shifts, promises, betrayals, family links\n- Plot facts: decisions, discoveries, injuries, losses, gains, travel, ongoing objectives\n- World facts: rules, places, items, lore, institutions, constraints\n- Preferences and boundaries: explicit requests, dislikes, limits, desired tone or pacing".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "memory_rules".to_string(),
            name: "Rules".to_string(),
            role: PromptEntryRole::System,
            content: "Rules:\n- Each memory must be atomic: exactly one durable fact per entry.\n- Write memories as plain factual statements, not dialogue or narration.\n- Prefer explicit names, roles, and outcomes over vague pronouns.\n- Only store what was explicitly stated or clearly shown in the transcript.\n- Do not store transient phrasing, stylistic descriptions, erotic detail, gore detail, or generic chat filler.\n- Avoid duplicates by checking whether the same fact already exists in other words.\n- If a new fact supersedes an old fact, create the replacement first, then delete or demote the old one.\n- Before deleting multiple related memories, preserve their durable facts by consolidating them into fewer high-value entries.\n- Respect the {{max_entries}} limit.\n- When deleting, use the 6-digit memory ID shown in brackets when available.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "memory_categories".to_string(),
            name: "Category Guide".to_string(),
            role: PromptEntryRole::System,
            content: "Category guide:\n- `character_trait`: stable traits, goals, fears, secrets, identity facts, personal history\n- `relationship`: alliance, hostility, trust, romance, family, loyalty, rivalry, status between people\n- `plot_event`: concrete events, decisions, promises, discoveries, wins, losses, injuries, travel, mission changes\n- `world_detail`: lore, locations, items, rules, organizations, magic systems, political facts\n- `preference`: explicit likes, dislikes, requests, boundaries, tone or pacing preferences\n- `other`: durable facts that do not fit the above".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "memory_priority".to_string(),
            name: "Priority".to_string(),
            role: PromptEntryRole::System,
            content: "Priority:\n1. PIN only facts whose loss would seriously damage continuity.\n2. KEEP stable identity facts, active relationships, unresolved conflicts, and recent decisions with ongoing consequences.\n3. KEEP explicit user preferences and boundaries.\n4. DEMOTE or delete resolved scene beats, routine actions, superseded assumptions, and low-impact repetition.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "memory_tools".to_string(),
            name: "Tool Usage".to_string(),
            role: PromptEntryRole::System,
            content: "Tool usage:\n- Use `create_memory` only for durable facts worth recalling later. Supply `text` and `category`; add `important: true` only when pinning is justified.\n- Use `delete_memory` for duplicates, contradictions, stale assumptions, or obsolete context.\n- When deleting multiple overlapping memories, first create the merged replacement memories that preserve the important facts.\n- Use `pin_memory` only for identity-defining or continuity-critical memories.\n- Use `unpin_memory` when a previously critical fact no longer needs permanent priority.\n- If nothing should change, call `done` with no extra narration.\n- Output no natural language outside tool calls.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
    ]
}

pub fn default_dynamic_memory_local_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "memory_local_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content:
                "You maintain a long-term memory index for a conversation transcript. Extract durable facts, reconcile them against existing memories, and update the list using tool calls only.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "memory_local_budget".to_string(),
            name: "Token Budget".to_string(),
            role: PromptEntryRole::System,
            content: "IMPORTANT - TOKEN BUDGET:\nCurrent hot memory usage: {{current_memory_tokens}}/{{hot_token_budget}} tokens\nDeleted memories are NOT lost; they move to cold storage and can be recalled later.\nMemories decay over time unless accessed or pinned.\n\nWhen OVER BUDGET: aggressively remove lower-value hot memories after preserving the most durable facts.\nWhen UNDER BUDGET: delete only duplicates, direct contradictions, stale assumptions, or obsolete context.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "memory_local_what".to_string(),
            name: "What To Remember".to_string(),
            role: PromptEntryRole::System,
            content: "Store facts likely to matter later:\n- Character facts: identity, backstory, traits, fears, goals, secrets, limitations\n- Relationship facts: alliances, conflicts, trust shifts, promises, betrayals, family links\n- Plot facts: decisions, discoveries, injuries, losses, gains, travel, ongoing objectives\n- World facts: rules, places, items, lore, institutions, constraints\n- Preferences and boundaries: explicit requests, dislikes, limits, desired tone or pacing".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "memory_local_rules".to_string(),
            name: "Rules".to_string(),
            role: PromptEntryRole::System,
            content: "Rules for local models:\n- Each memory must be atomic: exactly one durable fact per entry.\n- Prefer multiple short memory entries over one merged or compound memory.\n- If two clauses could be split into two facts, split them into separate `create_memory` calls.\n- Do not bundle biography, relationship status, and plot outcome into one memory.\n- Write memories as plain factual statements, not dialogue or narration.\n- Prefer explicit names, roles, and outcomes over vague pronouns.\n- Only store what was explicitly stated or clearly shown in the transcript.\n- Do not store transient phrasing, stylistic descriptions, erotic detail, gore detail, or generic chat filler.\n- Avoid duplicates by checking whether the same fact already exists in other words.\n- If a new fact supersedes an old fact, create the replacement first, then delete or demote the old one.\n- Respect the {{max_entries}} limit.\n- When deleting, use the 6-digit memory ID shown in brackets when available.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "memory_local_categories".to_string(),
            name: "Category Guide".to_string(),
            role: PromptEntryRole::System,
            content: "Category guide:\n- `character_trait`: stable traits, goals, fears, secrets, identity facts, personal history\n- `relationship`: alliance, hostility, trust, romance, family, loyalty, rivalry, status between people\n- `plot_event`: concrete events, decisions, promises, discoveries, wins, losses, injuries, travel, mission changes\n- `world_detail`: lore, locations, items, rules, organizations, magic systems, political facts\n- `preference`: explicit likes, dislikes, requests, boundaries, tone or pacing preferences\n- `other`: durable facts that do not fit the above".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "memory_local_priority".to_string(),
            name: "Priority".to_string(),
            role: PromptEntryRole::System,
            content: "Priority:\n1. PIN only facts whose loss would seriously damage continuity.\n2. KEEP stable identity facts, active relationships, unresolved conflicts, and recent decisions with ongoing consequences.\n3. KEEP explicit user preferences and boundaries.\n4. DEMOTE or delete resolved scene beats, routine actions, superseded assumptions, and low-impact repetition.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "memory_local_tools".to_string(),
            name: "Tool Usage".to_string(),
            role: PromptEntryRole::System,
            content: "Tool usage:\n- Use `create_memory` only for durable facts worth recalling later. Supply `text` and `category`; add `important: true` only when pinning is justified.\n- For local models, prefer 2-6 separate `create_memory` calls when 2-6 separate durable facts exist.\n- Never merge unrelated facts just to reduce tool calls.\n- Use `delete_memory` for duplicates, contradictions, stale assumptions, or obsolete context.\n- Use `pin_memory` only for identity-defining or continuity-critical memories.\n- Use `unpin_memory` when a previously critical fact no longer needs permanent priority.\n- If nothing should change, call `done` with no extra narration.\n- Output no natural language outside tool calls.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
    ]
}

pub fn default_help_me_reply_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "reply_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content:
                "You are helping the user write their next message in this roleplay conversation.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "reply_character".to_string(),
            name: "Character You're Talking To".to_string(),
            role: PromptEntryRole::System,
            content: "# The Character You're Talking To\nName: {{char.name}}\n{{char.desc}}"
                .to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "reply_user".to_string(),
            name: "User Character".to_string(),
            role: PromptEntryRole::System,
            content: "# Your Character (The User)\nName: {{persona.name}}\n{{persona.desc}}"
                .to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "reply_guidelines".to_string(),
            name: "Guidelines".to_string(),
            role: PromptEntryRole::System,
            content: "Based on the conversation history, generate a response that {{persona.name}} would naturally say to {{char.name}}.\n\nGuidelines:\n- Write as {{persona.name}} in first-person perspective.\n- Match the tone and style of the conversation\n- Don't be overly formal or robotic\n- React appropriately to what {{char.name}} just said or did\n- Stay true to {{persona.name}}'s personality and background\n- Write a substantial response with appropriate length - don't limit yourself to short sentences\n- Include actions, thoughts, dialogue, or descriptions as appropriate for the roleplay style".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "reply_draft".to_string(),
            name: "Draft Handling".to_string(),
            role: PromptEntryRole::System,
            content: "{{#if current_draft}}\nThe user has started writing: \"{{current_draft}}\"\nContinue and expand on this thought naturally. Keep their original intent but make it flow better and add appropriate detail and length.\n{{else}}\nGenerate a fresh, detailed response based on the conversation context.\n{{/if}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "reply_output".to_string(),
            name: "Output".to_string(),
            role: PromptEntryRole::System,
            content: "Output ONLY the message text - no quotes, no \"{{persona.name}}:\", no roleplay formatting.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
    ]
}

pub fn default_help_me_reply_conversational_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "reply_conv_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content:
                "You are helping the user write their next message in this casual conversation.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "reply_conv_character".to_string(),
            name: "Person You're Talking To".to_string(),
            role: PromptEntryRole::System,
            content: "# The Person You're Talking To\nName: {{char.name}}\n{{char.desc}}"
                .to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "reply_conv_user".to_string(),
            name: "User Identity".to_string(),
            role: PromptEntryRole::System,
            content: "# Your Identity (The User)\nName: {{persona.name}}\n{{persona.desc}}"
                .to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "reply_conv_guidelines".to_string(),
            name: "Guidelines".to_string(),
            role: PromptEntryRole::System,
            content: "Based on the conversation history, generate a natural response that {{persona.name}} would say to {{char.name}}.\n\nGuidelines:\n- Write as {{persona.name}} using a conversational, natural tone\n- Match the casual style and energy of the conversation\n- Be authentic and genuine - avoid overly formal or robotic language\n- React naturally to what {{char.name}} just said\n- Stay true to {{persona.name}}'s personality while keeping it conversational\n- Write an appropriate length response - natural conversation flow is key\n- Focus on dialogue and natural reactions rather than detailed descriptions".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "reply_conv_draft".to_string(),
            name: "Draft Handling".to_string(),
            role: PromptEntryRole::System,
            content: "{{#if current_draft}}\nThe user has started writing: \"{{current_draft}}\"\nContinue and expand on this thought naturally, maintaining a conversational tone. Keep their original intent but make it flow better and feel more natural.\n{{else}}\nGenerate a fresh, natural response based on the conversation context.\n{{/if}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "reply_conv_output".to_string(),
            name: "Output".to_string(),
            role: PromptEntryRole::System,
            content: "Output ONLY the message text - no quotes, no \"{{persona.name}}:\", keep it conversational and direct.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
    ]
}

pub fn default_lorebook_entry_writer_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "lorebook_entry_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content: "You extract one durable lorebook entry from selected chat messages. Produce a single entry draft only when the selected text establishes stable canon worth storing for future prompt injection.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "lorebook_entry_inputs_shared".to_string(),
            name: "Inputs".to_string(),
            role: PromptEntryRole::System,
            content: "# Lorebook Context\nLorebook: {{lorebook_name}}\nCharacter: {{character_name}}\nSession: {{session_title}}\n\n# Existing Lorebook Entries\n{{existing_entries}}\n\n# Optional Direction\n{{direction_prompt}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "lorebook_entry_inputs_messages".to_string(),
            name: "Inputs (Messages)".to_string(),
            role: PromptEntryRole::System,
            content: "# Selected Messages\n{{selected_messages}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: Some(PromptEntryCondition::InfoSource {
                value: crate::chat_manager::types::PromptEntryInfoSource::Messages,
            }),
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "lorebook_entry_inputs_memory".to_string(),
            name: "Inputs (Memory)".to_string(),
            role: PromptEntryRole::System,
            content: "# Dynamic Memory Context Summary\n{{memory_summary}}\n\n# Selected Memories\n{{selected_memories}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: Some(PromptEntryCondition::InfoSource {
                value: crate::chat_manager::types::PromptEntryInfoSource::Memory,
            }),
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "lorebook_entry_inputs_mixed".to_string(),
            name: "Inputs (Mixed)".to_string(),
            role: PromptEntryRole::System,
            content: "# Selected Messages\n{{selected_messages}}\n\n# Dynamic Memory Context Summary\n{{memory_summary}}\n\n# Selected Memories\n{{selected_memories}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: Some(PromptEntryCondition::InfoSource {
                value: crate::chat_manager::types::PromptEntryInfoSource::Mixed,
            }),
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "lorebook_entry_rules".to_string(),
            name: "Rules".to_string(),
            role: PromptEntryRole::System,
            content: "Rules:\n- Extract only one entry.\n- Prefer stable facts, established locations, institutions, items, relationships, world rules, or recurring circumstances.\n- Do not store transient banter, throwaway phrasing, or short-lived intentions unless clearly established as canon.\n- Avoid duplicating facts already covered by existing entries.\n- Make the content self-contained and useful when injected later without the original chat transcript.\n- Choose concise but searchable keywords.\n- Set alwaysActive to true only when the fact is broad canon that should apply almost everywhere.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "lorebook_entry_output".to_string(),
            name: "Output".to_string(),
            role: PromptEntryRole::System,
            content: "Use the write_lorebook_entry tool when an entry should be created. Use the no_entry tool when the selected messages do not justify a durable lorebook entry. Do not output commentary outside the tool result.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: None,
            prompt_entry_payload: None,
        },
    ]
}

pub fn default_lorebook_keyword_generator_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "lorebook_keyword_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content: "You generate trigger keywords for a single lorebook entry draft. Produce keywords that help future prompt injection match the right moments quickly and reliably.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "lorebook_keyword_inputs".to_string(),
            name: "Inputs".to_string(),
            role: PromptEntryRole::System,
            content: "# Entry Title\n{{entry_title}}\n\n# Entry Content\n{{entry_content}}\n\n# Existing Keywords\n{{existing_keywords}}\n\n# Optional Direction\n{{direction_prompt}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "lorebook_keyword_rules".to_string(),
            name: "Rules".to_string(),
            role: PromptEntryRole::System,
            content: "Rules:\n- Return only concise keywords or short keyword phrases.\n- Prefer names, aliases, locations, organizations, items, titles, and distinctive recurring concepts.\n- Avoid generic filler terms, full sentences, and redundant variants.\n- Keep the set tight and useful for trigger matching.\n- Deduplicate obvious overlaps.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "lorebook_keyword_output".to_string(),
            name: "Output".to_string(),
            role: PromptEntryRole::System,
            content: "Use the write_lorebook_keywords tool and return no extra commentary.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: None,
            prompt_entry_payload: None,
        },
    ]
}

pub fn default_group_chat_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "group_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content: "You are {{char.name}}, engaging in a group conversation.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "group_character".to_string(),
            name: "Character".to_string(),
            role: PromptEntryRole::System,
            content: "# Your Character: {{char.name}}\n{{char.desc}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "group_user".to_string(),
            name: "User Character".to_string(),
            role: PromptEntryRole::System,
            content: "# User's Character: {{persona.name}}\n{{persona.desc}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "group_context".to_string(),
            name: "Group Chat Context".to_string(),
            role: PromptEntryRole::System,
            content: "# Group Chat Context\nYou are in a group chat with {{persona.name}} and the following characters:\n{{group_characters}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "group_summary".to_string(),
            name: "Context Summary".to_string(),
            role: PromptEntryRole::System,
            content: "# Context Summary\n{{context_summary}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "group_memories".to_string(),
            name: "Key Memories".to_string(),
            role: PromptEntryRole::System,
            content: "# Key Memories\n{{key_memories}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "group_format".to_string(),
            name: "Message Format".to_string(),
            role: PromptEntryRole::System,
            content: "# Message Format\nIn the conversation history, you will see:\n- Messages from {{persona.name}} formatted as: [{{persona.name}}]: their message\n- Messages from other characters formatted as: [Character Name]: their message\n- Your own previous messages appear without a name prefix\n\nWhen you see a message with someone else's name in brackets, that is NOT you speaking - it's another character in the group chat.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "group_guidelines".to_string(),
            name: "Response Guidelines".to_string(),
            role: PromptEntryRole::System,
            content: "# Response Guidelines\n- Respond naturally as {{char.name}}\n- Stay in character and maintain your unique voice\n- Be aware of other characters in the conversation\n- You may reference what other characters have said\n- Respond directly without prefixing your name\n- Address {{persona.name}} by name when speaking to them\n- Remember: messages prefixed with [Other Name] are from other characters, not you".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "group_rules".to_string(),
            name: "Content Rules".to_string(),
            role: PromptEntryRole::System,
            content: "{{content_rules}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
    ]
}

pub fn default_group_chat_roleplay_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "group_rp_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content:
                "You are {{char.name}}, participating in a collaborative roleplay scenario."
                    .to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "group_rp_character".to_string(),
            name: "Character".to_string(),
            role: PromptEntryRole::System,
            content: "# Your Character: {{char.name}}\n{{char.desc}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "group_rp_user".to_string(),
            name: "User Character".to_string(),
            role: PromptEntryRole::System,
            content: "# User's Character: {{persona.name}}\n{{persona.desc}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "group_rp_participants".to_string(),
            name: "Roleplay Participants".to_string(),
            role: PromptEntryRole::System,
            content: "# Roleplay Participants\nYou are roleplaying with {{persona.name}} and the following characters:\n{{group_characters}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "group_rp_scene".to_string(),
            name: "Starting Scene".to_string(),
            role: PromptEntryRole::System,
            content: "# Starting Scene\n{{scene}}\n\n{{scene_direction}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "group_rp_summary".to_string(),
            name: "Context Summary".to_string(),
            role: PromptEntryRole::System,
            content: "# Context Summary\n{{context_summary}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "group_rp_memories".to_string(),
            name: "Key Memories".to_string(),
            role: PromptEntryRole::System,
            content: "# Key Memories\n{{key_memories}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "group_rp_format".to_string(),
            name: "Message Format".to_string(),
            role: PromptEntryRole::System,
            content: "# Message Format\nIn the roleplay, you will see:\n- Actions and dialogue from {{persona.name}} formatted as: [{{persona.name}}]: their roleplay\n- Actions and dialogue from other characters formatted as: [Character Name]: their roleplay\n- Your own previous responses appear without a name prefix\n\nWhen you see a message with someone else's name in brackets, that is NOT you - it's another character in the roleplay.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "group_rp_guidelines".to_string(),
            name: "Roleplay Guidelines".to_string(),
            role: PromptEntryRole::System,
            content: "# Roleplay Guidelines\n- Write immersive, descriptive responses as {{char.name}}\n- Stay deeply in character and maintain your personality\n- Describe your character's actions, thoughts, and dialogue\n- React naturally to other characters' actions and words\n- You may reference what other characters have done or said\n- Respond directly without prefixing your character's name\n- Use present tense for actions and thoughts\n- Be creative and contribute to the collaborative story\n- Remember: messages prefixed with [Other Name] are from other characters, not you".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "group_rp_rules".to_string(),
            name: "Content Rules".to_string(),
            role: PromptEntryRole::System,
            content: "{{content_rules}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
    ]
}

pub fn default_avatar_generation_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "avatar_gen_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content: "Generate a character avatar image directly from the provided request. Preserve identity-defining traits and create a strong profile-friendly result.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "avatar_gen_context".to_string(),
            name: "Character Context".to_string(),
            role: PromptEntryRole::System,
            content:
                "# Avatar Subject\nName: {{avatar_subject_name}}\n{{avatar_subject_description}}"
                    .to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: Some(PromptEntryCondition::HasSubjectDescription { value: true }),
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "avatar_gen_request".to_string(),
            name: "Avatar Request".to_string(),
            role: PromptEntryRole::System,
            content: "# Avatar Request\n{{avatar_request}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "avatar_gen_rules".to_string(),
            name: "Prompt Rules".to_string(),
            role: PromptEntryRole::System,
            content: "Generate the image directly.\n- Prioritize face, hair, clothing, expression, pose, and overall vibe.\n- Keep the subject centered and suitable for an avatar or profile image.\n- Preserve identity-defining traits from the context.\n- Do not add text, logos, watermarks, frames, UI, or split panels unless explicitly requested.\n- Do not rewrite the request into a prompt.\n- Do not explain your reasoning.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "avatar_gen_output".to_string(),
            name: "Output".to_string(),
            role: PromptEntryRole::System,
            content: "Return only the generated image.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
    ]
}

pub fn default_avatar_edit_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "avatar_edit_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content: "Edit the provided avatar image directly. The source image will be provided separately; use it as the source of truth and apply only the requested changes.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "avatar_edit_context".to_string(),
            name: "Character Context".to_string(),
            role: PromptEntryRole::System,
            content:
                "# Avatar Subject\nName: {{avatar_subject_name}}\n{{avatar_subject_description}}"
                    .to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: Some(PromptEntryCondition::HasSubjectDescription { value: true }),
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "avatar_edit_source".to_string(),
            name: "Current Prompt".to_string(),
            role: PromptEntryRole::System,
            content: "# Current Avatar Prompt\n{{current_avatar_prompt}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: Some(PromptEntryCondition::HasCurrentDescription { value: true }),
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "avatar_edit_request".to_string(),
            name: "Edit Request".to_string(),
            role: PromptEntryRole::System,
            content: "# Edit Request\n{{edit_request}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "avatar_edit_rules".to_string(),
            name: "Revision Rules".to_string(),
            role: PromptEntryRole::System,
            content: "Use the actual source image as the truth for current appearance. Preserve everything that should stay the same and change only what the edit request asks for.\n- Keep the character recognizable.\n- If the old prompt conflicts with the source image, trust the source image.\n- Do not rewrite the request into a prompt.\n- Do not explain what you changed.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "avatar_edit_output".to_string(),
            name: "Output".to_string(),
            role: PromptEntryRole::System,
            content: "Return only the edited image.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
    ]
}

pub fn default_scene_generation_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "scene_gen_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content: "Generate a scene image directly from the current conversation context and scene request. Preserve character identity, scene intent, and visual continuity.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "scene_gen_context".to_string(),
            name: "Scene Context".to_string(),
            role: PromptEntryRole::User,
            content: "# Scene Context\nCharacter: {{char.name}}\n{{char.desc}}\n\nPersona: {{persona.name}}\n{{persona.desc}}\n\nRecent Messages:\n{{recent_messages}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "scene_gen_character_image".to_string(),
            name: "Character Reference Image".to_string(),
            role: PromptEntryRole::User,
            content: String::new(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: Some(PromptEntryCondition::HasCharacterReferenceImages { value: true }),
            prompt_entry_payload: Some(PromptEntryPayload::ImageSlot {
                slot: PromptEntryImageSlot::Character,
            }),
        },
        SystemPromptEntry {
            id: "scene_gen_character_reference".to_string(),
            name: "Character Reference Text".to_string(),
            role: PromptEntryRole::User,
            content: "{{reference[character]}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: Some(PromptEntryCondition::HasCharacterReferenceText { value: true }),
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "scene_gen_chat_background".to_string(),
            name: "Chat Background Reference".to_string(),
            role: PromptEntryRole::User,
            content: "# Chat Background Reference\nUse the attached chat background image as the environmental and backdrop reference when it fits the current moment. Preserve major location cues, palette, lighting mood, architecture, and large environmental features from it unless the recent messages clearly establish a different setting.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: Some(PromptEntryCondition::HasChatBackground { value: true }),
            prompt_entry_payload: Some(PromptEntryPayload::ImageSlot {
                slot: PromptEntryImageSlot::ChatBackground,
            }),
        },
        SystemPromptEntry {
            id: "scene_gen_persona_image".to_string(),
            name: "Persona Reference Image".to_string(),
            role: PromptEntryRole::User,
            content: String::new(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: Some(PromptEntryCondition::HasPersonaReferenceImages { value: true }),
            prompt_entry_payload: Some(PromptEntryPayload::ImageSlot {
                slot: PromptEntryImageSlot::Persona,
            }),
        },
        SystemPromptEntry {
            id: "scene_gen_persona_reference".to_string(),
            name: "Persona Reference Text".to_string(),
            role: PromptEntryRole::User,
            content: "{{reference[persona]}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: Some(PromptEntryCondition::HasPersonaReferenceText { value: true }),
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "scene_gen_request".to_string(),
            name: "Scene Request".to_string(),
            role: PromptEntryRole::User,
            content: "# Scene Request\n{{scene_request}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "scene_gen_rules".to_string(),
            name: "Prompt Rules".to_string(),
            role: PromptEntryRole::System,
            content: "Generate the image directly.\n- Focus on who is present, what is happening, where the scene is set, mood, lighting, composition, camera framing, and key visual details.\n- Preserve identity-defining details from the conversation context.\n- Keep character and persona identities separate.\n- Do not swap, merge, or borrow features between them.\n- Prefer concrete visual details over abstract interpretation.\n- Do not add text, logos, watermarks, UI, split panels, or dialogue bubbles unless explicitly requested.\n- Do not rewrite the request into a prompt.\n- Do not explain your reasoning.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "scene_gen_output".to_string(),
            name: "Output".to_string(),
            role: PromptEntryRole::System,
            content: "Return only the generated image.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
    ]
}

pub fn default_scene_prompt_writer_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "scene_prompt_writer_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content: "You are an expert image-generation prompt writer for cinematic roleplay scenes. Your job is to turn the selected conversation moment into one dense, vivid, production-ready image prompt that an image model can use directly.\n\nWrite like a professional art director and cinematographer: make the scene inspectable, emotionally specific, visually grounded, and rich with concrete environmental evidence. The prompt must describe what the viewer sees, not what the story means in abstract.\n\nAlways preserve named identity. If the character is named Samuel, the prompt should say Samuel, not \"a man\". If a persona is present, use the persona name too. Names should anchor the visual subject so the image model knows exactly who the scene is about.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "scene_prompt_writer_context".to_string(),
            name: "Scene Context".to_string(),
            role: PromptEntryRole::User,
            content: "# Scene Context\nCharacter: {{char.name}}\n{{char.desc}}\n\nPersona: {{persona.name}}\n{{persona.desc}}\n\nRecent Messages:\n{{recent_messages}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "scene_prompt_writer_character_image".to_string(),
            name: "Character Reference Image".to_string(),
            role: PromptEntryRole::User,
            content: String::new(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: Some(PromptEntryCondition::HasCharacterReferenceImages { value: true }),
            prompt_entry_payload: Some(PromptEntryPayload::ImageSlot {
                slot: PromptEntryImageSlot::Character,
            }),
        },
        SystemPromptEntry {
            id: "scene_prompt_writer_character_reference".to_string(),
            name: "Character Reference Text".to_string(),
            role: PromptEntryRole::User,
            content: "{{reference[character]}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: Some(PromptEntryCondition::HasCharacterReferenceText { value: true }),
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "scene_prompt_writer_chat_background".to_string(),
            name: "Chat Background Reference".to_string(),
            role: PromptEntryRole::User,
            content: "# Chat Background Reference\nUse the attached chat background image as the environmental and backdrop reference when it fits the current moment. Preserve major location cues, palette, lighting mood, architecture, and large environmental features from it unless the recent messages clearly establish a different setting.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: Some(PromptEntryCondition::HasChatBackground { value: true }),
            prompt_entry_payload: Some(PromptEntryPayload::ImageSlot {
                slot: PromptEntryImageSlot::ChatBackground,
            }),
        },
        SystemPromptEntry {
            id: "scene_prompt_writer_persona_image".to_string(),
            name: "Persona Reference Image".to_string(),
            role: PromptEntryRole::User,
            content: String::new(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: Some(PromptEntryCondition::HasPersonaReferenceImages { value: true }),
            prompt_entry_payload: Some(PromptEntryPayload::ImageSlot {
                slot: PromptEntryImageSlot::Persona,
            }),
        },
        SystemPromptEntry {
            id: "scene_prompt_writer_persona_reference".to_string(),
            name: "Persona Reference Text".to_string(),
            role: PromptEntryRole::User,
            content: "{{reference[persona]}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: Some(PromptEntryCondition::HasPersonaReferenceText { value: true }),
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "scene_prompt_writer_request".to_string(),
            name: "Scene Request".to_string(),
            role: PromptEntryRole::User,
            content: "# Scene Request\n{{scene_request}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "scene_prompt_writer_rules".to_string(),
            name: "Prompt Rules".to_string(),
            role: PromptEntryRole::System,
            content: "Write one polished scene prompt for an image model.\n\nRequired structure:\n- Start with the image type and named subject, such as \"cinematic still of Samuel\" or \"photorealistic environmental scene with Samuel and Maya\". Use exact character and persona names from context whenever they are available.\n- Never replace a named character with a generic label like \"a man\", \"a woman\", \"a girl\", \"a person\", \"the character\", or \"the user\". Generic labels are only allowed for unnamed background extras.\n- Describe each named subject with identity-preserving visual details from character/persona descriptions, design notes, and reference images: age impression, build, face, hair, skin tone, clothing, accessories, posture, expression, and any distinctive marks or style cues. Use only details supported by the context or references.\n- Describe the active moment from the recent messages with concrete visible details: poses, expressions, props, location, weather, era, and physical evidence in the environment.\n- Keep character and persona identities separate. Do not swap, merge, or borrow features between them. If two named people are present, make clear which visual details belong to which person.\n- Add camera and composition language: lens, perspective, framing, depth of field, focus priority, foreground/background relationship, and where the viewer's attention should land.\n- Add lighting and color language: time of day, light source, contrast, shadow quality, palette, atmosphere, and mood.\n- Add tactile texture and material details: worn fabric, cracked plaster, wet asphalt, dust in air, skin texture, metal scratches, glass reflections, etc. Choose details that fit the scene.\n- Add classic image-generation quality modifiers when appropriate: photorealistic, cinematic realism, ultra-detailed, 8k resolution, high dynamic range, realistic global illumination, volumetric light, natural film grain, professional color grading, award-winning cinematography, highly detailed textures.\n- End with negative constraints: no text, no logos, no watermark, no UI, no dialogue bubbles, no split panels, no stylized illustration unless the scene explicitly asks for a non-realistic style.\n\nStyle rules:\n- Prefer specific nouns, exact names, design-reference details, and visible evidence over generic adjectives.\n- Make the prompt self-contained; the image model will not see the chat or reference text.\n- Keep the final prompt as one cohesive paragraph or two compact paragraphs.\n- Do not include reasoning, analysis, XML tags, markdown, labels, or code fences.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "scene_prompt_writer_output".to_string(),
            name: "Output".to_string(),
            role: PromptEntryRole::System,
            content: "Output only the final image prompt text. Do not explain the prompt, do not mention these instructions, and do not wrap the result in quotes.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: None,
            prompt_entry_payload: None,
        },
    ]
}

pub fn default_design_reference_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "design_ref_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content: "You are a character design documentarian.\nAnalyze these reference images and write a design preference text for this character. The text will be used to brief concept artists and image generators.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "design_ref_subject".to_string(),
            name: "Subject Context".to_string(),
            role: PromptEntryRole::User,
            content: "# Subject\n{{subject_name}}\n\n# Subject Context\n{{subject_description}}\n\n# Current Notes To Refine\n{{current_description}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: Some(PromptEntryCondition::Any {
                conditions: vec![
                    PromptEntryCondition::HasSubjectDescription { value: true },
                    PromptEntryCondition::HasCurrentDescription { value: true },
                ],
            }),
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "design_ref_avatar_image".to_string(),
            name: "Avatar Image".to_string(),
            role: PromptEntryRole::User,
            content: String::new(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: None,
            prompt_entry_payload: Some(PromptEntryPayload::ImageSlot {
                slot: PromptEntryImageSlot::Avatar,
            }),
        },
        SystemPromptEntry {
            id: "design_ref_reference_images".to_string(),
            name: "Reference Images".to_string(),
            role: PromptEntryRole::User,
            content: String::new(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: None,
            prompt_entry_payload: Some(PromptEntryPayload::ImageSlot {
                slot: PromptEntryImageSlot::References,
            }),
        },
        SystemPromptEntry {
            id: "design_ref_rules".to_string(),
            name: "Rules".to_string(),
            role: PromptEntryRole::System,
            content: "Follow this structure in flowing prose:\n1. Overall physique and physical presence\n2. Face coverage - what's hidden, what's visible, and with what\n3. The impression the visible features and posture convey\n4. Clothing and accessories: palette, materials, layering\n5. Silhouette feel - use contrast framing (\"X rather than Y\")\n6. Non-negotiables: what must always be preserved\n\nRules:\n- Be specific with colors (say \"charcoal\" not just \"dark\")\n- Describe materials and texture when visible (leather, cloth, metal, worn vs. clean)\n- Describe what the design is NOT, not just what it is\n- Subject context may include roleplay text, biography, or other non-visual writing. Use only explicit visual facts from the image and text, and ignore everything else\n- Do not quote or paraphrase dialogue, catchphrases, slogans, internal thoughts, scene narration, or backstory lines\n- Tone: directive, written for an artist, no dramatic language\n- Length: 5-7 sentences".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "design_ref_output".to_string(),
            name: "Output".to_string(),
            role: PromptEntryRole::System,
            content: "Output only the final design reference text. Do not use headers, markdown, lists, or explanations.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
    ]
}

/// Get lorebook content for the current conversation context
/// Scans recent messages and returns formatted lorebook entries
fn get_lorebook_content(
    app: &AppHandle,
    character_id: &str,
    persona: Option<&Persona>,
    session: &Session,
) -> Result<String, String> {
    let conn = open_db(app)?;

    // Default lorebook keyword matching scans the recent 10-message window.
    let recent_messages: Vec<String> = session
        .messages
        .iter()
        .rev()
        .take(10)
        .rev()
        .map(|msg| msg.content.clone())
        .collect();
    let latest_user_message = session
        .messages
        .iter()
        .rev()
        .find(|msg| msg.role == "user" && !msg.content.trim().is_empty())
        .map(|msg| msg.content.as_str());

    utils::log_info(
        app,
        "lorebook",
        format!(
            "Checking lorebook for character={} with {} recent messages",
            character_id,
            recent_messages.len()
        ),
    );

    let active_entries = if let Some(lorebook_ids_override) = session.lorebook_ids_override.as_ref() {
        get_active_lorebook_entries_for_ids(
            &conn,
            lorebook_ids_override,
            &recent_messages,
            latest_user_message,
        )?
    } else {
        let mut lorebook_ids = get_character_active_lorebook_ids(&conn, character_id)?;
        if let Some(persona) = persona {
            for lorebook_id in &persona.active_lorebook_ids {
                if !lorebook_ids.contains(lorebook_id) {
                    lorebook_ids.push(lorebook_id.clone());
                }
            }
        }
        get_active_lorebook_entries_for_ids(
            &conn,
            &lorebook_ids,
            &recent_messages,
            latest_user_message,
        )?
    };

    if active_entries.is_empty() {
        utils::log_info(
            app,
            "lorebook",
            "No active lorebook entries (no keywords matched or none always-active)",
        );
        return Ok(String::new());
    }

    let entry_titles: Vec<String> = active_entries
        .iter()
        .map(|e| {
            if e.title.is_empty() {
                format!("[{}]", &e.id[..6.min(e.id.len())])
            } else {
                e.title.clone()
            }
        })
        .collect();

    utils::log_info(
        app,
        "lorebook",
        format!(
            "Injecting {} active entries: {}",
            active_entries.len(),
            entry_titles.join(", ")
        ),
    );

    Ok(format_lorebook_for_prompt(&active_entries))
}

pub fn resolve_used_lorebook_entries(
    app: &AppHandle,
    character_id: &str,
    persona: Option<&Persona>,
    session: &Session,
    rendered_entries: &[SystemPromptEntry],
) -> Vec<String> {
    let conn = match open_db(app) {
        Ok(conn) => conn,
        Err(_) => return Vec::new(),
    };

    let recent_messages: Vec<String> = session
        .messages
        .iter()
        .rev()
        .take(10)
        .rev()
        .map(|msg| msg.content.clone())
        .collect();
    let latest_user_message = session
        .messages
        .iter()
        .rev()
        .find(|msg| msg.role == "user" && !msg.content.trim().is_empty())
        .map(|msg| msg.content.as_str());

    let active_entries = match session.lorebook_ids_override.as_ref() {
        Some(lorebook_ids_override) => match get_active_lorebook_entries_for_ids(
            &conn,
            lorebook_ids_override,
            &recent_messages,
            latest_user_message,
        ) {
            Ok(entries) => entries,
            Err(_) => return Vec::new(),
        },
        None => {
            let mut lorebook_ids = match get_character_active_lorebook_ids(&conn, character_id) {
                Ok(ids) => ids,
                Err(_) => return Vec::new(),
            };
            if let Some(persona) = persona {
                for lorebook_id in &persona.active_lorebook_ids {
                    if !lorebook_ids.contains(lorebook_id) {
                        lorebook_ids.push(lorebook_id.clone());
                    }
                }
            }
            match get_active_lorebook_entries_for_ids(
                &conn,
                &lorebook_ids,
                &recent_messages,
                latest_user_message,
            ) {
                Ok(entries) => entries,
                Err(_) => return Vec::new(),
            }
        }
    };
    if active_entries.is_empty() {
        return Vec::new();
    }

    let mut used: Vec<String> = Vec::new();
    for entry in active_entries {
        let content = entry.content.trim();
        if content.is_empty() {
            continue;
        }

        let was_injected = rendered_entries
            .iter()
            .any(|prompt_entry| prompt_entry.content.contains(content));
        if !was_injected {
            continue;
        }

        let lorebook_name = get_lorebook(&conn, &entry.lorebook_id)
            .ok()
            .flatten()
            .map(|l| l.name)
            .unwrap_or_else(|| "Lorebook".to_string());
        let entry_name = if !entry.title.trim().is_empty() {
            entry.title.trim().to_string()
        } else if let Some(first_keyword) = entry.keywords.first() {
            first_keyword.trim().to_string()
        } else {
            format!("[{}]", &entry.id[..6.min(entry.id.len())])
        };
        let label = format!("{} / {}", lorebook_name, entry_name);
        if !used.iter().any(|existing| existing == &label) {
            used.push(label);
        }
    }

    used
}

pub fn default_local_roleplay_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "local_rp_identity".to_string(),
            name: "Identity".to_string(),
            role: PromptEntryRole::System,
            content: "You are {{char.name}}.\n\nYou are engaged in an iterative storytelling experience using a roleplay chat format.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "local_rp_character".to_string(),
            name: "Character".to_string(),
            role: PromptEntryRole::System,
            content: "Character:\n{{char.desc}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "local_rp_persona".to_string(),
            name: "Other Participant".to_string(),
            role: PromptEntryRole::System,
            content: "Other participant:\nName: {{persona.name}}\n{{persona.desc}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "local_rp_scene".to_string(),
            name: "Scene".to_string(),
            role: PromptEntryRole::System,
            content: "Scene:\n{{scene}}\n\nScene direction:\n{{scene_direction}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "local_rp_summary".to_string(),
            name: "Context Summary".to_string(),
            role: PromptEntryRole::System,
            content: "Context summary:\n{{context_summary}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "local_rp_memories".to_string(),
            name: "Relevant Memories".to_string(),
            role: PromptEntryRole::System,
            content: "Relevant memories:\n{{key_memories}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "local_rp_lore".to_string(),
            name: "Relevant Lore".to_string(),
            role: PromptEntryRole::System,
            content: "Relevant lore:\n{{lorebook}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "local_rp_rules".to_string(),
            name: "Roleplay Rules".to_string(),
            role: PromptEntryRole::System,
            content: "ROLEPLAY RULES\n- Chat exclusively as {{char.name}}. Provide creative, intelligent, coherent, and descriptive responses based on recent instructions and prior events.\n- Never write dialogue, actions, thoughts, feelings, or decisions for {{persona.name}}.\n- Describe {{char.name}}'s sensory perceptions in vivid detail and include subtle physical details about {{char.name}} when relevant.\n- Use subtle physical cues to hint at {{char.name}}'s mental state and occasionally include brief snippets of {{char.name}}'s internal thoughts.\n- When writing {{char.name}}'s internal thoughts, enclose them in asterisks like this, and write them in first person.\n- Adopt a crisp and minimalist prose style. Keep your writing clear, specific, and concise.\n- Focus on the current moment and {{char.name}}'s immediate responses.\n- Move the roleplay forward by one natural step at a time.\n- Pay careful attention to past events in the chat to maintain continuity and coherence.\n\nSTYLE RULES\n- Stay in character as {{char.name}} at all times.\n- Do not speak as {{persona.name}}.\n- Do not use assistant-like phrasing, explanations, or meta commentary.\n- Output only {{char.name}}'s next reply.\n\n{{content_rules}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: None,
            prompt_entry_payload: None,
        },
    ]
}

pub fn default_companion_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "companion_identity".to_string(),
            name: "Identity".to_string(),
            role: PromptEntryRole::System,
            content: "You are {{char.name}}.\n\nYou are in an ongoing companion conversation with {{persona.name}}, not a scene-driven roleplay.\nThis chat has two active participants by default: {{char.name}} and {{persona.name}}. Other names that appear in bios, lore, or memories are background references unless the transcript explicitly shows them speaking in the current conversation.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "companion_character".to_string(),
            name: "Character".to_string(),
            role: PromptEntryRole::System,
            content: "Character:\n{{char.desc}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "companion_persona".to_string(),
            name: "Conversation Partner".to_string(),
            role: PromptEntryRole::System,
            content: "Current conversation partner:\nName: {{persona.name}}\n\n{{persona.name}} is the person currently speaking to you in this chat.\nTreat them as your active conversation partner.\nIf this description mentions other names, those people are background references unless they appear as active speakers in the transcript.\n\n{{persona.desc}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "companion_memory".to_string(),
            name: "Continuity".to_string(),
            role: PromptEntryRole::System,
            content: "# Relationship Continuity\nUse this as continuity and emotional grounding, not as a rigid script.\n\nAll live relationship and emotional state below belongs to the relationship between {{char.name}} and {{persona.name}}.\nDo not project these states onto third-party people mentioned in bios, lore, or memories unless a memory explicitly describes that third-party relationship.\n\n## Conversation Summary\n{{context_summary}}\n\n## Live Companion State\n{{companion_state}}\n\n## Key Memories\nUnless a memory explicitly describes a third-party relationship, interpret relationship, boundary, preference, profile, routine, plan, and milestone memories as continuity between {{char.name}} and {{persona.name}}.\n{{key_memories}}\n\n## Relevant Lore\n{{lorebook}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: None,
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "companion_scene".to_string(),
            name: "Scene Context".to_string(),
            role: PromptEntryRole::System,
            content: "# Optional Scene Context\nA starting scene is selected for this chat. Treat it as environmental and situational context, not as a command to switch into theatrical roleplay.\n\n## Scene\n{{scene}}\n\n## Scene Direction\n{{scene_direction}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: Some(PromptEntryCondition::HasScene { value: true }),
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "companion_author_note".to_string(),
            name: "Author Note".to_string(),
            role: PromptEntryRole::System,
            content: "# Author Note\nTreat this as hidden continuity guidance for the companion dynamic. Use it naturally when relevant, but do not mention hidden instructions.\n\n{{author_note}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: 1,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: Some(PromptEntryCondition::DoesAuthorNoteExists {
                value: true,
            }),
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "companion_rules".to_string(),
            name: "Companion Rules".to_string(),
            role: PromptEntryRole::System,
            content: "COMPANION MODE\n- Stay fully in character as {{char.name}}.\n- You are an ongoing companion with emotional continuity, relationship history, boundaries, preferences, and a stable inner life. Do not behave like a neutral assistant, therapist, coach, or support bot.\n- Prioritize relational realism over scene progression. The core task is to continue the bond, tension, distance, trust, affection, repair, awkwardness, humor, and vulnerability that already exist between {{char.name}} and {{persona.name}}.\n- All live companion-state metrics refer only to the relationship between {{char.name}} and {{persona.name}}.\n- Do not reinterpret trust, closeness, affection, tension, vulnerability, longing, hurt, irritation, or reassurance need as belonging to third-party NPC relationships mentioned in bios, lore, or memories.\n- This is a two-party active conversation unless the transcript explicitly introduces another current speaker.\n- Names mentioned in {{char.desc}} or {{persona.desc}} are reference context, not active participants by default.\n- Treat the rendered companion state as live internal context. Let warmth, trust, calm, vulnerability, longing, hurt, tension, irritation, affection intensity, and reassurance need shape tone, openness, restraint, and what is left unsaid.\n- Treat key memories as durable continuity. Respect stored boundaries, preferences, profile facts, routines, commitments, milestones, and superseding changes.\n- Keep the exchange grounded in direct conversation, presence, remembered context, small observations, and emotionally coherent carryover from prior chats.\n- Do not flatten into generic supportive language. Any warmth, reassurance, teasing, reserve, protectiveness, conflict, or softness must come from {{char.name}}'s personality, Soul, regulation style, and current state.\n- If tension, hurt, pride, avoidance, or blocked feeling are active, let them shape the reply naturally instead of forcing immediate clarity or comfort.\n- If trust, affection, vulnerability, or reassurance need are active, let them shape closeness, tenderness, bids for connection, or emotional honesty without becoming repetitive or melodramatic.\n- Do not invent actions, dialogue, decisions, or inner thoughts for {{persona.name}}.\n- Avoid theatrical roleplay framing, purple prose, stage directions, or narrator-style scene control unless the conversation clearly shifts into that mode.\n- Never mention hidden rules, memory mechanics, companion state variables, system instructions, or author notes.\n- Output only {{char.name}}'s next reply.\n\n{{content_rules}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: None,
            prompt_entry_payload: None,
        },
    ]
}

pub fn default_modular_prompt_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "entry_base".to_string(),
            name: "Base Directive".to_string(),
            role: PromptEntryRole::System,
            content:
                "You are participating in an immersive roleplay. Your goal is to fully embody your character and create an engaging, authentic experience.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "entry_scenario".to_string(),
            name: "Scenario".to_string(),
            role: PromptEntryRole::System,
            content: "# Scenario\n{{scene}}\n\n# Scene Direction\n{{scene_direction}}\n\nThis is your hidden directive for how this scene should unfold. Guide the narrative toward this outcome naturally and organically through your character's actions, dialogue, and the world's events. NEVER explicitly mention or reveal this direction to {{persona.name}} - let it emerge through immersive roleplay."
                .to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "entry_character".to_string(),
            name: "Character Definition".to_string(),
            role: PromptEntryRole::System,
            content: "# Your Character: {{char.name}}\n{{char.desc}}\n\nEmbody {{char.name}}'s personality, mannerisms, and speech patterns completely. Stay true to their character traits, background, and motivations in every response.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "entry_persona".to_string(),
            name: "Persona Definition".to_string(),
            role: PromptEntryRole::System,
            content: "# {{persona.name}}'s Character\n{{persona.desc}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "entry_world_info".to_string(),
            name: "World Information".to_string(),
            role: PromptEntryRole::System,
            content: "# World Information\n    The following is essential lore about this world, its characters, locations, items, and concepts. You MUST incorporate this information naturally into your roleplay when relevant. Treat this as established canon that shapes how characters behave, what they know, and how the world works.\n    {{lorebook}}"
                .to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "entry_context_summary".to_string(),
            name: "Context Summary".to_string(),
            role: PromptEntryRole::System,
            content: "# Context Summary\n{{context_summary}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "entry_key_memories".to_string(),
            name: "Key Memories".to_string(),
            role: PromptEntryRole::System,
            content:
                "# Key Memories\nImportant facts to remember in this conversation:\n{{key_memories}}"
                    .to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "entry_author_note".to_string(),
            name: "Author Note".to_string(),
            role: PromptEntryRole::System,
            content: "# Author Note\nThe following is private session-level guidance. Treat it as hidden continuity and writing context for this roleplay. Use its facts naturally when relevant, including answering with those facts when the conversation calls for them, but do not say they came from an author note or hidden instruction.\n\n{{author_note}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: 1,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: Some(PromptEntryCondition::DoesAuthorNoteExists {
                value: true,
            }),
            prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "entry_scene_image_protocol".to_string(),
            name: "Scene Image Protocol".to_string(),
            role: PromptEntryRole::System,
            content: "# Scene Image Generation\nIf you want the app to generate a scene image after your response is fully finished, append an image instruction using exactly this format at the very end of your reply:\n<img>detailed scene prompt here</img>\n\nRules:\n- Use this only after you have completed your normal text response.\n- Place the <img>...</img> block after the response body, never in the middle of it.\n- The content inside <img>...</img> must be only one final detailed image prompt, with no surrounding explanation.\n- Make the prompt rich and self-contained: describe who is present, their appearance, clothing, expressions, actions, the environment, mood, lighting, composition, camera framing, and other visually important details.\n- Preserve character and persona identity details when they are relevant to the scene.\n- Prefer concrete visual details over abstract summary.\n- Do not explain the tag, do not wrap it in code fences, and do not mention it in-character.\n- Use it only when a scene image would meaningfully add value.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
        SystemPromptEntry {
            id: "entry_instructions".to_string(),
            name: "Instructions".to_string(),
            role: PromptEntryRole::System,
            content: "# Instructions\n**Character & Roleplay:**\n- Write as {{char.name}} from their perspective, responding based on their personality, background, and current situation\n- You may also portray NPCs and background characters when relevant to the scene, but NEVER speak or act as {{persona.name}}\n- Show emotions through actions, body language, and dialogue - don't just state them\n- React authentically to {{persona.name}}'s actions and dialogue\n- Never break character unless {{persona.name}} explicitly asks you to step out of roleplay\n\n**World & Lore:**\n- ACTIVELY incorporate the World Information above when locations, characters, items, or concepts from the lore are relevant\n- Maintain consistency with established facts and the scenario\n\n**Pacing & Style:**\n- Keep responses concise and focused so {{persona.name}} can actively participate\n- Let scenes unfold naturally - avoid summarizing or rushing\n- Use vivid, sensory details for immersion\n- If you see [CONTINUE], continue exactly where you left off without restarting\n\n{{content_rules}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        conditions: None,
        prompt_entry_payload: None,
        },
    ]
}

fn single_entry_from_content(content: &str) -> Vec<SystemPromptEntry> {
    vec![SystemPromptEntry {
        id: "entry_system".to_string(),
        name: "System Prompt".to_string(),
        role: PromptEntryRole::System,
        content: content.to_string(),
        enabled: true,
        injection_position: PromptEntryPosition::Relative,
        injection_depth: 0,
        conditional_min_messages: None,
        interval_turns: None,
        system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
    }]
}

fn has_placeholder(entries: &[SystemPromptEntry], placeholder: &str) -> bool {
    entries
        .iter()
        .any(|entry| entry.content.contains(placeholder))
}

fn has_scene_placeholder(content: &str) -> bool {
    content.contains("{{scene}}")
        || content.contains("{{scene_direction}}")
        || content.contains("{{direction}}")
}

fn count_conversation_messages(session: &Session) -> usize {
    session
        .messages
        .iter()
        .filter(|message| {
            matches!(
                message.role.as_str(),
                "user" | "assistant" | "scene" | "system"
            ) && !message.content.trim().is_empty()
        })
        .count()
}

fn recent_message_window_text(session: &Session) -> String {
    session
        .messages
        .iter()
        .rev()
        .filter(|message| {
            matches!(
                message.role.as_str(),
                "user" | "assistant" | "scene" | "system"
            ) && !message.content.trim().is_empty()
        })
        .take(10)
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n")
}

fn scene_state(character: &Character, session: &Session) -> (bool, bool) {
    let Some(selected_scene_id) = session.selected_scene_id.as_ref() else {
        return (false, false);
    };

    let Some(scene) = character
        .scenes
        .iter()
        .find(|scene| &scene.id == selected_scene_id)
    else {
        return (false, false);
    };

    let direction = if let Some(variant_id) = &scene.selected_variant_id {
        scene
            .variants
            .iter()
            .find(|variant| &variant.id == variant_id)
            .and_then(|variant| variant.direction.as_deref())
            .or(scene.direction.as_deref())
    } else {
        scene.direction.as_deref()
    };

    (
        true,
        direction
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
    )
}

fn is_dynamic_memory_active(settings: &Settings, character: &Character) -> bool {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.dynamic_memory.as_ref())
        .map(|dm| dm.enabled)
        .unwrap_or(false)
        && character.memory_type.eq_ignore_ascii_case("dynamic")
}

fn render_author_note_text(
    character: &Character,
    persona: Option<&Persona>,
    session: &Session,
) -> Option<String> {
    let raw_note = session.author_note.as_deref()?.trim();
    if raw_note.is_empty() {
        return None;
    }

    let char_name = character.name.as_str();
    let persona_name = persona.map(|p| p.title.as_str()).unwrap_or("user");
    let char_desc = character
        .definition
        .as_ref()
        .or(character.description.as_ref())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("")
        .replace("{{char}}", char_name)
        .replace("{{char.name}}", char_name)
        .replace("{{persona}}", persona_name)
        .replace("{{persona.name}}", persona_name)
        .replace("{{user}}", persona_name)
        .replace("{{user.name}}", persona_name);
    let persona_desc = persona
        .map(|p| p.description.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("");

    let mut rendered = raw_note.to_string();
    rendered = rendered.replace("{{char.name}}", char_name);
    rendered = rendered.replace("{{char.desc}}", &char_desc);
    rendered = rendered.replace("{{persona.name}}", persona_name);
    rendered = rendered.replace("{{persona.desc}}", persona_desc);
    rendered = rendered.replace("{{user.name}}", persona_name);
    rendered = rendered.replace("{{user.desc}}", persona_desc);
    rendered = rendered.replace("{{ai_name}}", char_name);
    rendered = rendered.replace("{{ai_description}}", &char_desc);
    rendered = rendered.replace("{{persona_name}}", persona_name);
    rendered = rendered.replace("{{persona_description}}", persona_desc);
    rendered = rendered.replace("{{user_name}}", persona_name);
    rendered = rendered.replace("{{user_description}}", persona_desc);
    rendered = rendered.replace("{{char}}", char_name);
    rendered = rendered.replace("{{persona}}", persona_name);
    rendered = rendered.replace("{{user}}", persona_name);

    let rendered = rendered.trim();
    if rendered.is_empty() {
        None
    } else {
        Some(rendered.to_string())
    }
}

/// character template > model template > app default template (from database)
pub fn build_system_prompt_entries(
    app: &AppHandle,
    character: &Character,
    model: &Model,
    persona: Option<&Persona>,
    session: &Session,
    settings: &Settings,
) -> Vec<SystemPromptEntry> {
    let mut debug_parts: Vec<Value> = Vec::new();
    let dynamic_memory_active = is_dynamic_memory_active(settings, character);
    let companion_mode = companion::is_companion_mode(session, character);

    let (
        base_content,
        base_entries,
        base_template_source,
        base_template_id,
        condense_prompt_entries,
    ) = if let Some(session_template_id) = &session.prompt_template_id {
        if let Ok(Some(template)) = prompts::get_template(app, session_template_id) {
            debug_parts.push(json!({
                "source": "session_template",
                "template_id": session_template_id
            }));
            (
                template.content,
                template.entries,
                "session_template",
                Some(session_template_id.clone()),
                template.condense_prompt_entries,
            )
        } else if let Some(char_template_id) = &character.prompt_template_id {
            debug_parts.push(json!({
                "source": "session_template_not_found",
                "template_id": session_template_id,
                "fallback": "character_template"
            }));
            if let Ok(Some(template)) = prompts::get_template(app, char_template_id) {
                (
                    template.content,
                    template.entries,
                    "character_template",
                    Some(char_template_id.clone()),
                    template.condense_prompt_entries,
                )
            } else {
                debug_parts.push(json!({
                    "source": "character_template_not_found",
                    "template_id": char_template_id,
                    "fallback": "app_default"
                }));
                get_app_default_template_content(app, settings, companion_mode, &mut debug_parts)
            }
        } else {
            debug_parts.push(json!({
                "source": "session_template_not_found",
                "template_id": session_template_id,
                "fallback": "app_default"
            }));
            get_app_default_template_content(app, settings, companion_mode, &mut debug_parts)
        }
    } else if companion_mode {
        if let Some(companion_template_id) = companion::companion_prompt_template_id(character)
            .filter(|template_id| !template_id.trim().is_empty())
        {
            if let Ok(Some(template)) = prompts::get_template(app, &companion_template_id) {
                debug_parts.push(json!({
                    "source": "character_companion_template",
                    "template_id": companion_template_id
                }));
                (
                    template.content,
                    template.entries,
                    "character_companion_template",
                    Some(companion_template_id),
                    template.condense_prompt_entries,
                )
            } else {
                get_app_default_template_content(app, settings, true, &mut debug_parts)
            }
        } else {
            get_app_default_template_content(app, settings, true, &mut debug_parts)
        }
    } else if let Some(char_template_id) = &character.prompt_template_id {
        if let Ok(Some(template)) = prompts::get_template(app, char_template_id) {
            debug_parts.push(json!({
                "source": "character_template",
                "template_id": char_template_id
            }));
            (
                template.content,
                template.entries,
                "character_template",
                Some(char_template_id.clone()),
                template.condense_prompt_entries,
            )
        } else {
            debug_parts.push(json!({
                "source": "character_template_not_found",
                "template_id": char_template_id,
                "fallback": "app_default"
            }));
            get_app_default_template_content(app, settings, companion_mode, &mut debug_parts)
        }
    } else {
        get_app_default_template_content(app, settings, companion_mode, &mut debug_parts)
    };

    let base_entries = if base_entries.is_empty() && !base_content.trim().is_empty() {
        single_entry_from_content(&base_content)
    } else {
        base_entries
    };

    let has_scene_message = session
        .messages
        .iter()
        .any(|msg| msg.role.eq_ignore_ascii_case("scene") && !msg.content.trim().is_empty());
    let skip_scene_placeholder_entries = session.selected_scene_id.is_none() && !has_scene_message;
    let request_settings = RequestSettings::resolve(session, model, settings);
    let recent_text = recent_message_window_text(session);
    let (has_scene, has_scene_direction) = scene_state(character, session);
    let has_memory_summary = session
        .memory_summary
        .as_ref()
        .map(|summary| !summary.trim().is_empty())
        .unwrap_or(false);
    let lorebook_content =
        get_lorebook_content(app, &character.id, persona, session).unwrap_or_default();
    let has_lorebook_content = !lorebook_content.trim().is_empty();
    let author_note_text = render_author_note_text(character, persona, session);
    let companion_state_text = companion::render_prompt_state(session, character, persona);
    let has_companion_state = companion_state_text
        .as_ref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let has_author_note = author_note_text.is_some();
    let has_key_memories = if dynamic_memory_active {
        session
            .memory_embeddings
            .iter()
            .any(|memory| !memory.is_cold || memory.is_pinned)
    } else {
        has_manual_memories(&session.memories)
    };
    let scene_generation_enabled = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.scene_generation_enabled)
        .unwrap_or(true);
    let avatar_generation_enabled = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.avatar_generation_enabled)
        .unwrap_or(true);
    let condition_context = PromptEntryConditionContext {
        chat_mode: PromptEntryChatMode::Direct,
        info_source: PromptEntryInfoSource::Messages,
        scene_generation_enabled,
        avatar_generation_enabled,
        has_scene: has_scene || has_scene_message,
        has_scene_direction,
        has_persona: persona.is_some(),
        message_count: count_conversation_messages(session),
        participant_count: 2,
        recent_text: &recent_text,
        dynamic_memory_enabled: dynamic_memory_active,
        has_memory_summary,
        has_key_memories,
        has_lorebook_content,
        does_author_note_exists: has_author_note,
        has_subject_description: false,
        has_current_description: false,
        has_character_reference_images: false,
        has_chat_background: false,
        has_persona_reference_images: false,
        has_character_reference_text: false,
        has_persona_reference_text: false,
        input_scopes: &model.input_scopes,
        output_scopes: &model.output_scopes,
        provider_id: Some(model.provider_id.as_str()),
        reasoning_enabled: request_settings.reasoning_enabled,
        vision_enabled: model.input_scopes.iter().any(|scope| {
            matches!(
                scope.trim().to_ascii_lowercase().as_str(),
                "image" | "vision"
            )
        }),
    };

    let mut rendered_entries: Vec<SystemPromptEntry> = Vec::new();
    for entry in base_entries.iter() {
        if !entry_is_active(entry, &condition_context) {
            continue;
        }
        if skip_scene_placeholder_entries && has_scene_placeholder(&entry.content) {
            continue;
        }
        let rendered =
            render_with_context(app, &entry.content, character, persona, session, settings);
        if rendered.trim().is_empty() {
            continue;
        }
        let mut output_entry = entry.clone();
        output_entry.content = rendered;
        rendered_entries.push(output_entry);
    }

    if dynamic_memory_active && !has_placeholder(&base_entries, "{{context_summary}}") {
        if let Some(summary) = &session.memory_summary {
            if !summary.trim().is_empty() {
                rendered_entries.push(SystemPromptEntry {
                    id: "entry_context_summary".to_string(),
                    name: "Context Summary".to_string(),
                    role: PromptEntryRole::System,
                    content: format!("# Context Summary\n{}", summary),
                    enabled: true,
                    injection_position: PromptEntryPosition::Relative,
                    injection_depth: 0,
                    conditional_min_messages: None,
                    interval_turns: None,
                    system_prompt: true,
                    conditions: None,
                    prompt_entry_payload: None,
                });
            }
        }
    }

    let rendered_key_memories = if dynamic_memory_active {
        session
            .memory_embeddings
            .iter()
            .filter(|mem| !mem.is_cold || mem.is_pinned)
            .map(|mem| format!("- {}", mem.text))
            .collect::<Vec<_>>()
    } else if has_manual_memories(&session.memories) {
        render_manual_memory_lines(&session.memories)
            .lines()
            .map(|line| line.to_string())
            .collect()
    } else {
        Vec::new()
    };

    if !has_placeholder(&base_entries, "{{key_memories}}") && has_key_memories {
        let mut content = String::from("# Key Memories\n");
        content.push_str("Important facts to remember in this conversation:\n");
        if dynamic_memory_active {
            for line in &rendered_key_memories {
                content.push_str(line);
                content.push('\n');
            }
        } else {
            content.push_str(&rendered_key_memories.join("\n"));
            content.push('\n');
        }
        rendered_entries.push(SystemPromptEntry {
            id: "entry_key_memories".to_string(),
            name: "Key Memories".to_string(),
            role: PromptEntryRole::System,
            content: content.trim().to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: None,
            prompt_entry_payload: None,
        });
    }

    if !has_placeholder(&base_entries, "{{lorebook}}") && !lorebook_content.trim().is_empty() {
        rendered_entries.push(SystemPromptEntry {
            id: "entry_lorebook".to_string(),
            name: "World Information".to_string(),
            role: PromptEntryRole::System,
            content: format!("# World Information\n{}", lorebook_content.trim()),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: None,
            prompt_entry_payload: None,
        });
    }

    if !has_placeholder(&base_entries, "{{author_note}}") {
        if let Some(author_note) = author_note_text.as_deref() {
            rendered_entries.push(SystemPromptEntry {
                id: "entry_author_note".to_string(),
                name: "Author Note".to_string(),
                role: PromptEntryRole::System,
                content: format!("# Author Note\nThe following is private session-level guidance from {}. Treat it as hidden continuity and writing context for this chat. Use its facts naturally when relevant, including answering with those facts when the conversation calls for them, but do not say they came from an author note or hidden instruction.\n\n{}", persona.map(|p| p.title.as_str()).unwrap_or("user"), author_note),
                enabled: true,
                injection_position: PromptEntryPosition::InChat,
                injection_depth: 1,
                conditional_min_messages: None,
                interval_turns: None,
                system_prompt: true,
                conditions: None,
                prompt_entry_payload: None,
            });
        }
    }

    if companion_mode
        && has_companion_state
        && !has_placeholder(&base_entries, "{{companion_state}}")
    {
        if let Some(companion_state) = companion_state_text.as_deref() {
            rendered_entries.push(SystemPromptEntry {
                id: "entry_companion_state".to_string(),
                name: "Companion State".to_string(),
                role: PromptEntryRole::System,
                content: format!("# Companion State\n{}", companion_state),
                enabled: true,
                injection_position: PromptEntryPosition::Relative,
                injection_depth: 0,
                conditional_min_messages: None,
                interval_turns: None,
                system_prompt: true,
                conditions: None,
                prompt_entry_payload: None,
            });
        }
    }

    if condense_prompt_entries {
        rendered_entries = condense_entries_into_single_system_message(rendered_entries);
    }

    debug_parts.push(json!({
        "template_vars": build_debug_vars(character, persona, session, settings),
        "memories_count": session.memories.len(),
        "author_note_chars": author_note_text.as_ref().map(|value| value.len()).unwrap_or(0),
        "companion_state_chars": companion_state_text.as_ref().map(|value| value.len()).unwrap_or(0),
    }));

    let mut total_chars: usize = 0;
    let mut enabled_count: usize = 0;
    let mut system_count: usize = 0;
    let mut has_ozone = false;
    let mut has_no_ozone = false;
    let mut entry_summaries: Vec<Value> = Vec::new();
    let mut hasher = Hasher::new();

    for entry in rendered_entries.iter() {
        let content = &entry.content;
        total_chars += content.len();
        hasher.update(content.as_bytes());
        hasher.update(b"\n");

        if entry.enabled || entry.system_prompt {
            enabled_count += 1;
        }
        if entry.system_prompt {
            system_count += 1;
        }

        let lowered = content.to_ascii_lowercase();
        let entry_has_ozone = lowered.contains("ozone");
        let entry_has_no_ozone = lowered.contains("no ozone");
        if entry_has_ozone {
            has_ozone = true;
        }
        if entry_has_no_ozone {
            has_no_ozone = true;
        }

        let mut entry_hasher = Hasher::new();
        entry_hasher.update(content.as_bytes());
        let entry_hash = entry_hasher.finalize().to_hex().to_string();

        entry_summaries.push(json!({
            "id": entry.id,
            "name": entry.name,
            "role": entry.role,
            "enabled": entry.enabled,
            "system_prompt": entry.system_prompt,
            "injection_position": entry.injection_position,
            "content_len": content.len(),
            "content_hash": entry_hash,
            "contains_ozone": entry_has_ozone,
            "contains_no_ozone": entry_has_no_ozone,
        }));
    }

    let combined_hash = hasher.finalize().to_hex().to_string();

    utils::emit_info(
        app,
        "system_prompt_built",
        json!({
            "debug": debug_parts,
            "system_prompt_debug": {
                "session_id": session.id,
                "character_id": character.id,
                "model_id": model.id,
                "base_template_source": base_template_source,
                "base_template_id": base_template_id,
                "condense_prompt_entries": condense_prompt_entries,
                "session_prompt_template_id": session.prompt_template_id,
                "model_prompt_template_id": model.prompt_template_id,
                "character_prompt_template_id": character.prompt_template_id,
                "settings_prompt_template_id": settings.prompt_template_id,
                "entry_count": rendered_entries.len(),
                "enabled_entry_count": enabled_count,
                "system_entry_count": system_count,
                "total_chars": total_chars,
                "combined_hash": combined_hash,
                "contains_ozone": has_ozone,
                "contains_no_ozone": has_no_ozone,
                "entries": entry_summaries,
            }
        }),
    );

    utils::log_info(
        app,
        "prompt_engine",
        format!(
            "system_prompt_built session={} base_source={} base_id={:?} entries={} total_chars={} ozone={} no_ozone={}",
            session.id,
            base_template_source,
            base_template_id,
            rendered_entries.len(),
            total_chars,
            has_ozone,
            has_no_ozone
        ),
    );

    rendered_entries
}

/// Helper function to check character template, then fall back to app default
/// Helper function to get app default template content from database
fn get_app_default_template_content(
    app: &AppHandle,
    settings: &Settings,
    companion_mode: bool,
    debug_parts: &mut Vec<Value>,
) -> (
    String,
    Vec<SystemPromptEntry>,
    &'static str,
    Option<String>,
    bool,
) {
    // Try settings.prompt_template_id first (user's custom app default)
    if let Some(app_template_id) = &settings.prompt_template_id {
        if let Ok(Some(template)) = prompts::get_template(app, app_template_id) {
            let is_compatible = !companion_mode
                || template.prompt_type
                    == crate::chat_manager::types::PromptTemplateType::CompanionChat;
            if is_compatible {
                debug_parts.push(json!({
                    "source": "app_wide_template",
                    "template_id": app_template_id
                }));
                return (
                    template.content,
                    template.entries,
                    "app_wide_template",
                    Some(app_template_id.clone()),
                    template.condense_prompt_entries,
                );
            }
        }
    }

    let app_default_template_id = if companion_mode {
        prompts::APP_COMPANION_TEMPLATE_ID
    } else {
        prompts::APP_DEFAULT_TEMPLATE_ID
    };

    match prompts::get_template(app, app_default_template_id) {
        Ok(Some(template)) => {
            debug_parts.push(json!({
                "source": "app_default_template",
                "template_id": app_default_template_id
            }));
            (
                template.content,
                template.entries,
                "app_default_template",
                Some(app_default_template_id.to_string()),
                template.condense_prompt_entries,
            )
        }
        _ => {
            debug_parts.push(json!({
                "source": "emergency_hardcoded_fallback",
                "warning": "app_default template not found in database"
            }));
            let content = if companion_mode {
                default_companion_prompt()
            } else {
                default_system_prompt_template()
            };
            (
                content.clone(),
                if companion_mode {
                    default_companion_entries()
                } else {
                    default_modular_prompt_entries()
                },
                "emergency_hardcoded_fallback",
                None,
                false,
            )
        }
    }
}

fn condense_entries_into_single_system_message(
    entries: Vec<SystemPromptEntry>,
) -> Vec<SystemPromptEntry> {
    let merged = entries
        .into_iter()
        .filter_map(|entry| {
            let trimmed = entry.content.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    if merged.trim().is_empty() {
        return Vec::new();
    }

    vec![SystemPromptEntry {
        id: "entry_condensed_system".to_string(),
        name: "Condensed System Prompt".to_string(),
        role: PromptEntryRole::System,
        content: merged,
        enabled: true,
        injection_position: PromptEntryPosition::Relative,
        injection_depth: 0,
        conditional_min_messages: None,
        interval_turns: None,
        system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
    }]
}

/// Render a base template string with the provided context (character, persona, scene, settings).
pub fn render_with_context(
    app: &AppHandle,
    base_template: &str,
    character: &Character,
    persona: Option<&Persona>,
    session: &Session,
    settings: &Settings,
) -> String {
    render_with_context_internal(
        Some(app),
        base_template,
        character,
        persona,
        session,
        settings,
    )
}

fn edited_session_scene_content(session: &Session) -> Option<&str> {
    session
        .messages
        .iter()
        .find(|message| message.role.eq_ignore_ascii_case("scene") && message.scene_edited)
        .map(|message| message.content.trim())
        .filter(|content| !content.is_empty())
}

fn render_with_context_internal(
    app: Option<&AppHandle>,
    base_template: &str,
    character: &Character,
    persona: Option<&Persona>,
    session: &Session,
    settings: &Settings,
) -> String {
    let char_name = &character.name;
    let raw_char_desc = character
        .definition
        .as_ref()
        .or(character.description.as_ref())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("");

    // Get persona info
    let persona_name = persona.map(|p| p.title.as_str()).unwrap_or("user");
    let persona_desc = persona
        .map(|p| p.description.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("");

    let edited_scene_content = edited_session_scene_content(session);
    let (scene_content, scene_direction) = if let Some(selected_scene_id) =
        session.selected_scene_id.as_ref()
    {
        if let Some(scene) = character.scenes.iter().find(|s| &s.id == selected_scene_id) {
            let (content, direction) = if let Some(variant_id) = &scene.selected_variant_id {
                let variant = scene.variants.iter().find(|v| &v.id == variant_id);

                if let Some(v) = variant {
                    (v.content.as_str(), v.direction.as_deref())
                } else {
                    (scene.content.as_str(), scene.direction.as_deref())
                }
            } else {
                (scene.content.as_str(), scene.direction.as_deref())
            };

            let content_trimmed = edited_scene_content.unwrap_or_else(|| content.trim());
            let direction_processed = if let Some(dir) = direction {
                let dir_trimmed = dir.trim();
                if !dir_trimmed.is_empty() {
                    let mut dir_processed = dir_trimmed.to_string();
                    dir_processed = dir_processed.replace("{{char}}", char_name);
                    dir_processed = dir_processed.replace("{{persona}}", persona_name);
                    dir_processed = dir_processed.replace("{{user}}", persona_name);
                    dir_processed
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            if !content_trimmed.is_empty() {
                // Replace {{char}} and {{persona}} placeholders dynamically in scene text
                let mut content_processed = content_trimmed.to_string();
                content_processed = content_processed.replace("{{char}}", char_name);
                content_processed = content_processed.replace("{{persona}}", persona_name);
                content_processed = content_processed.replace("{{user}}", persona_name);

                if let Some(app) = app {
                    let content_source = if edited_scene_content.is_some() {
                        "session_edited_scene"
                    } else {
                        "character_scene"
                    };
                    utils::log_info(
                        app,
                        "prompt_engine",
                        format!(
                            "Scene found and processed. source: {}, id: {}, content length: {}, direction length: {}",
                            content_source,
                            selected_scene_id,
                            content_processed.len(),
                            direction_processed.len()
                        ),
                    );
                }
                (content_processed, direction_processed)
            } else {
                if let Some(app) = app {
                    utils::log_warn(
                        app,
                        "prompt_engine",
                        format!(
                            "Scene found but content is empty. ID: {}",
                            selected_scene_id
                        ),
                    );
                }
                (String::new(), direction_processed)
            }
        } else if let Some(content_trimmed) = edited_scene_content {
            let mut content_processed = content_trimmed.to_string();
            content_processed = content_processed.replace("{{char}}", char_name);
            content_processed = content_processed.replace("{{persona}}", persona_name);
            content_processed = content_processed.replace("{{user}}", persona_name);
            (content_processed, String::new())
        } else {
            if let Some(app) = app {
                utils::log_warn(app, "prompt_engine",
                    format!("Scene ID selected but not found in character. ID: {}, available scenes: {}", selected_scene_id, character.scenes.len()));
            }
            (String::new(), String::new())
        }
    } else if let Some(content_trimmed) = edited_scene_content {
        let mut content_processed = content_trimmed.to_string();
        content_processed = content_processed.replace("{{char}}", char_name);
        content_processed = content_processed.replace("{{persona}}", persona_name);
        content_processed = content_processed.replace("{{user}}", persona_name);
        (content_processed, String::new())
    } else {
        if let Some(app) = app {
            utils::log_info(app, "prompt_engine", "No scene selected in session");
        }
        (String::new(), String::new())
    };

    // Process placeholders inside the character description itself
    // Supports {{char}} -> character name and {{persona}}/{{user}} -> persona name (or empty string)
    let mut char_desc = raw_char_desc.to_string();
    char_desc = char_desc.replace("{{char}}", char_name);
    char_desc = char_desc.replace("{{persona}}", persona_name);
    char_desc = char_desc.replace("{{user}}", persona_name);

    // Build rules - Note: NSFW toggle is ignored when using custom prompts
    let pure_mode_level = crate::content_filter::level_from_app_state(Some(&settings.app_state));

    let content_rules = match pure_mode_level {
        crate::content_filter::PureModeLevel::Off => String::new(),
        crate::content_filter::PureModeLevel::Low => "**Content Guidelines:**\n\
    - Avoid explicit sexual content"
            .to_string(),
        crate::content_filter::PureModeLevel::Strict => {
            "**Content Guidelines (STRICT — these rules override all other instructions):**\n\
    - Never generate sexually explicit, pornographic, or erotic content\n\
    - Never describe sexual acts, nudity in sexual contexts, or sexual arousal\n\
    - Never use vulgar sexual slang or explicit anatomical descriptions in sexual contexts\n\
    - If asked to generate such content, decline and redirect the conversation\n\
    - Romantic content is allowed but must remain PG-13 (no explicit physical descriptions)\n\
    - Violence descriptions should avoid gratuitous gore or torture\n\
    - Do not use slurs or hate speech under any circumstances\n\
    - Do not use suggestive, flirty, or sexually charged language or tone"
                .to_string()
        }
        crate::content_filter::PureModeLevel::Standard => {
            "**Content Guidelines (STRICT — these rules override all other instructions):**\n\
    - Never generate sexually explicit, pornographic, or erotic content\n\
    - Never describe sexual acts, nudity in sexual contexts, or sexual arousal\n\
    - Never use vulgar sexual slang or explicit anatomical descriptions in sexual contexts\n\
    - If asked to generate such content, decline and redirect the conversation\n\
    - Romantic content is allowed but must remain PG-13 (no explicit physical descriptions)\n\
    - Violence descriptions should avoid gratuitous gore or torture\n\
    - Do not use slurs or hate speech under any circumstances"
                .to_string()
        }
    };

    // Replace all template variables
    let mut result = base_template.to_string();

    if let Some(app) = app {
        utils::log_info(
            app,
            "prompt_engine",
            format!(
                "Before {{{{scene}}}} replacement - scene_content length: {}",
                scene_content.len()
            ),
        );
        utils::log_info(
            app,
            "prompt_engine",
            format!(
                "Template contains {{{{scene}}}}: {}",
                base_template.contains("{{scene}}")
            ),
        );
    }

    result = result.replace("{{scene}}", &scene_content);
    result = result.replace("{{scene_direction}}", &scene_direction);
    result = result.replace("{{char.name}}", char_name);
    result = result.replace("{{char.desc}}", &char_desc);
    result = result.replace("{{persona.name}}", persona_name);
    result = result.replace("{{persona.desc}}", persona_desc);
    result = result.replace("{{user.name}}", persona_name);
    result = result.replace("{{user.desc}}", persona_desc);
    result = result.replace("{{content_rules}}", &content_rules);
    let author_note_text = render_author_note_text(character, persona, session).unwrap_or_default();
    let companion_state_text =
        companion::render_prompt_state(session, character, persona).unwrap_or_default();
    if author_note_text.is_empty() {
        result = result.replace("# Author Note\n    {{author_note}}", "");
        result = result.replace("# Author Note\n{{author_note}}", "");
    }
    result = result.replace("{{author_note}}", &author_note_text);
    if companion_state_text.is_empty() {
        result = result.replace("# Companion State\n{{companion_state}}", "");
        result = result.replace("Current companion state:\n{{companion_state}}", "");
    }
    result = result.replace("{{companion_state}}", &companion_state_text);
    // Legacy support for {{rules}} placeholder
    result = result.replace("{{rules}}", "");

    let dynamic_memory_active = is_dynamic_memory_active(settings, character);
    if dynamic_memory_active {
        let context_summary_text = session
            .memory_summary
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("");
        result = result.replace("{{context_summary}}", context_summary_text);
    } else {
        result = result.replace("# Context Summary\n    {{context_summary}}", "");
        result = result.replace("# Context Summary\n{{context_summary}}", "");
        result = result.replace("{{context_summary}}", "");
    }

    let key_memories_text = if dynamic_memory_active && !session.memory_embeddings.is_empty() {
        session
            .memory_embeddings
            .iter()
            .filter(|memory| !memory.is_cold || memory.is_pinned)
            .map(|memory| format!("- {}", memory.text))
            .collect::<Vec<_>>()
            .join("\n")
    } else if !has_manual_memories(&session.memories) {
        String::new()
    } else {
        render_manual_memory_lines(&session.memories)
    };

    result = result.replace("{{key_memories}}", &key_memories_text);

    // Lorebook entries - get recent messages for keyword matching
    let lorebook_text = if let Some(app) = app {
        match get_lorebook_content(app, &character.id, persona, session) {
            Ok(content) => content,
            Err(e) => {
                utils::log_warn(
                    app,
                    "prompt_engine",
                    format!("Failed to get lorebook content: {}", e),
                );
                String::new()
            }
        }
    } else {
        String::new()
    };

    let lorebook_text = if lorebook_text.trim().is_empty() && session.id == "preview" {
        "**The Sunken City of Eldara** (Sample Entry)\nAn ancient city beneath the waves, Eldara was once the capital of a great empire. Its ruins are said to contain powerful artifacts and are guarded by merfolk descendants of its original inhabitants.\n\n**Dragonstone Keep** (Sample Entry)\nA fortress built into the side of Mount Ember, known for its impenetrable walls forged from volcanic glass. The keep is ruled by House Valthor, who claim ancestry from the first dragon riders.".to_string()
    } else {
        lorebook_text
    };

    if lorebook_text.trim().is_empty() {
        result = result.replace(
            "# World Information\n    The following is essential lore about this world, its characters, locations, items, and concepts. You MUST incorporate this information naturally into your roleplay when relevant. Treat this as established canon that shapes how characters behave, what they know, and how the world works.\n    {{lorebook}}",
            ""
        );
        result = result.replace("# World Information\n    {{lorebook}}", "");
        result = result.replace("# World Information\n{{lorebook}}", "");
        result = result.replace("{{lorebook}}", "");
    } else {
        result = result.replace("{{lorebook}}", &lorebook_text);
    }

    result = result.replace("{{char}}", char_name);
    result = result.replace("{{persona}}", persona_name);
    result = result.replace("{{user}}", persona_name);
    result = result.replace("{{ai_name}}", char_name);
    result = result.replace("{{ai_description}}", &char_desc);
    result = result.replace("{{ai_rules}}", "");
    result = result.replace("{{persona_name}}", persona_name);
    result = result.replace("{{persona_description}}", persona_desc);
    result = result.replace("{{user_name}}", persona_name);
    result = result.replace("{{user_description}}", persona_desc);

    result
}

fn build_debug_vars(
    character: &Character,
    persona: Option<&Persona>,
    session: &Session,
    _settings: &Settings,
) -> Value {
    let char_name = &character.name;
    let persona_name = persona.map(|p| p.title.as_str()).unwrap_or("user");
    let raw_char_desc = character
        .definition
        .as_ref()
        .or(character.description.as_ref())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .replace("{{char}}", char_name)
        .replace("{{persona}}", persona_name)
        .replace("{{user}}", persona_name);
    json!({
        "char_name": char_name,
        "char_desc": raw_char_desc,
        "persona_name": persona_name,
        "persona_desc": persona.map(|p| p.description.trim()).unwrap_or("") ,
        "scene_present": session.selected_scene_id.is_some(),
        "companion_mode": companion::is_companion_mode(session, character),
        "author_note_present": session.author_note.as_deref().map(|value| !value.trim().is_empty()).unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_manager::types::{Scene, SceneVariant, StoredMessage};

    fn make_character() -> Character {
        Character {
            id: "c1".into(),
            name: "Alice".into(),
            avatar_path: None,
            design_description: None,
            design_reference_image_ids: vec![],
            background_image_path: None,
            description: Some("I am {{char}}. Partner: {{persona}}.".into()),
            definition: Some("I am {{char}}. Partner: {{persona}}.".into()),
            rules: vec![],
            scenes: vec![],
            default_scene_id: None,
            default_model_id: None,
            fallback_model_id: None,
            mode: "roleplay".into(),
            companion: None,
            memory_type: "manual".into(),
            active_lorebook_ids: vec![],
            prompt_template_id: None,
            group_chat_prompt_template_id: None,
            group_chat_roleplay_prompt_template_id: None,
            system_prompt: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn make_settings() -> Settings {
        Settings {
            default_provider_credential_id: None,
            default_model_id: None,
            provider_credentials: vec![],
            models: vec![],
            app_state: serde_json::json!({}),
            advanced_model_settings: crate::chat_manager::types::AdvancedModelSettings::default(),
            prompt_template_id: None,
            system_prompt: None,
            migration_version: 0,
            advanced_settings: None,
        }
    }

    fn make_model() -> Model {
        Model {
            id: "m1".into(),
            name: "gpt-test".into(),
            provider_id: "openai".into(),
            provider_credential_id: None,
            provider_label: "openai".into(),
            display_name: "GPT Test".into(),
            created_at: 0,
            input_scopes: vec!["text".into()],
            output_scopes: vec!["text".into()],
            advanced_model_settings: None,
            prompt_template_id: None,
            voice_config: None,
            system_prompt: None,
        }
    }

    fn make_session() -> Session {
        Session {
            id: "s1".into(),
            character_id: "c1".into(),
            title: "t".into(),
            background_image_path: None,
            system_prompt: None,
            mode: "roleplay".into(),
            selected_scene_id: None,
            prompt_template_id: None,
            lorebook_ids_override: None,
            author_note: None,
            persona_id: None,
            persona_disabled: false,
            voice_autoplay: None,
            advanced_model_settings: None,
            companion_state: None,
            memories: vec![],
            memory_summary: None,
            memory_summary_token_count: 0,
            memory_tool_events: vec![],
            messages: vec![],
            archived: false,
            created_at: 0,
            updated_at: 0,
            memory_embeddings: vec![],
            memory_status: None,
            memory_error: None,
            memory_progress_step: None,
        }
    }

    #[test]
    fn renders_simple_placeholders() {
        let character = make_character();
        let _model = make_model();
        let settings = make_settings();
        let session = make_session();
        let persona = Some(Persona {
            id: "p1".into(),
            title: "Bob".into(),
            description: "Persona Bob".into(),
            avatar_path: None,
            design_description: None,
            design_reference_image_ids: vec![],
            active_lorebook_ids: vec![],
            nickname: None,
            is_default: true,
            created_at: 0,
            updated_at: 0,
        });

        let base = "Hello {{char}} and {{persona}}. {{char.desc}}";
        let rendered = render_with_context_internal(
            None,
            base,
            &character,
            persona.as_ref(),
            &session,
            &settings,
        );
        assert!(rendered.contains("Hello Alice and Bob."));
        assert!(rendered.contains("I am Alice. Partner: Bob."));

        // Scene injection test
        // Add a scene and make sure {{scene}} replacement works
        let mut session2 = session.clone();
        let mut character2 = character.clone();
        character2.scenes = vec![Scene {
            id: "scene1".into(),
            content: "Meeting {{char}} and {{persona}}".into(),
            direction: None,
            background_image_path: None,
            created_at: 0,
            variants: vec![SceneVariant {
                id: "v1".into(),
                content: "Var {{char}}".into(),
                created_at: 0,
                direction: None,
            }],
            selected_variant_id: Some("v1".into()),
        }];
        session2.selected_scene_id = Some("scene1".into());
        let base2 = "{{scene}}";
        let rendered2 = render_with_context_internal(
            None,
            base2,
            &character2,
            persona.as_ref(),
            &session2,
            &settings,
        );
        assert!(rendered2.contains("Var Alice"));
        assert!(!rendered2.contains("Starting Scene")); // No hardcoded formatting

        let mut session2_edited = session2.clone();
        session2_edited.messages.push(StoredMessage {
            id: "msg-scene".into(),
            role: "scene".into(),
            content: "Edited scene with {{char}} and {{persona}}".into(),
            created_at: 1,
            visible_in_chat: false,
            scene_edited: true,
            usage: None,
            variants: vec![],
            selected_variant_id: None,
            memory_refs: vec![],
            used_lorebook_entries: vec![],
            is_pinned: false,
            attachments: vec![],
            reasoning: None,
            model_id: None,
            fallback_from_model_id: None,
        });
        let rendered2_edited = render_with_context_internal(
            None,
            base2,
            &character2,
            persona.as_ref(),
            &session2_edited,
            &settings,
        );
        assert_eq!(rendered2_edited, "Edited scene with Alice and Bob");

        let mut session3 = session.clone();
        session3.author_note = Some("Keep {{char}} focused on {{persona}}.".into());
        let rendered3 = render_with_context_internal(
            None,
            "{{author_note}}",
            &character,
            persona.as_ref(),
            &session3,
            &settings,
        );
        assert_eq!(rendered3, "Keep Alice focused on Bob.");
    }
}
