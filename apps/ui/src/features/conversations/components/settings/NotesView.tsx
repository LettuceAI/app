import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge, Button, confirm, icon, Input, ListGroup, ListRow, ListRowButton, LoadingState, Segmented, Switch, Textarea } from "@lettuceai/crisp";
import type { CompanionNoteRecurrence, CompanionNoteView } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { deleteNote, saveNote } from "../../data/chat-actions";
import { notesQuery } from "../../data/settings-queries";
import type { PanelCtx } from "./context";
import { ControlRow } from "./SettingRow";

const recurrences: readonly CompanionNoteRecurrence[] = ["none", "daily", "weekly", "monthly", "yearly"];
const hour = 3_600_000;

function toLocalInput(ms: number): string {
  const date = new Date(ms - new Date(ms).getTimezoneOffset() * 60_000);
  return date.toISOString().slice(0, 16);
}

function fromLocalInput(value: string): number | null {
  const ms = new Date(value).getTime();
  return Number.isNaN(ms) ? null : ms;
}

interface Draft {
  id: string | null;
  label: string;
  content: string;
  availableAt: string;
  expiresAt: string;
  recurrence: CompanionNoteRecurrence;
  windowHours: string;
  enabled: boolean;
}

function draftOf(note: CompanionNoteView | null): Draft {
  if (!note) return { id: null, label: "", content: "", availableAt: toLocalInput(Date.now()), expiresAt: "", recurrence: "none", windowHours: "", enabled: true };
  return {
    id: note.id,
    label: note.label,
    content: note.content,
    availableAt: toLocalInput(note.available_at),
    expiresAt: note.expires_at === null ? "" : toLocalInput(note.expires_at),
    recurrence: note.recurrence,
    windowHours: note.recurrence_window_ms === null ? "" : String(note.recurrence_window_ms / hour),
    enabled: note.enabled,
  };
}

function NoteForm({ ctx, characterId, initial, onDone }: { ctx: PanelCtx; characterId: string; initial: CompanionNoteView | null; onDone: () => void }) {
  const { t } = useTranslation("conversations");
  const [draft, setDraft] = useState(() => draftOf(initial));
  const patch = (change: Partial<Draft>) => setDraft((current) => ({ ...current, ...change }));
  const available = fromLocalInput(draft.availableAt);
  const expires = draft.expiresAt ? fromLocalInput(draft.expiresAt) : null;
  const hours = Number(draft.windowHours);
  const valid = draft.content.trim().length > 0 && available !== null && (draft.expiresAt === "" || expires !== null);

  const save = async () => {
    if (available === null) return;
    const done = await ctx.run(() =>
      saveNote(ctx.api, ctx.queryClient, {
        character_id: characterId,
        note_id: draft.id,
        label: draft.label.trim(),
        content: draft.content.trim(),
        available_at: available,
        expires_at: expires,
        recurrence: draft.recurrence,
        recurrence_window_ms: draft.recurrence !== "none" && hours > 0 ? Math.round(hours * hour) : null,
        enabled: draft.enabled,
      }).then(() => true),
    );
    if (done) onDone();
  };
  const remove = async () => {
    if (!draft.id) return;
    const sure = await confirm({ title: t("settings.notes.deleteTitle"), message: t("settings.notes.deleteBody"), confirmLabel: t("settings.delete.confirm"), cancelLabel: t("confirm.cancel"), destructive: true });
    if (!sure) return;
    const done = await ctx.run(() => deleteNote(ctx.api, ctx.queryClient, characterId, draft.id ?? "").then(() => true));
    if (done) onDone();
  };
  return (
    <div className="space-y-4 pb-1">
      <Input value={draft.label} onChange={(event) => patch({ label: event.target.value })} placeholder={t("settings.notes.labelPlaceholder")} aria-label={t("settings.notes.label")} />
      <Textarea rows={5} value={draft.content} onChange={(event) => patch({ content: event.target.value })} placeholder={t("settings.notes.contentPlaceholder")} aria-label={t("settings.notes.content")} />
      <label className="block space-y-1">
        <span className="text-sm text-fg-2">{t("settings.notes.startsOn")}</span>
        <Input type="datetime-local" value={draft.availableAt} onChange={(event) => patch({ availableAt: event.target.value })} />
      </label>
      <ListGroup>
        <ControlRow
          title={t("settings.notes.repeats")}
          control={
            <Segmented
              size="sm"
              aria-label={t("settings.notes.repeats")}
              value={draft.recurrence}
              onChange={(recurrence) => patch({ recurrence })}
              options={recurrences.map((value) => ({ value, label: t(`settings.notes.recurrence.${value}`) }))}
            />
          }
        />
      </ListGroup>
      {draft.recurrence !== "none" && (
        <label className="block space-y-1">
          <span className="text-sm text-fg-2">{t("settings.notes.windowHours")}</span>
          <Input type="number" min={0} value={draft.windowHours} onChange={(event) => patch({ windowHours: event.target.value })} />
        </label>
      )}
      <label className="block space-y-1">
        <span className="text-sm text-fg-2">{t("settings.notes.stopsOn")}</span>
        <Input type="datetime-local" value={draft.expiresAt} onChange={(event) => patch({ expiresAt: event.target.value })} />
      </label>
      <ListGroup>
        <ListRow title={t("settings.notes.enabled")} description={t("settings.notes.enabledHint")} trailing={<Switch checked={draft.enabled} onChange={(enabled) => patch({ enabled })} aria-label={t("settings.notes.enabled")} />} />
      </ListGroup>
      <div className="flex gap-2">
        {draft.id && <Button variant="secondary" className="flex-1" disabled={ctx.busy} onClick={() => void remove()}>{t("settings.history.delete")}</Button>}
        <Button variant="primary" className="flex-1" disabled={ctx.busy || !valid} onClick={() => void save()}>{t("note.save")}</Button>
      </div>
    </div>
  );
}

