import { queryOptions } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { LorebookEntryView, LorebookView_Serialize } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export function lorebookBookKey(lorebookId: string) {
  return [...queryKeys.lorebooks.all, "detail", lorebookId, "preview"] as const;
}

export function lorebookBookQuery(api: ApiClient, lorebookId: string) {
  return queryOptions({
    queryKey: lorebookBookKey(lorebookId),
    queryFn: (): Promise<LorebookView_Serialize> =>
      api.call("lorebookGet", { lorebook_id: lorebookId }),
  });
}

export function matchesEntry(entry: LorebookEntryView, needle: string): boolean {
  return (
    entry.title.toLowerCase().includes(needle) ||
    entry.content.toLowerCase().includes(needle) ||
    entry.keywords.some((keyword) => keyword.toLowerCase().includes(needle))
  );
}

export function movedEntry(
  before: readonly string[],
  after: readonly string[],
): { id: string; index: number } | null {
  const start = after.findIndex((id, index) => id !== before[index]);
  if (start < 0) return null;
  const forward = after[start]!;
  const without = (list: readonly string[], id: string) => list.filter((value) => value !== id);
  const same = (left: readonly string[], right: readonly string[]) =>
    left.length === right.length && left.every((id, index) => id === right[index]);
  const id = same(without(before, forward), without(after, forward)) ? forward : before[start]!;
  return { id, index: after.indexOf(id) };
}

export function moveEntry(
  entries: LorebookEntryView[],
  id: string,
  index: number,
): LorebookEntryView[] {
  const from = entries.findIndex((entry) => entry.id === id);
  if (from < 0 || from === index) return entries;
  const next = [...entries];
  const [entry] = next.splice(from, 1);
  next.splice(index, 0, entry!);
  return next;
}
