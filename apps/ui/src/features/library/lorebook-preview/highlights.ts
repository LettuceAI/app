import type { LorebookEntryView, LorebookPreviewEntry } from "@/api/generated/bindings";

export interface Inspection {
  entry: LorebookEntryView;
  match: LorebookPreviewEntry | undefined;
}
export interface WordSpan {
  start: number;
  end: number;
  entries: Inspection[];
}

export function literalSpans(
  text: string,
  entries: Inspection[],
  messageId: string | null,
): WordSpan[] {
  const hits: WordSpan[] = [];
  for (const item of entries) {
    if (!item.match || item.entry.keyword_mode !== "literal") continue;
    if (
      messageId === null
        ? !item.match.matched_in_composer
        : !item.match.matched_message_ids.includes(messageId)
    )
      continue;
    for (const keyword of item.match.matched_keywords) {
      if (!keyword.trim() || keyword.includes("*")) continue;
      const pattern = new RegExp(
        keyword.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"),
        item.entry.case_sensitive ? "gu" : "giu",
      );
      for (const found of text.matchAll(pattern)) {
        hits.push({ start: found.index, end: found.index + found[0].length, entries: [item] });
      }
    }
  }
  hits.sort((a, b) => a.start - b.start || b.end - a.end);
  const merged: WordSpan[] = [];
  for (const hit of hits) {
    const previous = merged.at(-1);
    if (previous && hit.start < previous.end) {
      previous.end = Math.max(previous.end, hit.end);
      previous.entries = [
        ...new Map(
          [...previous.entries, ...hit.entries].map((item) => [item.entry.id, item]),
        ).values(),
      ];
    } else merged.push(hit);
  }
  return merged;
}
