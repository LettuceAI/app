import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge, Button, Chip, confirm, icon, Input, ListGroup, ListRow, ListRowButton, LoadingState, Segmented, Switch, Textarea } from "@lettuceai/crisp";
import type { CharacterNoteRecurrence, CharacterNoteView, GroupNoteView } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { deleteGroupNote, deleteNote, saveGroupNote, saveNote } from "../../data/chat-actions";
import { groupNoteMembersQuery, groupNotesQuery, notesQuery } from "../../data/settings-queries";
import type { PanelCtx } from "./context";
import { ControlRow } from "./SettingRow";

type AnyNote = CharacterNoteView | GroupNoteView;

const recurrences: readonly CharacterNoteRecurrence[] = ["none", "daily", "weekly", "monthly", "yearly"];
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
  recurrence: CharacterNoteRecurrence;
  windowHours: string;
  enabled: boolean;
  visibleTo: string[];
}

function draftOf(note: AnyNote | null): Draft {
  if (!note) return { id: null, label: "", content: "", availableAt: toLocalInput(Date.now()), expiresAt: "", recurrence: "none", windowHours: "", enabled: true, visibleTo: [] };
  return {
    visibleTo: "visible_to" in note ? note.visible_to : [],
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

interface Owner {
  characterId: string | null;
  groupId: string | null;
}

function NoteForm({ ctx, owner, initial, onDone }: { ctx: PanelCtx; owner: Owner; initial: AnyNote | null; onDone: () => void }) {
  const { t } = useTranslation("conversations");
  const [draft, setDraft] = useState(() => draftOf(initial));
  const members = useQuery(groupNoteMembersQuery(ctx.api, owner.groupId));
  const patch = (change: Partial<Draft>) => setDraft((current) => ({ ...current, ...change }));
  const available = fromLocalInput(draft.availableAt);
  const expires = draft.expiresAt ? fromLocalInput(draft.expiresAt) : null;
  const hours = Number(draft.windowHours);
  const valid = draft.content.trim().length > 0 && available !== null && (draft.expiresAt === "" || expires !== null);

  const save = async () => {
    if (available === null) return;
    const body = {
      note_id: draft.id,
      label: draft.label.trim(),
      content: draft.content.trim(),
      available_at: available,
      expires_at: expires,
      recurrence: draft.recurrence,
      recurrence_window_ms: draft.recurrence !== "none" && hours > 0 ? Math.round(hours * hour) : null,
      enabled: draft.enabled,
    };
    const done = await ctx.run(() =>
      (owner.groupId
        ? saveGroupNote(ctx.api, ctx.queryClient, { ...body, group_id: owner.groupId, visible_to: draft.visibleTo })
        : saveNote(ctx.api, ctx.queryClient, { ...body, character_id: owner.characterId ?? "" })
      ).then(() => true),
    );
    if (done) onDone();
  };
  const remove = async () => {
    if (!draft.id) return;
    const sure = await confirm({ title: t("settings.notes.deleteTitle"), message: t("settings.notes.deleteBody"), confirmLabel: t("settings.delete.confirm"), cancelLabel: t("confirm.cancel"), destructive: true });
    if (!sure) return;
    const done = await ctx.run(() => (owner.groupId ? deleteGroupNote(ctx.api, ctx.queryClient, owner.groupId, draft.id ?? "") : deleteNote(ctx.api, ctx.queryClient, owner.characterId ?? "", draft.id ?? "")).then(() => true));
    if (done) onDone();
  };
  return (
    <div className="space-y-4 pb-1">
      <Input value={draft.label} onChange={(event) => patch({ label: event.target.value })} placeholder={t("settings.notes.labelPlaceholder")} aria-label={t("settings.notes.label")} />
      <Textarea rows={5} value={draft.content} onChange={(event) => patch({ content: event.target.value })} placeholder={t(owner.groupId ? "settings.notes.groupContentPlaceholder" : "settings.notes.contentPlaceholder")} aria-label={t("settings.notes.content")} />
      {owner.groupId && (
        <div className="space-y-1">
          <span className="text-sm text-fg-2">{t("settings.notes.whoSees")}</span>
          <div className="flex flex-wrap gap-2" role="group" aria-label={t("settings.notes.whoSees")}>
            {members.data?.map((member) => (
              <Chip key={member.id} selected={draft.visibleTo.includes(member.id)} onClick={() => patch({ visibleTo: draft.visibleTo.includes(member.id) ? draft.visibleTo.filter((id) => id !== member.id) : [...draft.visibleTo, member.id] })}>{member.name}</Chip>
            ))}
          </div>
          <p className="text-xs text-fg-3">{t(draft.visibleTo.length === 0 ? "settings.notes.everyone" : "settings.notes.whoSeesHint")}</p>
        </div>
      )}
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
  const groupId = ctx.view.source.type === "group" ? ctx.view.source.group_id : null;
  const characterNotes = useQuery(notesQuery(ctx.api, groupId ? null : ctx.characterId));
  const groupNotes = useQuery(groupNotesQuery(ctx.api, groupId));
  const notes = groupId ? groupNotes : characterNotes;
  const [editing, setEditing] = useState<AnyNote | "new" | null>(null);
  const owner: Owner = { characterId: ctx.characterId, groupId };
  if (!groupId && !ctx.characterId) return null;
  if (notes.isPending) return <LoadingState />;
  if (notes.isError) return <FailureState error={notes.error} onRetry={() => void notes.refetch()} />;
  if (editing) return <NoteForm ctx={ctx} owner={owner} initial={editing === "new" ? null : editing} onDone={() => setEditing(null)} />;
  const toggle = (note: AnyNote, enabled: boolean) =>
    void ctx.run(() =>
      "visible_to" in note
        ? saveGroupNote(ctx.api, ctx.queryClient, {
            group_id: note.group_id, note_id: note.id, visible_to: note.visible_to, label: note.label, content: note.content, available_at: note.available_at,
            expires_at: note.expires_at, recurrence: note.recurrence, recurrence_window_ms: note.recurrence_window_ms, enabled,
          })
        : saveNote(ctx.api, ctx.queryClient, {
            character_id: note.character_id, note_id: note.id, label: note.label, content: note.content, available_at: note.available_at,
            expires_at: note.expires_at, recurrence: note.recurrence, recurrence_window_ms: note.recurrence_window_ms, enabled,
          }),
    );
  return (
    <div className="space-y-3 pb-1">
      <p className="text-sm text-fg-muted">{t(groupId ? "settings.notes.groupHint" : "settings.notes.hint")}</p>
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
              <Switch className="mr-3 shrink-0" checked={note.enabled} disabled={ctx.busy} aria-label={t("settings.notes.enabled")} onChange={(enabled) => toggle(note, enabled)} />
            </div>
          ))}
        </ListGroup>
      )}
      <Button variant="secondary" leading={<Plus size={icon.md} />} onClick={() => setEditing("new")}>{t("settings.notes.add")}</Button>
    </div>
  );
}
