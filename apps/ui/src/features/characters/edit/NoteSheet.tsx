import { useState, type ReactNode } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, confirm, DatePicker, Field, icon, Input, ListGroup, ListRow, Select, Sheet, Switch, Textarea, toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { CharacterNoteRecurrence, CharacterNoteView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { attemptIds, defaultWindowHours, draftToUpsert, newNoteDraft, noteProblem, noteToDraft, recurrences, type NoteDraft } from "../data/notes";

const endOfDay = (date: Date) => {
  const next = new Date(date);
  next.setHours(23, 59, 0, 0);
  return next;
};

const pickedDayOnly = (date: Date) => date.getHours() === 0 && date.getMinutes() === 0;

export interface NoteSheetCopy {
  contentHint: string;
  contentPlaceholder: string;
  deleteBody: (label: string) => string;
}

export function NoteSheetShell({ open, isNew, draft, onChange, busy, saving, onSave, onClose, onDelete, extra, copy }: {
  open: boolean;
  isNew: boolean;
  draft: NoteDraft;
  onChange: (patch: Partial<NoteDraft>) => void;
  busy: boolean;
  saving: boolean;
  onSave: () => void;
  onClose: () => void;
  onDelete?: (() => void) | undefined;
  extra?: ReactNode;
  copy?: NoteSheetCopy;
}) {
  const { t } = useTranslation("characters");
  const problem = noteProblem(draft);
  const askDelete = async () => {
    if (!onDelete) return;
    const label = draft.label.trim() || t("notes.untitled");
    const sure = await confirm({ title: t("notes.deleteTitle"), message: copy ? copy.deleteBody(label) : t("notes.deleteBody", { label }), confirmLabel: t("notes.delete"), cancelLabel: t("notes.cancel"), destructive: true });
    if (sure) onDelete();
  };
  const recurring = draft.recurrence !== "none";

  return (
    <Sheet
      open={open}
      onClose={() => { if (!busy) onClose(); }}
      size="md"
      title={t(isNew ? "notes.newTitle" : "notes.editTitle")}
      footer={<div className="flex gap-2">
        <Button variant="secondary" className="flex-1" disabled={busy} onClick={onClose}>{t("notes.cancel")}</Button>
        <Button variant="primary" className="flex-1" loading={saving} disabled={busy || problem !== null} onClick={onSave}>{t("notes.save")}</Button>
      </div>}
    >
      <div className="space-y-5 pb-1">
        <Field label={t("notes.label")}>
          <Input value={draft.label} placeholder={t("notes.labelPlaceholder")} disabled={busy} onChange={(event) => onChange({ label: event.target.value })} />
        </Field>
        <Field label={t("notes.content")} required hint={copy?.contentHint ?? t("notes.contentHint")}>
          <Textarea rows={4} autoGrow maxRows={10} value={draft.content} placeholder={copy?.contentPlaceholder ?? t("notes.contentPlaceholder")} disabled={busy} onChange={(event) => onChange({ content: event.target.value })} />
        </Field>
        {extra}
        <Field label={t("notes.repeat")}>
          <Select value={draft.recurrence} disabled={busy} options={recurrences.map((value) => ({ value, label: t(`notes.recurrence.${value}`) }))} onChange={(event) => onChange({ recurrence: event.target.value as CharacterNoteRecurrence })} />
        </Field>
        <Field label={t(draft.recurrence === "none" ? "notes.showOn" : "notes.startsOn")}>
          <DatePicker withTime value={draft.availableAt} disabled={busy} onChange={(availableAt) => onChange({ availableAt })} />
        </Field>
        <Field label={t(recurring ? "notes.stopsAfter" : "notes.hideOn")} hint={problem === "end" ? t("notes.endBeforeStart") : undefined}>
          <div className="flex items-center gap-2">
            <div className="min-w-0 flex-1">
              <DatePicker withTime value={draft.expiresAt} placeholder={t("notes.noEnd")} disabled={busy} onChange={(date) => onChange({ expiresAt: draft.expiresAt === null && recurring && pickedDayOnly(date) ? endOfDay(date) : date })} />
            </div>
            {draft.expiresAt && <Button variant="ghost" size="sm" disabled={busy} onClick={() => onChange({ expiresAt: null })}>{t("notes.clearEnd")}</Button>}
          </div>
        </Field>
        {recurring && <Field label={t("notes.window")} hint={problem === "window" ? t("notes.windowInvalid") : t("notes.windowHint")}>
          <Input type="number" min={1} step={1} inputMode="numeric" value={draft.windowHours} placeholder={defaultWindowHours} disabled={busy} onChange={(event) => onChange({ windowHours: event.target.value })} />
        </Field>}
        <ListGroup>
          <ListRow title={t("notes.enabled")} description={t("notes.enabledHint")} trailing={<Switch checked={draft.enabled} disabled={busy} onChange={(enabled) => onChange({ enabled })} aria-label={t("notes.enabled")} />} />
        </ListGroup>
        {onDelete && <Button variant="danger" block disabled={busy} leading={<Trash2 size={icon.md} />} onClick={() => void askDelete()}>{t("notes.delete")}</Button>}
      </div>
    </Sheet>
  );
}

export function NoteSheet({ api, characterId, note, open, onClose }: { api: ApiClient; characterId: string; note: CharacterNoteView | null; open: boolean; onClose: () => void }) {
  const { t } = useTranslation("characters");
  const { t: common } = useTranslation("common");
  const queryClient = useQueryClient();
  const [draft, setDraft] = useState(() => (note ? noteToDraft(note) : newNoteDraft(Date.now())));
  const [ids] = useState(attemptIds);
  const [deleteId] = useState(() => crypto.randomUUID());
  const set = (patch: Partial<typeof draft>) => setDraft((current) => ({ ...current, ...patch }));
  const finish = async () => {
    await queryClient.invalidateQueries({ queryKey: queryKeys.characterNotes.list(characterId) });
    onClose();
  };
  const fail = (error: unknown) => toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
  const save = useMutation({
    mutationFn: async () => {
      const { client_operation_id: _ignored, ...payload } = draftToUpsert(characterId, note?.id ?? null, draft, "");
      void _ignored;
      await api.call("characterNotesUpsert", { ...payload, client_operation_id: ids.idFor(payload) });
    },
    onSuccess: async () => {
      ids.settled();
      toast.success(t("notes.saved"));
      await finish();
    },
    onError: fail,
  });
  const remove = useMutation({
    mutationFn: () => api.call("characterNotesDelete", { note_id: note!.id, client_operation_id: deleteId }),
    onSuccess: async () => {
      toast.success(t("notes.deleted"));
      await finish();
    },
    onError: fail,
  });
  return <NoteSheetShell open={open} isNew={!note} draft={draft} onChange={set} busy={save.isPending || remove.isPending} saving={save.isPending} onSave={() => save.mutate()} onClose={onClose} onDelete={note ? () => remove.mutate() : undefined} />;
}
