import { useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Info } from "lucide-react";
import { Collapsible, icon, ListGroup, ListRow, PartitionBar, Skeleton, Tooltip, type Partition } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { PromptSectionKind } from "@/api/generated/bindings";
import { promptSnapshotQuery, usageQuery } from "../data/queries";

/** One legend line: a colour swatch, the name (with any muted suffix) and the
 *  right-aligned amount. Rendered one per row so every category keeps its line. */
function LegendRow({ color, name, note, value }: { color: string; name: ReactNode; note?: ReactNode; value: string }) {
  return (
    <div className="flex items-center justify-between gap-3 text-sm">
      <span className="flex min-w-0 items-center gap-1.5 text-fg-2">
        <span className="h-2.5 w-2.5 shrink-0 rounded-sm" style={{ backgroundColor: color }} aria-hidden />
        <span className="min-w-0 truncate">{name}</span>
        {note}
      </span>
      <span className="shrink-0 tabular-nums text-fg-3">{value}</span>
    </div>
  );
}

type Category =
  | "system"
  | "character"
  | "persona"
  | "groupCast"
  | "memories"
  | "lorebook"
  | "authorNote"
  | "companion"
  | "history";

/** Which breakdown slice a recorded section belongs to. Scene is part of the
 *  character setup; media and tools ride along the base system scaffolding. */
const categoryOf: Record<PromptSectionKind, Category> = {
  character: "character",
  scene: "character",
  persona: "persona",
  group_cast: "groupCast",
  memories: "memories",
  lorebook: "lorebook",
  author_note: "authorNote",
  companion_state: "companion",
  scheduled_notes: "companion",
  history: "history",
  prompt_entry: "system",
  media: "system",
  tools: "system",
};

/** Distinct, theme-independent colours for the breakdown bar and its legend. */
const categoryColor: Record<Category, string> = {
  system: "#6366f1",
  character: "#ec4899",
  persona: "#14b8a6",
  groupCast: "#f97316",
  memories: "#10b981",
  lorebook: "#f59e0b",
  authorNote: "#84cc16",
  companion: "#8b5cf6",
  history: "#3b82f6",
};

const order: Category[] = [
  "system",
  "character",
  "persona",
  "groupCast",
  "memories",
  "lorebook",
  "authorNote",
  "companion",
  "history",
];

// Context-window bar colours: indigo prompt, red reserved completion, grey free.
const OCCUPIED_COLOR = "#6366f1";
const RESERVED_COLOR = "#ef4444";
const FREE_COLOR = "#9ca3af";

// Only local runtimes reserve the completion budget inside the context window
// (n_ctx / num_ctx = prompt + completion). Remote providers manage the window
// server-side and treat the output cap as a pure response limit — nothing to
// reserve. Mirrors the backend's completion-allowance resolution.
const reservingProviders = new Set(["llamacpp", "llama_cpp", "ollama"]);

