import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useRouteContext } from "@tanstack/react-router";
import { Sparkles } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge, Button, cn, Collapsible, confirm, Field, icon, Meter, Sheet, Spinner, Textarea } from "@lettuceai/crisp";
import type { MemorySummaryPartsView, MemorySummaryPartView, MemorySummaryView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { budgetTokens, joinSummaryParts, nextMerge, summarizedUpTo } from "./summary-parts";

export function SummaryCard({ summary, parts = null, onOpen, companion = false }: { summary: MemorySummaryView | null; parts?: MemorySummaryPartsView | null; onOpen: () => void; companion?: boolean }) {
  const { t } = useTranslation("conversations");
  const text = summary?.text ?? "";
  const tokens = summary?.token_count ?? null;
  const shown = parts?.parts ?? [];
  const chapters = shown.filter((part) => partTier(part) === "chapter").length;
  const arcs = shown.filter((part) => partTier(part) === "arc").length;
  return (
    <button type="button" onClick={onOpen} className="group block w-full rounded-xl border border-line bg-surface-1 p-4 text-left transition-colors hover:bg-fill">
      <div className="flex items-center justify-between gap-3">
        <span className="inline-flex items-center gap-2 text-base font-medium text-fg">
          <Sparkles size={icon.md} className="text-fg-3" />
          {t("memory.summary.title")}
        </span>
        {tokens !== null && <span className="text-sm text-fg-3">{t("memory.tokens", { count: tokens })}</span>}
      </div>
      {shown.length > 0 && (
        <p className="mt-1 text-xs text-fg-3">
          {[t("memory.summary.parts.count", { count: shown.length }), ...(chapters > 0 ? [t("memory.summary.parts.chapters", { count: chapters })] : []), ...(arcs > 0 ? [t("memory.summary.parts.arcs", { count: arcs })] : [])].join(" · ")}
        </p>
      )}
      <p className={cn("mt-2 text-sm leading-relaxed", companion ? "line-clamp-4" : "line-clamp-2 @4xl:line-clamp-5", text ? "text-fg-2" : "text-fg-3")}>{text || t("memory.summary.empty")}</p>
    </button>
  );
}

export interface SummarySheetProps {
  open: boolean;
  text: string;
  parts?: MemorySummaryPartsView | null;
  conversationId?: string;
  dynamic: boolean;
  busy: boolean;
  onClose: () => void;
  onSave: (text: string, close: boolean) => void;
  onClear: () => void;
}

export function SummarySheet({ open, text, parts = null, conversationId, dynamic, busy, onClose, onSave, onClear }: SummarySheetProps) {
  const { t } = useTranslation("conversations");
  const chapters = dynamic && parts !== null;
  const [mode, setMode] = useState<"parts" | "text">(chapters ? "parts" : "text");
  const [draft, setDraft] = useState(text);
  const trimmed = draft.trim();
  const showParts = chapters && mode === "parts";
  const switchMode = () => {
    setDraft(text);
    setMode(showParts ? "text" : "parts");
  };
  return (
    <Sheet
      open={open}
      onClose={() => {
        if (!busy) onClose();
      }}
      placement="auto"
      size="md"
      title={t("memory.summary.title")}
      description={t(chapters ? "memory.summary.parts.description" : "memory.summary.description")}
      footer={
        <div className="flex items-center gap-2">
          <Button variant="ghost" disabled={busy || text === ""} onClick={onClear}>
            {t("memory.summary.clear")}
          </Button>
          {chapters && (
            <Button variant="ghost" disabled={busy} onClick={switchMode}>
              {t(showParts ? "memory.summary.parts.asText" : "memory.summary.parts.asParts")}
            </Button>
          )}
          <span className="flex-1" />
          <Button variant="ghost" disabled={busy} onClick={onClose}>
            {t(showParts ? "memory.summary.parts.done" : "memory.summary.cancel")}
          </Button>
          {!showParts && (
            <Button loading={busy} disabled={busy || trimmed === "" || trimmed === text} onClick={() => onSave(trimmed, true)}>
              {t("memory.summary.save")}
            </Button>
          )}
        </div>
      }
    >
      {showParts ? (
        <PartsEditor view={parts} conversationId={conversationId} busy={busy} onSave={(next) => (next === "" ? onClear() : onSave(next, false))} onWrite={switchMode} />
      ) : (
        <div className="pb-2">
          <Field label={t("memory.summary.field")} {...(dynamic ? { hint: t(chapters ? "memory.summary.parts.textHint" : "memory.summary.dynamicHint") } : {})}>
            <Textarea value={draft} disabled={busy} rows={7} onChange={(event) => setDraft(event.target.value)} />
          </Field>
        </div>
      )}
    </Sheet>
  );
}

type PartTier = "seed" | "cycle" | "chapter" | "arc";

const tierTone: Record<PartTier, string> = {
  seed: "bg-fg-4",
  arc: "bg-accent",
  chapter: "bg-accent/60",
  cycle: "bg-accent/30",
};

function partTier(part: MemorySummaryPartView): PartTier {
  if (part.kind !== "chapter") return part.kind;
  return part.level >= 2 ? "arc" : "chapter";
}

function partTone(part: MemorySummaryPartView): string {
  return part.pending ? "bg-info" : tierTone[partTier(part)];
}

function Coverage({ view, onPick }: { view: MemorySummaryPartsView; onPick: (index: number) => void }) {
  const { t } = useTranslation("conversations");
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
          const label = part.kind === "seed" ? t("memory.summary.parts.seed") : t("memory.window", { from: part.message_start, to: part.message_end });
          return (
            <button
              key={index}
              type="button"
              title={label}
              aria-label={label}
              className={cn("h-full min-w-1 transition-opacity hover:opacity-80", partTone(part))}
              style={{ flex: `${span} 1 0` }}
              onClick={() => onPick(index)}
            />
          );
        })}
        {rest > 0 && <span className="h-full min-w-1 bg-fill-2" style={{ flex: `${rest} 1 0` }} />}
      </div>
      <div className="flex justify-between text-xs text-fg-3">
        <span>{0}</span>
        <span>{t("memory.summary.parts.messages", { count: total })}</span>
      </div>
      <ul className="flex flex-wrap gap-x-3 gap-y-1 text-xs text-fg-3">
        {legend.map((entry) => (
          <li key={entry.key} className="inline-flex items-center gap-1.5">
            <span className={cn("size-2 rounded-full", entry.tone)} />
            {t(`memory.summary.parts.legend.${entry.key}`)}
          </li>
        ))}
      </ul>
    </div>
  );
}

