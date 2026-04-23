// Centralized client surface for prompt templates and preview/rendering.
// For now, we delegate to the existing index.ts functions to avoid breaking callers.
// Future refactor: remove scope-dependent calls and add renderPreview(context).

export {
  listPromptTemplates,
  createPromptTemplate,
  updatePromptTemplate,
  deletePromptTemplate,
  getPromptTemplate,
  getDefaultSystemPromptTemplate,
  getAppDefaultTemplateId,
  isAppDefaultTemplate,
  resetAppDefaultTemplate,
  resetLocalRoleplayTemplate,
  resetCompanionTemplate,
  resetDynamicSummaryTemplate,
  resetDynamicMemoryTemplate,
  resetDynamicMemoryLocalTemplate,
  resetGroupChatTemplate,
  resetGroupChatRoleplayTemplate,
  resetHelpMeReplyTemplate,
  resetHelpMeReplyConversationalTemplate,
  resetLorebookEntryWriterTemplate,
  resetAvatarGenerationTemplate,
  resetAvatarEditTemplate,
  resetSceneGenerationTemplate,
  resetScenePromptWriterTemplate,
  resetDesignReferenceTemplate,
  getPromptParameterEngine,
  getRequiredTemplateVariables,
  validateTemplateVariables,
} from "./index";

import { invoke } from "@tauri-apps/api/core";

export async function exportPromptTemplateAsUsc(id: string): Promise<string> {
  return await invoke<string>("export_prompt_template_as_usc", { id });
}

export async function renderPromptPreview(
  content: string,
  opts: { characterId: string; sessionId?: string; personaId?: string },
): Promise<string> {
  return await invoke<string>("render_prompt_preview", {
    content,
    characterId: opts.characterId,
    sessionId: opts.sessionId,
    personaId: opts.personaId,
  });
}