export function NotesView({ ctx }: { ctx: PanelCtx }) {
  const { t } = useTranslation("conversations");
  const notes = useQuery(notesQuery(ctx.api, ctx.characterId));
  const [editing, setEditing] = useState<CompanionNoteView | "new" | null>(null);
  if (!ctx.characterId) return null;
  if (notes.isPending) return <LoadingState />;
  if (notes.isError) return <FailureState error={notes.error} onRetry={() => void notes.refetch()} />;
  if (editing) return <NoteForm ctx={ctx} characterId={ctx.characterId} initial={editing === "new" ? null : editing} onDone={() => setEditing(null)} />;
  const characterId = ctx.characterId;
  return (
    <div className="space-y-3 pb-1">
      <p className="text-sm text-fg-muted">{t("settings.notes.hint")}</p>
      {notes.data.length === 0 ? (
        <p className="py-4 text-center text-sm text-fg-muted">{t("settings.notes.empty")}</p>
      ) : (
        <ListGroup>
          {notes.data.map((note) => (
            <div key={note.id} className="flex items-center">
              <ListRowButton
                title={note.label || t("settings.notes.untitled")}
                description={<span className="block truncate">{note.content}</span>}
                trailing={note.recurrence !== "none" ? <Badge tone="neutral">{t(`settings.notes.recurrence.${note.recurrence}`)}</Badge> : undefined}
                className="min-w-0 flex-1"
                onClick={() => setEditing(note)}
              />
              <Switch
                className="mr-3 shrink-0"
                checked={note.enabled}
                disabled={ctx.busy}
                aria-label={t("settings.notes.enabled")}
                onChange={(enabled) =>
                  void ctx.run(() =>
                    saveNote(ctx.api, ctx.queryClient, {
                      character_id: characterId, note_id: note.id, label: note.label, content: note.content, available_at: note.available_at,
                      expires_at: note.expires_at, recurrence: note.recurrence, recurrence_window_ms: note.recurrence_window_ms, enabled,
                    }),
                  )
                }
              />
            </div>
          ))}
        </ListGroup>
      )}
      <Button variant="secondary" leading={<Plus size={icon.md} />} onClick={() => setEditing("new")}>{t("settings.notes.add")}</Button>
    </div>
  );
}
