import { useEffect, useMemo, useState, useSyncExternalStore, useCallback } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { ArrowLeft, Copy, ChevronRight, ChevronDown } from "lucide-react";

import { getMessageDebugSnapshot, type ChatMessageDebugSnapshot } from "../../../core/chat/manager";
import { getSession, readSettings } from "../../../core/storage/repo";
import { useI18n, type TranslationKey } from "../../../core/i18n/context";
import { countTokensBatch } from "../../../core/tokens";
import type { Session, Settings, StoredMessage } from "../../../core/storage/schemas";
import {
  getMessageDebugTrace,
  subscribeChatDebugStore,
  type ChatDebugEventRecord,
  type ChatRequestDebugTrace,
} from "../../../core/debug/chatDebugStore";

type DebugAttempt = {
  request?: ChatDebugEventRecord;
  response?: ChatDebugEventRecord;
  providerError?: ChatDebugEventRecord;
  transportRetries: ChatDebugEventRecord[];
};

const REQUEST_STATES = new Set(["sending_request", "regenerate_request", "continue_request"]);
const RESPONSE_STATES = new Set(["response", "regenerate_response", "continue_response"]);
const ERROR_STATES = new Set([
  "provider_error",
  "regenerate_provider_error",
  "continue_provider_error",
]);

function formatTimestamp(value: number) {
  return new Date(value).toLocaleString();
}

function stringify(value: unknown) {
  return JSON.stringify(value, null, 2);
}

function getPayloadObject(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object") return null;
  return value as Record<string, unknown>;
}

function extractAttempts(trace: ChatRequestDebugTrace | null): DebugAttempt[] {
  if (!trace) return [];

  const attempts: DebugAttempt[] = [];
  let current: DebugAttempt | null = null;

  for (const event of trace.events) {
    if (REQUEST_STATES.has(event.state)) {
      current = { request: event, transportRetries: [] };
      attempts.push(current);
      continue;
    }
    if (!current) continue;
    if (event.state === "transport_retry") {
      current.transportRetries.push(event);
      continue;
    }
    if (RESPONSE_STATES.has(event.state)) {
      current.response = event;
      continue;
    }
    if (ERROR_STATES.has(event.state)) {
      current.providerError = event;
    }
  }

  return attempts;
}

type PromptEntryLike = {
  id?: unknown;
  name?: unknown;
  role?: unknown;
  content?: unknown;
};

type BreakdownCategory =
  | "systemPersona"
  | "memory"
  | "lorebook"
  | "authorNote"
  | "companion"
  | "history";

const CATEGORY_ORDER: BreakdownCategory[] = [
  "systemPersona",
  "memory",
  "lorebook",
  "authorNote",
  "companion",
  "history",
];

const CATEGORY_LABEL_KEYS: Record<BreakdownCategory, TranslationKey> = {
  systemPersona: "chats.debugPage.tokenBreakdownCat.systemPersona",
  memory: "chats.debugPage.tokenBreakdownCat.memory",
  lorebook: "chats.debugPage.tokenBreakdownCat.lorebook",
  authorNote: "chats.debugPage.tokenBreakdownCat.authorNote",
  companion: "chats.debugPage.tokenBreakdownCat.companion",
  history: "chats.debugPage.tokenBreakdownCat.history",
};

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
  return "systemPersona";
}

type TokenBreakdown = {
  rows: { category: BreakdownCategory; tokens: number }[];
  total: number;
};

function Collapsible({
  title,
  defaultOpen = false,
  children,
}: {
  title: string;
  defaultOpen?: boolean;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const toggle = useCallback(() => setOpen((prev) => !prev), []);

  return (
    <section className="overflow-hidden rounded-xl border border-fg/10 bg-fg/5">
      <button
        type="button"
        onClick={toggle}
        className="flex w-full items-center gap-2 px-4 py-3 text-left text-sm font-medium text-fg hover:bg-fg/5 transition-colors"
      >
        {open ? (
          <ChevronDown className="h-4 w-4 shrink-0 text-fg/40" />
        ) : (
          <ChevronRight className="h-4 w-4 shrink-0 text-fg/40" />
        )}
        {title}
      </button>
      {open && <div className="border-t border-fg/10 px-4 py-3">{children}</div>}
    </section>
  );
}

function SummaryRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-start justify-between gap-6 border-b border-fg/10 py-2 last:border-b-0">
      <div className="text-fg/45">{label}</div>
      <div className="text-right text-fg">{value}</div>
    </div>
  );
}

