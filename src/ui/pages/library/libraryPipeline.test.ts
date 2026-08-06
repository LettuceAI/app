import { describe, it, expect } from "vitest";
import {
  tagKey,
  getItemName,
  getItemTags,
  indexItems,
  TAGGABLE_TYPES,
  scopeByTab,
  filterByName,
  filterByTags,
  scopeTypes,
  isTagRowVisible,
  tagControlsVisibility,
  pruneSelectedTags,
  deriveFacets,
  sortItems,
  type LibraryEntry,
  type Facet,
} from "./libraryPipeline";

const char = (over: Partial<LibraryEntry> = {}): LibraryEntry => ({
  id: "c1",
  itemType: "character",
  createdAt: 1,
  name: "Aria",
  ...over,
});
const persona = (over: Partial<LibraryEntry> = {}): LibraryEntry => ({
  id: "p1",
  itemType: "persona",
  createdAt: 1,
  title: "Jen",
  ...over,
});

describe("tagKey", () => {
  it("lowercases and strips whitespace, hyphen, underscore", () => {
    expect(tagKey("Sci-Fi")).toBe("scifi");
    expect(tagKey("sci fi")).toBe("scifi");
    expect(tagKey("SciFi")).toBe("scifi");
    expect(tagKey("sci_fi")).toBe("scifi");
  });
  it("returns empty for punctuation-only tags", () => {
    expect(tagKey("-")).toBe("");
    expect(tagKey("___")).toBe("");
    expect(tagKey("  ")).toBe("");
  });
  it("preserves meaningful punctuation and non-Latin text", () => {
    expect(tagKey("18+")).toBe("18+");
    expect(tagKey("日本語")).toBe("日本語");
  });
});

describe("getItemName", () => {
  it("reads name for characters and lorebooks, title for personas", () => {
    expect(getItemName(char({ name: "Aria" }))).toBe("Aria");
    expect(getItemName(persona({ title: "Jen" }))).toBe("Jen");
    expect(getItemName({ id: "l1", itemType: "lorebook", createdAt: 1, name: "Lore" })).toBe("Lore");
  });
});

describe("getItemTags", () => {
  it("returns tags for taggable types", () => {
    expect(getItemTags(char({ tags: ["Fluff"] }))).toEqual(["Fluff"]);
    expect(getItemTags(persona({ tags: ["Fluff"] }))).toEqual(["Fluff"]);
  });
  it("all entity types are taggable", () => {
    expect(TAGGABLE_TYPES.has("character")).toBe(true);
    expect(TAGGABLE_TYPES.has("persona")).toBe(true);
    expect(TAGGABLE_TYPES.has("lorebook")).toBe(true);
  });
});

describe("indexItems", () => {
  it("builds a normalized, deduped tag-key set per item", () => {
    const [idx] = indexItems([char({ tags: ["Sci-Fi", "SciFi", "Fluff"] })]);
    expect([...idx.tagKeys].sort()).toEqual(["fluff", "scifi"]);
  });
  it("drops empty keys and untagged items get an empty set", () => {
    expect(indexItems([char({ tags: ["-", "  "] })])[0].tagKeys.size).toBe(0);
    expect(indexItems([persona({ tags: undefined })])[0].tagKeys.size).toBe(0);
    expect(indexItems([char({ tags: undefined })])[0].tagKeys.size).toBe(0);
  });
});

describe("scopeByTab", () => {
  const items = indexItems([
    char({ id: "c1" }),
    persona({ id: "p1" }),
    { id: "l1", itemType: "lorebook", createdAt: 1, name: "Lore" },
  ]);
  it("returns everything for All", () => {
    expect(scopeByTab(items, "All").map((i) => i.item.id)).toEqual(["c1", "p1", "l1"]);
  });
  it("filters to one type otherwise", () => {
    expect(scopeByTab(items, "Personas").map((i) => i.item.id)).toEqual(["p1"]);
  });
});

describe("filterByTags (AND)", () => {
  const scoped = indexItems([
    char({ id: "a", tags: ["Fluff", "Sci-Fi"] }),
    char({ id: "b", tags: ["Fluff"] }),
    char({ id: "c", tags: [] }),
  ]);
  it("no selection returns all", () => {
    expect(filterByTags(scoped, []).map((i) => i.item.id)).toEqual(["a", "b", "c"]);
  });
  it("narrows with each key (AND)", () => {
    expect(filterByTags(scoped, ["fluff"]).map((i) => i.item.id)).toEqual(["a", "b"]);
    expect(filterByTags(scoped, ["fluff", "scifi"]).map((i) => i.item.id)).toEqual(["a"]);
  });
  it("excludes untagged items once any tag is selected", () => {
    expect(filterByTags(scoped, ["fluff"]).some((i) => i.item.id === "c")).toBe(false);
  });
});

