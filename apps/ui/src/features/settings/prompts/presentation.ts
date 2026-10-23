import type { Kind, Role, Position, ImageSlot, RuleType, RuleValue } from "./types";

export const KIND_LABEL: Record<Kind, string> = {
  direct_chat: "One-on-one chat",
  companion_chat: "Companion chat",
  group_chat_roleplay: "Group roleplay",
  group_chat_conversational: "Group conversation",
  reply_helper_roleplay: "Help me reply, roleplay",
  reply_helper_conversational: "Help me reply, conversation",
  dynamic_memory_summarizer: "Memory summary",
  dynamic_memory_manager: "Memory manager",
  lorebook_entry_writer: "Lorebook entry writer",
  lorebook_keyword_generator: "Lorebook keywords",
  avatar_generation: "Avatar picture",
  avatar_edit_request: "Avatar edit",
  scene_generation: "Scene picture",
  scene_prompt_writer: "Scene prompt writer",
  design_reference_writer: "Design reference writer",
  companion_soul_writer: "Companion soul writer",
  companion_growthcycle: "Companion growth",
  companion_consolidation: "Companion consolidation",
  lorebook_generator_planner: "Lorebook planner",
  lorebook_generator_writer: "Lorebook writer",
  lorebook_generator_refine: "Lorebook refinement",
  lorebook_generator_coherence: "Lorebook coherence",
};

export const KIND_PURPOSE: Record<Kind, string> = {
  direct_chat: "Talks as the character in one-on-one chats",
  companion_chat: "Talks as a companion, with feelings and memories",
  group_chat_roleplay: "Group chats that play out a scene",
  group_chat_conversational: "Group chats that are just talk",
  reply_helper_roleplay: "Drafts your next message in a roleplay",
  reply_helper_conversational: "Drafts your next message in a conversation",
  dynamic_memory_summarizer: "Writes the running summary of a chat",
  dynamic_memory_manager: "Decides what to remember and what to let go",
  lorebook_entry_writer: "Turns messages into lorebook entries",
  lorebook_keyword_generator: "Suggests keywords for a lorebook entry",
  avatar_generation: "Writes the picture prompt for a new avatar",
  avatar_edit_request: "Turns an edit request into an avatar prompt",
  scene_generation: "Draws a moment from the chat",
  scene_prompt_writer: "Writes the picture prompt for a scene",
  design_reference_writer: "Describes how a character looks",
  companion_soul_writer: "Writes a companion's soul from the character",
  companion_growthcycle: "Updates a companion from recent conversations",
  companion_consolidation: "Consolidates a companion's memories",
  lorebook_generator_planner: "Plans a lorebook",
  lorebook_generator_writer: "Writes planned lorebook entries",
  lorebook_generator_refine: "Refines a lorebook entry",
  lorebook_generator_coherence: "Checks lorebook coherence",
};

export type KindGroup = "chat" | "helpers" | "memory" | "lorebook" | "images" | "companion";
export const KIND_GROUPS: readonly { id: KindGroup; label: string; kinds: readonly Kind[] }[] = [
  { id: "chat", label: "Chat", kinds: ["direct_chat", "companion_chat", "group_chat_roleplay", "group_chat_conversational"] },
  { id: "helpers", label: "Help me reply", kinds: ["reply_helper_roleplay", "reply_helper_conversational"] },
  { id: "memory", label: "Memory", kinds: ["dynamic_memory_summarizer", "dynamic_memory_manager"] },
  { id: "lorebook", label: "Lorebooks", kinds: ["lorebook_entry_writer", "lorebook_keyword_generator", "lorebook_generator_planner", "lorebook_generator_writer", "lorebook_generator_refine", "lorebook_generator_coherence"] },
  { id: "images", label: "Pictures", kinds: ["avatar_generation", "avatar_edit_request", "scene_generation", "scene_prompt_writer", "design_reference_writer"] },
  { id: "companion", label: "Companion", kinds: ["companion_soul_writer", "companion_growthcycle", "companion_consolidation"] },
];
export const groupOf = (kind: Kind): KindGroup => KIND_GROUPS.find((g) => g.kinds.includes(kind))!.id;

export const CAN_BE_DEFAULT = (kind: Kind) => kind === "direct_chat";

export const ROLE_LABEL: Record<Role, string> = { system: "System", user: "User", assistant: "Assistant" };
export const POSITION: Record<Position, { label: string; hint: string }> = {
  relative: { label: "Before the chat", hint: "With the character and the scene, ahead of the messages" },
  in_chat: { label: "Inside the chat", hint: "Between the messages, counted from the newest" },
  conditional: { label: "Once the chat is long enough", hint: "Only after the chat reaches a number of messages" },
  interval: { label: "Every few messages", hint: "Comes back on a fixed rhythm" },
};
export const SLOT_LABEL: Record<ImageSlot, string> = {
  character: "Character picture",
  persona: "Persona picture",
  chat_background: "Chat background",
  avatar: "Current avatar",
  references: "Reference pictures",
};
type RuleShape =
  | { kind: "flag" }
  | { kind: "number"; unit: string; min: number }
  | { kind: "words"; placeholder: string }
  | { kind: "choice"; options: readonly { value: string; label: string }[] };
export interface RuleDef { type: RuleType; label: string; group: string; shape: RuleShape;  phrase: (value: RuleValue) => string }

