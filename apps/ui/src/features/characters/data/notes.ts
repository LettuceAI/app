import type { CompanionNoteRecurrence, CompanionNoteUpsertRequest, CompanionNoteView } from "@/api/generated/bindings";

export const recurrences: readonly CompanionNoteRecurrence[] = ["none", "daily", "weekly", "monthly", "yearly"];

export const defaultWindowHours = "24";

const hourMs = 60 * 60 * 1000;

export interface NoteDraft {
  label: string;
  content: string;
  availableAt: Date;
  expiresAt: Date | null;
  recurrence: CompanionNoteRecurrence;
  windowHours: string;
  enabled: boolean;
}

export function newNoteDraft(now: number): NoteDraft {
  const start = new Date(now);
  start.setSeconds(0, 0);
  return { label: "", content: "", availableAt: start, expiresAt: null, recurrence: "none", windowHours: defaultWindowHours, enabled: true };
}

export function noteToDraft(note: CompanionNoteView): NoteDraft {
  return {
    label: note.label,
    content: note.content,
    availableAt: new Date(note.available_at),
    expiresAt: note.expires_at === null ? null : new Date(note.expires_at),
    recurrence: note.recurrence,
    windowHours: note.recurrence_window_ms ? String(Math.round(note.recurrence_window_ms / hourMs)) : "",
    enabled: note.enabled,
  };
}

export function validWindow(draft: NoteDraft): boolean {
  if (draft.recurrence === "none" || !draft.windowHours.trim()) return true;
  const hours = Number(draft.windowHours);
  return Number.isFinite(hours) && hours > 0;
}

export function noteProblem(draft: NoteDraft): "content" | "end" | "window" | null {
  if (!draft.content.trim()) return "content";
  if (!validWindow(draft)) return "window";
  if (draft.expiresAt && draft.expiresAt.getTime() <= draft.availableAt.getTime()) return "end";
  return null;
}

export function draftToUpsert(characterId: string, noteId: string | null, draft: NoteDraft, operationId: string): CompanionNoteUpsertRequest {
  const hours = Number(draft.windowHours);
  return {
    character_id: characterId,
    note_id: noteId,
    label: draft.label.trim(),
    content: draft.content.trim(),
    available_at: draft.availableAt.getTime(),
    expires_at: draft.expiresAt ? draft.expiresAt.getTime() : null,
    recurrence: draft.recurrence,
    recurrence_window_ms: draft.recurrence !== "none" && Number.isFinite(hours) && hours > 0 ? Math.round(hours * hourMs) : null,
    enabled: draft.enabled,
    client_operation_id: operationId,
  };
}

export function noteToUpsert(note: CompanionNoteView, enabled: boolean, operationId: string): CompanionNoteUpsertRequest {
  return {
    character_id: note.character_id,
    note_id: note.id,
    label: note.label,
    content: note.content,
    available_at: note.available_at,
    expires_at: note.expires_at,
    recurrence: note.recurrence,
    recurrence_window_ms: note.recurrence_window_ms,
    enabled,
    client_operation_id: operationId,
  };
}

export function attemptIds() {
  let last: { payload: string; id: string } | null = null;
  return {
    idFor(payload: unknown): string {
      const key = JSON.stringify(payload);
      if (last?.payload !== key) last = { payload: key, id: crypto.randomUUID() };
      return last.id;
    },
    settled() {
      last = null;
    },
  };
}