describe("filterByName", () => {
  const scoped = indexItems([
    char({ id: "a", name: "Aria" }),
    persona({ id: "p", title: "Marina" }),
    { id: "l", itemType: "lorebook", createdAt: 1, name: "Lore" },
  ]);
  it("blank or whitespace-only query returns all", () => {
    expect(filterByName(scoped, "").map((i) => i.item.id)).toEqual(["a", "p", "l"]);
    expect(filterByName(scoped, "   ").map((i) => i.item.id)).toEqual(["a", "p", "l"]);
  });
  it("matches case-insensitive substrings across name and title", () => {
    expect(filterByName(scoped, "ri").map((i) => i.item.id)).toEqual(["a", "p"]);
    expect(filterByName(scoped, "ARIA").map((i) => i.item.id)).toEqual(["a"]);
    expect(filterByName(scoped, " lore ").map((i) => i.item.id)).toEqual(["l"]);
  });
  it("returns empty when nothing matches", () => {
    expect(filterByName(scoped, "zzz")).toEqual([]);
  });
});

// Regression: a selection surviving a tab switch can AND to zero. Facet counts derive from
// the filtered set, so an unguarded chip list empties too — and the clear affordance must
// not depend on it.
describe("zero-result selection after a tab switch", () => {
  const indexed = indexItems([
    char({ id: "c1", tags: ["Sci-Fi", "Fluff"] }),
    persona({ id: "p1", tags: ["Sci-Fi"] }),
    persona({ id: "p2", tags: ["Fluff"] }),
  ]);
  const personaScope = scopeByTab(indexed, "Personas");

  it("pruning keeps both keys because each exists in the new scope", () => {
    const availableKeys = new Set(deriveFacets(personaScope, personaScope).map((f) => f.key));
    expect(pruneSelectedTags(["scifi", "fluff"], availableKeys)).toEqual(["scifi", "fluff"]);
  });

  it("but no persona carries both, so the result set is empty", () => {
    expect(filterByTags(personaScope, ["scifi", "fluff"])).toEqual([]);
  });

  it("keeps the selected tags as zero-count chips so each stays deselectable", () => {
    const filtered = filterByTags(personaScope, ["scifi", "fluff"]);
    const facets = deriveFacets(personaScope, filtered, ["scifi", "fluff"]);
    expect(facets.map((f) => [f.key, f.count])).toEqual([
      ["fluff", 0],
      ["scifi", 0],
    ]);
    expect(facets.map((f) => f.display)).toEqual(["Fluff", "Sci-Fi"]);
  });

  it("still drops the chip list entirely when nothing is selected", () => {
    expect(deriveFacets(personaScope, [], [])).toEqual([]);
  });

  it("does not inflate availableKeys, so pruning can still prune", () => {
    const charScope = scopeByTab(indexed, "Characters");
    const availableKeys = new Set(deriveFacets(charScope, charScope).map((f) => f.key));
    expect(pruneSelectedTags(["scifi", "nonexistent"], availableKeys)).toEqual(["scifi"]);
  });
});

describe("tagControlsVisibility", () => {
  it("hides both when the tab delegates to its own panel", () => {
    expect(
      tagControlsVisibility({ scopeSupportsTags: false, facetCount: 3, selectionCount: 2 }),
    ).toEqual({ chips: false, clear: false });
  });

  it("shows chips only when facets exist", () => {
    expect(
      tagControlsVisibility({ scopeSupportsTags: true, facetCount: 0, selectionCount: 0 }).chips,
    ).toBe(false);
    expect(
      tagControlsVisibility({ scopeSupportsTags: true, facetCount: 1, selectionCount: 0 }).chips,
    ).toBe(true);
  });

  // The bug: clear used to be gated on facetCount too, so it vanished exactly when needed.
  it("keeps clear available with a selection even when no facets remain", () => {
    expect(
      tagControlsVisibility({ scopeSupportsTags: true, facetCount: 0, selectionCount: 2 }).clear,
    ).toBe(true);
  });

  it("hides clear when nothing is selected", () => {
    expect(
      tagControlsVisibility({ scopeSupportsTags: true, facetCount: 5, selectionCount: 0 }).clear,
    ).toBe(false);
  });
});