function Details({ api, conversationId, messageId }: { api: ApiClient; conversationId: string; messageId: string }) {
  const { t, i18n } = useTranslation("conversations");
  const snapshot = useQuery(promptSnapshotQuery(api, conversationId, messageId));
  const usage = useQuery({ ...usageQuery(api, conversationId, messageId) });
  const number = new Intl.NumberFormat(i18n.language);
  if (snapshot.isPending) {
    return (
      <div className="space-y-3 p-4" aria-hidden="true">
        <Skeleton height={10} width="100%" />
        <Skeleton height={32} width="100%" />
      </div>
    );
  }
  if (snapshot.isError) return <p className="p-4 text-sm text-fg-3">{t("menu.details.unavailable")}</p>;
  const { sections, parameters, model, messages, used_sources } = snapshot.data;
  if (!sections) return <p className="p-4 text-sm text-fg-3">{t("menu.details.noBreakdown")}</p>;

  // The sections carry our own o200k (tiktoken) count per piece — no provider
  // reports a per-section split. It gives the *shares*; the absolute numbers are
  // calibrated onto the real prompt-token count the provider reported (when it
  // did), so the total and the context bar are exact and only the split between
  // categories stays approximate. Falls back to the estimate for replies that
  // never reported usage (e.g. imported chats).
  const estimateTotal = sections.reduce((sum, section) => sum + section.estimated_tokens, 0);
  const reported = usage.data?.input_tokens ?? null;
  const calibrated = reported !== null && reported > 0 && estimateTotal > 0;
  const scale = (estimate: number) => (calibrated ? Math.round((estimate / estimateTotal) * reported) : estimate);
  const promptTotal = calibrated ? reported : estimateTotal;

  // Per-category token totals, largest share first.
  const totals = new Map<Category, number>();
  for (const section of sections) {
    const category = categoryOf[section.kind];
    totals.set(category, (totals.get(category) ?? 0) + section.estimated_tokens);
  }

  // Counts shown as a muted suffix next to the category. History is the number
  // of chat turns carried, not the whole message count (which includes the
  // system scaffolding messages).
  const counts: Partial<Record<Category, number | undefined>> = {
    history: messages.filter((message) => message.role === "user" || message.role === "assistant").length,
    memories: used_sources?.memories.length,
    lorebook: used_sources?.lorebook_entries.length,
  };
  const suffix = (category: Category): string | null => {
    switch (category) {
      case "companion":
        return t("menu.details.companionSuffix");
      case "history":
        return t("menu.details.historySuffix", { count: counts.history ?? 0 });
      case "memories":
        return counts.memories === undefined ? null : t("menu.details.entriesSuffix", { count: counts.memories });
      case "lorebook":
        return counts.lorebook === undefined ? null : t("menu.details.entriesSuffix", { count: counts.lorebook });
      default:
        return null;
    }
  };
  const breakdownItems = order
    .flatMap((category) => {
      const estimate = totals.get(category) ?? 0;
      return estimate > 0
        ? [
            {
              id: category,
              color: categoryColor[category],
              name: t(`menu.details.${category}`),
              note: suffix(category),
              value: scale(estimate),
            },
          ]
        : [];
    })
    .sort((a, b) => b.value - a.value);

  // Context window: local runtimes hold the completion budget inside the window,
  // so it eats into the usable context before generation. Remote providers do
  // not, so the reserved slice is theirs to show only when they reserve.
  const contextLength = parameters.context_length ?? null;
  const reserves = reservingProviders.has(model.provider_kind);
  const reserved = reserves ? parameters.total_completion_allowance ?? 0 : 0;
  const response = Math.min(parameters.max_output_tokens ?? reserved, reserved);
  const reasoning = Math.max(0, reserved - response);
  const occupiedTotal = promptTotal + reserved;
  const free = contextLength === null ? 0 : Math.max(0, contextLength - occupiedTotal);
  const overflow = contextLength === null ? 0 : Math.max(0, occupiedTotal - contextLength);

  const windowSegments: Partition[] = [
    { id: "occupied", label: t("menu.details.occupied"), value: promptTotal, color: OCCUPIED_COLOR },
    ...(reserved > 0 ? [{ id: "reserved", label: t("menu.details.reserved"), value: reserved, color: RESERVED_COLOR }] : []),
    { id: "free", label: t("menu.details.free"), value: free, color: FREE_COLOR },
  ];
  const windowUsage = (value: number) =>
    contextLength === null
      ? ""
      : t("menu.details.usage", {
          used: number.format(value),
          limit: number.format(contextLength),
          percent: Math.round((value / contextLength) * 100),
        });

  // How the reserved budget splits between the visible answer and the reasoning
  // the model hides — on a hover (desktop) or tap (mobile), so a long line never
  // gets clipped in the row.
  const reservedSplit = t("menu.details.reservedSplit", { response: number.format(response), reasoning: number.format(reasoning) });
  const reservedInfo = (
    <Tooltip content={reservedSplit}>
      <button type="button" aria-label={reservedSplit} className="inline-flex shrink-0 text-fg-4 hover:text-fg-2">
        <Info size={icon.sm} aria-hidden />
      </button>
    </Tooltip>
  );

  const entries = sections.filter((section) => section.kind === "lorebook");

  return (
    <div className="space-y-5 p-4">
      {overflow > 0 && (
        <p className="rounded-md border border-danger/40 bg-danger/10 px-3 py-2 text-sm text-danger" role="alert">
          {t("menu.details.overReserved")}
        </p>
      )}
      {contextLength !== null && contextLength > 0 && (
        <div className="space-y-2.5">
          <p className="text-sm font-medium text-fg-2">{t("menu.details.contextWindow")}</p>
          <PartitionBar segments={windowSegments} total={contextLength} labels="none" size="md" aria-label={t("menu.details.contextWindow")} />
          <div className="space-y-1.5">
            <LegendRow color={OCCUPIED_COLOR} name={t("menu.details.occupied")} value={windowUsage(promptTotal)} />
            {reserved > 0 && <LegendRow color={RESERVED_COLOR} name={t("menu.details.reserved")} note={reservedInfo} value={windowUsage(reserved)} />}
            <LegendRow color={FREE_COLOR} name={t("menu.details.free")} value={windowUsage(free)} />
          </div>
        </div>
      )}
      <div className="space-y-2.5">
        <p className="text-sm font-medium text-fg-2">{t("menu.details.breakdown")}</p>
        <PartitionBar
          segments={breakdownItems.map((item) => ({ id: item.id, label: item.name, value: item.value, color: item.color }))}
          total={promptTotal}
          labels="none"
          size="md"
          aria-label={t("menu.details.partsLabel")}
        />
        {!calibrated && <p className="text-xs text-fg-4">{t("menu.details.estimateNote")}</p>}
        <div className="space-y-1.5">
          {breakdownItems.map((item) => {
            const percent = promptTotal > 0 ? Math.round((item.value / promptTotal) * 100) : 0;
            return (
              <LegendRow
                key={item.id}
                color={item.color}
                name={
                  <>
                    {item.name}
                    {item.note ? <span className="text-fg-4"> {item.note}</span> : null}
                  </>
                }
                value={`${number.format(item.value)} · ${percent}%`}
              />
            );
          })}
        </div>
        <div className="flex items-center justify-between border-t border-line pt-2 text-sm">
          <span className="text-fg-2">{t("menu.details.totalPrompt")}</span>
          <span className="tabular-nums text-fg-2">{number.format(promptTotal)}</span>
        </div>
      </div>
      {entries.length > 0 && (
        <ListGroup label={t("menu.details.entries")}>
          {entries.map((entry, index) => (
            <ListRow key={`${entry.label ?? ""}-${index}`} title={entry.label ?? t("menu.details.untitled")} trailing={t("menu.details.tokens", { count: entry.estimated_tokens })} />
          ))}
        </ListGroup>
      )}
    </div>
  );
}

export function SpeakerSelectionReason({ api, conversationId, messageId }: { api: ApiClient; conversationId: string; messageId: string }) {
  const { t } = useTranslation("conversations");
  const snapshot = useQuery(promptSnapshotQuery(api, conversationId, messageId));
  const selection = snapshot.data?.speaker_selection;
  if (!selection || (!selection.reason && !selection.heuristic_fallback)) return null;
  return (
    <ListGroup>
      <ListRow title={t("menu.selection.title")} description={selection.reason ?? t("menu.selection.fallback")} />
      {selection.reason && selection.heuristic_fallback && <p className="px-4 pb-3 text-xs text-fg-3">{t("menu.selection.fallback")}</p>}
    </ListGroup>
  );
}

export function MessagePromptDetails({ api, conversationId, messageId }: { api: ApiClient; conversationId: string; messageId: string }) {
  const { t } = useTranslation("conversations");
  const [open, setOpen] = useState(false);
  return (
    <Collapsible title={t("menu.details.title")} description={t("menu.details.hint")} open={open} onOpenChange={setOpen}>
      {open && <Details api={api} conversationId={conversationId} messageId={messageId} />}
    </Collapsible>
  );
}
