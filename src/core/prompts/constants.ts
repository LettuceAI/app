export const APP_DEFAULT_TEMPLATE_ID = "prompt_app_default";
export const APP_LOCAL_ROLEPLAY_TEMPLATE_ID = "prompt_app_local_roleplay";
export const APP_COMPANION_TEMPLATE_ID = "prompt_app_companion";
export const APP_DYNAMIC_SUMMARY_TEMPLATE_ID = "prompt_app_dynamic_summary";
export const APP_DYNAMIC_MEMORY_TEMPLATE_ID = "prompt_app_dynamic_memory";
export const APP_DYNAMIC_MEMORY_LOCAL_TEMPLATE_ID = "prompt_app_dynamic_memory_local";
export const APP_HELP_ME_REPLY_TEMPLATE_ID = "prompt_app_help_me_reply";
export const APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID =
  "prompt_app_help_me_reply_conversational";
export const APP_LOREBOOK_ENTRY_WRITER_TEMPLATE_ID = "prompt_app_lorebook_entry_writer";
export const APP_GROUP_CHAT_TEMPLATE_ID = "prompt_app_group_chat";
export const APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID = "prompt_app_group_chat_roleplay";
export const APP_AVATAR_GENERATION_TEMPLATE_ID = "prompt_app_avatar_generation";
export const APP_AVATAR_EDIT_TEMPLATE_ID = "prompt_app_avatar_edit";
export const APP_SCENE_GENERATION_TEMPLATE_ID = "prompt_app_scene_generation";
export const APP_SCENE_PROMPT_WRITER_TEMPLATE_ID = "prompt_app_scene_prompt_writer";
export const APP_DESIGN_REFERENCE_TEMPLATE_ID = "prompt_app_design_reference";

const PROTECTED_TEMPLATE_IDS = new Set([
  APP_DEFAULT_TEMPLATE_ID,
  APP_LOCAL_ROLEPLAY_TEMPLATE_ID,
  APP_COMPANION_TEMPLATE_ID,
  APP_DYNAMIC_SUMMARY_TEMPLATE_ID,
  APP_DYNAMIC_MEMORY_TEMPLATE_ID,
  APP_DYNAMIC_MEMORY_LOCAL_TEMPLATE_ID,
  APP_HELP_ME_REPLY_TEMPLATE_ID,
  APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID,
  APP_LOREBOOK_ENTRY_WRITER_TEMPLATE_ID,
  APP_GROUP_CHAT_TEMPLATE_ID,
  APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID,
  APP_AVATAR_GENERATION_TEMPLATE_ID,
  APP_AVATAR_EDIT_TEMPLATE_ID,
  APP_SCENE_GENERATION_TEMPLATE_ID,
  APP_SCENE_PROMPT_WRITER_TEMPLATE_ID,
  APP_DESIGN_REFERENCE_TEMPLATE_ID,
]);

const NON_SYSTEM_TEMPLATE_IDS = new Set([
  APP_DYNAMIC_SUMMARY_TEMPLATE_ID,
  APP_DYNAMIC_MEMORY_TEMPLATE_ID,
  APP_DYNAMIC_MEMORY_LOCAL_TEMPLATE_ID,
  APP_HELP_ME_REPLY_TEMPLATE_ID,
  APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID,
  APP_LOREBOOK_ENTRY_WRITER_TEMPLATE_ID,
  APP_GROUP_CHAT_TEMPLATE_ID,
  APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID,
  APP_AVATAR_GENERATION_TEMPLATE_ID,
  APP_AVATAR_EDIT_TEMPLATE_ID,
  APP_SCENE_GENERATION_TEMPLATE_ID,
  APP_SCENE_PROMPT_WRITER_TEMPLATE_ID,
  APP_DESIGN_REFERENCE_TEMPLATE_ID,
]);

export function isProtectedPromptTemplate(id: string): boolean {
  return PROTECTED_TEMPLATE_IDS.has(id);
}

export function isSystemPromptTemplate(id: string): boolean {
  return !NON_SYSTEM_TEMPLATE_IDS.has(id);
}

export function getPromptTypeLabel(id: string): string {
  switch (id) {
    case APP_LOCAL_ROLEPLAY_TEMPLATE_ID:
      return "Local RP";
    case APP_COMPANION_TEMPLATE_ID:
      return "Companion";
    case APP_DYNAMIC_SUMMARY_TEMPLATE_ID:
      return "Dynamic Summary";
    case APP_DYNAMIC_MEMORY_TEMPLATE_ID:
      return "Dynamic Memory";
    case APP_DYNAMIC_MEMORY_LOCAL_TEMPLATE_ID:
      return "Dynamic Memory (Local LLM)";
    case APP_HELP_ME_REPLY_TEMPLATE_ID:
      return "Reply Helper";
    case APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID:
      return "Reply Helper (Conversational)";
    case APP_LOREBOOK_ENTRY_WRITER_TEMPLATE_ID:
      return "Lorebook Entry Writer";
    case APP_GROUP_CHAT_TEMPLATE_ID:
      return "Group Chat (Conversation)";
    case APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID:
      return "Group Chat (Roleplay)";
    case APP_AVATAR_GENERATION_TEMPLATE_ID:
      return "Avatar Generation";
    case APP_AVATAR_EDIT_TEMPLATE_ID:
      return "Avatar Image Edit";
    case APP_SCENE_GENERATION_TEMPLATE_ID:
      return "Scene Generation";
    case APP_SCENE_PROMPT_WRITER_TEMPLATE_ID:
      return "Scene Prompt Writer";
    case APP_DESIGN_REFERENCE_TEMPLATE_ID:
      return "Design Reference Writer";
    default:
      return "System";
  }
}
