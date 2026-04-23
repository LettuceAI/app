import type { CompanionConfig } from "../../../../core/storage/schemas";

export function createDefaultCompanionConfig(
  promptTemplateId: string | null = null,
): CompanionConfig {
  return {
    soul: {
      essence: "",
      voice: "",
      relationalStyle: "",
      vulnerabilities: "",
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
    },
    prompting: {
      promptTemplateId,
      styleNotes: "",
    },
  };
}

export function withCompanionPromptTemplate(
  companion: CompanionConfig | null | undefined,
  promptTemplateId: string | null,
): CompanionConfig {
  const base = normalizeCompanionConfig(companion);

  return {
    ...base,
    prompting: {
      ...base.prompting,
      promptTemplateId,
    },
  };
}

export function normalizeCompanionConfig(
  companion: CompanionConfig | null | undefined,
): CompanionConfig {
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
