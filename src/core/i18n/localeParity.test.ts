import { describe, it, expect } from "vitest";
import { localeRegistry } from "./locales/registry";

/**
 * Locales are typed `DeepPartialMessageTree`, so a key missing from a translation is not a
 * compile error — `t()` silently falls back to English. This spec pins the keys added
 * alongside persona/lorebook tags so that fallback can't quietly reappear.
 *
 * Scoped deliberately: the locales carry unrelated pre-existing gaps, so asserting full
 * parity here would fail on work this area doesn't own.
 */
const REQUIRED_KEYS = [
  "library.sort.label",
  "library.sort.recent",
  "library.sort.alphabetical",
  "library.tagFilter.clear",
  "library.emptyStates.noTagMatches.title",
  "library.emptyStates.noTagMatches.description",
  "characters.lorebook.metadataTitle",
  "characters.lorebook.tagsLabel",
  "characters.lorebook.tagsPlaceholder",
  "personas.edit.tagsLabel",
  "personas.edit.tagsPlaceholder",
  "personas.edit.tagsHint",
] as const;

function lookup(messages: unknown, key: string): unknown {
  return key
    .split(".")
    .reduce<unknown>(
      (node, part) =>
        node && typeof node === "object" ? (node as Record<string, unknown>)[part] : undefined,
      messages,
    );
}

const locales = Object.keys(localeRegistry) as Array<keyof typeof localeRegistry>;

describe("tag/sort translation parity", () => {
  it("covers every registered locale", () => {
    expect(locales.length).toBeGreaterThanOrEqual(20);
    expect(locales).toContain("en");
  });

  it.each(locales)("%s defines all tag and sort keys", (locale) => {
    const { messages } = localeRegistry[locale];
    const missing = REQUIRED_KEYS.filter((key) => {
      const value = lookup(messages, key);
      return typeof value !== "string" || value.trim() === "";
    });
    expect(missing).toEqual([]);
  });
});
