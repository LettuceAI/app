import type { CompanionEmotionVector, CompanionRegulationStyle, CompanionRelationshipDefaults, CompanionSoulConfigView, CompanionSoulDraft, CompanionSoulIdentity } from "@/api/generated/bindings";
import { hasSceneContent, sceneText, type SceneDraft } from "./form";

export type SoulTextKey = { [K in keyof CompanionSoulIdentity]: CompanionSoulIdentity[K] extends string ? K : never }[keyof CompanionSoulIdentity];

export const soulTextKeys: readonly SoulTextKey[] = ["essence", "traits", "backstory", "appearance", "goals", "likes", "voice", "relational_style", "vulnerabilities", "fears", "habits", "boundaries"];

const noticeableChange = 0.02;

export type FeelingGroup = "affect" | "regulation" | "relationship";

export interface TextChange {
  key: SoulTextKey;
  now: string;
  proposed: string;
}

export interface FeelingChange {
  group: FeelingGroup;
  key: string;
  now: number;
  proposed: number;
}

export interface SoulChanges {
  texts: TextChange[];
  feelings: FeelingChange[];
  facts: number;
  factsChanged: boolean;
}

export function soulDraftOf(config: CompanionSoulConfigView): CompanionSoulDraft {
  return { soul: config.soul, authored_facts: config.authored_facts, relationship_defaults: config.relationship_defaults };
}

export function openingContext(scenes: SceneDraft[]): string | null {
  const text = scenes
    .filter((scene) => hasSceneContent(scene) && sceneText(scene).trim())
    .slice(0, 3)
    .map((scene, index) => {
      const direction = scene.direction?.trim();
      const body = `Scene ${index + 1}\n${sceneText(scene).trim()}`;
      return direction ? `${body}\nDirection: ${direction}` : body;
    })
    .join("\n\n");
  return text || null;
}

function vectorChanges(group: FeelingGroup, now: object, proposed: object): FeelingChange[] {
  const current = now as Record<string, number | null>;
  return Object.entries(proposed as Record<string, number | null>).flatMap(([key, value]) => {
    const before = current[key] ?? 0;
    return value !== null && Math.abs(value - before) >= noticeableChange ? [{ group, key, now: before, proposed: value }] : [];
  });
}

export function soulChanges(config: CompanionSoulConfigView, proposal: CompanionSoulDraft): SoulChanges {
  const texts = soulTextKeys.flatMap((key): TextChange[] => {
    const now = config.soul[key];
    const proposed = proposal.soul[key];
    return now.trim() !== proposed.trim() ? [{ key, now, proposed }] : [];
  });
  const feelings = [
    ...vectorChanges("affect", config.soul.baseline_affect, proposal.soul.baseline_affect),
    ...vectorChanges("regulation", config.soul.regulation_style, proposal.soul.regulation_style),
    ...vectorChanges("relationship", config.relationship_defaults, proposal.relationship_defaults),
  ];
  return { texts, feelings, facts: proposal.authored_facts.length, factsChanged: JSON.stringify(proposal.authored_facts) !== JSON.stringify(config.authored_facts) };
}

export interface SoulSelection {
  texts: Partial<Record<SoulTextKey, string>>;
  feelings: boolean;
  facts: boolean;
}

function mergedVector<T extends CompanionEmotionVector | CompanionRegulationStyle | CompanionRelationshipDefaults>(now: T, proposed: T): T {
  const merged: Record<string, number | null> = { ...now };
  for (const [key, value] of Object.entries(proposed)) if (value !== null) merged[key] = value;
  return merged as T;
}

export function applySoulSelection(config: CompanionSoulConfigView, proposal: CompanionSoulDraft, selection: SoulSelection): CompanionSoulConfigView {
  const soul = { ...config.soul, ...selection.texts };
  if (selection.feelings) {
    soul.baseline_affect = mergedVector(config.soul.baseline_affect, proposal.soul.baseline_affect);
    soul.regulation_style = mergedVector(config.soul.regulation_style, proposal.soul.regulation_style);
  }
  return {
    ...config,
    soul,
    relationship_defaults: selection.feelings ? mergedVector(config.relationship_defaults, proposal.relationship_defaults) : config.relationship_defaults,
    authored_facts: selection.facts ? proposal.authored_facts : config.authored_facts,
  };
}
