import { useEffect, useMemo, useState } from "react";

import {
  getMessageDebugSnapshot,
  type ChatMessageDebugSnapshot,
} from "../../../../core/chat/manager";
import { useI18n, type TranslationKey } from "../../../../core/i18n/context";
import { countTokensBatch } from "../../../../core/tokens";
import type { StoredMessage } from "../../../../core/storage/schemas";

type PromptEntryLike = {
  id?: unknown;
  name?: unknown;
  role?: unknown;
  content?: unknown;
};

export type BreakdownCategory =
  | "system"
  | "character"
  | "persona"
  | "memory"
  | "lorebook"
  | "authorNote"
  | "companion"
  | "history";

const CATEGORY_ORDER: BreakdownCategory[] = [
  "system",
  "character",
  "persona",
  "memory",
  "lorebook",
  "authorNote",
  "companion",
  "history",
];

const CATEGORY_LABEL_KEYS: Record<BreakdownCategory, TranslationKey> = {
  system: "chats.debugPage.tokenBreakdownCat.system",
  character: "chats.debugPage.tokenBreakdownCat.character",
  persona: "chats.debugPage.tokenBreakdownCat.persona",
  memory: "chats.debugPage.tokenBreakdownCat.memory",
  lorebook: "chats.debugPage.tokenBreakdownCat.lorebook",
  authorNote: "chats.debugPage.tokenBreakdownCat.authorNote",
  companion: "chats.debugPage.tokenBreakdownCat.companion",
  history: "chats.debugPage.tokenBreakdownCat.history",
};

// Distinct, theme-independent colors for the stacked breakdown bar + legend.
const CATEGORY_COLORS: Record<BreakdownCategory, string> = {
  system: "#6366f1", // indigo
  character: "#ec4899", // pink
  persona: "#14b8a6", // teal
  memory: "#10b981", // emerald
  lorebook: "#f59e0b", // amber
  authorNote: "#84cc16", // lime (kept clear of the red reserved segment)
  companion: "#8b5cf6", // violet
  history: "#3b82f6", // blue
};

// Red for the completion budget reserved inside the context window
// (llama.cpp / Ollama) — it eats into the usable context before generation.
const RESERVED_COLOR = "#ef4444"; // red
// Grey for the still-free, usable remainder of the context window.
const FREE_COLOR = "#9ca3af"; // grey
// Indigo for the occupied portion in the simple context usage bar (bar 2).
const OCCUPIED_COLOR = "#6366f1"; // indigo

/** Flatten an entry/message content (string or multimodal parts) into plain text. */
function contentToText(content: unknown): string {
  if (typeof content === "string") return content;
  if (Array.isArray(content)) {
    return content
      .map((part) => {
        if (typeof part === "string") return part;
        if (part && typeof part === "object" && "text" in part) {
          const text = (part as { text?: unknown }).text;
          return typeof text === "string" ? text : "";
        }
        return "";
      })
      .join("");
  }
  return "";
}

/** Map a system prompt entry to a breakdown category via its stable id/name. */
function categorizeEntry(entry: PromptEntryLike): BreakdownCategory {
  const id = typeof entry.id === "string" ? entry.id.toLowerCase() : "";
  const name = typeof entry.name === "string" ? entry.name.toLowerCase() : "";
  const has = (needle: string) => id.includes(needle) || name.includes(needle);

  if (
    id === "runtime_retrieved_memories" ||
    id === "entry_context_summary" ||
    id === "entry_key_memories" ||
    has("memor") ||
    has("context summary")
  ) {
    return "memory";
  }
  if (id === "entry_lorebook" || has("lorebook") || has("world information")) {
    return "lorebook";
  }
  if (id === "entry_author_note" || has("author")) {
    return "authorNote";
  }
  if (
    id === "entry_companion_state" ||
    id === "entry_scheduled_notes" ||
    has("companion") ||
    has("scheduled notes")
  ) {
    return "companion";
  }
  return "system";
}

type TokenBreakdown = {
  rows: { category: BreakdownCategory; tokens: number }[];
  total: number;
};

type BreakdownSegment = {
  key: string;
  labelKey: TranslationKey;
  color: string;
  tokens: number;
};