const flag = (type: RuleType, label: string, group: string, yes: string, no: string): RuleDef => ({
  type, label, group, shape: { kind: "flag" }, phrase: (v) => (v ? yes : no),
});
const join = (v: RuleValue) => (Array.isArray(v) && v.length ? v.map((w) => `"${w}"`).join(", ") : "…");

export const RULES: readonly RuleDef[] = [
  { type: "chat_mode", label: "Chat type", group: "Chat", shape: { kind: "choice", options: [{ value: "direct", label: "One-on-one" }, { value: "group", label: "Group" }] }, phrase: (v) => (v === "group" ? "it's a group chat" : "it's a one-on-one chat") },
  flag("is_companion_mode", "Companion chat", "Chat", "it's a companion chat", "it isn't a companion chat"),
  flag("has_persona", "A persona is set", "Chat", "a persona is set", "no persona is set"),
  flag("has_scene", "The chat has a scene", "Chat", "the chat has a scene", "the chat has no scene"),
  flag("has_scene_direction", "The scene has a direction", "Chat", "the scene has a direction", "the scene has no direction"),
  flag("does_author_note_exists", "There's an author note", "Chat", "there's an author note", "there's no author note"),
  flag("has_active_scheduled_note", "A scheduled note is due", "Chat", "a scheduled note is due", "no scheduled note is due"),
  { type: "message_count_at_least", label: "Messages in the chat", group: "Chat", shape: { kind: "number", unit: "or more", min: 1 }, phrase: (v) => `the chat has ${v} or more messages` },
  { type: "participant_count_at_least", label: "Characters in the chat", group: "Chat", shape: { kind: "number", unit: "or more", min: 2 }, phrase: (v) => `${v} or more characters are in it` },
  flag("is_time_awareness_enabled", "Time awareness is on", "Chat", "time awareness is on", "time awareness is off"),

  flag("dynamic_memory_enabled", "Dynamic memory is on", "Memory and lore", "dynamic memory is on", "dynamic memory is off"),
  flag("has_memory_summary", "There's a memory summary", "Memory and lore", "there's a memory summary", "there's no memory summary"),
  flag("has_key_memories", "There are key memories", "Memory and lore", "there are key memories", "there are no key memories"),
  flag("has_lorebook_content", "Lorebook entries matched", "Memory and lore", "lorebook entries matched", "no lorebook entry matched"),
  { type: "info_source", label: "Information comes from", group: "Memory and lore", shape: { kind: "choice", options: [{ value: "messages", label: "Messages" }, { value: "memory", label: "Memory" }, { value: "mixed", label: "Both" }] }, phrase: (v) => `information comes from ${v === "mixed" ? "both" : v}` },

  { type: "keyword_any", label: "Mentions any of", group: "Words", shape: { kind: "words", placeholder: "rain, storm" }, phrase: (v) => `the chat mentions any of ${join(v)}` },
  { type: "keyword_all", label: "Mentions all of", group: "Words", shape: { kind: "words", placeholder: "gate, key" }, phrase: (v) => `the chat mentions all of ${join(v)}` },
  { type: "keyword_none", label: "Mentions none of", group: "Words", shape: { kind: "words", placeholder: "goodbye" }, phrase: (v) => `the chat mentions none of ${join(v)}` },

  flag("scene_generation_enabled", "Scene pictures are on", "Pictures", "scene pictures are on", "scene pictures are off"),
  flag("avatar_generation_enabled", "Avatar pictures are on", "Pictures", "avatar pictures are on", "avatar pictures are off"),
  flag("is_local_image_generation_model", "Picture model is on this computer", "Pictures", "the picture model is local", "the picture model is remote"),
  flag("is_scene_generation_local_image_model", "Scene model is on this computer", "Pictures", "the scene model is local", "the scene model is remote"),
  { type: "scene_image_protocol", label: "Scene prompt style", group: "Pictures", shape: { kind: "choice", options: [{ value: "remote", label: "Remote" }, { value: "local", label: "Local" }] }, phrase: (v) => `scene prompts use the ${v} style` },
  flag("has_chat_background", "The chat has a background", "Pictures", "the chat has a background", "the chat has no background"),
  flag("has_character_reference_images", "Character has reference pictures", "Pictures", "the character has reference pictures", "the character has no reference pictures"),
  flag("has_persona_reference_images", "Persona has reference pictures", "Pictures", "the persona has reference pictures", "the persona has no reference pictures"),
  flag("has_character_reference_text", "Character has a design reference", "Pictures", "the character has a design reference", "the character has no design reference"),
  flag("has_persona_reference_text", "Persona has a design reference", "Pictures", "the persona has a design reference", "the persona has no design reference"),
  flag("has_subject_description", "The subject has a description", "Pictures", "the subject has a description", "the subject has no description"),
  flag("has_current_description", "There's a current description", "Pictures", "there's a current description", "there's no current description"),

  flag("reasoning_enabled", "Reasoning is on", "Model", "reasoning is on", "reasoning is off"),
  flag("vision_enabled", "The model can see pictures", "Model", "the model can see pictures", "the model can't see pictures"),
  { type: "input_scope_any", label: "Model reads", group: "Model", shape: { kind: "words", placeholder: "text, image, audio" }, phrase: (v) => `the model reads ${join(v)}` },
  { type: "output_scope_any", label: "Model writes", group: "Model", shape: { kind: "words", placeholder: "text, image" }, phrase: (v) => `the model writes ${join(v)}` },
  { type: "provider_id_any", label: "Provider is", group: "Model", shape: { kind: "words", placeholder: "openrouter, llamacpp" }, phrase: (v) => `the provider is ${join(v)}` },
];
