import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, confirm, DatePicker, Field, icon, Input, ListGroup, ListRow, Select, Sheet, Switch, Textarea, toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { CompanionNoteRecurrence, CompanionNoteView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { attemptIds, defaultWindowHours, draftToUpsert, newNoteDraft, noteProblem, noteToDraft, recurrences } from "../data/notes";

const endOfDay = (date: Date) => {
  const next = new Date(date);
  next.setHours(23, 59, 0, 0);
  return next;
};

const pickedDayOnly = (date: Date) => date.getHours() === 0 && date.getMinutes() === 0;

export function NoteSheet({ api, characterId, note, open, onClose }: { api: ApiClient; characterId: string; note: CompanionNoteView | null; open: boolean; onClose: () => void }) {
  const { t } = useTranslation("characters");
  const { t: common } = useTranslation("common");
  const queryClient = useQueryClient();
  const [draft, setDraft] = useState(() => (note ? noteToDraft(note) : newNoteDraft(Date.now())));
  const [ids] = useState(attemptIds);
  const [deleteId] = useState(() => crypto.randomUUID());
  const problem = noteProblem(draft);
  const set = (patch: Partial<typeof draft>) => setDraft((current) => ({ ...current, ...patch }));
  const finish = async () => {
    await queryClient.invalidateQueries({ queryKey: queryKeys.companionNotes.list(characterId) });
    onClose();
  };
  const fail = (error: unknown) => toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
  const save = useMutation({
    mutationFn: async () => {
      const { client_operation_id: _ignored, ...payload } = draftToUpsert(characterId, note?.id ?? null, draft, "");
      void _ignored;
      await api.call("companionNotesUpsert", { ...payload, client_operation_id: ids.idFor(payload) });
    },
    onSuccess: async () => {
      ids.settled();
      toast.success(t("notes.saved"));
      await finish();
    },
    onError: fail,
  });
  const remove = useMutation({
    mutationFn: () => api.call("companionNotesDelete", { note_id: note!.id, client_operation_id: deleteId }),
    onSuccess: async () => {
      toast.success(t("notes.deleted"));
      await finish();
    },
    onError: fail,
  });
  const busy = save.isPending || remove.isPending;
  const askDelete = async () => {
    if (!note) return;
    const sure = await confirm({ title: t("notes.deleteTitle"), message: t("notes.deleteBody", { label: note.label || t("notes.untitled") }), confirmLabel: t("notes.delete"), cancelLabel: t("notes.cancel"), destructive: true });
    if (sure) remove.mutate();
  };
  const recurring = draft.recurrence !== "none";

  return (
    <Sheet
      open={open}
      onClose={() => { if (!busy) onClose(); }}
      size="md"
      title={t(note ? "notes.editTitle" : "notes.newTitle")}
      footer={<div className="flex gap-2">
        <Button variant="secondary" className="flex-1" disabled={busy} onClick={onClose}>{t("notes.cancel")}</Button>
        <Button variant="primary" className="flex-1" loading={save.isPending} disabled={busy || problem !== null} onClick={() => save.mutate()}>{t("notes.save")}</Button>
      </div>}
    >
      <div className="space-y-5 pb-1">
        <Field label={t("notes.label")}>
          <Input value={draft.label} placeholder={t("notes.labelPlaceholder")} disabled={busy} onChange={(event) => set({ label: event.target.value })} />
        </Field>
        <Field label={t("notes.content")} required hint={t("notes.contentHint")}>
          <Textarea rows={4} autoGrow maxRows={10} value={draft.content} placeholder={t("notes.contentPlaceholder")} disabled={busy} onChange={(event) => set({ content: event.target.value })} />
        </Field>
        <Field label={t("notes.repeat")}>
          <Select value={draft.recurrence} disabled={busy} options={recurrences.map((value) => ({ value, label: t(`notes.recurrence.${value}`) }))} onChange={(event) => set({ recurrence: event.target.value as CompanionNoteRecurrence })} />
        </Field>
        <Field label={t(draft.recurrence === "none" ? "notes.showOn" : "notes.startsOn")}>
          <DatePicker withTime value={draft.availableAt} disabled={busy} onChange={(availableAt) => set({ availableAt })} />
        </Field>
        <Field label={t(recurring ? "notes.stopsAfter" : "notes.hideOn")} hint={problem === "end" ? t("notes.endBeforeStart") : undefined}>
          <div className="flex items-center gap-2">
            <div className="min-w-0 flex-1">
              <DatePicker withTime value={draft.expiresAt} placeholder={t("notes.noEnd")} disabled={busy} onChange={(date) => set({ expiresAt: draft.expiresAt === null && recurring && pickedDayOnly(date) ? endOfDay(date) : date })} />
            </div>
            {draft.expiresAt && <Button variant="ghost" size="sm" disabled={busy} onClick={() => set({ expiresAt: null })}>{t("notes.clearEnd")}</Button>}
          </div>
        </Field>
        {recurring && <Field label={t("notes.window")} hint={problem === "window" ? t("notes.windowInvalid") : t("notes.windowHint")}>
          <Input type="number" min={1} step={1} inputMode="numeric" value={draft.windowHours} placeholder={defaultWindowHours} disabled={busy} onChange={(event) => set({ windowHours: event.target.value })} />
        </Field>}
        <ListGroup>
          <ListRow title={t("notes.enabled")} description={t("notes.enabledHint")} trailing={<Switch checked={draft.enabled} disabled={busy} onChange={(enabled) => set({ enabled })} aria-label={t("notes.enabled")} />} />
        </ListGroup>
        {note && <Button variant="danger" block disabled={busy} leading={<Trash2 size={icon.md} />} onClick={() => void askDelete()}>{t("notes.delete")}</Button>}
      </div>
    </Sheet>
  );
}
