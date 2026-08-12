import { useEffect, useMemo, useState } from "react";
import { useLocation, useParams, useSearchParams } from "react-router-dom";
import { AnimatePresence, motion } from "framer-motion";
import { Library, MessageSquare, Wand2 } from "lucide-react";
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
import { useI18n } from "../../core/i18n/context";
import { Routes, useNavigationManager } from "../navigation";
import { toast } from "../components/toast";
import { confirmBottomMenu } from "../components/ConfirmBottomMenu";
import { EntryEditorMenu } from "./characters/LorebookEditor";
import { countTokensBatch } from "../../core/tokens";
import {
  buildInspections,
  ComposePanel,
  InspectorPanel,
  KeywordContextMenu,
  LATEST_USER_LOAD,
  makeBlankEntry,
  RECENT_MESSAGE_LIMIT,
  resolveContext,
  SessionPanel,
  type MessageMatch,
  type Mode,
} from "./LorebookTriggerPreviewPage";

type MobileTab = "content" | "entries";

export function LorebookTriggerPreviewPageMobile() {
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
  const [activeTab, setActiveTab] = useState<MobileTab>("content");
  const [editingEntry, setEditingEntry] = useState<LorebookEntry | null>(null);
  const [tokenCounts, setTokenCounts] = useState<Map<string, number>>(new Map());
  const [expandedEntries, setExpandedEntries] = useState<Set<string>>(new Set());
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    matches: MessageMatch[];
  } | null>(null);

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
    setActiveTab("entries");
    requestAnimationFrame(() => {
      const attempt = (tries: number) => {
        const el = document.querySelector<HTMLElement>(`[data-entry-id="${CSS.escape(id)}"]`);
        if (!el && tries > 0) {
          requestAnimationFrame(() => attempt(tries - 1));
          return;
        }
        if (!el) return;
        el.scrollIntoView({ behavior: "smooth", block: "nearest" });
        el.classList.remove("entry-focus-flash");
        void el.offsetWidth;
        el.classList.add("entry-focus-flash");
      };
      attempt(6);
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
      <div className="flex h-full flex-col bg-surface pt-[calc(72px+env(safe-area-inset-top))] pb-[env(safe-area-inset-bottom)]">
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

  return (
    <div className="flex h-[100dvh] flex-col overflow-hidden bg-surface pt-[calc(72px+env(safe-area-inset-top))] pb-[env(safe-area-inset-bottom)]">
      <TopNav
        currentPath={location.pathname + location.search}
        titleOverride={pageTitle}
        onBackOverride={handleBack}
      />

      <div className="flex items-center gap-1 border-b border-fg/10 bg-surface-el/20 px-3 py-1.5">
        {canUseSession && (
          <MobileTabButton
            active={activeTab === "content" && mode === "session"}
            onClick={() => {
              setMode("session");
              setActiveTab("content");
            }}
            icon={<MessageSquare className="h-3.5 w-3.5" />}
            label={t("characters.lorebook.preview.tabSession")}
          />
        )}
        <MobileTabButton
          active={activeTab === "content" && mode === "compose"}
          onClick={() => {
            setMode("compose");
            setActiveTab("content");
          }}
          icon={<Wand2 className="h-3.5 w-3.5" />}
          label={t("characters.lorebook.preview.tabCompose")}
        />
        <button
          type="button"
          onClick={() => setActiveTab("entries")}
          className={`ml-auto flex min-w-0 items-center gap-1.5 rounded-full px-3 py-1.5 text-[12px] font-medium transition ${
            activeTab === "entries"
              ? "bg-fg/10 text-fg shadow-[inset_0_0_0_1px_rgba(255,255,255,0.05)]"
              : "text-fg/55 hover:bg-fg/5 hover:text-fg"
          }`}
        >
          <Library className="h-3.5 w-3.5 shrink-0" />
          <span className="shrink-0">{t("characters.lorebook.preview.entriesTab")}</span>
          <span className="shrink-0 text-fg/45">·</span>
          <span className="shrink-0 font-mono text-[11px] text-accent/85">
            {activeInspections.length}/{inspections.length}
          </span>
          <span className="shrink-0 text-fg/45">·</span>
          <span className="shrink-0 font-mono text-[11px] text-fg/50">{totalInjectedTokens}t</span>
        </button>
      </div>

      <div className="relative flex-1 min-h-0 overflow-hidden">
        <AnimatePresence mode="wait" initial={false}>
          {activeTab === "content" ? (
            <motion.div
              key="content"
              initial={{ opacity: 0, x: -6 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -6 }}
              transition={{ duration: 0.16 }}
              className="flex h-full flex-col"
            >
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
            </motion.div>
          ) : (
            <motion.div
              key="entries"
              initial={{ opacity: 0, x: 6 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: 6 }}
              transition={{ duration: 0.16 }}
              className="flex h-full flex-col"
            >
              <InspectorPanel
                inspections={inspections}
                isLoadingLorebook={isLoadingLorebook}
                isResizing={false}
                entriesCount={entries.length}
                totalTokens={totalInjectedTokens}
                finalInjection={finalInjection}
                onCopyInjection={handleCopyInjection}
                onEditEntry={handleEditEntry}
                onDeleteEntry={handleDeleteEntry}
                expanded={expandedEntries}
                onToggleExpand={toggleExpandEntry}
              />
            </motion.div>
          )}
        </AnimatePresence>
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

function MobileTabButton({
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
      className={`flex items-center gap-1.5 rounded-full px-3 py-1.5 text-[12px] font-medium transition ${
        active
          ? "bg-fg/10 text-fg shadow-[inset_0_0_0_1px_rgba(255,255,255,0.05)]"
          : "text-fg/55 hover:bg-fg/5 hover:text-fg"
      }`}
    >
      {icon}
      <span className="truncate">{label}</span>
    </button>
  );
}
