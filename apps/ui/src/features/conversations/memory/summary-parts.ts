import type { MemorySummaryPartView } from "@/api/generated/bindings";

export function joinSummaryParts(texts: readonly string[]): string {
  return texts
    .map((text) => text.trim())
    .filter((text) => text !== "")
    .join("\n\n");
}

export function budgetTokens(parts: readonly MemorySummaryPartView[]): number {
  return parts.reduce((sum, part) => (part.kind === "seed" ? sum : sum + (part.token_count ?? 0)), 0);
}

export function nextMerge(parts: readonly MemorySummaryPartView[], chapterSize: number): number[] {
  const byLevel = new Map<number, number[]>();
  parts.forEach((part, index) => {
    if (part.kind === "seed") return;
    byLevel.set(part.level, [...(byLevel.get(part.level) ?? []), index]);
  });
  const levels = [...byLevel.keys()].sort((left, right) => left - right);
  const target = Math.max(2, chapterSize);
  const full = levels.find((level) => (byLevel.get(level)?.length ?? 0) >= target);
  if (full !== undefined) return (byLevel.get(full) ?? []).slice(0, target);
  const pair = levels.find((level) => (byLevel.get(level)?.length ?? 0) >= 2);
  if (pair !== undefined) return (byLevel.get(pair) ?? []).slice(0, target);
  const compactable = [...byLevel.values()].flat().sort((left, right) => left - right);
  return compactable.length >= 2 ? compactable.slice(0, 2) : [];
}

export function summarizedUpTo(parts: readonly MemorySummaryPartView[]): number {
  return parts.reduce((end, part) => Math.max(end, part.message_end), 0);
}

export type PartTier = "seed" | "cycle" | "chapter" | "arc";

export function partTier(part: MemorySummaryPartView): PartTier {
  if (part.kind !== "chapter") return part.kind;
  return part.level >= 2 ? "arc" : "chapter";
}