function PartsEditor({ view, conversationId, busy, onSave, onWrite }: { view: MemorySummaryPartsView; conversationId: string | undefined; busy: boolean; onSave: (text: string) => void; onWrite: () => void }) {
  const { t } = useTranslation("conversations");
  const [editing, setEditing] = useState<number | null>(null);
  const [draft, setDraft] = useState("");
  const [expanded, setExpanded] = useState<ReadonlySet<number>>(new Set());
  const { parts } = view;
  const used = budgetTokens(parts);
  const merging = new Set(nextMerge(parts, view.chapter_size));
  const toggle = (index: number, open?: boolean) =>
    setExpanded((current) => {
      const next = new Set(current);
      if (open ?? !next.has(index)) next.add(index);
      else next.delete(index);
      return next;
    });
  const pick = (index: number) => {
    toggle(index, true);
    document.getElementById(`summary-part-${index}`)?.scrollIntoView?.({ block: "nearest", behavior: "smooth" });
  };
  const partTitle = usePartTitle();
  const [sourcesOpen, setSourcesOpen] = useState<ReadonlySet<number>>(new Set());
  const toggleSources = (index: number) =>
    setSourcesOpen((current) => {
      const next = new Set(current);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });

  const replace = (index: number, next: string | null) => joinSummaryParts(parts.flatMap((part, position) => (position !== index ? [part.text] : next === null ? [] : [next])));

  const remove = async (index: number) => {
    const sure = await confirm({
      title: t("memory.summary.parts.deleteTitle"),
      message: t("memory.summary.parts.deleteBody"),
      confirmLabel: t("memory.summary.parts.deleteConfirm"),
      destructive: true,
    });
    if (sure) onSave(replace(index, null));
  };

  if (parts.length === 0) {
    return (
      <div className="space-y-3 pb-2">
        <p className="text-sm text-fg-2">{t("memory.summary.parts.empty")}</p>
        <Button variant="secondary" disabled={busy} onClick={onWrite}>
          {t("memory.summary.parts.writeStart")}
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-4 pb-2">
      <p className="text-sm text-fg-2">{t("memory.summary.parts.lead")}</p>
      <Coverage view={view} onPick={pick} />
      <div className="space-y-1">
        <Meter size="sm" value={used} max={view.token_budget} thresholds={{ warning: 1, danger: 1.0001 }} label={t("memory.summary.parts.budget")} valueLabel={t("memory.summary.parts.budgetValue", { used, budget: view.token_budget })} />
        {used > view.token_budget ? (
          <p className="text-xs text-fg-3">{t("memory.summary.parts.overBudget")}</p>
        ) : merging.size > 0 ? (
          <p className="text-xs text-fg-3">{t("memory.summary.parts.nextMerge", { count: merging.size, budget: view.token_budget })}</p>
        ) : null}
      </div>
      <Collapsible variant="plain" title={t("memory.summary.parts.how.title")}>
        <ul className="list-disc space-y-1 pl-5 text-sm text-fg-2">
          <li>{t("memory.summary.parts.how.update")}</li>
          <li>{t("memory.summary.parts.how.merge", { budget: view.token_budget, size: view.chapter_size })}</li>
          <li>{t("memory.summary.parts.how.seed")}</li>
          <li>{t("memory.summary.parts.how.edit")}</li>
          <li>{t("memory.summary.parts.how.recent")}</li>
        </ul>
      </Collapsible>
      <ol className="space-y-3" aria-label={t("memory.summary.parts.list")}>
        {parts.map((part, index) => {
          const isEditing = editing === index;
          const changed = draft.trim();
          return (
            <li id={`summary-part-${index}`} key={`${part.kind}:${part.message_start}:${part.message_end}:${index}`} className="flex gap-3 rounded-xl border border-line bg-surface-1 p-3">
              <span aria-hidden className={cn("w-1 shrink-0 rounded-full", partTone(part))} />
              <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                <span className="text-sm font-medium text-fg">{partTitle(part)}</span>
                {part.kind !== "seed" && <span className="text-xs text-fg-3">{t("memory.window", { from: part.message_start, to: part.message_end })}</span>}
                {part.origin === "user" && <Badge tone="accent">{t("memory.summary.parts.edited")}</Badge>}
                {part.pending && <Badge tone="info">{t("memory.summary.parts.pending")}</Badge>}
                {merging.has(index) && <Badge>{t("memory.summary.parts.mergesNext")}</Badge>}
                <span className="flex-1" />
                {part.token_count !== null && <span className="text-xs text-fg-3">{t("memory.tokens", { count: part.token_count })}</span>}
              </div>
              {part.kind === "seed" && <p className="mt-1 text-xs text-fg-3">{t("memory.summary.parts.seedNote")}</p>}
              {isEditing ? (
                <div className="mt-2 space-y-2">
                  <Textarea aria-label={partTitle(part)} value={draft} disabled={busy} rows={5} onChange={(event) => setDraft(event.target.value)} />
                  <div className="flex justify-end gap-2">
                    <Button size="sm" variant="ghost" disabled={busy} onClick={() => setEditing(null)}>
                      {t("memory.summary.cancel")}
                    </Button>
                    <Button
                      size="sm"
                      loading={busy}
                      disabled={busy || changed === "" || changed === part.text.trim()}
                      onClick={() => {
                        onSave(replace(index, changed));
                        setEditing(null);
                      }}
                    >
                      {t("memory.summary.save")}
                    </Button>
                  </div>
                </div>
              ) : (
                <>
                  <p className={cn("mt-2 whitespace-pre-wrap text-sm leading-relaxed text-fg-2", !expanded.has(index) && "line-clamp-3")}>{part.text}</p>
                  <div className="mt-2 flex gap-1">
                    <Button size="sm" variant="ghost" onClick={() => toggle(index)}>
                      {t(expanded.has(index) ? "memory.summary.parts.showLess" : "memory.summary.parts.showMore")}
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={busy || editing !== null}
                      onClick={() => {
                        setDraft(part.text);
                        setEditing(index);
                      }}
                    >
                      {t("memory.summary.parts.edit")}
                    </Button>
                    <Button size="sm" variant="ghost" disabled={busy || editing !== null} onClick={() => void remove(index)}>
                      {t("memory.summary.parts.delete")}
                    </Button>
                    {conversationId && part.id && part.source_count > 0 && (
                      <Button size="sm" variant="ghost" aria-expanded={sourcesOpen.has(index)} onClick={() => toggleSources(index)}>
                        {sourcesOpen.has(index) ? t("memory.summary.parts.sources.hide") : sourcesLabel(t, part)}
                      </Button>
                    )}
                  </div>
                  {conversationId && part.id && sourcesOpen.has(index) && <PartSources conversationId={conversationId} partId={part.id} />}
                </>
              )}
              </div>
            </li>
          );
        })}
      </ol>
    </div>
  );
}

function usePartTitle() {
  const { t } = useTranslation("conversations");
  return (part: MemorySummaryPartView) => {
    if (part.kind === "seed") return t("memory.summary.parts.seed");
    if (part.kind === "cycle") return t("memory.summary.parts.cycle");
    if (part.level <= 1) return t("memory.summary.parts.chapter");
    return part.level === 2 ? t("memory.summary.parts.arc") : t("memory.summary.parts.arcLevel", { level: part.level - 1 });
  };
}

function sourcesLabel(t: ReturnType<typeof useTranslation<"conversations">>["t"], part: MemorySummaryPartView): string {
  return part.origin === "user" || part.kind === "seed" ? t("memory.summary.parts.sources.replaced", { count: part.source_count }) : t("memory.summary.parts.sources.madeFrom", { count: part.source_count });
}

function PartSources({ conversationId, partId }: { conversationId: string; partId: string }) {
  const { t } = useTranslation("conversations");
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const sources = useQuery({
    queryKey: queryKeys.memory.partSources(conversationId, partId),
    queryFn: () => api.call("memorySummaryPartSources", { conversation_id: conversationId, part_id: partId }),
    staleTime: Number.POSITIVE_INFINITY,
  });
  if (sources.isPending) return <Spinner size="sm" className="mt-2" />;
  if (sources.isError) return <p className="mt-2 text-xs text-danger">{t("memory.summary.parts.sources.failed")}</p>;
  return (
    <div className="mt-2 space-y-2 border-l-2 border-line pl-3">
      <p className="text-xs text-fg-3">{t("memory.summary.parts.sources.note")}</p>
      <ol className="space-y-2" aria-label={t("memory.summary.parts.sources.list")}>
        {sources.data.map((source, index) => (
          <SourceItem key={source.id ?? index} conversationId={conversationId} source={source} />
        ))}
      </ol>
    </div>
  );
}

function SourceItem({ conversationId, source }: { conversationId: string; source: MemorySummaryPartView }) {
  const { t } = useTranslation("conversations");
  const partTitle = usePartTitle();
  const [expanded, setExpanded] = useState(false);
  const [open, setOpen] = useState(false);
  return (
    <li className="space-y-1">
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
        <span className={cn("size-2 shrink-0 rounded-full", partTone(source))} aria-hidden />
        <span className="text-sm font-medium text-fg-2">{partTitle(source)}</span>
        {source.kind !== "seed" && <span className="text-xs text-fg-3">{t("memory.window", { from: source.message_start, to: source.message_end })}</span>}
        {source.origin === "user" && <Badge tone="accent">{t("memory.summary.parts.edited")}</Badge>}
        <span className="flex-1" />
        {source.token_count !== null && <span className="text-xs text-fg-3">{t("memory.tokens", { count: source.token_count })}</span>}
      </div>
      <p className={cn("whitespace-pre-wrap text-sm leading-relaxed text-fg-3", !expanded && "line-clamp-3")}>{source.text}</p>
      <div className="flex gap-1">
        <Button size="sm" variant="ghost" onClick={() => setExpanded((value) => !value)}>
          {t(expanded ? "memory.summary.parts.showLess" : "memory.summary.parts.showMore")}
        </Button>
        {source.id && source.source_count > 0 && (
          <Button size="sm" variant="ghost" aria-expanded={open} onClick={() => setOpen((value) => !value)}>
            {open ? t("memory.summary.parts.sources.hide") : sourcesLabel(t, source)}
          </Button>
        )}
      </div>
      {source.id && open && <PartSources conversationId={conversationId} partId={source.id} />}
    </li>
  );
}
