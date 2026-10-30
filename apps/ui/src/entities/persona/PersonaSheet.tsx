import { useState, type ReactNode } from "react";
import { Check, UserRound, UserX } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Alert, Avatar, Badge, Button, EmptyState, icon, ListGroup, ListRowButton, LoadingState, SearchField, Sheet } from "@lettuceai/crisp";
import type { PersonaSummary } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";

export interface PersonaPickerProps {
  personas: { items: PersonaSummary[]; defaultId: string | null } | undefined;
  status: "pending" | "error" | "success";
  error?: unknown;
  onRetry: () => void;
  selectedId: string | null;
  query: string;
  onQuery: (query: string) => void;
  onSelect: (id: string | null) => void;
  defaultChoice?: { title: string; description: string; selected: boolean; onSelect: () => void };
}

function PersonaRow({ leading, title, detail, badge, selected, onClick }: {
  leading: ReactNode; title: string; detail?: string | null; badge?: ReactNode; selected: boolean; onClick: () => void;
}) {
  return <ListRowButton
    icon={leading}
    title={<span className="flex min-w-0 items-center gap-2"><span className="truncate">{title}</span>{badge}</span>}
    description={detail ? <span className="line-clamp-2">{detail}</span> : undefined}
    aria-pressed={selected}
    chevron={false}
    trailing={selected ? <Check size={icon.md} className="text-accent" /> : undefined}
    onClick={onClick}
  />;
}

export function PersonaPicker({ personas, status, error, onRetry, selectedId, query, onQuery, onSelect, defaultChoice }: PersonaPickerProps) {
  const { t } = useTranslation("conversations");
  const items = personas?.items ?? [];
  const needle = query.trim().toLocaleLowerCase();
  const shown = items
    .filter((persona) => !needle || [persona.title, persona.nickname, persona.description].some((text) => text?.toLocaleLowerCase().includes(needle)))
    .sort((a, b) => Number(b.id === personas?.defaultId) - Number(a.id === personas?.defaultId) || a.title.localeCompare(b.title));
  if (status === "pending") return <LoadingState />;
  if (status === "error") return error ? <FailureState error={error} onRetry={onRetry} /> : <Alert tone="danger" title={t("persona.loadFailed")} action={<Button variant="secondary" size="sm" onClick={onRetry}>{t("persona.retry")}</Button>} />;
  return <div className="space-y-3">
    {(items.length > 3 || query) && <SearchField value={query} onChange={onQuery} placeholder={t("persona.search")} aria-label={t("persona.search")} />}
    <ListGroup>
      {defaultChoice && <PersonaRow leading={<UserRound size={icon.lg} />} title={defaultChoice.title} detail={defaultChoice.description} selected={defaultChoice.selected} onClick={defaultChoice.onSelect} />}
      <PersonaRow leading={<UserX size={icon.lg} />} title={t("persona.none")} selected={selectedId === null && !defaultChoice?.selected} onClick={() => onSelect(null)} />
      {shown.map((persona) => <PersonaRow key={persona.id} leading={<Avatar src={persona.avatar?.url ?? null} name={persona.title} size="md" />} title={persona.title} detail={[persona.nickname, persona.description.trim()].filter(Boolean).join(" · ")} badge={persona.id === personas?.defaultId ? <Badge tone="info">{t("persona.default")}</Badge> : undefined} selected={persona.id === selectedId} onClick={() => onSelect(persona.id)} />)}
    </ListGroup>
    {items.length === 0 ? <Alert tone="warning" title={t("persona.emptyTitle")} description={t("persona.emptyBody")} /> : needle && shown.length === 0 && <EmptyState size="sm" title={t("persona.noMatch")} />}
  </div>;
}

export interface PersonaSheetProps extends Omit<PersonaPickerProps, "query" | "onQuery"> {
  open: boolean;
  onClose: () => void;
}

export function PersonaSheet({ open, onClose, onSelect, ...picker }: PersonaSheetProps) {
  const { t } = useTranslation("conversations");
  const [query, setQuery] = useState("");
  const [wasOpen, setWasOpen] = useState(open);
  if (open !== wasOpen) { setWasOpen(open); if (open) setQuery(""); }
  const close = () => { setQuery(""); onClose(); };
  return <Sheet open={open} onClose={close} size="sm" title={t("persona.title")}>
    <div className="pb-1"><PersonaPicker {...picker} query={query} onQuery={setQuery} onSelect={(id) => { setQuery(""); onSelect(id); }} /></div>
  </Sheet>;
}
