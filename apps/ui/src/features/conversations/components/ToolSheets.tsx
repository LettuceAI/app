import { useState, type ReactNode } from "react";
import { Check, UserX } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Alert, Avatar, Badge, Button, cn, icon, ListGroup, ListRow, ListRowButton, LoadingState, SearchField, Sheet, Switch, Textarea } from "@lettuceai/crisp";
import type { ConversationSettingsView, PersonaSummary } from "@/api/generated/bindings";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { setInlineAuthorNote, useInlineAuthorNote } from "../data/use-inline-author-note";

export interface PersonaPickerProps {
  personas: { items: PersonaSummary[]; defaultId: string | null } | undefined;
  status: "pending" | "error" | "success";
  onRetry: () => void;
  selectedId: string | null;
  query: string;
  onQuery: (query: string) => void;
  onSelect: (id: string | null) => void;
}

function descriptionOf(persona: PersonaSummary): string | null {
  return persona.description.trim() || null;
}

function PersonaRow({ leading, title, detail, badge, selected, onClick }: { leading: ReactNode; title: string; detail?: string | null; badge?: ReactNode; selected: boolean; onClick: () => void }) {
  return (
    <button type="button" aria-pressed={selected} onClick={onClick} className={cn("flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-fill", selected && "bg-fill")}>
      <span className="flex h-10 w-10 shrink-0 items-center justify-center">{leading}</span>
      <span className="min-w-0 flex-1">
        <span className="flex items-center gap-2">
          <span className="truncate text-base font-medium text-fg">{title}</span>
          {badge}
        </span>
        {detail && <span className="mt-0.5 line-clamp-2 text-sm text-fg-3">{detail}</span>}
      </span>
      {selected && <Check size={icon.md} className="shrink-0 text-accent" />}
    </button>
  );
}

export function PersonaPicker({ personas, status, onRetry, selectedId, query, onQuery, onSelect }: PersonaPickerProps) {
  const { t } = useTranslation("conversations");
  const items = personas?.items ?? [];
  const needle = query.trim().toLowerCase();
  const shown = items
    .filter((persona) => !needle || [persona.title, persona.nickname, descriptionOf(persona)].some((text) => text?.toLowerCase().includes(needle)))
    .sort((a, b) => Number(b.id === personas?.defaultId) - Number(a.id === personas?.defaultId) || a.title.localeCompare(b.title));
  if (status === "pending") return <LoadingState />;
  if (status === "error") {
    return <Alert tone="danger" title={t("persona.loadFailed")} action={<Button variant="secondary" size="sm" onClick={onRetry}>{t("persona.retry")}</Button>} />;
  }
  if (items.length === 0) return <Alert tone="warning" title={t("persona.emptyTitle")} description={t("persona.emptyBody")} />;
  return (
    <div className="space-y-3">
      {items.length > 3 && <SearchField value={query} onChange={onQuery} placeholder={t("persona.search")} aria-label={t("persona.search")} />}
      <ListGroup>
        <PersonaRow leading={<UserX size={icon.lg} className="text-fg-3" />} title={t("persona.none")} selected={selectedId === null} onClick={() => onSelect(null)} />
        {shown.map((persona) => {
          const detail = [persona.nickname, descriptionOf(persona)].filter(Boolean).join(" · ");
          return (
            <PersonaRow
              key={persona.id}
              leading={<Avatar src={persona.avatar?.url ?? null} name={persona.title} size="md" />}
              title={persona.title}
              detail={detail || null}
              badge={persona.id === personas?.defaultId ? <Badge tone="info">{t("persona.default")}</Badge> : undefined}
              selected={persona.id === selectedId}
              onClick={() => onSelect(persona.id)}
            />
          );
        })}
      </ListGroup>
      {needle && shown.length === 0 && <p className="py-4 text-center text-sm text-fg-muted">{t("persona.noMatch")}</p>}
    </div>
  );
}

export interface PersonaSheetProps extends Omit<PersonaPickerProps, "query" | "onQuery"> {
  open: boolean;
  onClose: () => void;
}

export function PersonaSheet({ open, onClose, onSelect, ...picker }: PersonaSheetProps) {
  const { t } = useTranslation("conversations");
  const [query, setQuery] = useState("");
  const close = () => {
    setQuery("");
    onClose();
  };
  return (
    <Sheet open={open} onClose={close} size="sm" title={t("persona.title")}>
      <div className="pb-1">
        <PersonaPicker
          {...picker}
          query={query}
          onQuery={setQuery}
          onSelect={(id) => {
            setQuery("");
            onSelect(id);
          }}
        />
      </div>
    </Sheet>
  );
}

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