export type TokenBreakdownData = {
  breakdown: TokenBreakdown | null;
  loading: boolean;
  calibrated: boolean;
  displayTotal: number;
  reservedTokens: number;
  responseTokens: number;
  reasoningTokens: number;
  occupiedTotal: number;
  contextLength: number | null;
  historyCount: number;
  memoryEntryCount: number;
  lorebookEntryCount: number;
  segments: BreakdownSegment[];
};

/**
 * Load the reconstructed debug snapshot for an (assistant) message on demand.
 * Returns null until loaded / when disabled. Shared by the debug page and the
 * user-facing message sheet.
 */
export function useMessageDebugSnapshot(
  sessionId: string | null | undefined,
  messageId: string | null | undefined,
  enabled: boolean,
): ChatMessageDebugSnapshot | null {
  const [snapshot, setSnapshot] = useState<ChatMessageDebugSnapshot | null>(null);

  useEffect(() => {
    if (!enabled || !sessionId || !messageId) {
      setSnapshot(null);
      return;
    }
    let cancelled = false;
    void getMessageDebugSnapshot({ sessionId, messageId })
      .then((next) => {
        if (!cancelled) setSnapshot(next);
      })
      .catch(() => {
        if (!cancelled) setSnapshot(null);
      });
    return () => {
      cancelled = true;
    };
  }, [sessionId, messageId, enabled]);

  return snapshot;
}

/**
 * Compute the prompt token breakdown from a debug snapshot: per-category token
 * shares (tiktoken), scaled onto the provider's real prompt tokens when known,
 * plus the reserved completion budget (llama.cpp / Ollama) and context usage.
 */
