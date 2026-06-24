import { invoke } from "@tauri-apps/api/core";
import type { CompanionConfig } from "../storage/schemas";

export interface GenerateCompanionSoulRequest {
  characterName: string;
  characterDefinition?: string | null;
  characterDescription?: string | null;
  openingContext?: string | null;
  currentSoul?: unknown;
  userNotes?: string | null;
  modelId?: string | null;
  requestId?: string | null;
  stream?: boolean;
}

export async function generateCompanionSoulDraft(
  request: GenerateCompanionSoulRequest,
): Promise<Partial<CompanionConfig>> {
  return await invoke<Partial<CompanionConfig>>("chat_generate_companion_soul", {
    args: {
      characterName: request.characterName,
      characterDefinition: request.characterDefinition ?? null,
      characterDescription: request.characterDescription ?? null,
      openingContext: request.openingContext ?? null,
      currentSoul: request.currentSoul ?? null,
      userNotes: request.userNotes ?? null,
      modelId: request.modelId ?? null,
      requestId: request.requestId ?? null,
      stream: request.stream ?? true,
    },
  });
}

export async function clearCompanionSoulGrowth(sessionId: string): Promise<number> {
  return await invoke<number>("companion_clear_soul_growth", { sessionId });
}

export async function removeCompanionSoulGrowth(
  sessionId: string,
  index: number,
): Promise<boolean> {
  return await invoke<boolean>("companion_remove_soul_growth", { sessionId, index });
}

export function mergeCompanionSoulDraft(
  current: CompanionConfig | null | undefined,
  draft: Partial<CompanionConfig>,
): CompanionConfig {
  const base = normalizeCompanionConfig(current);
  return normalizeCompanionConfig({
    ...base,
    ...draft,
    soul: {
      ...base.soul,
      ...draft.soul,
      baselineAffect: {
        ...base.soul.baselineAffect,
        ...draft.soul?.baselineAffect,
      },
      regulationStyle: {
        ...base.soul.regulationStyle,
        ...draft.soul?.regulationStyle,
      },
    },
    relationshipDefaults: {
      ...base.relationshipDefaults,
      ...draft.relationshipDefaults,
    },
  });
}

function createDefaultCompanionConfig(): CompanionConfig {
  return {
    soul: {
      essence: "",
      traits: "",
      backstory: "",
      appearance: "",
      goals: "",
      likes: "",
      voice: "",
      relationalStyle: "",
      vulnerabilities: "",
      fears: "",
      habits: "",
      boundaries: "",
      baselineAffect: {
        warmth: 0.45,
        trust: 0.35,
        calm: 0.65,
        vulnerability: 0.2,
        longing: 0.15,
        hurt: 0.05,
        tension: 0.1,
        irritation: 0.05,
        affectionIntensity: 0.25,
        reassuranceNeed: 0.15,
      },
      regulationStyle: {
        suppression: 0.35,
        volatility: 0.25,
        recoverySpeed: 0.55,
        conflictAvoidance: 0.45,
        reassuranceSeeking: 0.4,
        protestBehavior: 0.2,
        emotionalTransparency: 0.55,
        attachmentActivation: 0.45,
        pride: 0.3,
      },
    },
    relationshipDefaults: {
      closeness: 0.2,
      trust: 0.3,
      affection: 0.15,
      tension: 0,
    },
    memory: {
      enabled: true,
      retrievalLimit: 8,
      maxEntries: 120,
      prioritizeRelationship: true,
      prioritizeEpisodic: true,
      useEmotionalSnapshots: true,
      sharedAcrossSessions: false,
    },
    prompting: {
      promptTemplateId: null,
      styleNotes: "",
    },
    timeAwareness: false,
  };
}

function normalizeCompanionConfig(companion: CompanionConfig | null | undefined): CompanionConfig {
  const defaults = createDefaultCompanionConfig();
  if (!companion) return defaults;

  return {
    ...defaults,
    ...companion,
    soul: {
      ...defaults.soul,
      ...companion.soul,
      baselineAffect: {
        ...defaults.soul.baselineAffect,
        ...companion.soul?.baselineAffect,
      },
      regulationStyle: {
        ...defaults.soul.regulationStyle,
        ...companion.soul?.regulationStyle,
      },
    },
    relationshipDefaults: {
      ...defaults.relationshipDefaults,
      ...companion.relationshipDefaults,
    },
    memory: {
      ...defaults.memory,
      ...companion.memory,
    },
    prompting: {
      ...defaults.prompting,
      ...companion.prompting,
    },
  };
}
