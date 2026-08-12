import { lazy, Suspense, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { useLocation, useParams, useSearchParams } from "react-router-dom";
import { AnimatePresence, LayoutGroup, motion } from "framer-motion";
import {
  Bot,
  Check,
  ChevronDown,
  ChevronRight,
  Copy,
  FileText,
  History,
  Pencil,
  Radar,
  Sparkles,
  Trash2,
  User as UserIcon,
  Wand2,
  X,
} from "lucide-react";
import type { Lorebook, LorebookEntry, StoredMessage } from "../../core/storage/schemas";
import {
  deleteLorebookEntry,
  listLorebookEntries,
  listLorebooks,
  listMessages,
  listSessionPreviews,
  saveLorebookEntry,
  type SessionPreview,
} from "../../core/storage/repo";
import { TopNav } from "../components/App";
import { type TranslationKey, useI18n } from "../../core/i18n/context";
import { Routes, useNavigationManager } from "../navigation";
import { toast } from "../components/toast";
import { confirmBottomMenu } from "../components/ConfirmBottomMenu";
import { EntryEditorMenu } from "./characters/LorebookEditor";
import { countTokensBatch } from "../../core/tokens";

export const RECENT_MESSAGE_LIMIT = 10;
export const LATEST_USER_LOAD = 30;
const INSPECTOR_MIN_WIDTH = 380;
const INSPECTOR_MAX_WIDTH = 760;
const MESSAGE_AREA_MIN_WIDTH = 520;

export type Mode = "session" | "compose";

export type PageContext =
  | { kind: "character"; characterId: string; editorPath: string }
  | { kind: "library"; editorPath: string }
  | { kind: "group"; groupId: string; editorPath: string }
  | { kind: "groupSession"; groupSessionId: string; editorPath: string };

export interface EntryInspection {
  entry: LorebookEntry;
  active: boolean;
  matchedKeywords: string[];
  status: "matched" | "alwaysActive" | "notMatched" | "noKeywords" | "disabled";
  tokenEstimate: number;
}

export interface MessageMatch {
  entryId: string;
  entryTitle: string;
  keyword: string;
}

interface HighlightSegment {
  text: string;
  matches: MessageMatch[];
}

function normalizeKeywordText(value: string): string {
  return Array.from(value)
    .map((char) => (/[\p{L}\p{N}\s]/u.test(char) ? char : " "))
    .join("")
    .split(/\s+/)
    .filter(Boolean)
    .join(" ");
}

function containsUnsegmentedScript(text: string): boolean {
  return /[\u0E00-\u0EFF\u1000-\u109F\u1780-\u17FF\u3040-\u30FF\u3400-\u4DBF\u4E00-\u9FFF\uAC00-\uD7AF\uF900-\uFAFF]/u.test(
    text,
  );
}

function keywordMatches(
  keyword: string,
  text: string,
  caseSensitive: boolean,
  keywordMatchMode: LorebookEntry["keywordMatchMode"],
): boolean {
  const trimmedKeyword = keyword.trim();
  if (!trimmedKeyword) return false;

  const searchKeyword = caseSensitive ? trimmedKeyword : trimmedKeyword.toLowerCase();
  const searchText = caseSensitive ? text : text.toLowerCase();

  if (keywordMatchMode === "regex") {
    try {
      return new RegExp(searchKeyword, caseSensitive ? "u" : "iu").test(searchText);
    } catch {
      return false;
    }
  }

  if (searchKeyword.endsWith("*")) {
    const prefix = searchKeyword.slice(0, -1);
    if (!prefix) return false;
    const normalizedText = normalizeKeywordText(searchText);
    if (containsUnsegmentedScript(prefix) || containsUnsegmentedScript(normalizedText)) {
      return normalizedText.includes(prefix);
    }
    return normalizedText
      .split(/\s+/)
      .some((word) => word.startsWith(prefix));
  }

  const normalizedKeyword = normalizeKeywordText(searchKeyword);
  const normalizedText = normalizeKeywordText(searchText);

  if (!normalizedKeyword) return false;
  if (containsUnsegmentedScript(normalizedKeyword) || containsUnsegmentedScript(normalizedText)) {
    return normalizedText.includes(normalizedKeyword);
  }
  if (normalizedKeyword.includes(" ")) {
    return normalizedText.includes(normalizedKeyword);
  }

  return normalizedText.split(/\s+/).some((word) => word === normalizedKeyword);
}

function estimateTokens(text: string): number {
  const trimmed = text.trim();
  if (!trimmed) return 0;
  return Math.max(1, Math.ceil(trimmed.length / 4));
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function buildKeywordRegex(
  keyword: string,
  text: string,
  caseSensitive: boolean,
  keywordMatchMode: LorebookEntry["keywordMatchMode"],
): RegExp | null {
  const trimmed = keyword.trim();
  if (!trimmed) return null;
  const flags = caseSensitive ? "gu" : "giu";

  if (keywordMatchMode === "regex") {
    try {
      return new RegExp(trimmed, flags);
    } catch {
      return null;
    }
  }

  if (trimmed.endsWith("*")) {
    const prefix = trimmed.slice(0, -1);
    if (!prefix) return null;
    return new RegExp(`(?<![\\p{L}\\p{N}])${escapeRegExp(prefix)}[\\p{L}\\p{N}]*`, flags);
  }

  if (/\s/.test(trimmed)) {
    return new RegExp(escapeRegExp(trimmed), flags);
  }

  if (containsUnsegmentedScript(trimmed) || containsUnsegmentedScript(text)) {
    return new RegExp(escapeRegExp(trimmed), flags);
  }

  return new RegExp(`(?<![\\p{L}\\p{N}])${escapeRegExp(trimmed)}(?![\\p{L}\\p{N}])`, flags);
}

function findMessageMatches(
  text: string,
  inspections: EntryInspection[],
): Array<{ start: number; end: number; match: MessageMatch }> {
  const hits: Array<{ start: number; end: number; match: MessageMatch }> = [];
  for (const inspection of inspections) {
    if (!inspection.entry.enabled) continue;
    for (const keyword of inspection.entry.keywords) {
      const regex = buildKeywordRegex(
        keyword,
        text,
        inspection.entry.caseSensitive,
        inspection.entry.keywordMatchMode,
      );
      if (!regex) continue;
      let m: RegExpExecArray | null;
      while ((m = regex.exec(text)) !== null) {
        if (m[0].length === 0) {
          regex.lastIndex += 1;
          continue;
        }
        hits.push({
          start: m.index,
          end: m.index + m[0].length,
          match: {
            entryId: inspection.entry.id,
            entryTitle: inspection.entry.title?.trim() || inspection.entry.keywords[0] || "",
            keyword,
          },
        });
      }
    }
  }
  hits.sort((a, b) => a.start - b.start || a.end - b.end);
  return hits;
}

function segmentText(
  text: string,
  hits: Array<{ start: number; end: number; match: MessageMatch }>,
): HighlightSegment[] {
  if (hits.length === 0) return [{ text, matches: [] }];

  const boundaries = new Set<number>([0, text.length]);
  for (const h of hits) {
    boundaries.add(h.start);
    boundaries.add(h.end);
  }
  const sorted = Array.from(boundaries).sort((a, b) => a - b);
  const segments: HighlightSegment[] = [];
  for (let i = 0; i < sorted.length - 1; i += 1) {
    const start = sorted[i];
    const end = sorted[i + 1];
    if (end <= start) continue;
    const matches = hits.filter((h) => h.start < end && h.end > start).map((h) => h.match);
    const unique: MessageMatch[] = [];
    const seen = new Set<string>();
    for (const m of matches) {
      const key = `${m.entryId}::${m.keyword}`;
      if (seen.has(key)) continue;
      seen.add(key);
      unique.push(m);
    }
    segments.push({ text: text.slice(start, end), matches: unique });
  }
  return segments;
}

function getEntryTitle(entry: LorebookEntry, fallback: string): string {
  return entry.title?.trim() || entry.keywords[0] || fallback;
}

export function makeBlankEntry(lorebookId: string, displayOrder: number): LorebookEntry {
  const timestamp = Date.now();
  return {
    id: crypto.randomUUID(),
    lorebookId,
    title: "",
    enabled: true,
    alwaysActive: false,
    keywords: [],
    caseSensitive: false,
    keywordMatchMode: "literal",
    content: "",
    priority: 0,
    displayOrder,
    createdAt: timestamp,
    updatedAt: timestamp,
  };
}

export function buildInspections(
  entries: LorebookEntry[],
  context: string,
  tokenCounts: Map<string, number>,
): EntryInspection[] {
  return entries
    .map<EntryInspection>((entry) => {
      const matchedKeywords = entry.keywords.filter((keyword) =>
        keywordMatches(keyword, context, entry.caseSensitive, entry.keywordMatchMode),
      );
      let status: EntryInspection["status"];
      if (!entry.enabled) status = "disabled";
      else if (entry.alwaysActive) status = "alwaysActive";
      else if (matchedKeywords.length > 0) status = "matched";
      else if (entry.keywords.length === 0) status = "noKeywords";
      else status = "notMatched";
      const active = entry.enabled && (entry.alwaysActive || matchedKeywords.length > 0);
      return {
        entry,
        active,
        matchedKeywords,
        status,
        tokenEstimate: tokenCounts.get(entry.id) ?? estimateTokens(entry.content),
      };
    })
    .sort((a, b) => {
      if (a.active !== b.active) return a.active ? -1 : 1;
      if (a.entry.priority !== b.entry.priority) return b.entry.priority - a.entry.priority;
      return a.entry.displayOrder - b.entry.displayOrder || a.entry.createdAt - b.entry.createdAt;
    });
}

export function resolveContext(
  params: { characterId?: string; groupId?: string; groupSessionId?: string; lorebookId?: string },
  lorebookIdSearch: string | null,
): { context: PageContext; lorebookId: string } | null {
  const lorebookId = params.lorebookId || lorebookIdSearch || undefined;

  if (params.characterId && lorebookId) {
    return {
      context: {
        kind: "character",
        characterId: params.characterId,
        editorPath: `${Routes.characterLorebook(params.characterId)}?lorebookId=${encodeURIComponent(lorebookId)}`,
      },
      lorebookId,
    };
  }
  if (params.groupSessionId && lorebookId) {
    return {
      context: {
        kind: "groupSession",
        groupSessionId: params.groupSessionId,
        editorPath: `${Routes.groupChatLorebook(params.groupSessionId)}?lorebookId=${encodeURIComponent(lorebookId)}`,
      },
      lorebookId,
    };
  }
  if (params.groupId && lorebookId) {
    return {
      context: {
        kind: "group",
        groupId: params.groupId,
        editorPath: `${Routes.groupLorebook(params.groupId)}?lorebookId=${encodeURIComponent(lorebookId)}`,
      },
      lorebookId,
    };
  }
  if (params.lorebookId) {
    return {
      context: {
        kind: "library",
        editorPath: `/library/lorebooks/${params.lorebookId}`,
      },
      lorebookId: params.lorebookId,
    };
  }
  return null;
}

export function LorebookTriggerPreviewPageDesktop() {
  const { t } = useI18n();
  const location = useLocation();
  const { backOrReplace } = useNavigationManager();
  const params = useParams();
  const [searchParams] = useSearchParams();

  const resolved = useMemo(
    () => resolveContext(params, searchParams.get("lorebookId")),
    [params, searchParams],
  );

  const context = resolved?.context ?? null;
  const lorebookId = resolved?.lorebookId ?? null;

  const [lorebook, setLorebook] = useState<Lorebook | null>(null);
  const [entries, setEntries] = useState<LorebookEntry[]>([]);
  const [isLoadingLorebook, setIsLoadingLorebook] = useState(true);

  const [sessions, setSessions] = useState<SessionPreview[]>([]);
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<StoredMessage[]>([]);
  const [isLoadingSessions, setIsLoadingSessions] = useState(false);
  const [isLoadingMessages, setIsLoadingMessages] = useState(false);
  const [sessionPickerOpen, setSessionPickerOpen] = useState(false);

  const [composerText, setComposerText] = useState("");
  const [mode, setMode] = useState<Mode>(context?.kind === "character" ? "session" : "compose");
  const [editingEntry, setEditingEntry] = useState<LorebookEntry | null>(null);
  const [tokenCounts, setTokenCounts] = useState<Map<string, number>>(new Map());
  const [expandedEntries, setExpandedEntries] = useState<Set<string>>(new Set());
  const [inspectorWidth, setInspectorWidth] = useState(INSPECTOR_MIN_WIDTH);
  const [isResizingInspector, setIsResizingInspector] = useState(false);
  const layoutRef = useRef<HTMLDivElement | null>(null);
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    matches: MessageMatch[];
  } | null>(null);

  const getInspectorMaxWidth = () => {
    const layoutWidth = layoutRef.current?.getBoundingClientRect().width ?? 0;
    if (layoutWidth <= 0) return INSPECTOR_MAX_WIDTH;
    return Math.max(
      INSPECTOR_MIN_WIDTH,
      Math.min(INSPECTOR_MAX_WIDTH, layoutWidth - MESSAGE_AREA_MIN_WIDTH),
    );
  };

  const clampInspectorWidth = (width: number) =>
    Math.max(INSPECTOR_MIN_WIDTH, Math.min(getInspectorMaxWidth(), width));

  useEffect(() => {
    const handleResize = () => {
      setInspectorWidth((width) => clampInspectorWidth(width));
    };

    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  useEffect(() => {
    if (!isResizingInspector) return;

    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;

    const handlePointerMove = (event: PointerEvent) => {
      const rect = layoutRef.current?.getBoundingClientRect();
      if (!rect) return;
      setInspectorWidth(clampInspectorWidth(rect.right - event.clientX));
    };

    const stopResize = () => setIsResizingInspector(false);

    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", stopResize, { once: true });
    window.addEventListener("pointercancel", stopResize, { once: true });

    return () => {
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", stopResize);
      window.removeEventListener("pointercancel", stopResize);
    };
  }, [isResizingInspector]);

  const toggleExpandEntry = (id: string) => {
    setExpandedEntries((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const focusEntry = (id: string) => {
    setExpandedEntries((prev) => {
      const next = new Set(prev);
      next.add(id);
      return next;
    });
    setContextMenu(null);
    requestAnimationFrame(() => {
      const el = document.querySelector<HTMLElement>(`[data-entry-id="${CSS.escape(id)}"]`);
      el?.scrollIntoView({ behavior: "smooth", block: "nearest" });
      el?.classList.remove("entry-focus-flash");
      // trigger reflow then re-add so animation replays
      void el?.offsetWidth;
      el?.classList.add("entry-focus-flash");
    });
  };

  const openKeywordMenu = (x: number, y: number, matches: MessageMatch[]) => {
    if (matches.length === 0) return;
    setContextMenu({ x, y, matches });
  };

  useEffect(() => {
    if (!lorebookId) return;
    let cancelled = false;
    (async () => {
      setIsLoadingLorebook(true);
      try {
        const [all, loadedEntries] = await Promise.all([
          listLorebooks(),
          listLorebookEntries(lorebookId),
        ]);
        if (cancelled) return;
        const found = all.find((lb) => lb.id === lorebookId) ?? null;
        setLorebook(found);
        setEntries(loadedEntries);
      } catch (error) {
        console.error("Failed to load lorebook preview data:", error);
      } finally {
        if (!cancelled) setIsLoadingLorebook(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [lorebookId]);

  useEffect(() => {
    if (context?.kind !== "character") return;
    let cancelled = false;
    (async () => {
      setIsLoadingSessions(true);
      try {
        const previews = await listSessionPreviews(context.characterId);
        if (cancelled) return;
        const active = previews
          .filter((p) => !p.archived)
          .sort((a, b) => b.updatedAt - a.updatedAt);
        setSessions(active);
        if (active.length > 0 && !selectedSessionId) {
          setSelectedSessionId(active[0].id);
        }
      } catch (error) {
        console.error("Failed to load sessions:", error);
      } finally {
        if (!cancelled) setIsLoadingSessions(false);
      }
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [context?.kind, context?.kind === "character" ? context.characterId : null]);

  const detectionMode: Lorebook["keywordDetectionMode"] =
    lorebook?.keywordDetectionMode ?? "recentMessageWindow";
  const scanDepth = detectionMode === "latestUserMessage" ? 1 : RECENT_MESSAGE_LIMIT;

  useEffect(() => {
    if (!selectedSessionId) {
      setMessages([]);
      return;
    }
    let cancelled = false;
    (async () => {
      setIsLoadingMessages(true);
      try {
        const limit = detectionMode === "latestUserMessage" ? LATEST_USER_LOAD : scanDepth;
        const loaded = await listMessages(selectedSessionId, { limit });
        if (cancelled) return;
        if (detectionMode === "latestUserMessage") {
          const chronological = [...loaded].sort((a, b) => a.createdAt - b.createdAt);
          const latestUser = [...chronological].reverse().find((m) => m.role === "user");
          setMessages(latestUser ? [latestUser] : []);
        } else {
          setMessages(loaded);
        }
      } catch (error) {
        console.error("Failed to load messages:", error);
      } finally {
        if (!cancelled) setIsLoadingMessages(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [selectedSessionId, detectionMode, scanDepth]);

  const sessionContext = useMemo(() => {
    if (messages.length === 0) return "";
    const chronological = [...messages].sort((a, b) => a.createdAt - b.createdAt);
    return chronological.map((m) => m.content).join("\n");
  }, [messages]);

  const composerContext = composerText;

  const activeContext =
    mode === "session" && context?.kind === "character" ? sessionContext : composerContext;

  const inspections = useMemo(
    () => buildInspections(entries, activeContext, tokenCounts),
    [entries, activeContext, tokenCounts],
  );

  useEffect(() => {
    if (entries.length === 0) {
      setTokenCounts(new Map());
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const texts = entries.map((e) => e.content ?? "");
        const counts = await countTokensBatch(texts);
        if (cancelled) return;
        const map = new Map<string, number>();
        entries.forEach((e, i) => {
          const count = counts[i];
          if (typeof count === "number") map.set(e.id, count);
        });
        setTokenCounts(map);
      } catch (error) {
        if (cancelled) return;
        console.error("Failed to count tokens, falling back to estimate:", error);
        setTokenCounts(new Map());
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [entries]);

  const activeInspections = inspections.filter((i) => i.active);
  const totalInjectedTokens = activeInspections.reduce((sum, i) => sum + i.tokenEstimate, 0);

  const finalInjection = useMemo(
    () =>
      activeInspections
        .map((i) => i.entry.content)
        .filter(Boolean)
        .join("\n\n"),
    [activeInspections],
  );

  const handleBack = () => {
    if (context) backOrReplace(context.editorPath);
    else backOrReplace(Routes.library);
  };

  const handleEditEntry = (entry: LorebookEntry) => {
    setEditingEntry(entry);
  };

  const handleSaveEntry = async (updated: LorebookEntry) => {
    try {
      const saved = await saveLorebookEntry(updated);
      setEntries((prev) => {
        const exists = prev.some((e) => e.id === saved.id);
        if (exists) return prev.map((e) => (e.id === saved.id ? saved : e));
        return [...prev, saved];
      });
    } catch (error) {
      console.error("Failed to save entry:", error);
      toast.error(t("characters.lorebook.preview.saveFailed"));
    }
  };

  const handleDeleteEntry = async (entry: LorebookEntry) => {
    const title =
      entry.title?.trim() || entry.keywords[0] || t("characters.lorebook.untitledEntry");
    const confirmed = await confirmBottomMenu({
      title: t("characters.lorebook.preview.deleteConfirmTitle"),
      message: t("characters.lorebook.preview.deleteConfirmMessage", { title }),
      confirmLabel: t("common.buttons.delete"),
      destructive: true,
    });
    if (!confirmed) return;
    try {
      await deleteLorebookEntry(entry.id);
      setEntries((prev) => prev.filter((e) => e.id !== entry.id));
    } catch (error) {
      console.error("Failed to delete entry:", error);
      toast.error(t("characters.lorebook.preview.deleteFailed"));
    }
  };

  useEffect(() => {
    if (!lorebookId) return;
    const handleAdd = () => {
      const nextOrder = entries.reduce((max, e) => Math.max(max, e.displayOrder + 1), 0);
      setEditingEntry(makeBlankEntry(lorebookId, nextOrder));
    };
    window.addEventListener("lorebook:add", handleAdd);
    return () => window.removeEventListener("lorebook:add", handleAdd);
  }, [lorebookId, entries]);

  const handleCopyInjection = async () => {
    if (!finalInjection) return;
    try {
      await navigator.clipboard.writeText(finalInjection);
      toast.success(t("common.buttons.copied"));
    } catch {
      toast.error(t("characters.lorebook.preview.copyFailed"));
    }
  };

  if (!context || !lorebookId) {
    return (
      <div className="flex h-full flex-col bg-surface pt-[calc(72px+env(safe-area-inset-top))]">
        <TopNav
          currentPath={location.pathname + location.search}
          titleOverride={t("characters.lorebook.preview.title")}
          onBackOverride={() => backOrReplace(Routes.library)}
        />
        <div className="flex flex-1 items-center justify-center text-fg/50">
          {t("characters.lorebook.preview.missingContext")}
        </div>
      </div>
    );
  }

  const pageTitle = lorebook
    ? `${lorebook.name} · ${t("characters.lorebook.preview.title")}`
    : t("characters.lorebook.preview.title");

  const canUseSession = context.kind === "character";
  const selectedSession = sessions.find((s) => s.id === selectedSessionId) ?? null;
  const windowCount =
    mode === "session" && canUseSession
      ? messages.length
      : composerContext.split(/\r?\n/).filter(Boolean).length;

  return (
    <div className="flex h-[100dvh] flex-col overflow-hidden bg-surface pt-[calc(72px+env(safe-area-inset-top))]">
      <TopNav
        currentPath={location.pathname + location.search}
        titleOverride={pageTitle}
        onBackOverride={handleBack}
      />
      <Toolbar
        mode={mode}
        onModeChange={setMode}
        canUseSession={canUseSession}
        detectionMode={detectionMode}
        scanDepth={scanDepth}
        windowCount={windowCount}
        activeCount={activeInspections.length}
        totalCount={inspections.length}
        totalTokens={totalInjectedTokens}
      />
      <div className="flex-1 min-h-0 overflow-hidden">
        <div
          ref={layoutRef}
          className={`mx-auto flex h-full max-w-[1400px] flex-col divide-y divide-fg/10 overflow-hidden lg:flex-row lg:divide-x lg:divide-y-0 lg:divide-fg/10 ${
            isResizingInspector ? "select-none" : ""
          }`}
        >
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden lg:min-w-0">
            {mode === "session" && canUseSession ? (
              <SessionPanel
                sessions={sessions}
                selectedSession={selectedSession}
                onSelectSession={(id) => {
                  setSelectedSessionId(id);
                  setSessionPickerOpen(false);
                }}
                pickerOpen={sessionPickerOpen}
                onTogglePicker={() => setSessionPickerOpen((v) => !v)}
                isLoadingSessions={isLoadingSessions}
                messages={messages}
                isLoadingMessages={isLoadingMessages}
                inspections={inspections}
                detectionMode={detectionMode}
                scanDepth={scanDepth}
                onKeywordContextMenu={openKeywordMenu}
              />
            ) : (
              <ComposePanel
                text={composerText}
                onChange={setComposerText}
                inspections={inspections}
                onKeywordContextMenu={openKeywordMenu}
              />
            )}
          </div>
          <div
            className="relative flex min-h-0 flex-1 flex-col lg:w-[var(--inspector-width)] lg:flex-none"
            style={{ "--inspector-width": `${inspectorWidth}px` } as CSSProperties}
          >
            <button
              type="button"
              aria-label="Resize inspector"
              title="Resize inspector"
              aria-orientation="vertical"
              onPointerDown={(event) => {
                event.preventDefault();
                setIsResizingInspector(true);
              }}
              className="group absolute left-0 top-0 z-20 hidden h-full w-3 -translate-x-1/2 cursor-col-resize touch-none lg:block"
            >
              <span className="absolute left-1/2 top-0 h-full w-px -translate-x-1/2 bg-transparent transition group-hover:bg-accent/45" />
              <span
                className={`absolute left-1/2 top-1/2 h-14 w-1 -translate-x-1/2 -translate-y-1/2 rounded-full transition ${
                  isResizingInspector ? "bg-accent/65" : "bg-fg/15 group-hover:bg-accent/55"
                }`}
              />
            </button>
            <InspectorPanel
              inspections={inspections}
              isLoadingLorebook={isLoadingLorebook}
              isResizing={isResizingInspector}
              entriesCount={entries.length}
              totalTokens={totalInjectedTokens}
              finalInjection={finalInjection}
              onCopyInjection={handleCopyInjection}
              onEditEntry={handleEditEntry}
              onDeleteEntry={handleDeleteEntry}
              expanded={expandedEntries}
              onToggleExpand={toggleExpandEntry}
            />
          </div>
        </div>
      </div>
      <EntryEditorMenu
        entry={editingEntry}
        isOpen={Boolean(editingEntry)}
        onClose={() => setEditingEntry(null)}
        onSave={handleSaveEntry}
      />
      <AnimatePresence>
        {contextMenu && (
          <KeywordContextMenu
            x={contextMenu.x}
            y={contextMenu.y}
            matches={contextMenu.matches}
            onSelect={(entryId) => focusEntry(entryId)}
            onClose={() => setContextMenu(null)}
          />
        )}
      </AnimatePresence>
    </div>
  );
}

interface ToolbarProps {
  mode: Mode;
  onModeChange: (m: Mode) => void;
  canUseSession: boolean;
  detectionMode: Lorebook["keywordDetectionMode"];
  scanDepth: number;
  windowCount: number;
  activeCount: number;
  totalCount: number;
  totalTokens: number;
}

export function Toolbar({
  mode,
  onModeChange,
  canUseSession,
  detectionMode,
  scanDepth,
  windowCount,
  activeCount,
  totalCount,
  totalTokens,
}: ToolbarProps) {
  const { t } = useI18n();
  const modeLabel =
    detectionMode === "latestUserMessage"
      ? t("characters.lorebook.preview.modeLatestUserShort")
      : t("characters.lorebook.preview.modeRecentShort", { count: scanDepth });

  return (
    <div className="border-b border-fg/10 bg-surface-el/30 px-4 py-2 backdrop-blur">
      <div className="mx-auto flex max-w-[1400px] flex-wrap items-center gap-2">
        <div className="flex items-center gap-1.5 rounded-full border border-fg/10 bg-fg/5 px-2.5 py-1 text-[11px] font-medium text-fg/70">
          <Radar className="h-3 w-3 text-fg/50" />
          <span>{modeLabel}</span>
          <span className="text-fg/35">·</span>
          <span className="text-fg/50">
            {t("characters.lorebook.preview.inWindow", { count: windowCount })}
          </span>
        </div>

        {canUseSession && (
          <div className="flex items-center rounded-full border border-fg/10 bg-fg/5 p-0.5">
            <ToolbarTab
              active={mode === "session"}
              onClick={() => onModeChange("session")}
              icon={<History className="h-3 w-3" />}
              label={t("characters.lorebook.preview.tabSession")}
            />
            <ToolbarTab
              active={mode === "compose"}
              onClick={() => onModeChange("compose")}
              icon={<Wand2 className="h-3 w-3" />}
              label={t("characters.lorebook.preview.tabCompose")}
            />
          </div>
        )}

        <div className="ml-auto flex items-center gap-2">
          <motion.div
            key={`active-${activeCount}`}
            initial={{ scale: 0.96, opacity: 0.7 }}
            animate={{ scale: 1, opacity: 1 }}
            transition={{ type: "spring", stiffness: 500, damping: 28 }}
            className="flex items-center gap-1.5 rounded-full border border-accent/30 bg-accent/10 px-2.5 py-1 text-[11px] font-medium text-accent/85"
          >
            <motion.span
              animate={{ scale: [1, 1.4, 1] }}
              transition={{ duration: 0.35 }}
              key={`dot-${activeCount}`}
              className="h-1.5 w-1.5 rounded-full bg-accent"
            />
            <span>
              {t("characters.lorebook.preview.activeStat", {
                active: activeCount,
                total: totalCount,
              })}
            </span>
          </motion.div>
          <motion.div
            key={`tok-${totalTokens}`}
            initial={{ scale: 0.96, opacity: 0.7 }}
            animate={{ scale: 1, opacity: 1 }}
            transition={{ type: "spring", stiffness: 500, damping: 28 }}
            className="rounded-full border border-fg/10 bg-fg/5 px-2.5 py-1 font-mono text-[11px] text-fg/70"
          >
            {t("characters.lorebook.preview.tokensStat", { count: totalTokens })}
          </motion.div>
        </div>
      </div>
    </div>
  );
}

function ToolbarTab({
  active,
  onClick,
  icon,
  label,
}: {
  active: boolean;
  onClick: () => void;
  icon: React.ReactNode;
  label: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`flex items-center gap-1 rounded-full px-2.5 py-1 text-[11px] font-medium transition ${
        active
          ? "bg-fg/10 text-fg shadow-[inset_0_0_0_1px_rgba(255,255,255,0.04)]"
          : "text-fg/55 hover:text-fg"
      }`}
    >
      {icon}
      {label}
    </button>
  );
}

interface SessionPanelProps {
  sessions: SessionPreview[];
  selectedSession: SessionPreview | null;
  onSelectSession: (id: string) => void;
  pickerOpen: boolean;
  onTogglePicker: () => void;
  isLoadingSessions: boolean;
  messages: StoredMessage[];
  isLoadingMessages: boolean;
  inspections: EntryInspection[];
  detectionMode: Lorebook["keywordDetectionMode"];
  scanDepth: number;
  onKeywordContextMenu: (x: number, y: number, matches: MessageMatch[]) => void;
}

export function SessionPanel({
  sessions,
  selectedSession,
  onSelectSession,
  pickerOpen,
  onTogglePicker,
  isLoadingSessions,
  messages,
  isLoadingMessages,
  inspections,
  detectionMode,
  scanDepth,
  onKeywordContextMenu,
}: SessionPanelProps) {
  const { t } = useI18n();
  const scrollRef = useRef<HTMLDivElement>(null);

  const chronological = useMemo(
    () => [...messages].sort((a, b) => a.createdAt - b.createdAt),
    [messages],
  );

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [chronological.length, selectedSession?.id]);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="relative border-b border-fg/10 px-4 py-2">
        <button
          type="button"
          onClick={onTogglePicker}
          className="group flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left transition hover:bg-fg/5"
        >
          <div className="flex-1 min-w-0">
            {isLoadingSessions ? (
              <div className="h-4 w-40 animate-pulse rounded bg-fg/10" />
            ) : selectedSession ? (
              <div className="flex items-baseline gap-2 min-w-0">
                <span className="truncate text-sm font-semibold text-fg">
                  {selectedSession.title}
                </span>
                <span className="shrink-0 text-[11px] text-fg/45">
                  {t("characters.lorebook.preview.sessionMeta", {
                    count: selectedSession.messageCount,
                  })}
                </span>
                <span className="shrink-0 text-[11px] text-fg/35">
                  · {formatRelative(selectedSession.updatedAt)}
                </span>
              </div>
            ) : (
              <div className="text-sm text-fg/55">
                {t("characters.lorebook.preview.noSessionsHint")}
              </div>
            )}
          </div>
          <ChevronDown
            className={`h-4 w-4 shrink-0 text-fg/40 transition group-hover:text-fg/70 ${
              pickerOpen ? "rotate-180" : ""
            }`}
          />
        </button>
        <AnimatePresence>
          {pickerOpen && (
            <SessionDropdown
              sessions={sessions}
              selectedId={selectedSession?.id ?? null}
              onSelect={onSelectSession}
              onClose={() => onTogglePicker()}
            />
          )}
        </AnimatePresence>
      </div>

      <div ref={scrollRef} className="scrollbar-thin flex-1 overflow-y-auto overflow-x-hidden">
        {isLoadingMessages ? (
          <MessagesSkeleton />
        ) : chronological.length === 0 ? (
          <EmptyMessages />
        ) : (
          <motion.div
            key={selectedSession?.id ?? "empty"}
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.24 }}
            className="px-4 py-3"
          >
            <ScanHeader
              count={chronological.length}
              detectionMode={detectionMode}
              scanDepth={scanDepth}
            />
            <div className="divide-y divide-fg/5">
              {chronological.map((msg, i) => (
                <motion.div
                  key={msg.id}
                  initial={{ opacity: 0, y: 3 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ duration: 0.2, delay: Math.min(i * 0.02, 0.2) }}
                >
                  <MessageRow
                    message={msg}
                    inspections={inspections}
                    onKeywordContextMenu={onKeywordContextMenu}
                  />
                </motion.div>
              ))}
            </div>
          </motion.div>
        )}
      </div>
    </div>
  );
}

function SessionDropdown({
  sessions,
  selectedId,
  onSelect,
  onClose,
}: {
  sessions: SessionPreview[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onClose: () => void;
}) {
  const { t } = useI18n();
  return (
    <>
      <motion.div
        className="fixed inset-0 z-10"
        onClick={onClose}
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
      />
      <motion.div
        initial={{ opacity: 0, y: -6, scale: 0.98 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        exit={{ opacity: 0, y: -6, scale: 0.98 }}
        transition={{ duration: 0.18, ease: [0.4, 0, 0.2, 1] }}
        className="absolute left-4 right-4 top-full z-20 mt-1 origin-top overflow-hidden rounded-xl border border-fg/15 bg-surface shadow-2xl"
      >
        <div className="flex items-center justify-between border-b border-fg/10 px-3 py-2">
          <div className="text-[11px] font-medium uppercase tracking-wider text-fg/55">
            {t("characters.lorebook.preview.sessionPickerLabel")}
          </div>
          <button type="button" onClick={onClose} className="text-fg/40 transition hover:text-fg">
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
        {sessions.length === 0 ? (
          <div className="px-3 py-6 text-center text-sm text-fg/50">
            {t("characters.lorebook.preview.noSessions")}
          </div>
        ) : (
          <div className="scrollbar-thin max-h-80 overflow-y-auto py-1">
            {sessions.map((s) => {
              const isSelected = selectedId === s.id;
              return (
                <button
                  key={s.id}
                  type="button"
                  onClick={() => onSelect(s.id)}
                  className={`flex w-full items-start gap-2 px-3 py-2 text-left transition ${
                    isSelected ? "bg-accent/10" : "hover:bg-fg/5"
                  }`}
                >
                  <div className="mt-0.5 shrink-0">
                    {isSelected ? (
                      <Check className="h-3.5 w-3.5 text-accent" />
                    ) : (
                      <div className="h-3.5 w-3.5" />
                    )}
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-medium text-fg">{s.title}</div>
                    {s.lastMessage && (
                      <div className="mt-0.5 truncate text-[11px] text-fg/45">{s.lastMessage}</div>
                    )}
                    <div className="mt-0.5 text-[11px] font-mono text-fg/35">
                      {formatRelative(s.updatedAt)} ·{" "}
                      {t("characters.lorebook.preview.sessionMeta", { count: s.messageCount })}
                    </div>
                  </div>
                </button>
              );
            })}
          </div>
        )}
      </motion.div>
    </>
  );
}

function ScanHeader({
  count,
  detectionMode,
  scanDepth,
}: {
  count: number;
  detectionMode: Lorebook["keywordDetectionMode"];
  scanDepth: number;
}) {
  const { t } = useI18n();
  const label =
    detectionMode === "latestUserMessage"
      ? t("characters.lorebook.preview.scanHeaderLatest")
      : t("characters.lorebook.preview.scanHeaderRecent", { shown: count, depth: scanDepth });
  return (
    <div className="mb-3 flex items-center gap-2 px-2 text-[10px] font-semibold uppercase tracking-[0.12em] text-accent/75">
      <span className="h-1 w-1 rounded-full bg-accent" />
      <span>{label}</span>
      <span className="h-px flex-1 bg-gradient-to-r from-accent/25 to-transparent" />
    </div>
  );
}

function MessagesSkeleton() {
  return (
    <div className="divide-y divide-fg/5 px-4 py-2">
      {[1, 2, 3, 4].map((i) => (
        <div key={i} className="space-y-1.5 py-3">
          <div className="h-3 w-20 animate-pulse rounded bg-fg/10" />
          <div className="h-3 w-full animate-pulse rounded bg-fg/10" />
          <div className="h-3 w-4/5 animate-pulse rounded bg-fg/10" />
        </div>
      ))}
    </div>
  );
}

function EmptyMessages() {
  const { t } = useI18n();
  return (
    <div className="flex h-full flex-col items-center justify-center gap-2 px-4 py-12 text-center">
      <div className="flex h-10 w-10 items-center justify-center rounded-full border border-fg/10 bg-fg/5">
        <History className="h-4 w-4 text-fg/40" />
      </div>
      <div className="text-sm text-fg/55">{t("characters.lorebook.preview.noMessages")}</div>
    </div>
  );
}

function MessageRow({
  message,
  inspections,
  onKeywordContextMenu,
}: {
  message: StoredMessage;
  inspections: EntryInspection[];
  onKeywordContextMenu: (x: number, y: number, matches: MessageMatch[]) => void;
}) {
  const { t } = useI18n();
  const displayContent = extractDisplayText(message);

  const hits = useMemo(
    () => findMessageMatches(displayContent, inspections),
    [displayContent, inspections],
  );
  const matchedEntryIds = useMemo(() => {
    const set = new Set<string>();
    for (const h of hits) set.add(h.match.entryId);
    return set;
  }, [hits]);
  const segments = useMemo(() => segmentText(displayContent, hits), [displayContent, hits]);
  const role = roleMeta(message.role, t);
  const uniqueHitMatches = useMemo(() => {
    const seen = new Set<string>();
    const unique: MessageMatch[] = [];
    for (const h of hits) {
      const key = `${h.match.entryId}::${h.match.keyword}`;
      if (seen.has(key)) continue;
      seen.add(key);
      unique.push(h.match);
    }
    return unique;
  }, [hits]);

  const openMatchesAtElementCenter = (element: HTMLElement, matches: MessageMatch[]) => {
    const rect = element.getBoundingClientRect();
    onKeywordContextMenu(rect.left + rect.width / 2, rect.top + rect.height / 2, matches);
  };

  return (
    <div className="py-3">
      <div className="mb-1.5 flex items-center gap-2">
        {role.icon}
        <span className={`text-[10px] font-bold uppercase tracking-[0.12em] ${role.labelClass}`}>
          {role.label}
        </span>
        {hits.length > 0 && (
          <button
            type="button"
            onClick={(e) => {
              openMatchesAtElementCenter(e.currentTarget, uniqueHitMatches);
            }}
            onContextMenu={(e) => {
              e.preventDefault();
              onKeywordContextMenu(e.clientX, e.clientY, uniqueHitMatches);
            }}
            onTouchStart={(e) => {
              const touch = e.touches[0];
              if (!touch) return;
              const startX = touch.clientX;
              const startY = touch.clientY;
              const target = e.currentTarget;
              const timer = window.setTimeout(() => {
                onKeywordContextMenu(startX, startY, uniqueHitMatches);
              }, 450);
              const cancel = () => {
                window.clearTimeout(timer);
                target.removeEventListener("touchend", cancel);
                target.removeEventListener("touchcancel", cancel);
                target.removeEventListener("touchmove", onMove);
              };
              const onMove = (ev: TouchEvent) => {
                const tt = ev.touches[0];
                if (!tt) return;
                if (Math.hypot(tt.clientX - startX, tt.clientY - startY) > 8) cancel();
              };
              target.addEventListener("touchend", cancel);
              target.addEventListener("touchcancel", cancel);
              target.addEventListener("touchmove", onMove);
            }}
            style={{ WebkitTouchCallout: "none", WebkitUserSelect: "none", userSelect: "none" }}
            className="cursor-pointer rounded-full bg-accent/15 px-1.5 py-px text-[10px] font-medium text-accent/90 ring-1 ring-accent/30 transition hover:bg-accent/25"
          >
            {t("characters.lorebook.preview.matchCount", {
              hits: hits.length,
              entries: matchedEntryIds.size,
            })}
          </button>
        )}
        <span className="ml-auto font-mono text-[10px] text-fg/30">
          {formatRelative(message.createdAt)}
        </span>
      </div>
      <div className="whitespace-pre-wrap break-words text-[13px] leading-[1.55] text-fg/85">
        {displayContent.length === 0 ? (
          <span className="italic text-fg/35">{t("characters.lorebook.preview.emptyMessage")}</span>
        ) : (
          segments.map((seg, idx) =>
            seg.matches.length === 0 ? (
              <span key={idx}>{seg.text}</span>
            ) : (
              <span
                key={idx}
                role="button"
                tabIndex={0}
                onClick={(e) => {
                  openMatchesAtElementCenter(e.currentTarget, seg.matches);
                }}
                onKeyDown={(e) => {
                  if (e.key !== "Enter" && e.key !== " ") return;
                  e.preventDefault();
                  openMatchesAtElementCenter(e.currentTarget, seg.matches);
                }}
                onContextMenu={(e) => {
                  e.preventDefault();
                  onKeywordContextMenu(e.clientX, e.clientY, seg.matches);
                }}
                onTouchStart={(e) => {
                  const touch = e.touches[0];
                  if (!touch) return;
                  const startX = touch.clientX;
                  const startY = touch.clientY;
                  const target = e.currentTarget;
                  const timer = window.setTimeout(() => {
                    onKeywordContextMenu(startX, startY, seg.matches);
                  }, 450);
                  const cancel = () => {
                    window.clearTimeout(timer);
                    target.removeEventListener("touchend", cancel);
                    target.removeEventListener("touchcancel", cancel);
                    target.removeEventListener("touchmove", onMove);
                  };
                  const onMove = (ev: TouchEvent) => {
                    const t = ev.touches[0];
                    if (!t) return;
                    if (Math.hypot(t.clientX - startX, t.clientY - startY) > 8) cancel();
                  };
                  target.addEventListener("touchend", cancel);
                  target.addEventListener("touchcancel", cancel);
                  target.addEventListener("touchmove", onMove);
                }}
                style={{ WebkitTouchCallout: "none", WebkitUserSelect: "none", userSelect: "none" }}
                className="cursor-pointer rounded-[3px] bg-accent/20 px-0.5 text-accent/95 ring-1 ring-accent/30 outline-none transition hover:bg-accent/30 focus-visible:bg-accent/30"
                title={seg.matches.map((m) => `${m.entryTitle} · ${m.keyword}`).join("\n")}
              >
                {seg.text}
              </span>
            ),
          )
        )}
      </div>
    </div>
  );
}

function extractDisplayText(message: StoredMessage): string {
  if (message.selectedVariantId && message.variants) {
    const variant = message.variants.find((v) => v.id === message.selectedVariantId);
    if (variant) return variant.content;
  }
  return message.content;
}

function roleMeta(
  role: StoredMessage["role"],
  t: (key: TranslationKey) => string,
): {
  label: string;
  icon: React.ReactNode;
  labelClass: string;
} {
  switch (role) {
    case "user":
      return {
        label: t("characters.lorebook.preview.roleUser"),
        icon: <UserIcon className="h-3 w-3 text-accent/80" />,
        labelClass: "text-accent/85",
      };
    case "assistant":
      return {
        label: t("characters.lorebook.preview.roleAssistant"),
        icon: <Bot className="h-3 w-3 text-fg/60" />,
        labelClass: "text-fg/75",
      };
    case "scene":
      return {
        label: t("characters.lorebook.preview.roleScene"),
        icon: <Sparkles className="h-3 w-3 text-fg/40" />,
        labelClass: "text-fg/55",
      };
    default:
      return {
        label: t("characters.lorebook.preview.roleSystem"),
        icon: <FileText className="h-3 w-3 text-fg/40" />,
        labelClass: "text-fg/50",
      };
  }
}

interface ComposePanelProps {
  text: string;
  onChange: (v: string) => void;
  inspections: EntryInspection[];
  onKeywordContextMenu: (x: number, y: number, matches: MessageMatch[]) => void;
}

export function ComposePanel({ text, onChange, inspections, onKeywordContextMenu }: ComposePanelProps) {
  const { t } = useI18n();
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const overlayRef = useRef<HTMLDivElement>(null);
  const activeCount = inspections.filter((i) => i.active).length;
  const matchCount = useMemo(
    () => (text ? findMessageMatches(text, inspections).length : 0),
    [text, inspections],
  );

  const segments = useMemo(() => {
    if (!text) return [] as HighlightSegment[];
    const hits = findMessageMatches(text, inspections);
    return segmentText(text, hits);
  }, [text, inspections]);

  const syncScroll = () => {
    if (textareaRef.current && overlayRef.current) {
      overlayRef.current.scrollTop = textareaRef.current.scrollTop;
      overlayRef.current.scrollLeft = textareaRef.current.scrollLeft;
    }
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center gap-3 border-b border-fg/10 px-4 py-2">
        <div className="flex items-center gap-2 text-[11px] font-medium uppercase tracking-wider text-fg/55">
          <Wand2 className="h-3 w-3" />
          {t("characters.lorebook.preview.composeHeader")}
        </div>
        <div className="ml-auto flex items-center gap-2 text-[11px] text-fg/45">
          <span className="font-mono">
            {t("characters.lorebook.preview.composeMatches", { count: matchCount })}
          </span>
          <span className="text-fg/25">·</span>
          <span className="text-accent/85">
            {t("characters.lorebook.preview.activeLabel", { count: activeCount })}
          </span>
        </div>
      </div>
      <div className="flex min-h-0 flex-1 flex-col px-4 py-4">
        <div className="mx-auto flex min-h-0 w-full max-w-3xl flex-1 flex-col">
          <div className="relative flex min-h-0 flex-1 rounded-xl border border-fg/10 bg-surface-el/40 transition focus-within:border-accent/40 focus-within:bg-surface-el/60">
            <div
              ref={overlayRef}
              aria-hidden="true"
              className="pointer-events-none absolute inset-0 overflow-hidden whitespace-pre-wrap break-words px-4 py-3 font-mono text-[13px] leading-relaxed text-transparent"
            >
              {segments.length === 0 ? (
                <span>{"\u200b"}</span>
              ) : (
                <>
                  {segments.map((seg, idx) =>
                    seg.matches.length === 0 ? (
                      <span key={idx}>{seg.text}</span>
                    ) : (
                      <span key={idx} className="rounded-[3px] bg-accent/25 ring-1 ring-accent/35">
                        {seg.text}
                      </span>
                    ),
                  )}
                  <span>{"\u200b"}</span>
                </>
              )}
            </div>
            <textarea
              ref={textareaRef}
              value={text}
              onChange={(e) => onChange(e.target.value)}
              onScroll={syncScroll}
              onContextMenu={(e) => {
                const textarea = e.currentTarget;
                const offset = textarea.selectionStart;
                const hits = findMessageMatches(text, inspections);
                const matched = hits
                  .filter((h) => h.start <= offset && h.end > offset)
                  .map((h) => h.match);
                if (matched.length === 0) return;
                e.preventDefault();
                const seen = new Set<string>();
                const unique = matched.filter((m) => {
                  const key = `${m.entryId}::${m.keyword}`;
                  if (seen.has(key)) return false;
                  seen.add(key);
                  return true;
                });
                onKeywordContextMenu(e.clientX, e.clientY, unique);
              }}
              spellCheck={false}
              placeholder={t("characters.lorebook.preview.composePlaceholder")}
              className="scrollbar-thin relative z-[1] w-full flex-1 resize-none bg-transparent px-4 py-3 font-mono text-[13px] leading-relaxed text-fg caret-fg placeholder-fg/35 outline-none"
            />
          </div>
        </div>
      </div>
    </div>
  );
}

interface InspectorPanelProps {
  inspections: EntryInspection[];
  isLoadingLorebook: boolean;
  isResizing: boolean;
  entriesCount: number;
  totalTokens: number;
  finalInjection: string;
  onCopyInjection: () => void;
  onEditEntry: (entry: LorebookEntry) => void;
  onDeleteEntry: (entry: LorebookEntry) => void;
  expanded: Set<string>;
  onToggleExpand: (id: string) => void;
}

export function InspectorPanel({
  inspections,
  isLoadingLorebook,
  isResizing,
  entriesCount,
  totalTokens,
  finalInjection,
  onCopyInjection,
  onEditEntry,
  onDeleteEntry,
  expanded,
  onToggleExpand,
}: InspectorPanelProps) {
  const { t } = useI18n();
  const [showInjection, setShowInjection] = useState(false);

  const groups = useMemo(() => {
    const active: EntryInspection[] = [];
    const inactive: EntryInspection[] = [];
    for (const i of inspections) {
      if (i.active) active.push(i);
      else inactive.push(i);
    }
    return { active, inactive };
  }, [inspections]);

  return (
    <aside className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-surface-el/20 lg:flex-initial">
      <div className="scrollbar-thin flex min-h-0 flex-1 flex-col overflow-x-hidden overflow-y-auto">
        {isLoadingLorebook ? (
          <InspectorSkeleton />
        ) : entriesCount === 0 ? (
          <div className="flex flex-1 items-center justify-center px-6 py-12 text-center text-sm text-fg/50">
            {t("characters.lorebook.preview.noEntries")}
          </div>
        ) : (
          <LayoutGroup>
            <AnimatePresence initial={false}>
              {groups.active.length > 0 && (
                <InspectorSection
                  key="active"
                  label={t("characters.lorebook.preview.sectionActive", {
                    count: groups.active.length,
                  })}
                  animateLayout={!isResizing}
                  accent
                >
                  {groups.active.map((i) => (
                    <EntryRow
                      key={i.entry.id}
                      inspection={i}
                      expanded={expanded.has(i.entry.id)}
                      onToggleExpand={() => onToggleExpand(i.entry.id)}
                      onEdit={onEditEntry}
                      onDelete={onDeleteEntry}
                      animateLayout={!isResizing}
                    />
                  ))}
                </InspectorSection>
              )}
              {groups.inactive.length > 0 && (
                <InspectorSection
                  key="inactive"
                  label={t("characters.lorebook.preview.sectionInactive", {
                    count: groups.inactive.length,
                  })}
                  animateLayout={!isResizing}
                >
                  {groups.inactive.map((i) => (
                    <EntryRow
                      key={i.entry.id}
                      inspection={i}
                      expanded={expanded.has(i.entry.id)}
                      onToggleExpand={() => onToggleExpand(i.entry.id)}
                      onEdit={onEditEntry}
                      onDelete={onDeleteEntry}
                      animateLayout={!isResizing}
                    />
                  ))}
                </InspectorSection>
              )}
            </AnimatePresence>
          </LayoutGroup>
        )}
      </div>

      <InjectionSection
        show={showInjection}
        onToggle={() => setShowInjection((v) => !v)}
        totalTokens={totalTokens}
        content={finalInjection}
        onCopy={onCopyInjection}
      />
    </aside>
  );
}

function InspectorSection({
  label,
  accent,
  animateLayout,
  children,
}: {
  label: string;
  accent?: boolean;
  animateLayout: boolean;
  children: React.ReactNode;
}) {
  return (
    <motion.section
      layout={animateLayout}
      initial={{ opacity: 0, y: -4 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -4 }}
      transition={animateLayout ? { duration: 0.2 } : { duration: 0 }}
    >
      <motion.div
        layout={animateLayout ? "position" : false}
        className={`sticky top-0 z-[1] flex items-center gap-2 border-b border-fg/5 bg-surface/95 px-4 py-1.5 text-[10px] font-semibold uppercase tracking-[0.1em] backdrop-blur ${
          accent ? "text-accent/75" : "text-fg/40"
        }`}
      >
        <motion.span
          layout={animateLayout}
          animate={animateLayout ? { scale: [1, 1.3, 1] } : { scale: 1 }}
          transition={animateLayout ? { duration: 0.35 } : { duration: 0 }}
          key={label}
          className={`h-1 w-1 rounded-full ${accent ? "bg-accent" : "bg-fg/30"}`}
        />
        <motion.span key={`label-${label}`} layout={animateLayout}>
          {label}
        </motion.span>
      </motion.div>
      <div className="divide-y divide-fg/5">{children}</div>
    </motion.section>
  );
}

function EntryRow({
  inspection,
  expanded,
  onToggleExpand,
  onEdit,
  onDelete,
  animateLayout,
}: {
  inspection: EntryInspection;
  expanded: boolean;
  onToggleExpand: () => void;
  onEdit: (entry: LorebookEntry) => void;
  onDelete: (entry: LorebookEntry) => void;
  animateLayout: boolean;
}) {
  const { t } = useI18n();
  const { entry, active, matchedKeywords, status, tokenEstimate } = inspection;
  const title = getEntryTitle(entry, t("characters.lorebook.untitledEntry"));

  const statusText =
    status === "matched"
      ? t("characters.lorebook.preview.statusMatched")
      : status === "alwaysActive"
        ? t("characters.lorebook.preview.statusAlways")
        : status === "disabled"
          ? t("characters.lorebook.preview.statusDisabled")
          : status === "noKeywords"
            ? t("characters.lorebook.preview.statusNoKeywords")
            : t("characters.lorebook.preview.statusNotMatched");

  const dotColor =
    status === "matched"
      ? "bg-accent"
      : status === "alwaysActive"
        ? "bg-info"
        : status === "noKeywords"
          ? "bg-warning"
          : status === "disabled"
            ? "bg-fg/20"
            : "bg-fg/20";

  const previewContent = entry.content?.trim() || "";

  return (
    <motion.div
      layout={animateLayout ? "position" : false}
      transition={
        animateLayout ? { type: "spring", stiffness: 480, damping: 42, mass: 0.6 } : { duration: 0 }
      }
      data-entry-id={entry.id}
      className={`group ${
        active ? "bg-accent/[0.04] hover:bg-accent/[0.08]" : "hover:bg-fg/[0.03]"
      }`}
    >
      <button
        type="button"
        onClick={onToggleExpand}
        className="flex w-full items-center gap-2 px-4 py-2 text-left"
      >
        <motion.span
          layout={animateLayout}
          animate={{ scale: active ? 1.1 : 1 }}
          className={`h-1.5 w-1.5 shrink-0 rounded-full ${dotColor}`}
        />
        <span className="min-w-0 flex-1 truncate text-sm font-medium text-fg">{title}</span>
        <span className="shrink-0 text-[10px] uppercase tracking-wider text-fg/40">
          {statusText}
        </span>
        <motion.span
          key={tokenEstimate}
          initial={{ opacity: 0.4 }}
          animate={{ opacity: 1 }}
          className="shrink-0 font-mono text-[10px] text-fg/35"
        >
          {t("characters.lorebook.preview.tokensShort", { count: tokenEstimate })}
        </motion.span>
        <motion.span animate={{ rotate: expanded ? 90 : 0 }}>
          <ChevronRight className="h-3.5 w-3.5 shrink-0 text-fg/30" />
        </motion.span>
      </button>

      <AnimatePresence initial={false}>
        {expanded && (
          <motion.div
            key="expanded"
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.22, ease: [0.4, 0, 0.2, 1] }}
            className="overflow-hidden border-t border-fg/5 bg-surface/40"
          >
            <div className="space-y-2 px-4 py-2.5">
              {matchedKeywords.length > 0 && (
                <div className="flex flex-wrap gap-1">
                  {matchedKeywords.map((kw) => (
                    <span
                      key={`${entry.id}-matched-${kw}`}
                      className="rounded-md bg-accent/15 px-1.5 py-0.5 font-mono text-[10px] text-accent/90 ring-1 ring-accent/30"
                    >
                      {kw}
                    </span>
                  ))}
                  {entry.keywords
                    .filter((k) => !matchedKeywords.includes(k))
                    .slice(0, 8)
                    .map((kw) => (
                      <span
                        key={`${entry.id}-unmatched-${kw}`}
                        className="rounded-md px-1.5 py-0.5 font-mono text-[10px] text-fg/40 ring-1 ring-fg/10"
                      >
                        {kw}
                      </span>
                    ))}
                </div>
              )}
              {matchedKeywords.length === 0 && entry.keywords.length > 0 && (
                <div className="flex flex-wrap gap-1">
                  {entry.keywords.slice(0, 8).map((kw) => (
                    <span
                      key={`${entry.id}-kw-${kw}`}
                      className="rounded-md px-1.5 py-0.5 font-mono text-[10px] text-fg/40 ring-1 ring-fg/10"
                    >
                      {kw}
                    </span>
                  ))}
                </div>
              )}
              {previewContent && (
                <div className="line-clamp-4 whitespace-pre-wrap text-[12px] leading-relaxed text-fg/60">
                  {previewContent}
                </div>
              )}
              <div className="flex items-center gap-3 text-[10px] font-mono text-fg/35">
                <span>ord {entry.displayOrder + 1}</span>
                <span>·</span>
                <span>pri {entry.priority}</span>
                {!entry.enabled && (
                  <>
                    <span>·</span>
                    <span className="text-fg/50">
                      {t("characters.lorebook.preview.statusDisabled")}
                    </span>
                  </>
                )}
                <div className="ml-auto flex items-center gap-1.5">
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      onEdit(entry);
                    }}
                    className="flex items-center gap-1 rounded-md border border-fg/10 bg-fg/5 px-2 py-1 text-[11px] font-medium text-fg/65 transition hover:border-fg/25 hover:text-fg"
                  >
                    <Pencil className="h-3 w-3" />
                    {t("common.buttons.edit")}
                  </button>
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      onDelete(entry);
                    }}
                    className="flex items-center gap-1 rounded-md border border-danger/30 bg-danger/5 px-2 py-1 text-[11px] font-medium text-danger/80 transition hover:border-danger/50 hover:bg-danger/10 hover:text-danger"
                  >
                    <Trash2 className="h-3 w-3" />
                    {t("common.buttons.delete")}
                  </button>
                </div>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  );
}

function InspectorSkeleton() {
  return (
    <div className="divide-y divide-fg/5">
      {[1, 2, 3, 4].map((i) => (
        <div key={i} className="flex items-center gap-2 px-4 py-2.5">
          <div className="h-1.5 w-1.5 animate-pulse rounded-full bg-fg/15" />
          <div className="h-3 flex-1 animate-pulse rounded bg-fg/10" />
          <div className="h-3 w-10 animate-pulse rounded bg-fg/10" />
        </div>
      ))}
    </div>
  );
}

function InjectionSection({
  show,
  onToggle,
  totalTokens,
  content,
  onCopy,
}: {
  show: boolean;
  onToggle: () => void;
  totalTokens: number;
  content: string;
  onCopy: () => void;
}) {
  const { t } = useI18n();
  return (
    <div className="border-t border-fg/10 bg-surface/70">
      <button
        type="button"
        onClick={onToggle}
        className="flex w-full items-center gap-2 px-4 py-2.5 text-left transition hover:bg-fg/5"
      >
        <FileText className="h-3.5 w-3.5 text-fg/50" />
        <span className="text-[11px] font-semibold uppercase tracking-[0.1em] text-fg/60">
          {t("characters.lorebook.preview.injectionTitle")}
        </span>
        <motion.span
          key={totalTokens}
          initial={{ opacity: 0.4, y: -2 }}
          animate={{ opacity: 1, y: 0 }}
          className="ml-auto font-mono text-[11px] text-fg/50"
        >
          {t("characters.lorebook.preview.tokensStat", { count: totalTokens })}
        </motion.span>
        <motion.span animate={{ rotate: show ? 180 : 0 }} transition={{ duration: 0.2 }}>
          <ChevronDown className="h-3.5 w-3.5 shrink-0 text-fg/40" />
        </motion.span>
      </button>
      <AnimatePresence initial={false}>
        {show && (
          <motion.div
            key="injection-body"
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.22, ease: [0.4, 0, 0.2, 1] }}
            className="overflow-hidden"
          >
            <div className="space-y-2 border-t border-fg/10 px-4 py-3">
              {content.length === 0 ? (
                <div className="py-3 text-center text-[12px] text-fg/45">
                  {t("characters.lorebook.preview.injectionEmpty")}
                </div>
              ) : (
                <>
                  <pre className="scrollbar-thin max-h-64 overflow-auto whitespace-pre-wrap break-words rounded-lg border border-fg/10 bg-surface-el/40 px-3 py-2.5 font-mono text-[11px] leading-relaxed text-fg/80">
                    {content}
                  </pre>
                  <button
                    type="button"
                    onClick={onCopy}
                    className="inline-flex items-center gap-1.5 rounded-md border border-fg/10 bg-fg/5 px-2 py-1 text-[11px] font-medium text-fg/70 transition hover:border-fg/25 hover:text-fg"
                  >
                    <Copy className="h-3 w-3" />
                    {t("characters.lorebook.preview.copy")}
                  </button>
                </>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

export function KeywordContextMenu({
  x,
  y,
  matches,
  onSelect,
  onClose,
}: {
  x: number;
  y: number;
  matches: MessageMatch[];
  onSelect: (entryId: string) => void;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const [pos, setPos] = useState({ left: x, top: y });
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [onClose]);

  useEffect(() => {
    if (!menuRef.current) return;
    const rect = menuRef.current.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    let left = x;
    let top = y;
    if (left + rect.width > vw - 8) left = vw - rect.width - 8;
    if (top + rect.height > vh - 8) top = vh - rect.height - 8;
    if (left < 8) left = 8;
    if (top < 8) top = 8;
    setPos({ left, top });
  }, [x, y]);

  const grouped = useMemo(() => {
    const byEntry = new Map<string, { title: string; keywords: Set<string> }>();
    for (const m of matches) {
      const existing = byEntry.get(m.entryId);
      if (existing) {
        existing.keywords.add(m.keyword);
      } else {
        byEntry.set(m.entryId, {
          title: m.entryTitle || t("characters.lorebook.untitledEntry"),
          keywords: new Set([m.keyword]),
        });
      }
    }
    return Array.from(byEntry.entries()).map(([entryId, v]) => ({
      entryId,
      title: v.title,
      keywords: Array.from(v.keywords),
    }));
  }, [matches, t]);

  return (
    <>
      <div
        className="fixed inset-0 z-[60]"
        onClick={onClose}
        onContextMenu={(e) => {
          e.preventDefault();
          onClose();
        }}
      />
      <motion.div
        ref={menuRef}
        initial={{ opacity: 0, scale: 0.96, y: -4 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.96, y: -4 }}
        transition={{ duration: 0.12, ease: [0.4, 0, 0.2, 1] }}
        style={{
          left: pos.left,
          top: pos.top,
          WebkitTouchCallout: "none",
          WebkitUserSelect: "none",
          userSelect: "none",
        }}
        onContextMenu={(e) => e.preventDefault()}
        className="fixed z-[61] min-w-[200px] origin-top-left overflow-hidden rounded-xl border border-fg/15 bg-surface shadow-2xl"
      >
        <div className="border-b border-fg/10 px-3 py-1.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-fg/45">
          {t("characters.lorebook.preview.contextMenuTitle", { count: grouped.length })}
        </div>
        <div className="py-1">
          {grouped.map((g) => (
            <button
              key={g.entryId}
              type="button"
              onClick={() => onSelect(g.entryId)}
              className="flex w-full items-center gap-2 px-3 py-2 text-left transition hover:bg-accent/10"
            >
              <ChevronRight className="h-3 w-3 shrink-0 text-fg/30" />
              <span className="min-w-0 flex-1 truncate text-sm font-medium text-fg">{g.title}</span>
              <span className="shrink-0 font-mono text-[10px] text-accent/75">
                {g.keywords.join(", ")}
              </span>
            </button>
          ))}
        </div>
      </motion.div>
    </>
  );
}

const LorebookTriggerPreviewPageMobileLazy = lazy(() =>
  import("./LorebookTriggerPreviewPage.mobile").then((m) => ({
    default: m.LorebookTriggerPreviewPageMobile,
  })),
);

export function LorebookTriggerPreviewPage() {
  const [isMobileViewport, setIsMobileViewport] = useState(() =>
    typeof window !== "undefined" ? window.matchMedia("(max-width: 1023px)").matches : false,
  );

  useEffect(() => {
    if (typeof window === "undefined") return;
    const mq = window.matchMedia("(max-width: 1023px)");
    const sync = () => setIsMobileViewport(mq.matches);
    sync();
    mq.addEventListener("change", sync);
    return () => mq.removeEventListener("change", sync);
  }, []);

  if (isMobileViewport) {
    return (
      <Suspense fallback={null}>
        <LorebookTriggerPreviewPageMobileLazy />
      </Suspense>
    );
  }
  return <LorebookTriggerPreviewPageDesktop />;
}

export function formatRelative(timestamp: number): string {
  const now = Date.now();
  const diff = now - timestamp;
  const sec = Math.round(diff / 1000);
  if (sec < 60) return "just now";
  const min = Math.round(sec / 60);
  if (min < 60) return `${min}m`;
  const hr = Math.round(min / 60);
  if (hr < 24) return `${hr}h`;
  const d = Math.round(hr / 24);
  if (d < 30) return `${d}d`;
  return new Date(timestamp).toLocaleDateString();
}