export function useTokenBreakdown(
  snapshot: ChatMessageDebugSnapshot | null,
  message: StoredMessage | null,
): TokenBreakdownData {
  const [breakdown, setBreakdown] = useState<TokenBreakdown | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!snapshot) {
      setBreakdown(null);
      setLoading(false);
      return;
    }
    let cancelled = false;

    // Entry buckets come from the reconstructed system prompt entries (which
    // still carry their id/name); chat history is counted from the user/
    // assistant request messages. System-role request messages are skipped to
    // avoid double-counting the entries they were assembled from.
    const entryItems: { category: BreakdownCategory; text: string }[] = [];
    for (const raw of snapshot.promptEntries as PromptEntryLike[]) {
      const text = contentToText(raw?.content);
      if (!text) continue;
      entryItems.push({ category: categorizeEntry(raw), text });
    }
    for (const raw of snapshot.requestMessages as PromptEntryLike[]) {
      const role = typeof raw?.role === "string" ? raw.role.toLowerCase() : "";
      if (role !== "user" && role !== "assistant") continue;
      const text = contentToText(raw?.content);
      if (!text) continue;
      entryItems.push({ category: "history", text });
    }

    // Raw placeholder sources. When a template renders these inline (via
    // {{lorebook}} etc.) they never become standalone entries — their tokens sit
    // hidden inside the system/persona bucket. We count them separately and, for
    // categories that have no standalone entry, reattribute them out of the
    // system bucket so they show up as their own slice.
    const inlineSources: { category: BreakdownCategory; text: string }[] = [];
    const addInline = (category: BreakdownCategory, text: string | undefined) => {
      if (text && text.trim()) inlineSources.push({ category, text });
    };
    addInline("character", snapshot.characterProfileContent);
    addInline("persona", snapshot.personaContent);
    addInline("lorebook", snapshot.lorebookContent);
    addInline("memory", snapshot.contextSummaryContent);
    addInline("memory", snapshot.keyMemoriesContent);
    addInline("authorNote", snapshot.authorNoteContent);
    addInline("companion", snapshot.companionStateContent);
    addInline("companion", snapshot.scheduledNotesContent);

    if (entryItems.length === 0 && inlineSources.length === 0) {
      setBreakdown({ rows: [], total: 0 });
      setLoading(false);
      return;
    }

    const INLINE_CATEGORIES: BreakdownCategory[] = [
      "character",
      "persona",
      "lorebook",
      "memory",
      "authorNote",
      "companion",
    ];

    setLoading(true);
    const allTexts = [
      ...entryItems.map((item) => item.text),
      ...inlineSources.map((item) => item.text),
    ];
    void countTokensBatch(allTexts)
      .then((counts) => {
        if (cancelled) return;
        const entryTotals = new Map<BreakdownCategory, number>();
        entryItems.forEach((item, index) => {
          entryTotals.set(
            item.category,
            (entryTotals.get(item.category) ?? 0) + (counts[index] ?? 0),
          );
        });
        const inlineTotals = new Map<BreakdownCategory, number>();
        inlineSources.forEach((item, index) => {
          const count = counts[entryItems.length + index] ?? 0;
          inlineTotals.set(item.category, (inlineTotals.get(item.category) ?? 0) + count);
        });

        const finalTotals = new Map<BreakdownCategory, number>(entryTotals);
        for (const category of INLINE_CATEGORIES) {
          const inlineTokens = inlineTotals.get(category) ?? 0;
          const entryTokens = entryTotals.get(category) ?? 0;
          if (inlineTokens > 0 && entryTokens === 0) {
            finalTotals.set(category, inlineTokens);
            const system = finalTotals.get("system") ?? 0;
            finalTotals.set("system", Math.max(0, system - inlineTokens));
          }
        }

        const rows = CATEGORY_ORDER.filter(
          (category) => (finalTotals.get(category) ?? 0) > 0,
        ).map((category) => ({ category, tokens: finalTotals.get(category) ?? 0 }));
        const total = rows.reduce((sum, row) => sum + row.tokens, 0);
        setBreakdown({ rows, total });
      })
      .catch((error) => {
        console.error("Failed to compute prompt token breakdown:", error);
        if (!cancelled) setBreakdown(null);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [snapshot]);

  const reportedPromptTokens = message?.usage?.promptTokens ?? 0;
  const calibrated = reportedPromptTokens > 0 && (breakdown?.total ?? 0) > 0;
  const displayTotal = calibrated ? reportedPromptTokens : breakdown?.total ?? 0;

  // llama.cpp and Ollama reserve the completion budget inside the context window
  // — that space is effectively occupied before generation. The real reserved
  // amount is the completion cap in the actual request body, which already
  // includes the reasoning budget the adapter adds on top of max_tokens (see
  // provider_adapter/llamacpp.rs / ollama). We read that instead of the plain
  // configured max_tokens so the bar reflects what really gets held back.
  const providerId = (snapshot?.providerId ?? "").toLowerCase();
  const reservesCompletion = providerId === "llamacpp" || providerId === "ollama";
  const configuredMaxTokens = (() => {
    const requestSettings = snapshot?.requestSettings as Record<string, unknown> | undefined;
    return typeof requestSettings?.maxTokens === "number"
      ? (requestSettings.maxTokens as number)
      : 0;
  })();
  const reservedTokens = (() => {
    if (!reservesCompletion) return 0;
    const body = snapshot?.requestBody as Record<string, unknown> | undefined;
    if (providerId === "ollama") {
      const options = body?.options as Record<string, unknown> | undefined;
      const numPredict = options?.num_predict;
      if (typeof numPredict === "number" && numPredict > 0) return numPredict;
    } else {
      // llama.cpp uses the OpenAI-style body: max_completion_tokens when
      // reasoning is on, otherwise max_tokens — both already boosted.
      const maxCompletion = body?.max_completion_tokens;
      if (typeof maxCompletion === "number" && maxCompletion > 0) return maxCompletion;
      const maxTokens = body?.max_tokens;
      if (typeof maxTokens === "number" && maxTokens > 0) return maxTokens;
    }
    return configuredMaxTokens;
  })();
  // Split reserved into the configured response budget and the reasoning budget
  // the adapter adds on top (reserved = response + reasoning).
  const responseTokens = Math.min(configuredMaxTokens, reservedTokens);
  const reasoningTokens = Math.max(0, reservedTokens - responseTokens);
  const occupiedTotal = displayTotal + reservedTokens;

  const contextLength = useMemo(() => {
    const settings = snapshot?.requestSettings as Record<string, unknown> | undefined;
    const value = settings?.contextLength;
    return typeof value === "number" && value > 0 ? value : null;
  }, [snapshot]);

  // Counts for the category label suffixes.
  const historyCount = useMemo(() => {
    if (!snapshot) return 0;
    return (snapshot.requestMessages as PromptEntryLike[]).filter((raw) => {
      const role = typeof raw?.role === "string" ? raw.role.toLowerCase() : "";
      return role === "user" || role === "assistant";
    }).length;
  }, [snapshot]);
  const memoryEntryCount = snapshot?.memoryEntryCount ?? 0;
  const lorebookEntryCount = message?.usedLorebookEntries?.length ?? 0;

  // Breakdown segments are the prompt categories only, sorted by share (largest
  // first). The reserved completion budget is no longer part of the breakdown —
  // it is shown separately in the context-window bar.
  const segments: BreakdownSegment[] = breakdown
    ? breakdown.rows
        .map((row) => {
          const share = breakdown.total > 0 ? row.tokens / breakdown.total : 0;
          return {
            key: row.category as string,
            labelKey: CATEGORY_LABEL_KEYS[row.category],
            color: CATEGORY_COLORS[row.category],
            tokens: calibrated ? Math.round(reportedPromptTokens * share) : row.tokens,
          };
        })
        .sort((a, b) => b.tokens - a.tokens)
    : [];

  return {
    breakdown,
    loading,
    calibrated,
    displayTotal,
    reservedTokens,
    responseTokens,
    reasoningTokens,
    occupiedTotal,
    contextLength,
    historyCount,
    memoryEntryCount,
    lorebookEntryCount,
    segments,
  };
}