function CopyButton({ onClick }: { onClick: () => void }) {
  const { t } = useI18n();
  return (
    <button
      type="button"
      onClick={onClick}
      className="inline-flex items-center gap-2 rounded border border-fg/10 bg-fg/5 px-2.5 py-1.5 text-[11px] text-fg/60 hover:bg-fg/10 hover:text-fg transition-colors"
    >
      <Copy className="h-3.5 w-3.5" />
      {t("chats.debugPage.copy")}
    </button>
  );
}

function JsonBlock({ title, value }: { title: string; value: unknown }) {
  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(stringify(value));
    } catch (error) {
      console.error(`Failed to copy ${title}:`, error);
    }
  };

  return (
    <Collapsible title={title}>
      <div className="mb-2 flex justify-end">
        <CopyButton onClick={() => void handleCopy()} />
      </div>
      <pre className="overflow-x-auto rounded-lg border border-fg/10 bg-surface px-3 py-3 text-xs leading-6 text-fg/85">
        {stringify(value)}
      </pre>
    </Collapsible>
  );
}

function MessageRoleBadge({ role }: { role: string }) {
  const tone =
    role === "system" || role === "developer"
      ? "border-info/30 bg-info/10 text-info/80"
      : role === "assistant" || role === "model"
        ? "border-accent/30 bg-accent/10 text-accent/80"
        : "border-fg/10 bg-fg/5 text-fg/60";

  return (
    <span className={`rounded border px-2 py-1 text-[10px] uppercase tracking-[0.18em] ${tone}`}>
      {role}
    </span>
  );
}

function MessageListBlock({ title, messages }: { title: string; messages: unknown[] }) {
  const handleCopy = async (value: unknown) => {
    try {
      await navigator.clipboard.writeText(stringify(value));
    } catch (error) {
      console.error(`Failed to copy ${title}:`, error);
    }
  };

  return (
    <Collapsible title={`${title} (${messages.length})`}>
      <div className="space-y-3">
        {messages.map((value, index) => {
          const message = getPayloadObject(value);
          const role = String(message?.role ?? "unknown");
          return (
            <div
              key={`${title}-${index}`}
              className="rounded-lg border border-fg/10 bg-surface px-3 py-3"
            >
              <div className="mb-3 flex items-center justify-between gap-3">
                <div className="flex items-center gap-2">
                  <MessageRoleBadge role={role} />
                  <span className="font-mono text-[11px] text-fg/40">#{index + 1}</span>
                </div>
                <CopyButton onClick={() => void handleCopy(value)} />
              </div>
              <pre className="overflow-x-auto text-xs leading-6 text-fg/85">
                {stringify(value)}
              </pre>
            </div>
          );
        })}
      </div>
    </Collapsible>
  );
}

