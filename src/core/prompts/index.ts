import { invoke } from "@tauri-apps/api/core";
import type { SystemPromptTemplate, PromptScope } from "../storage/schemas";

export async function listPromptTemplates(): Promise<SystemPromptTemplate[]> {
  return await invoke<SystemPromptTemplate[]>("list_prompt_templates");
}

export async function createPromptTemplate(
  name: string,
  scope: PromptScope,
  targetIds: string[],
  content: string,
  entries?: SystemPromptTemplate["entries"],
  condensePromptEntries?: boolean,
): Promise<SystemPromptTemplate> {
  return await invoke<SystemPromptTemplate>("create_prompt_template", {
    name,
    scope,
    targetIds,
    content,
    entries,
    condensePromptEntries,
  });
}

export async function updatePromptTemplate(
  id: string,
  updates: {
    name?: string;
    scope?: PromptScope;
    targetIds?: string[];
    content?: string;
    entries?: SystemPromptTemplate["entries"];
    condensePromptEntries?: boolean;
  },
): Promise<SystemPromptTemplate> {
  return await invoke<SystemPromptTemplate>("update_prompt_template", {
    id,
    name: updates.name,
    scope: updates.scope,
    targetIds: updates.targetIds,
    content: updates.content,
    entries: updates.entries,
    condensePromptEntries: updates.condensePromptEntries,
  });
}

export async function deletePromptTemplate(id: string): Promise<void> {
  await invoke("delete_prompt_template", { id });
}

export async function getPromptTemplate(id: string): Promise<SystemPromptTemplate | null> {
  return await invoke<SystemPromptTemplate | null>("get_prompt_template", { id });
}

export async function getDefaultSystemPromptTemplate(): Promise<string> {
  return await invoke<string>("get_default_system_prompt_template");
}

export async function getAppDefaultTemplateId(): Promise<string> {
  return await invoke<string>("get_app_default_template_id");
}

export async function isAppDefaultTemplate(id: string): Promise<boolean> {
  return await invoke<boolean>("is_app_default_template", { id });
}

export async function resetAppDefaultTemplate(): Promise<SystemPromptTemplate> {
  return await invoke<SystemPromptTemplate>("reset_app_default_template");
}

export async function resetLocalRoleplayTemplate(): Promise<SystemPromptTemplate> {
  return await invoke<SystemPromptTemplate>("reset_local_roleplay_template");
}

export async function resetDynamicSummaryTemplate(): Promise<SystemPromptTemplate> {
  return await invoke<SystemPromptTemplate>("reset_dynamic_summary_template");
}

export async function resetDynamicMemoryTemplate(): Promise<SystemPromptTemplate> {
  return await invoke<SystemPromptTemplate>("reset_dynamic_memory_template");
}

export async function resetHelpMeReplyTemplate(): Promise<SystemPromptTemplate> {
  return await invoke<SystemPromptTemplate>("reset_help_me_reply_template");
}

export async function resetHelpMeReplyConversationalTemplate(): Promise<SystemPromptTemplate> {
  return await invoke<SystemPromptTemplate>("reset_help_me_reply_conversational_template");
}

export async function resetAvatarGenerationTemplate(): Promise<SystemPromptTemplate> {
  return await invoke<SystemPromptTemplate>("reset_avatar_generation_template");
}

export async function resetAvatarEditTemplate(): Promise<SystemPromptTemplate> {
  return await invoke<SystemPromptTemplate>("reset_avatar_edit_template");
}

export async function resetSceneGenerationTemplate(): Promise<SystemPromptTemplate> {
  return await invoke<SystemPromptTemplate>("reset_scene_generation_template");
}

export async function resetDesignReferenceTemplate(): Promise<SystemPromptTemplate> {
  return await invoke<SystemPromptTemplate>("reset_design_reference_template");
}

export async function getRequiredTemplateVariables(templateId: string): Promise<string[]> {
  return await invoke<string[]>("get_required_template_variables", { templateId });
}

export async function validateTemplateVariables(
  templateId: string,
  content: string,
  entries?: SystemPromptTemplate["entries"],
): Promise<void> {
  await invoke("validate_template_variables", { templateId, content, entries });
}