/**
 * Renders the two stacked bars (prompt composition + context usage) with legend
 * and totals. Presentation only — pass the result of {@link useTokenBreakdown}.
 */
export function PromptTokenBreakdownBars({ data }: { data: TokenBreakdownData }) {
  const { t } = useI18n();
  const {
    breakdown,
    loading,
    calibrated,
    displayTotal,
    reservedTokens,
    responseTokens,
    reasoningTokens,
    occupiedTotal,
    contextLength,
    historyCount,
    memoryEntryCount,
    lorebookEntryCount,
    segments,
  } = data;

  // Optional per-category label suffix (dynamic counts / static note).
  const segmentSuffix = (key: string): string | null => {
    switch (key) {
      case "companion":
        return t("chats.debugPage.tokenBreakdownCompanionSuffix");
      case "history":
        return t("chats.debugPage.tokenBreakdownHistorySuffix", {
          count: String(historyCount),
        });
      case "memory":
        return t("chats.debugPage.tokenBreakdownEntriesSuffix", {
          count: String(memoryEntryCount),
        });
      case "lorebook":
        return t("chats.debugPage.tokenBreakdownEntriesSuffix", {
          count: String(lorebookEntryCount),
        });
      default:
        return null;
    }
  };

  if (!breakdown && !loading) return null;

  if (!breakdown || breakdown.rows.length === 0) {
    return <div className="text-fg/50">{t("chats.debugPage.tokenBreakdownComputing")}</div>;
  }

  return (
    <div className="space-y-3 text-xs">
      {contextLength ? (
        <div className="space-y-1.5">
          <div className="text-fg/80">{t("chats.debugPage.tokenBreakdownContext")}</div>
          <div className="flex h-4 w-full overflow-hidden rounded bg-fg/10">
            <div
              className="h-full"
              style={{
                width: `${Math.min(100, (displayTotal / contextLength) * 100)}%`,
                backgroundColor: OCCUPIED_COLOR,
              }}
              title={`${t("chats.debugPage.tokenBreakdownOccupied")} · ${displayTotal}`}
            />
            {reservedTokens > 0 ? (
              <div
                className="h-full"
                style={{
                  width: `${Math.min(100, (reservedTokens / contextLength) * 100)}%`,
                  backgroundColor: RESERVED_COLOR,
                }}
                title={`${t("chats.debugPage.tokenBreakdownReserved")} · ${reservedTokens}`}
              />
            ) : null}
            {occupiedTotal < contextLength ? (
              <div
                className="h-full"
                style={{
                  width: `${Math.max(0, 100 - (occupiedTotal / contextLength) * 100)}%`,
                  backgroundColor: FREE_COLOR,
                }}
                title={`${t("chats.debugPage.tokenBreakdownFree")} · ${Math.max(0, contextLength - occupiedTotal)}`}
              />
            ) : null}
          </div>
          <div className="space-y-1.5">
            <div className="flex items-center justify-between text-fg/80">
              <span className="flex items-center gap-2">
                <span
                  className="h-2.5 w-2.5 shrink-0 rounded-sm"
                  style={{ backgroundColor: OCCUPIED_COLOR }}
                />
                {t("chats.debugPage.tokenBreakdownOccupied")}
              </span>
              <span className="tabular-nums text-fg/60">
                {t("chats.debugPage.tokenBreakdownUsage", {
                  used: String(displayTotal),
                  limit: String(contextLength),
                  percent: String(Math.round((displayTotal / contextLength) * 100)),
                })}
              </span>
            </div>
            {reservedTokens > 0 ? (
              <div className="flex items-center justify-between gap-3 text-fg/80">
                <span className="flex items-center gap-2">
                  <span
                    className="h-2.5 w-2.5 shrink-0 rounded-sm"
                    style={{ backgroundColor: RESERVED_COLOR }}
                  />
                  <span>
                    {t("chats.debugPage.tokenBreakdownReserved")}{" "}
                    <span className="text-fg/40">
                      {t("chats.debugPage.tokenBreakdownReservedSplit", {
                        response: String(responseTokens),
                        reasoning: String(reasoningTokens),
                      })}
                    </span>
                  </span>
                </span>
                <span className="tabular-nums text-fg/60">
                  {t("chats.debugPage.tokenBreakdownUsage", {
                    used: String(reservedTokens),
                    limit: String(contextLength),
                    percent: String(Math.round((reservedTokens / contextLength) * 100)),
                  })}
                </span>
              </div>
            ) : null}
            <div className="flex items-center justify-between text-fg/50">
              <span className="flex items-center gap-2">
                <span
                  className="h-2.5 w-2.5 shrink-0 rounded-sm"
                  style={{ backgroundColor: FREE_COLOR }}
                />
                {t("chats.debugPage.tokenBreakdownFree")}
              </span>
              <span className="tabular-nums">
                {t("chats.debugPage.tokenBreakdownUsage", {
                  used: String(Math.max(0, contextLength - occupiedTotal)),
                  limit: String(contextLength),
                  percent: String(
                    Math.max(0, 100 - Math.round((occupiedTotal / contextLength) * 100)),
                  ),
                })}
              </span>
            </div>
          </div>
        </div>
      ) : null}
      <div className={contextLength ? "space-y-3 border-t border-fg/10 pt-3" : "space-y-3"}>
        <div className="text-fg/50">
          {calibrated
            ? t("chats.debugPage.tokenBreakdownCalibrated")
            : t("chats.debugPage.tokenBreakdownEstimate")}
        </div>
        <div className="flex h-4 w-full overflow-hidden rounded bg-fg/10">
          {segments.map((seg) => {
            const width = displayTotal > 0 ? (seg.tokens / displayTotal) * 100 : 0;
            return (
              <div
                key={seg.key}
                className="h-full"
                style={{ width: `${width}%`, backgroundColor: seg.color }}
                title={`${t(seg.labelKey)} · ${Math.round(width)}%`}
              />
            );
          })}
        </div>
        <div className="space-y-1.5">
          {segments.map((seg) => {
            const pct = displayTotal > 0 ? Math.round((seg.tokens / displayTotal) * 100) : 0;
            const suffix = segmentSuffix(seg.key);
            return (
              <div key={seg.key} className="flex items-center justify-between gap-3">
                <span className="flex items-center gap-2 text-fg/80">
                  <span
                    className="h-2.5 w-2.5 shrink-0 rounded-sm"
                    style={{ backgroundColor: seg.color }}
                  />
                  <span>
                    {t(seg.labelKey)}
                    {suffix ? <span className="text-fg/40"> {suffix}</span> : null}
                  </span>
                </span>
                <span className="tabular-nums text-fg/60">
                  {seg.tokens} · {pct}%
                </span>
              </div>
            );
          })}
        </div>
        <div className="border-t border-fg/10 pt-2">
          <div className="flex items-center justify-between text-fg/80">
            <span>{t("chats.debugPage.tokenBreakdownTotal")}</span>
            <span className="tabular-nums">{displayTotal}</span>
          </div>
        </div>
      </div>
    </div>
  );
}
