import { Cpu, Heart, Tag, Waypoints, type LucideIcon } from "lucide-react";

import { readAdvancedSettings, type AdvancedSettings } from "../core/storage/advanced";
import { storageBridge } from "../core/storage/files";
import { getEmbeddingModelInfo, listCharacters } from "../core/storage/repo";
import type { Character } from "../core/storage/schemas";

export type ModelRequirementKind = "embedding" | "emotion" | "ner" | "router";

export interface ModelRequirement {
  kind: ModelRequirementKind;
  title: string;
  subtitle: string;
  approxSize: string;
  icon: LucideIcon;
}

export interface ModelRequirementPolicy {
  requireEmbedding?: boolean;
  requireCompanion?: boolean;
}

const REQUIREMENT_META: Record<ModelRequirementKind, Omit<ModelRequirement, "kind">> = {
  embedding: {
    title: "Embedding model",
    subtitle: "Powers dynamic memory recall and semantic search across past turns.",
    approxSize: "~90 MB",
    icon: Cpu,
  },
  emotion: {
    title: "Emotion classifier",
    subtitle: "Reads turns and updates the companion's felt, expressed, and blocked emotion vectors.",
    approxSize: "~120 MB",
    icon: Heart,
  },
  ner: {
    title: "Entity extractor (NER)",
    subtitle: "Identifies people, places, and objects so memories can be canonicalized and linked.",
    approxSize: "~140 MB",
    icon: Tag,
  },
  router: {
    title: "Memory router",
    subtitle:
      "Decides whether new turns should be stored as relationship, milestone, episodic, or other memory categories.",
    approxSize: "~70 MB",
    icon: Waypoints,
  },
};

export function describeRequirement(kind: ModelRequirementKind): ModelRequirement {
  return { kind, ...REQUIREMENT_META[kind] };
}

export function requiresDynamicMemoryModels(advanced: AdvancedSettings): boolean {
  return advanced.dynamicMemory?.enabled === true || advanced.groupDynamicMemory?.enabled === true;
}

export function hasCompanionCharacters(characters: Pick<Character, "mode">[]): boolean {
  return characters.some((character) => character.mode === "companion");
}

export async function getPostSyncRequirementPolicy(): Promise<ModelRequirementPolicy> {
  const [advanced, characters, hasMemories] = await Promise.all([
    readAdvancedSettings(),
    listCharacters(),
    storageBridge.memoryEmbeddingsExist().catch(() => false),
  ]);
  return {
    requireEmbedding: requiresDynamicMemoryModels(advanced) || hasMemories,
    requireCompanion: hasCompanionCharacters(characters),
  };
}

export async function getMissingModelRequirements(
  policy: ModelRequirementPolicy,
): Promise<ModelRequirement[]> {
  const requireEmbedding = policy.requireEmbedding === true;
  const requireCompanion = policy.requireCompanion === true;
  if (!requireEmbedding && !requireCompanion) {
    return [];
  }

  const info = await getEmbeddingModelInfo();
  const missingKinds: ModelRequirementKind[] = [];

  if ((requireEmbedding || requireCompanion) && !info.installed) {
    missingKinds.push("embedding");
  }
  if (requireCompanion) {
    if (!info.companionEmotionInstalled) missingKinds.push("emotion");
    if (!info.companionNerInstalled) missingKinds.push("ner");
    if (!info.companionRouterInstalled) missingKinds.push("router");
  }

  return missingKinds.map(describeRequirement);
}

export async function getPostSyncMissingModelRequirements(): Promise<ModelRequirement[]> {
  const policy = await getPostSyncRequirementPolicy();
  return getMissingModelRequirements(policy);
}

export async function getPostSyncMissingModelRequirementsSettled(options?: {
  attempts?: number;
  delayMs?: number;
}): Promise<ModelRequirement[]> {
  const attempts = Math.max(1, options?.attempts ?? 5);
  const delayMs = Math.max(0, options?.delayMs ?? 250);

  let latestMissing: ModelRequirement[] = [];

  for (let attempt = 0; attempt < attempts; attempt += 1) {
    latestMissing = await getPostSyncMissingModelRequirements();
    if (latestMissing.length > 0) {
      return latestMissing;
    }

    if (attempt < attempts - 1 && delayMs > 0) {
      await new Promise((resolve) => window.setTimeout(resolve, delayMs));
    }
  }

  return latestMissing;
}

export function buildModelRequirementsQueuePath(
  missing: Pick<ModelRequirement, "kind">[],
  returnTo: string,
): string {
  const queue = missing.map((requirement) => requirement.kind).join(",");
  return `/settings/companion-download-queue?queue=${queue}&returnTo=${encodeURIComponent(returnTo)}`;
}