export function MessageDebugPage() {
  const navigate = useNavigate();
  const { t } = useI18n();
  const { sessionId, messageId } = useParams<{
    characterId: string;
    sessionId: string;
    messageId: string;
  }>();
  const [session, setSession] = useState<Session | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [snapshot, setSnapshot] = useState<ChatMessageDebugSnapshot | null>(null);
  const [snapshotError, setSnapshotError] = useState<string | null>(null);
  const [breakdown, setBreakdown] = useState<TokenBreakdown | null>(null);
  const [breakdownLoading, setBreakdownLoading] = useState(false);

  useEffect(() => {
    if (!sessionId) return;
    let cancelled = false;

    void getSession(sessionId)
      .then((next) => {
        if (!cancelled) setSession(next);
      })
      .catch((error) => {
        console.error("Failed to load session for debug page:", error);
        if (!cancelled) setSession(null);
      });

    return () => {
      cancelled = true;
    };
  }, [sessionId]);

  useEffect(() => {
    void readSettings()
      .then(setSettings)
      .catch((error) => console.error("Failed to load settings for debug page:", error));
  }, []);

  useEffect(() => {
    if (!sessionId || !messageId) return;
    let cancelled = false;

    setSnapshot(null);
    setSnapshotError(null);
    void getMessageDebugSnapshot({ sessionId, messageId })
      .then((next) => {
        if (!cancelled) setSnapshot(next);
      })
      .catch((error) => {
        console.error("Failed to reconstruct message debug snapshot:", error);
        if (!cancelled) {
          setSnapshotError(error instanceof Error ? error.message : String(error));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [messageId, sessionId]);

  useEffect(() => {
    if (!snapshot) {
      setBreakdown(null);
      setBreakdownLoading(false);
      return;
    }
    let cancelled = false;

    // Category buckets come from the reconstructed system prompt entries (which
    // still carry their id/name), while chat history is counted from the
    // user/assistant request messages. System-role request messages are skipped
    // to avoid double-counting the entries they were assembled from.
    const items: { category: BreakdownCategory; text: string }[] = [];
    for (const raw of snapshot.promptEntries as PromptEntryLike[]) {
      const text = contentToText(raw?.content);
      if (!text) continue;
      items.push({ category: categorizeEntry(raw), text });
    }
    for (const raw of snapshot.requestMessages as PromptEntryLike[]) {
      const role = typeof raw?.role === "string" ? raw.role.toLowerCase() : "";
      if (role !== "user" && role !== "assistant") continue;
      const text = contentToText(raw?.content);
      if (!text) continue;
      items.push({ category: "history", text });
    }

    if (items.length === 0) {
      setBreakdown({ rows: [], total: 0 });
      setBreakdownLoading(false);
      return;
    }

    setBreakdownLoading(true);
    void countTokensBatch(items.map((item) => item.text))
      .then((counts) => {
        if (cancelled) return;
        const totals = new Map<BreakdownCategory, number>();
        counts.forEach((count, index) => {
          const category = items[index]?.category;
          if (!category) return;
          totals.set(category, (totals.get(category) ?? 0) + count);
        });
        const rows = CATEGORY_ORDER.filter((category) => totals.has(category)).map(
          (category) => ({ category, tokens: totals.get(category) ?? 0 }),
        );
        const total = rows.reduce((sum, row) => sum + row.tokens, 0);
        setBreakdown({ rows, total });
      })
      .catch((error) => {
        console.error("Failed to compute prompt token breakdown:", error);
        if (!cancelled) setBreakdown(null);
      })
      .finally(() => {
        if (!cancelled) setBreakdownLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [snapshot]);

  const contextLength = useMemo(() => {
    const settings = snapshot?.requestSettings as Record<string, unknown> | undefined;
    const value = settings?.contextLength;
    return typeof value === "number" && value > 0 ? value : null;
  }, [snapshot]);

  const trace = useSyncExternalStore(
    subscribeChatDebugStore,
    () => (sessionId && messageId ? getMessageDebugTrace(sessionId, messageId) : null),
    () => null,
  );

  const message = useMemo<StoredMessage | null>(() => {
    if (!session || !messageId) return null;
    return session.messages.find((item) => item.id === messageId) ?? null;
  }, [messageId, session]);

  const attempts = useMemo(() => extractAttempts(trace), [trace]);
  const latestAttempt = attempts[attempts.length - 1];
  const totalTransportRetries = attempts.reduce(
    (sum, attempt) => sum + attempt.transportRetries.length,
    0,
  );
  const latestRequestPayload = getPayloadObject(latestAttempt?.request?.payload);
  const latestResponsePayload = getPayloadObject(latestAttempt?.response?.payload);
  const resolvedModel = useMemo(() => {
    if (!settings || !message?.modelId) return null;
    return settings.models.find((item) => item.id === message.modelId) ?? null;
  }, [message?.modelId, settings]);
  const providerLabel = String(
    latestRequestPayload?.providerId ??
      snapshot?.providerId ??
      resolvedModel?.providerId ??
      t("chats.debugPage.unknown"),
  );
  const modelLabel = String(
    latestRequestPayload?.model ??
      snapshot?.modelDisplayName ??
      resolvedModel?.displayName ??
      t("chats.debugPage.unknown"),
  );
  const inferredOperation =
    trace?.operation ??
    snapshot?.operation ??
    (message?.variants && message.variants.length > 1 ? "regenerate_or_variant" : "completion");
  const requestBody = latestRequestPayload?.requestBody ?? snapshot?.requestBody;
  const requestBodyObject = getPayloadObject(requestBody);
  const requestMessages = requestBodyObject?.messages ?? snapshot?.requestMessages;
  const requestSettings = latestRequestPayload?.requestSettings ?? snapshot?.requestSettings;
  const promptEntries = snapshot?.promptEntries ?? null;
  const relativePromptEntries = snapshot?.relativePromptEntries ?? null;
  const inChatPromptEntries = snapshot?.inChatPromptEntries ?? null;
  const providerMessages = Array.isArray(requestBodyObject?.messages)
    ? requestBodyObject.messages
    : null;
  const providerSystem = requestBodyObject?.system;

  return (
    <div className="h-full overflow-y-auto scrollbar-thin bg-surface px-4 py-4 text-sm text-fg">
      <div className="mx-auto max-w-5xl space-y-4">
        <button
          type="button"
          onClick={() => navigate(-1)}
          className="inline-flex items-center gap-2 rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 font-mono text-xs text-fg/70 hover:bg-fg/10 hover:text-fg transition-colors"
        >
          <ArrowLeft className="h-4 w-4" />
          {t("chats.debugPage.return")}
        </button>

        <Collapsible title={t("chats.debugPage.title")} defaultOpen>
          <div className="font-mono text-xs">
            <SummaryRow label={t("chats.debugPage.sessionId")} value={sessionId ?? t("chats.debugPage.unknown")} />
            <SummaryRow label={t("chats.debugPage.messageId")} value={messageId ?? t("chats.debugPage.unknown")} />
            <SummaryRow label={t("chats.debugPage.role")} value={message?.role ?? t("chats.debugPage.unknown")} />
            <SummaryRow label={t("chats.debugPage.requestId")} value={trace?.requestId ?? t("chats.debugPage.missing")} />
            <SummaryRow label={t("chats.debugPage.operation")} value={inferredOperation} />
            <SummaryRow label={t("chats.debugPage.provider")} value={providerLabel} />
            <SummaryRow label={t("chats.debugPage.model")} value={modelLabel} />
            <SummaryRow label={t("chats.debugPage.promptTemplate")} value={snapshot?.promptTemplateName ?? t("chats.debugPage.unknown")} />
            <SummaryRow label={t("chats.debugPage.promptTemplateId")} value={snapshot?.promptTemplateId ?? t("chats.debugPage.missing")} />
            <SummaryRow
              label={t("chats.debugPage.promptTemplateSource")}
              value={snapshot?.promptTemplateSource ?? t("chats.debugPage.unknown")}
            />
            <SummaryRow label={t("chats.debugPage.attemptCount")} value={String(attempts.length)} />
            <SummaryRow label={t("chats.debugPage.transportRetries")} value={String(totalTransportRetries)} />
            <SummaryRow
              label={t("chats.debugPage.requestTime")}
              value={
                latestResponsePayload?.elapsedMs != null
                  ? t("chats.debugPage.milliseconds", { value: String(latestResponsePayload.elapsedMs) })
                  : t("chats.debugPage.unknown")
              }
            />
            <SummaryRow
              label={t("chats.debugPage.tokens")}
              value={String(message?.usage?.totalTokens ?? message?.usage?.completionTokens ?? 0)}
            />
            <SummaryRow
              label={t("chats.debugPage.promptCompletion")}
              value={`${message?.usage?.promptTokens ?? 0} / ${message?.usage?.completionTokens ?? 0}`}
            />
            <SummaryRow
              label={t("chats.debugPage.created")}
              value={message ? formatTimestamp(message.createdAt) : t("chats.debugPage.unknown")}
            />
          </div>
        </Collapsible>

        {snapshot && (breakdown || breakdownLoading) ? (
          <Collapsible title={t("chats.debugPage.tokenBreakdown")} defaultOpen>
            <div className="space-y-3 font-mono text-xs">
              {breakdown && breakdown.rows.length > 0 ? (
                <>
                  <div className="text-fg/50">
                    {t("chats.debugPage.tokenBreakdownEstimate")}
                  </div>
                  <div className="space-y-2">
                    {breakdown.rows.map((row) => {
                      const pct =
                        breakdown.total > 0
                          ? Math.round((row.tokens / breakdown.total) * 100)
                          : 0;
                      return (
                        <div key={row.category}>
                          <div className="flex items-center justify-between gap-3">
                            <span className="text-fg/80">{t(CATEGORY_LABEL_KEYS[row.category])}</span>
                            <span className="tabular-nums text-fg/60">
                              {row.tokens} · {pct}%
                            </span>
                          </div>
                          <div className="mt-1 h-1.5 w-full overflow-hidden rounded bg-fg/10">
                            <div
                              className="h-full rounded bg-fg/40"
                              style={{ width: `${pct}%` }}
                            />
                          </div>
                        </div>
                      );
                    })}
                  </div>
                  <div className="flex items-center justify-between border-t border-fg/10 pt-2 text-fg/80">
                    <span>{t("chats.debugPage.tokenBreakdownTotal")}</span>
                    <span className="tabular-nums">{breakdown.total}</span>
                  </div>
                  {contextLength ? (
                    <div className="flex items-center justify-between text-fg/60">
                      <span>{t("chats.debugPage.tokenBreakdownContext")}</span>
                      <span className="tabular-nums">
                        {t("chats.debugPage.tokenBreakdownUsage", {
                          used: String(breakdown.total),
                          limit: String(contextLength),
                          percent: String(Math.round((breakdown.total / contextLength) * 100)),
                        })}
                      </span>
                    </div>
                  ) : null}
                  <div className="leading-relaxed text-fg/40">
                    {t("chats.debugPage.tokenBreakdownNote")}
                  </div>
                </>
              ) : (
                <div className="text-fg/50">{t("chats.debugPage.tokenBreakdownComputing")}</div>
              )}
            </div>
          </Collapsible>
        ) : null}

        {!trace ? (
          <section className="rounded-xl border border-fg/10 bg-fg/5 p-4 font-mono text-xs text-fg/60">
            {t("chats.debugPage.noTraceFound")}
          </section>
        ) : null}

        {snapshot?.notes?.length ? (
          <Collapsible title={t("chats.debugPage.reconstructionNotes", { count: snapshot.notes.length })}>
            <div className="space-y-2 font-mono text-xs">
              {snapshot.notes.map((note, index) => (
                <div key={`${index}-${note}`} className="text-fg/60">
                  {note}
                </div>
              ))}
            </div>
          </Collapsible>
        ) : null}

        {snapshotError ? (
          <section className="rounded-xl border border-danger/20 bg-danger/5 p-4 font-mono text-xs text-danger/80">
            {t("chats.debugPage.reconstructFailed", { error: snapshotError })}
          </section>
        ) : null}

        {attempts.map((attempt, index) => (
          <Collapsible
            key={`${trace?.requestId ?? "trace"}-${index}`}
            title={`${t("chats.debugPage.attempt", { number: index + 1 })}${attempt.providerError ? ` ${t("chats.debugPage.attemptError")}` : ""}${attempt.transportRetries.length > 0 ? `. ${t("chats.debugPage.attemptRetryCount", { count: attempt.transportRetries.length })}` : ""}`}
          >
            <div className="space-y-3 font-mono text-xs">
              {attempt.transportRetries.length > 0 ? (
                <div className="rounded-lg border border-warning/20 bg-warning/5 px-3 py-2 text-warning/80">
                  {attempt.transportRetries.length === 1
                    ? t("chats.debugPage.transportRetryNoticeOne")
                    : t("chats.debugPage.transportRetryNoticeMany", {
                        count: attempt.transportRetries.length,
                      })}
                </div>
              ) : null}
              {attempt.request ? <JsonBlock title={t("chats.debugPage.request")} value={attempt.request.payload} /> : null}
              {attempt.request && getPayloadObject(attempt.request.payload)?.requestBody !== undefined ? (
                <JsonBlock
                  title={t("chats.debugPage.requestBody")}
                  value={getPayloadObject(attempt.request.payload)?.requestBody}
                />
              ) : null}
              {attempt.response ? (
                <JsonBlock title={t("chats.debugPage.response")} value={attempt.response.payload} />
              ) : null}
              {attempt.providerError ? (
                <JsonBlock title={t("chats.debugPage.providerError")} value={attempt.providerError.payload} />
              ) : null}
              {attempt.transportRetries.length > 0 ? (
                <JsonBlock
                  title={t("chats.debugPage.transportRetriesTitle")}
                  value={attempt.transportRetries.map((event) => event.payload)}
                />
              ) : null}
            </div>
          </Collapsible>
        ))}

        {requestSettings ? <JsonBlock title={t("chats.debugPage.requestSettings")} value={requestSettings} /> : null}
        {promptEntries ? <JsonBlock title={t("chats.debugPage.systemPromptEntries")} value={promptEntries} /> : null}
        {relativePromptEntries ? (
          <JsonBlock title={t("chats.debugPage.relativePromptEntries")} value={relativePromptEntries} />
        ) : null}
        {inChatPromptEntries ? (
          <JsonBlock title={t("chats.debugPage.inChatPromptEntries")} value={inChatPromptEntries} />
        ) : null}
        {Array.isArray(requestMessages) ? (
          <MessageListBlock title={t("chats.debugPage.preAdapterMessages")} messages={requestMessages} />
        ) : null}
        {providerSystem !== undefined ? (
          <JsonBlock title={t("chats.debugPage.finalProviderSystem")} value={providerSystem} />
        ) : null}
        {providerMessages ? (
          <MessageListBlock title={t("chats.debugPage.finalProviderMessages")} messages={providerMessages} />
        ) : null}
        {requestBody !== undefined ? (
          <JsonBlock title={t("chats.debugPage.fullRequestBody")} value={requestBody} />
        ) : null}
        <JsonBlock title={t("chats.debugPage.fullSessionJson")} value={session} />
        <JsonBlock title={t("chats.debugPage.storedMessageJson")} value={message} />
        <JsonBlock title={t("chats.debugPage.fullTraceEvents")} value={trace?.events ?? []} />
      </div>
    </div>
  );
}