describe("scopeTypes / isTagRowVisible", () => {
  it("All spans all three types; entity tabs span one", () => {
    expect(scopeTypes("All").sort()).toEqual(["character", "lorebook", "persona"]);
    expect(scopeTypes("Characters")).toEqual(["character"]);
  });
  it("tag row shows on every entity tab now that all types are taggable", () => {
    expect(isTagRowVisible("Characters")).toBe(true);
    expect(isTagRowVisible("All")).toBe(true);
    expect(isTagRowVisible("Personas")).toBe(true);
    expect(isTagRowVisible("Lorebooks")).toBe(true);
  });
});

describe("pruneSelectedTags", () => {
  it("keeps only keys present in the new scope", () => {
    expect(pruneSelectedTags(["fluff", "scifi"], new Set(["fluff"]))).toEqual(["fluff"]);
    expect(pruneSelectedTags(["fluff"], new Set())).toEqual([]);
  });
});

const keys = (f: Facet[]) => f.map((x) => x.key);

describe("deriveFacets", () => {
  it("counts distinct items in filtered, one per item even with dup keys", () => {
    const scoped = indexItems([
      char({ id: "a", tags: ["Fluff", "Sci-Fi", "SciFi"] }),
      char({ id: "b", tags: ["Fluff"] }),
    ]);
    const facets = deriveFacets(scoped, scoped);
    expect(facets.find((f) => f.key === "scifi")?.count).toBe(1); // one item, dup keys counted once
    expect(facets.find((f) => f.key === "fluff")?.count).toBe(2);
  });
  it("display casing is the last raw spelling seen in scoped order", () => {
    const scoped = indexItems([
      char({ id: "a", tags: ["Sci-Fi"] }),
      char({ id: "b", tags: ["SciFi"] }),
    ]);
    expect(deriveFacets(scoped, scoped).find((f) => f.key === "scifi")?.display).toBe("SciFi");
  });
  it("returns facets sorted alphabetically by display", () => {
    const scoped = indexItems([char({ tags: ["Zeta", "alpha", "Mid"] })]);
    expect(keys(deriveFacets(scoped, scoped))).toEqual(["alpha", "mid", "zeta"]);
  });
  it("counts come from filtered; a tag with zero filtered co-occurrence is absent", () => {
    const scoped = indexItems([
      char({ id: "a", tags: ["Fluff", "Sci-Fi"] }),
      char({ id: "b", tags: ["Smut"] }),
    ]);
    const filtered = filterByTags(scoped, ["fluff"]); // only item a
    const facets = deriveFacets(scoped, filtered);
    expect(facets.find((f) => f.key === "fluff")?.count).toBe(1);
    expect(facets.find((f) => f.key === "scifi")?.count).toBe(1);
    expect(facets.find((f) => f.key === "smut")).toBeUndefined(); // absent — dead-end
  });
  it("display casing stays stable even when filtered shrinks (derived from scoped)", () => {
    const scoped = indexItems([
      char({ id: "a", tags: ["Sci-Fi", "Fluff"] }),
      char({ id: "b", tags: ["SciFi"] }),
    ]);
    const filtered = filterByTags(scoped, ["fluff"]); // only item a, whose spelling is "Sci-Fi"
    // display still last-in-scoped ("SciFi"), not filtered's "Sci-Fi"
    expect(deriveFacets(scoped, filtered).find((f) => f.key === "scifi")?.display).toBe("SciFi");
  });
});

describe("sortItems", () => {
  it("Recent = createdAt desc, id tie-break for stability", () => {
    const items = [
      char({ id: "a", createdAt: 10 }),
      char({ id: "c", createdAt: 20 }),
      char({ id: "b", createdAt: 20 }),
    ];
    expect(sortItems(items, "Recent").map((i) => i.id)).toEqual(["b", "c", "a"]);
  });
  it("Alphabetical uses localeCompare across name/title, accents included", () => {
    const items = [
      char({ id: "1", name: "Zoe" }),
      persona({ id: "2", title: "ábel" }),
      char({ id: "3", name: "Mia" }),
    ];
    expect(sortItems(items, "Alphabetical").map((i) => i.id)).toEqual(["2", "3", "1"]);
  });
  it("does not mutate the input array", () => {
    const items = [char({ id: "a", createdAt: 1 }), char({ id: "b", createdAt: 2 })];
    const before = items.map((i) => i.id);
    sortItems(items, "Recent");
    expect(items.map((i) => i.id)).toEqual(before);
  });
});
