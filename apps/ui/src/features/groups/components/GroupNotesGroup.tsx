import { useState, type ReactNode } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CalendarClock, Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, icon, ListGroup, ListRow, ListRowButton, Skeleton, Switch, toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import { queryKeys } from "@/api/query-keys";
import { attemptIds } from "@/features/characters";
import { groupDraftToUpsert, groupNoteToDraft, type DraftNote, type GroupNoteDraft } from "../data/notes";
import { groupNotesQuery } from "../data/queries";
import { GroupNoteSheet, type NoteMember } from "./GroupNoteSheet";

interface Entry {
  key: string;
  draft: GroupNoteDraft;
  noteId: string | null;
}

export function GroupNotesGroup({ api, groupId, members, draftNotes, onDraftNotes, leading }: {
  api: ApiClient;
  groupId: string | null;
  members: NoteMember[];
  draftNotes: DraftNote[];
  onDraftNotes: (notes: DraftNote[]) => void;
  leading?: ReactNode;
}) {
  const { t, i18n } = useTranslation("groups");
  const { t: characters } = useTranslation("characters");
  const { t: common } = useTranslation("common");
  const queryClient = useQueryClient();
  const notes = useQuery({ ...groupNotesQuery(api, groupId ?? ""), enabled: groupId !== null });
  const [editing, setEditing] = useState<{ entry: Entry | null; key: number } | null>(null);
  const [ids] = useState(attemptIds);
  const [now] = useState(Date.now);
  const key = queryKeys.groupNotes.list(groupId ?? "");
  const saved = groupId !== null;
  const entries: Entry[] = saved
    ? (notes.data ?? []).map((note) => ({ key: note.id, noteId: note.id, draft: groupNoteToDraft(note) }))
    : draftNotes.map((note) => ({ key: note.key, noteId: null, draft: note.draft }));
  const toggle = useMutation({
    mutationFn: ({ entry, enabled }: { entry: Entry; enabled: boolean }) => {
      const { client_operation_id: _ignored, ...payload } = groupDraftToUpsert(groupId ?? "", entry.noteId, { ...entry.draft, enabled }, "");
      void _ignored;
      return api.call("groupNotesUpsert", { ...payload, client_operation_id: ids.idFor(payload) });
    },
    onError: (error) => toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`)),
    onSuccess: () => ids.settled(),
    onSettled: () => queryClient.invalidateQueries({ queryKey: key }),
  });
  const setEnabled = (entry: Entry, enabled: boolean) => {
    if (saved) toggle.mutate({ entry, enabled });
    else onDraftNotes(draftNotes.map((note) => (note.key === entry.key ? { ...note, draft: { ...note.draft, enabled } } : note)));
  };
  const submit = (entry: Entry | null) => async (draft: GroupNoteDraft) => {
    if (groupId !== null) {
      const { client_operation_id: _ignored, ...payload } = groupDraftToUpsert(groupId, entry?.noteId ?? null, draft, "");
      void _ignored;
      await api.call("groupNotesUpsert", { ...payload, client_operation_id: ids.idFor(payload) });
      ids.settled();
      await queryClient.invalidateQueries({ queryKey: key });
      toast.success(characters("notes.saved"));
      return;
    }
    onDraftNotes(entry ? draftNotes.map((note) => (note.key === entry.key ? { ...note, draft } : note)) : [...draftNotes, { key: crypto.randomUUID(), draft }]);
  };
  const remove = (entry: Entry) => async () => {
    if (groupId !== null) {
      await api.call("groupNotesDelete", { note_id: entry.noteId ?? "", client_operation_id: crypto.randomUUID() });
      await queryClient.invalidateQueries({ queryKey: key });
      toast.success(characters("notes.deleted"));
      return;
    }
    onDraftNotes(draftNotes.filter((note) => note.key !== entry.key));
  };
  const formatter = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" });
  const name = (id: string) => members.find((member) => member.id === id)?.name ?? t("unavailable");
  const when = (entry: Entry) => [
    formatter.format(entry.draft.availableAt),
    entry.draft.recurrence === "none" ? null : characters(`notes.recurrence.${entry.draft.recurrence}`),
    entry.draft.expiresAt !== null && entry.draft.expiresAt.getTime() <= now ? characters("notes.ended") : null,
    entry.draft.visibleTo.length ? entry.draft.visibleTo.map(name).join(", ") : t("notes.everyoneShort"),
  ].filter(Boolean).join(" · ");

  return <>
    <ListGroup label={t("notes.title")}>
      {leading}
      {saved && notes.isPending && <ListRow icon={<CalendarClock size={icon.lg} />} title={<Skeleton className="h-4 w-32" />} />}
      {saved && notes.isError && <ListRowButton title={common(`errors.code.${isApiFailure(notes.error) ? notes.error.code : "internal"}`)} chevron={false} trailing={<span className="text-sm text-fg-3">{common("actions.retry")}</span>} onClick={() => void notes.refetch()} />}
      {entries.map((entry) => <ListRow
        key={entry.key}
        icon={<CalendarClock size={icon.lg} />}
        title={entry.draft.label || characters("notes.untitled")}
        description={when(entry)}
        trailing={<div className="flex items-center gap-2">
          <Button size="sm" variant="ghost" onClick={() => setEditing({ entry, key: Date.now() })}>{characters("notes.edit")}</Button>
          <Switch checked={entry.draft.enabled} onChange={(enabled) => setEnabled(entry, enabled)} aria-label={characters("notes.enabledFor", { label: entry.draft.label || characters("notes.untitled") })} />
        </div>}
      />)}
      {(!saved || notes.isSuccess) && <ListRowButton icon={<Plus size={icon.lg} />} title={characters("notes.add")} description={t("notes.addBody")} onClick={() => setEditing({ entry: null, key: Date.now() })} />}
    </ListGroup>
    {editing && <GroupNoteSheet key={editing.key} initial={editing.entry?.draft ?? null} members={members} open onClose={() => setEditing(null)} onSubmit={submit(editing.entry)} onRemove={editing.entry ? remove(editing.entry) : undefined} />}
  </>;
}
