export type ItemType = "character" | "persona" | "lorebook";

export type EntityTab = "All" | "Characters" | "Personas" | "Lorebooks";

/** Structural supertype of LibraryPage's LibraryItem — only what the pipeline needs. */
export type LibraryEntry = {
  id: string;
  itemType: ItemType;
  createdAt: number;
  name?: string;
  title?: string;
  tags?: string[];
};

/** Which item types expose tags. */
export const TAGGABLE_TYPES: ReadonlySet<ItemType> = new Set<ItemType>([
  "character",
  "persona",
  "lorebook",
]);

/** Index-only normalization: lowercase, strip whitespace/hyphen/underscore. Never persisted. */
export function tagKey(raw: string): string {
  return raw.toLowerCase().replace(/[\s_-]+/g, "");
}

export function getItemName(item: LibraryEntry): string {
  if (item.itemType === "persona") return item.title ?? "";
  return item.name ?? "";
}

export function getItemTags(item: LibraryEntry): string[] | undefined {
  return TAGGABLE_TYPES.has(item.itemType) ? item.tags : undefined;
}

export type IndexedItem<T> = { item: T; tagKeys: Set<string> };

/** Precompute each item's normalized tag-key set once (call when the data changes, not per interaction). */
export function indexItems<T extends LibraryEntry>(items: T[]): IndexedItem<T>[] {
  return items.map((item) => {
    const tagKeys = new Set<string>();
    for (const raw of getItemTags(item) ?? []) {
      const key = tagKey(raw);
      if (key) tagKeys.add(key);
    }
    return { item, tagKeys };
  });
}

export function scopeTypes(tab: EntityTab): ItemType[] {
  if (tab === "Characters") return ["character"];
  if (tab === "Personas") return ["persona"];
  if (tab === "Lorebooks") return ["lorebook"];
  return ["character", "persona", "lorebook"];
}

export function scopeByTab<T extends LibraryEntry>(
  indexed: IndexedItem<T>[],
  tab: EntityTab,
): IndexedItem<T>[] {
  if (tab === "All") return indexed;
  const types = new Set(scopeTypes(tab));
  return indexed.filter(({ item }) => types.has(item.itemType));
}

export function filterByTags<T>(
  scoped: IndexedItem<T>[],
  selectedKeys: string[],
): IndexedItem<T>[] {
  if (selectedKeys.length === 0) return scoped;
  return scoped.filter(({ tagKeys }) => selectedKeys.every((k) => tagKeys.has(k)));
}

/** Case-insensitive substring match on the item's display name. Blank query is a no-op. */
export function filterByName<T extends LibraryEntry>(
  scoped: IndexedItem<T>[],
  query: string,
): IndexedItem<T>[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return scoped;
  return scoped.filter(({ item }) => getItemName(item).toLowerCase().includes(needle));
}

/** Show the tag row only when every type in the tab's scope is taggable. */
export function isTagRowVisible(tab: EntityTab): boolean {
  return scopeTypes(tab).every((type) => TAGGABLE_TYPES.has(type));
}

export type TagControlsVisibility = { chips: boolean; clear: boolean };

/**
 * Chips need facets to render; the clear affordance must not depend on them. A selection
 * that matches nothing is exactly when the user needs a way out, and gating both on the
 * facet count hid the escape hatch in the only case that needed it.
 */
export function tagControlsVisibility(input: {
  /** False for tabs that delegate to their own panel (images, audio). */
  scopeSupportsTags: boolean;
  facetCount: number;
  selectionCount: number;
}): TagControlsVisibility {
  return {
    chips: input.scopeSupportsTags && input.facetCount > 0,
    clear: input.scopeSupportsTags && input.selectionCount > 0,
  };
}

/** Drop selected keys that no longer exist in the new scope's available keys. */
export function pruneSelectedTags(selectedKeys: string[], availableKeys: Set<string>): string[] {
  return selectedKeys.filter((k) => availableKeys.has(k));
}

export type Facet = { key: string; display: string; count: number };

/**
 * Build the chip list. Display casing comes from `scoped` (unfiltered, load order,
 * last-raw-spelling-wins) so labels don't drift as the selection narrows; counts come
 * from `filtered` (distinct items), so a chip's number is "results if you add this" and
 * zero-co-occurrence tags simply don't appear. Sorted alphabetically by display.
 *
 * `selectedKeys` are always kept, at count 0 if they match nothing. Without that, a
 * selection narrowing to zero empties `filtered` and so empties the chip list, leaving
 * no way to deselect one tag — clear-everything would be the only way out.
 */
export function deriveFacets<T extends LibraryEntry>(
  scoped: IndexedItem<T>[],
  filtered: IndexedItem<T>[],
  selectedKeys: readonly string[] = [],
): Facet[] {
  // Display casing: last raw spelling per key, in scoped load order.
  const display = new Map<string, string>();
  for (const { item } of scoped) {
    for (const raw of getItemTags(item) ?? []) {
      const key = tagKey(raw);
      if (key) display.set(key, raw);
    }
  }
  // Counts: distinct filtered items per key (dedup keys within an item via its tagKeys set).
  const counts = new Map<string, number>();
  for (const { tagKeys } of filtered) {
    for (const key of tagKeys) counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  for (const key of selectedKeys) {
    if (!counts.has(key)) counts.set(key, 0);
  }
  const facets: Facet[] = [];
  for (const [key, count] of counts) {
    facets.push({ key, display: display.get(key) ?? key, count });
  }
  facets.sort((a, b) => a.display.localeCompare(b.display));
  return facets;
}

export type SortOption = "Recent" | "Alphabetical";

export const SORT_OPTIONS: SortOption[] = ["Recent", "Alphabetical"];

export function sortItems<T extends LibraryEntry>(items: T[], sort: SortOption): T[] {
  const next = [...items];
  if (sort === "Alphabetical") {
    return next.sort(
      (a, b) => getItemName(a).localeCompare(getItemName(b)) || a.id.localeCompare(b.id),
    );
  }
  // Recent: newest createdAt first, id tie-break for a stable order.
  return next.sort((a, b) => b.createdAt - a.createdAt || a.id.localeCompare(b.id));
}
