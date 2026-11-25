import { describe, expect, it } from "vitest";
import type { MemorySummaryPartView } from "@/api/generated/bindings";
import { budgetTokens, joinSummaryParts, nextMerge, summarizedUpTo } from "./summary-parts";

const part = (kind: MemorySummaryPartView["kind"], level: number, start: number, end: number, tokens = 10): MemorySummaryPartView => ({
  id: `${kind}-${start}`, kind, level, origin: "model", message_start: start, message_end: end, text: `${kind} ${start}`, token_count: tokens, pending: false, source_count: 0,
});

describe("summary parts helpers", () => {
  it("joins trimmed parts and drops empty ones", () => {
    expect(joinSummaryParts([" One. ", "", "Two."])).toBe("One.\n\nTwo.");
  });

  it("leaves the starting summary out of the budget", () => {
    expect(budgetTokens([part("seed", 0, 0, 6, 500), part("cycle", 0, 6, 12, 40)])).toBe(40);
  });

  it("picks a full level, else the lowest pair, else the two oldest parts", () => {
    const parts = [part("seed", 0, 0, 6), part("chapter", 1, 6, 18), part("chapter", 1, 18, 30), part("cycle", 0, 30, 36), part("cycle", 0, 36, 42)];
    expect(nextMerge(parts, 2)).toEqual([3, 4]);
    expect(nextMerge(parts.slice(0, 4), 2)).toEqual([1, 2]);
    expect(nextMerge([part("seed", 0, 0, 6), part("chapter", 2, 6, 30), part("chapter", 1, 30, 42), part("cycle", 0, 42, 48)], 2)).toEqual([1, 2]);
    expect(nextMerge([part("seed", 0, 0, 6), part("chapter", 3, 6, 54)], 2)).toEqual([]);
  });

  it("reports how far the parts reach", () => {
    expect(summarizedUpTo([part("chapter", 1, 0, 12), part("cycle", 0, 12, 18)])).toBe(18);
  });
});
