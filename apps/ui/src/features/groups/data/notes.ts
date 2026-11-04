import type { GroupNoteUpsertRequest, GroupNoteView } from "@/api/generated/bindings";
import { draftToUpsert, newNoteDraft, noteToDraft, type NoteDraft } from "@/features/characters";

export interface GroupNoteDraft extends NoteDraft {
  visibleTo: string[];
}

export interface DraftNote {
  key: string;
  draft: GroupNoteDraft;
}

export function newGroupNoteDraft(now: number): GroupNoteDraft {
  return { ...newNoteDraft(now), visibleTo: [] };
}

export function groupNoteToDraft(note: GroupNoteView): GroupNoteDraft {
  return { ...noteToDraft({ ...note, character_id: "" }), visibleTo: note.visible_to };
}

export function groupDraftToUpsert(groupId: string, noteId: string | null, draft: GroupNoteDraft, operationId: string): GroupNoteUpsertRequest {
  const { character_id: _ignored, ...rest } = draftToUpsert("", noteId, draft, operationId);
  void _ignored;
  return { ...rest, group_id: groupId, visible_to: draft.visibleTo };
}

/**
 * The notes to write after a group is created. A note aimed at members who
 * are no longer in the group would reach everyone if its list were emptied,
 * so it is left out instead.
 */
export function notesToCreate(notes: DraftNote[], memberIds: string[]): GroupNoteDraft[] {
  return notes.flatMap(({ draft }) => {
    if (draft.visibleTo.length === 0) return [draft];
    const visibleTo = draft.visibleTo.filter((id) => memberIds.includes(id));
    return visibleTo.length > 0 ? [{ ...draft, visibleTo }] : [];
  });
}
