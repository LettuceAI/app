import type { ConversationView, MediaPartKind, MessageRole, ParticipantView, TimelineMessage } from "@/api/generated/bindings";

export type Group = "only" | "start" | "middle" | "end";

/** Where a message sits in a run from the same speaker. Narration always stands alone. */
export function groupOf(list: readonly TimelineMessage[], index: number): Group {
  const message = list[index];
  if (!message) return "only";
  const role: MessageRole = message.role;
  if (role === "scene" || role === "system") return "only";
  const same = (other: TimelineMessage | undefined) => other !== undefined && other.role === role && other.author_participant_id === message.author_participant_id;
  const prev = same(list[index - 1]);
  const next = same(list[index + 1]);
  return prev && next ? "middle" : prev ? "end" : next ? "start" : "only";
}

export function textOf(message: TimelineMessage): string {
  return message.parts.flatMap((part) => (part.type === "text" ? [part.text] : [])).join("\n\n");
}

export interface MediaItem {
  id: string;
  url: string | null;
  kind: MediaPartKind | null;
  name?: string;
}

export function mediaOf(message: TimelineMessage): MediaItem[] {
  return message.parts.flatMap((part): MediaItem[] => (part.type === "media" ? [{ id: part.asset.asset_id, url: part.asset.url, kind: part.kind }] : []));
}

export function paragraphsOf(text: string): string[] {
  return text.split(/\n{2,}/).filter((paragraph) => paragraph.trim().length > 0);
}

export function characterParticipant(view: Pick<ConversationView, "participants"> | undefined): ParticipantView | null {
  return view?.participants.find((participant) => participant.role === "character") ?? null;
}

export function userParticipant(view: Pick<ConversationView, "participants"> | undefined): ParticipantView | null {
  return view?.participants.find((participant) => participant.role === "user") ?? null;
}

export interface Author {
  name: string;
  avatar: string | null;
}

export function authorOf(participants: readonly ParticipantView[], message: TimelineMessage): Author | undefined {
  if (message.role === "scene" || message.role === "system") return undefined;
  const found =
    (message.author_participant_id ? participants.find((participant) => participant.id === message.author_participant_id) : undefined) ??
    participants.find((participant) => participant.role === (message.role === "user" ? "user" : "character"));
  return found ? { name: found.name, avatar: found.avatar?.url ?? null } : undefined;
}

export function sameDay(a: number, b: number): boolean {
  const left = new Date(a);
  const right = new Date(b);
  return left.getFullYear() === right.getFullYear() && left.getMonth() === right.getMonth() && left.getDate() === right.getDate();
}

export function formatTime(timestamp: number, locale: string): string {
  return new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(timestamp);
}

export type DayLabel = { kind: "today" } | { kind: "yesterday" } | { kind: "date"; text: string };

export function dayLabel(timestamp: number, now: number, locale: string): DayLabel {
  if (sameDay(timestamp, now)) return { kind: "today" };
  if (sameDay(timestamp, now - 24 * 60 * 60 * 1000)) return { kind: "yesterday" };
  return { kind: "date", text: new Intl.DateTimeFormat(locale, { dateStyle: "long" }).format(timestamp) };
}

export interface MessageStats {
  model: string | null;
  promptTokens: number | null;
  completionTokens: number | null;
  tokensPerSecond: number | null;
  firstTokenSeconds: number | null;
}

function numberAt(record: Record<string, unknown>, key: string): number | null {
  const value = record[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function modelLabel(value: unknown): string | null {
  if (typeof value !== "string" || value.trim() === "") return null;
  const file = value.split(/[\\/]/).pop() ?? value;
  return file.replace(/\.gguf$/i, "");
}

/** Reads the runtime's summary keys (`modelName`, `promptTokens`, `completionTokens`, `ttftMs`, `decodeTokensPerSecond`); anything missing stays null. */
export function statsOf(summary: unknown): MessageStats | null {
  if (typeof summary !== "object" || summary === null || Array.isArray(summary)) return null;
  const record = summary as Record<string, unknown>;
  const firstTokenMs = numberAt(record, "ttftMs");
  const stats: MessageStats = {
    model: modelLabel(record.modelName),
    promptTokens: numberAt(record, "promptTokens"),
    completionTokens: numberAt(record, "completionTokens"),
    tokensPerSecond: numberAt(record, "decodeTokensPerSecond"),
    firstTokenSeconds: firstTokenMs === null ? null : firstTokenMs / 1000,
  };
  const any = Object.values(stats).some((value) => value !== null);
  return any ? stats : null;
}

/** The loaded messages oldest first, from pages kept newest first. */
export function flattenPages<T extends { page: { items: TimelineMessage[] } }>(pages: readonly T[]): TimelineMessage[] {
  const seen = new Set<string>();
  const out: TimelineMessage[] = [];
  for (const page of [...pages].reverse()) {
    for (const message of page.page.items) {
      if (seen.has(message.id)) continue;
      seen.add(message.id);
      out.push(message);
    }
  }
  return out;
}

export function spacing(group: Group): string {
  return group === "start" || group === "only" ? "mt-5" : "mt-1";
}
