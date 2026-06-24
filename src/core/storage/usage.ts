import { listCharacters, readSettings } from "./repo";
import type { Settings } from "./schemas";

export type EntityUsage = {
  characters: number;
  features: string[];
};

type Ref = { id: string; label: string };

function collect(raw: Array<[string | null | undefined, string]>): Ref[] {
  return raw
    .filter(([id]) => typeof id === "string" && id.length > 0)
    .map(([id, label]) => ({ id: id as string, label }));
}

export function promptFeatureRefs(settings: Settings): Ref[] {
  const adv = settings.advancedSettings ?? {};
  return collect([
    [settings.promptTemplateId, "Default prompt"],
    [adv.dynamicMemorySummarizerPromptTemplateId, "Dynamic Memory"],
    [adv.dynamicMemoryManagerPromptTemplateId, "Dynamic Memory"],
    [adv.helpMeReplyRoleplayPromptTemplateId, "Reply Helper"],
    [adv.helpMeReplyConversationalPromptTemplateId, "Reply Helper"],
    [adv.lorebookEntryGeneratorPromptTemplateId, "Lorebooks"],
    [adv.lorebookKeywordGeneratorPromptTemplateId, "Lorebooks"],
    [adv.lorebookGeneratorPlannerPromptTemplateId, "Lorebooks"],
    [adv.lorebookGeneratorWriterPromptTemplateId, "Lorebooks"],
    [adv.lorebookGeneratorRefinePromptTemplateId, "Lorebooks"],
    [adv.lorebookGeneratorCoherencePromptTemplateId, "Lorebooks"],
    [adv.companionSoulWriterPromptTemplateId, "Companion Soul Writer"],
  ]);
}

export function modelFeatureRefs(settings: Settings): Ref[] {
  const adv = settings.advancedSettings ?? {};
  return collect([
    [settings.defaultModelId, "Default model"],
    [adv.summarisationModelId, "Dynamic Memory"],
    [adv.creationHelperModelId, "Creation Helper"],
    [adv.creationHelperImageModelId, "Creation Helper"],
    [adv.helpMeReplyModelId, "Reply Helper"],
    [adv.lorebookEntryGeneratorModelId, "Lorebooks"],
    [adv.lorebookGeneratorModelId, "Lorebooks"],
    [adv.companionSoulWriterModelId, "Companion Soul Writer"],
    [adv.companionSoulWriterFallbackModelId, "Companion Soul Writer"],
    [adv.avatarGenerationModelId, "Image Generation"],
    [adv.sceneGenerationModelId, "Image Generation"],
    [adv.sceneWriterModelId, "Image Generation"],
  ]);
}

export function featuresUsing(id: string, refs: Ref[]): string[] {
  const labels: string[] = [];
  for (const ref of refs) {
    if (ref.id === id && !labels.includes(ref.label)) {
      labels.push(ref.label);
    }
  }
  return labels;
}

export async function getModelUsage(modelId: string): Promise<EntityUsage> {
  const [characters, settings] = await Promise.all([listCharacters(), readSettings()]);
  const charCount = characters.filter(
    (character) => character.defaultModelId === modelId,
  ).length;
  return { characters: charCount, features: featuresUsing(modelId, modelFeatureRefs(settings)) };
}

export function describeUsage(
  usage: EntityUsage,
  kind: "prompt" | "model",
): { title: string; body: string } | null {
  if (usage.characters === 0 && usage.features.length === 0) return null;
  const items: string[] = [];
  if (usage.characters > 0) {
    items.push(usage.characters === 1 ? "1 character" : `${usage.characters} characters`);
  }
  items.push(...usage.features);
  return {
    title: kind === "prompt" ? "This prompt is in use" : "This model is in use",
    body: `Currently used by: ${items.join(", ")}. Deleting it will make them fall back to the default. Are you sure?`,
  };
}
