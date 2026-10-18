import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CalendarClock, Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, icon, ListGroup, ListRow, ListRowButton, Skeleton, Switch, toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { CompanionNoteView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { attemptIds, noteToUpsert } from "../data/notes";
import { companionNotesQuery } from "../data/queries";
import { NoteSheet } from "./NoteSheet";

export function NotesGroup({ api, characterId }: { api: ApiClient; characterId: string }) {
  const { t, i18n } = useTranslation("characters");
  const { t: common } = useTranslation("common");
  const queryClient = useQueryClient();
  const notes = useQuery(companionNotesQuery(api, characterId));
  const [editing, setEditing] = useState<{ note: CompanionNoteView | null; key: number } | null>(null);
  const [ids] = useState(attemptIds);
  const [now] = useState(Date.now);
  const key = queryKeys.companionNotes.list(characterId);
  const toggle = useMutation({
    mutationFn: ({ note, enabled }: { note: CompanionNoteView; enabled: boolean }) => {
      const { client_operation_id: _ignored, ...payload } = noteToUpsert(note, enabled, "");
      void _ignored;
      return api.call("companionNotesUpsert", { ...payload, client_operation_id: ids.idFor(payload) });
    },
    onMutate: async ({ note, enabled }) => {
      await queryClient.cancelQueries({ queryKey: key });
      const previous = queryClient.getQueryData<CompanionNoteView[]>(key);
      queryClient.setQueryData<CompanionNoteView[]>(key, (current) => current?.map((entry) => (entry.id === note.id ? { ...entry, enabled } : entry)));
      return { previous };
    },
    onError: (error, _variables, context) => {
      queryClient.setQueryData(key, context?.previous);
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    },
    onSuccess: () => ids.settled(),
    onSettled: () => queryClient.invalidateQueries({ queryKey: key }),
  });
  const formatter = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" });
  const when = (note: CompanionNoteView) => [
    formatter.format(note.available_at),
    note.recurrence === "none" ? null : t(`notes.recurrence.${note.recurrence}`),
    note.expires_at !== null && note.expires_at <= now ? t("notes.ended") : null,
  ].filter(Boolean).join(" · ");

  return <>
    <ListGroup label={t("notes.title")}>
      {notes.isPending && <ListRow icon={<CalendarClock size={icon.lg} />} title={<Skeleton className="h-4 w-32" />} />}
      {notes.isError && <ListRowButton title={common(`errors.code.${isApiFailure(notes.error) ? notes.error.code : "internal"}`)} chevron={false} trailing={<span className="text-sm text-fg-3">{common("actions.retry")}</span>} onClick={() => void notes.refetch()} />}
      {notes.data?.map((note) => <ListRow
        key={note.id}
        icon={<CalendarClock size={icon.lg} />}
        title={note.label || t("notes.untitled")}
        description={when(note)}
        trailing={<div className="flex items-center gap-2">
          <Button size="sm" variant="ghost" onClick={() => setEditing({ note, key: Date.now() })}>{t("notes.edit")}</Button>
          <Switch checked={note.enabled} onChange={(enabled) => toggle.mutate({ note, enabled })} aria-label={t("notes.enabledFor", { label: note.label || t("notes.untitled") })} />
        </div>}
      />)}
      {notes.isSuccess && <ListRowButton icon={<Plus size={icon.lg} />} title={t("notes.add")} description={t("notes.addBody")} onClick={() => setEditing({ note: null, key: Date.now() })} />}
    </ListGroup>
    {editing && <NoteSheet key={editing.key} api={api} characterId={characterId} note={editing.note} open onClose={() => setEditing(null)} />}
  </>;
}
