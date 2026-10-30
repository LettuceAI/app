import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, ListGroup, ListRow, ListRowButton, Sheet, Switch, Textarea } from "@lettuceai/crisp";
import type { ConversationSettingsView } from "@/api/generated/bindings";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { setInlineAuthorNote, useInlineAuthorNote } from "../data/use-inline-author-note";

export interface BackgroundPanelProps {
  settings: ConversationSettingsView | undefined;
  busy: boolean;
  onPick: () => void;
  onReset: () => void;
}

export function BackgroundPanel({ settings, busy, onPick, onReset }: BackgroundPanelProps) {
  const { t } = useTranslation("conversations");
  const background = settings?.background;
  const own = background?.source === "conversation";
  return (
    <div className="space-y-4 pb-1">
      {background?.asset && !background.hidden && (
        <div className="overflow-hidden rounded-xl border border-line">
          <img src={background.asset.url} alt={t("background.current")} className="h-32 w-full object-cover" />
          <p className="border-t border-line px-4 py-3 text-sm text-fg-muted">{own ? t("background.currentOwn") : t("background.currentDefault")}</p>
        </div>
      )}
      <ListGroup>
        <ListRowButton title={own ? t("background.replace") : t("background.choose")} description={t("background.chooseHint")} disabled={busy || !settings} onClick={onPick} />
        {own && <ListRowButton title={t("background.useDefault")} description={t("background.useDefaultHint")} chevron={false} disabled={busy} onClick={onReset} />}
      </ListGroup>
    </div>
  );
}

export interface BackgroundSheetProps extends BackgroundPanelProps {
  open: boolean;
  onClose: () => void;
}

export function BackgroundSheet({ open, onClose, ...panel }: BackgroundSheetProps) {
  const { t } = useTranslation("conversations");
  return (
    <Sheet open={open} onClose={onClose} size="sm" title={t("background.title")} dismissible={!panel.busy}>
      <BackgroundPanel {...panel} />
    </Sheet>
  );
}

export interface NoteEditorProps {
  saved: string | null;
  busy: boolean;
  onSave: (note: string) => void;
}

export function NoteEditor({ saved, busy, onSave }: NoteEditorProps) {
  const { t } = useTranslation("conversations");
  const inline = useInlineAuthorNote();
  const [text, setText] = useState(saved ?? "");
  const trimmed = text.trim();
  const current = saved?.trim() ?? "";
  return (
    <div className="space-y-3 pb-1">
      <ListGroup>
        <ListRow title={t("note.inlineTitle")} description={t("note.inlineHint")} trailing={<Switch checked={inline} onChange={setInlineAuthorNote} aria-label={t("note.inlineToggle")} />} />
      </ListGroup>
      <Textarea data-autofocus rows={8} value={text} onChange={(event) => setText(event.target.value)} placeholder={t("note.placeholder")} aria-label={t("note.title")} />
      <p className="text-xs text-fg-muted">{trimmed ? t("note.count", { count: trimmed.length }) : current ? t("note.willRemove") : t("note.none")}</p>
      <div className="flex gap-2">
        {current && <Button variant="secondary" className="flex-1" disabled={busy} onClick={() => onSave("")}>{t("note.clear")}</Button>}
        <Button variant="primary" className="flex-1" disabled={busy || trimmed === current} onClick={() => onSave(trimmed)}>{t("note.save")}</Button>
      </div>
    </div>
  );
}

export interface NoteSheetProps extends NoteEditorProps {
  open: boolean;
  onClose: () => void;
}

export function NoteSheet({ open, onClose, ...editor }: NoteSheetProps) {
  const { t } = useTranslation("conversations");
  const { session } = useClosingValue(open ? true : null);
  return (
    <Sheet open={open} onClose={onClose} size="sm" title={t("note.title")} description={t("note.hint")} dismissible={!editor.busy}>
      <NoteEditor key={session} {...editor} />
    </Sheet>
  );
}
