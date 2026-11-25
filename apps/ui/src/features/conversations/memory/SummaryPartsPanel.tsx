import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useRouteContext } from "@tanstack/react-router";
import { ChevronLeft, Layers } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge, Button, cn, confirm, EmptyState, icon, IconButton, ListGroup, Meter, Sheet, Spinner, Text, Textarea } from "@lettuceai/crisp";
import type { MemorySummaryPartsView, MemorySummaryPartView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { budgetTokens, joinSummaryParts, nextMerge, partTier, summarizedUpTo, type PartTier } from "./summary-parts";

const tierTone: Record<PartTier, string> = {
  seed: "bg-fg-4",
  arc: "bg-accent",
  chapter: "bg-accent/60",
  cycle: "bg-accent/30",
};

function partTone(part: MemorySummaryPartView): string {
  return part.pending ? "bg-info" : tierTone[partTier(part)];
}

function usePartLabels() {
  const { t } = useTranslation("conversations");
  const title = (part: MemorySummaryPartView) => {
    if (part.kind === "seed") return t("memory.summary.parts.seed");
    if (part.kind === "cycle") return t("memory.summary.parts.cycle");
    if (part.level <= 1) return t("memory.summary.parts.chapter");
    return part.level === 2 ? t("memory.summary.parts.arc") : t("memory.summary.parts.arcLevel", { level: part.level - 1 });
  };
  const span = (part: MemorySummaryPartView) => (part.kind === "seed" ? null : t("memory.window", { from: part.message_start, to: part.message_end }));
  const sources = (part: MemorySummaryPartView) =>
    part.origin === "user" || part.kind === "seed" ? t("memory.summary.parts.sources.replaced", { count: part.source_count }) : t("memory.summary.parts.sources.madeFrom", { count: part.source_count });
  return { title, span, sources };
}

export interface SummaryPartsPanelProps {
  view: MemorySummaryPartsView;
  conversationId: string | undefined;
  busy: boolean;
  onSave: (text: string) => void;
  onClear: () => void;
  onWriteText: () => void;
}

export function SummaryPartsPanel({ view, conversationId, busy, onSave, onClear, onWriteText }: SummaryPartsPanelProps) {
  const { t } = useTranslation("conversations");
  const labels = usePartLabels();
  const [selected, setSelected] = useState<number | null>(null);
  const { parts } = view;
  const used = budgetTokens(parts);
  const merging = new Set(nextMerge(parts, view.chapter_size));
  const current = selected === null ? null : (parts[selected] ?? null);

  const replace = (index: number, next: string | null) => joinSummaryParts(parts.flatMap((part, position) => (position !== index ? [part.text] : next === null ? [] : [next])));
  const save = (text: string) => (text === "" ? onClear() : onSave(text));

  if (parts.length === 0) {
    return (
      <EmptyState
        icon={<Layers size={icon.xl} />}
        title={t("memory.summary.parts.emptyTitle")}
        description={t("memory.summary.parts.empty")}
        action={
          <Button variant="secondary" disabled={busy} onClick={onWriteText}>
            {t("memory.summary.parts.writeStart")}
          </Button>
        }
      />
    );
  }

  return (
    <div className="space-y-5">
      <div className="space-y-3">
        <div className="flex flex-wrap items-start gap-x-4 gap-y-2">
          <Text variant="secondary" className="min-w-0 flex-1">
            {t("memory.summary.parts.lead")}
          </Text>
          <div className="flex gap-1">
            <Button size="sm" variant="ghost" disabled={busy} onClick={onWriteText}>
              {t("memory.summary.parts.asText")}
            </Button>
            <Button size="sm" variant="ghost" disabled={busy} onClick={onClear}>
              {t("memory.summary.clear")}
            </Button>
          </div>
        </div>
        <Coverage view={view} onPick={setSelected} />
        <div className="space-y-1.5">
          <Meter className="w-full" value={used} max={view.token_budget} thresholds={{ warning: 1, danger: 1.0001 }} label={t("memory.summary.parts.budget")} valueLabel={t("memory.summary.parts.budgetValue", { used, budget: view.token_budget })} />
          {used > view.token_budget ? (
            <Text variant="hint">{t("memory.summary.parts.overBudget")}</Text>
          ) : merging.size > 0 ? (
            <Text variant="hint">{t("memory.summary.parts.nextMerge", { count: merging.size, budget: view.token_budget })}</Text>
          ) : null}
        </div>
      </div>
      <ListGroup label={t("memory.summary.parts.list")}>
        {parts.map((part, index) => (
          <PartRow key={part.id ?? `pending:${index}`} part={part} mergesNext={merging.has(index)} onOpen={() => setSelected(index)} />
        ))}
      </ListGroup>
      {current && selected !== null && (
        <PartDrawer
          key={current.id ?? `pending:${selected}`}
          part={current}
          conversationId={conversationId}
          busy={busy}
          title={labels.title(current)}
          onClose={() => setSelected(null)}
          onSave={(text) => save(replace(selected, text))}
          onDelete={async () => {
            const sure = await confirm({
              title: t("memory.summary.parts.deleteTitle"),
              message: t("memory.summary.parts.deleteBody"),
              confirmLabel: t("memory.summary.parts.deleteConfirm"),
              destructive: true,
            });
            if (!sure) return;
            setSelected(null);
            save(replace(selected, null));
          }}
        />
      )}
    </div>
  );
}

function Coverage({ view, onPick }: { view: MemorySummaryPartsView; onPick: (index: number) => void }) {
  const { t } = useTranslation("conversations");
  const labels = usePartLabels();
  const end = summarizedUpTo(view.parts);
  const total = Math.max(view.message_count, end);
  const rest = total - end;
  const tiers = new Set(view.parts.map(partTier));
  const legend = [...(["seed", "arc", "chapter", "cycle"] as const).filter((tier) => tiers.has(tier)).map((tier) => ({ key: tier, tone: tierTone[tier] })), ...(rest > 0 ? [{ key: "unsummarized" as const, tone: "bg-fill-2" }] : [])];
  return (
    <div className="space-y-1.5">
      <div className="flex h-3 w-full gap-px overflow-hidden rounded-full bg-fill" role="group" aria-label={t("memory.summary.parts.coverage")}>
        {view.parts.map((part, index) => {
          const span = Math.max(1, part.message_end - (part.kind === "seed" ? 0 : part.message_start));
          const label = labels.span(part) ?? labels.title(part);
          return <button key={index} type="button" title={label} aria-label={label} className={cn("h-full min-w-1 transition-opacity hover:opacity-80", partTone(part))} style={{ flex: `${span} 1 0` }} onClick={() => onPick(index)} />;
        })}
        {rest > 0 && <span className="h-full min-w-1 bg-fill-2" style={{ flex: `${rest} 1 0` }} />}
      </div>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-fg-3">
        {legend.map((entry) => (
          <span key={entry.key} className="inline-flex items-center gap-1.5">
            <span className={cn("size-2 rounded-full", entry.tone)} />
            {t(`memory.summary.parts.legend.${entry.key}`)}
          </span>
        ))}
        <span className="flex-1" />
        <span>{t("memory.summary.parts.messages", { count: total })}</span>
      </div>
    </div>
  );
}

function PartRow({ part, mergesNext = false, onOpen }: { part: MemorySummaryPartView; mergesNext?: boolean; onOpen: () => void }) {
  const { t } = useTranslation("conversations");
  const labels = usePartLabels();
  const span = labels.span(part);
  return (
    <button type="button" onClick={onOpen} className="group flex w-full flex-col gap-1.5 px-4 py-3.5 text-left transition-colors hover:bg-fill">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm">
        <span className={cn("size-2.5 shrink-0 rounded-full", partTone(part))} aria-hidden />
        <span className="font-medium text-fg">{labels.title(part)}</span>
        {span && <span className="text-fg-3">{span}</span>}
        {part.origin === "user" && <Badge tone="accent">{t("memory.summary.parts.edited")}</Badge>}
        {part.pending && <Badge tone="info">{t("memory.summary.parts.pending")}</Badge>}
        {mergesNext && <Badge>{t("memory.summary.parts.mergesNext")}</Badge>}
        <span className="flex-1" />
        {part.token_count !== null && <span className="text-fg-3">{t("memory.tokens", { count: part.token_count })}</span>}
      </div>
      <p className="line-clamp-2 text-sm leading-relaxed text-fg-2">{part.text}</p>
    </button>
  );
}

interface PartDrawerProps {
  part: MemorySummaryPartView;
  conversationId: string | undefined;
  busy: boolean;
  title: string;
  onClose: () => void;
  onSave: (text: string) => void;
  onDelete: () => void;
}

function PartDrawer({ part, conversationId, busy, title, onClose, onSave, onDelete }: PartDrawerProps) {
  const { t } = useTranslation("conversations");
  const labels = usePartLabels();
  const [trail, setTrail] = useState<readonly MemorySummaryPartView[]>([]);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(part.text);
  const shown = trail.at(-1) ?? part;
  const original = trail.length > 0;
  const changed = draft.trim();
  const meta = [labels.span(shown), shown.token_count === null ? null : t("memory.tokens", { count: shown.token_count })].filter((value) => value !== null).join(" · ");

  return (
    <Sheet
      open
      onClose={() => {
        if (!busy) onClose();
      }}
      placement="side"
      size="md"
      title={original ? labels.title(shown) : title}
      description={meta}
      {...(original
        ? {
            leftAction: (
              <IconButton label={t("memory.summary.parts.sources.back")} shape="round" onClick={() => setTrail((path) => path.slice(0, -1))}>
                <ChevronLeft size={icon.md} />
              </IconButton>
            ),
          }
        : {})}
      footer={
        original ? undefined : editing ? (
          <div className="flex items-center justify-end gap-2">
            <Button variant="ghost" disabled={busy} onClick={() => setEditing(false)}>
              {t("memory.summary.cancel")}
            </Button>
            <Button
              loading={busy}
              disabled={busy || changed === "" || changed === part.text.trim()}
              onClick={() => {
                onSave(changed);
                setEditing(false);
              }}
            >
              {t("memory.summary.save")}
            </Button>
          </div>
        ) : (
          <div className="flex items-center gap-2">
            <Button variant="ghost" disabled={busy} onClick={onDelete}>
              {t("memory.summary.parts.delete")}
            </Button>
            <span className="flex-1" />
            <Button
              variant="secondary"
              disabled={busy}
              onClick={() => {
                setDraft(part.text);
                setEditing(true);
              }}
            >
              {t("memory.summary.parts.edit")}
            </Button>
          </div>
        )
      }
    >
      <div className="space-y-5 pb-2">
        {(shown.origin === "user" || shown.pending) && (
          <div className="flex flex-wrap gap-2">
            {shown.origin === "user" && <Badge tone="accent">{t("memory.summary.parts.edited")}</Badge>}
            {shown.pending && <Badge tone="info">{t("memory.summary.parts.pending")}</Badge>}
          </div>
        )}
        {original && <Text variant="hint">{t("memory.summary.parts.sources.readOnly")}</Text>}
        {shown.kind === "seed" && <Text variant="hint">{t("memory.summary.parts.seedNote")}</Text>}
        {editing && !original ? (
          <Textarea aria-label={title} value={draft} disabled={busy} rows={12} onChange={(event) => setDraft(event.target.value)} />
        ) : (
          <p className="whitespace-pre-wrap text-base leading-relaxed text-fg">{shown.text}</p>
        )}
        {conversationId && shown.id && shown.source_count > 0 && !editing && (
          <PartSources key={shown.id} conversationId={conversationId} partId={shown.id} heading={labels.sources(shown)} onOpen={(source) => setTrail((path) => [...path, source])} />
        )}
      </div>
    </Sheet>
  );
}

function PartSources({ conversationId, partId, heading, onOpen }: { conversationId: string; partId: string; heading: string; onOpen: (source: MemorySummaryPartView) => void }) {
  const { t } = useTranslation("conversations");
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const sources = useQuery({
    queryKey: queryKeys.memory.partSources(conversationId, partId),
    queryFn: () => api.call("memorySummaryPartSources", { conversation_id: conversationId, part_id: partId }),
    staleTime: Number.POSITIVE_INFINITY,
  });
  if (sources.isPending) return <Spinner size="sm" />;
  if (sources.isError) return <Text variant="hint">{t("memory.summary.parts.sources.failed")}</Text>;
  return (
    <ListGroup label={heading}>
      {sources.data.map((source, index) => (
        <PartRow key={source.id ?? index} part={source} onOpen={() => onOpen(source)} />
      ))}
    </ListGroup>
  );
}
