import { useEffect, useMemo, useReducer, useState, useCallback } from "react";
import type { ComponentType, ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { useParams, useSearchParams } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import {
  ArrowLeft,
  Sparkles,
  Clock,
  ChevronDown,
  Search,
  Bot,
  User,
  Trash2,
  Edit2,
  Check,
  Plus,
  Pin,
  MessageSquare,
  AlertTriangle,
  X,
  RefreshCw,
  Snowflake,
  Flame,
  Cpu,
  EllipsisVertical,
  PinOff,
} from "lucide-react";
import type { Character, Session, StoredMessage, Model } from "../../../core/storage/schemas";
import {
  addMemory,
  removeMemory,
  updateMemory,
  getSessionMeta,
  listPinnedMessages,
  listSessionPreviews,
  listCharacters,
  saveSession,
  setMemoryColdState,
  toggleMessagePin,
  toggleMemoryPin,
  readSettings,
} from "../../../core/storage/repo";

import { storageBridge } from "../../../core/storage/files";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import {
  typography,
  radius,
  cn,
  interactive,
  spacing,
  colors,
  components,
} from "../../design-tokens";
import { Routes, useNavigationManager } from "../../navigation";
import { BottomMenu, MenuSection } from "../../components/BottomMenu";

type MemoryToolEvent = NonNullable<Session["memoryToolEvents"]>[number];
const MEMORY_CATEGORY_OPTIONS = [
  "character_trait",
  "relationship",
  "plot_event",
  "world_detail",
  "preference",
  "other",
] as const;
const isValidMemoryCategory = (value: string): value is (typeof MEMORY_CATEGORY_OPTIONS)[number] =>
  (MEMORY_CATEGORY_OPTIONS as readonly string[]).includes(value);
const formatMemoryCategoryLabel = (category: string) => category.replace(/_/g, " ");

type MemoriesTab = "memories" | "tools" | "pinned";
type RetryStatus = "idle" | "retrying" | "success";
type MemoryStatus = "idle" | "processing" | "failed";

type UiState = {
  activeTab: MemoriesTab;
  searchTerm: string;
  editingIndex: number | null;
  editingValue: string;
  editingCategory: string;
  newMemory: string;
  newMemoryCategory: string;
  isAdding: boolean;
  summaryDraft: string;
  summaryDirty: boolean;
  isSavingSummary: boolean;
  retryStatus: RetryStatus;
  actionError: string | null;
  memoryStatus: MemoryStatus; // This is now for local UI transitions, but session.memoryStatus is source of truth
  expandedMemories: Set<number>;
  memoryTempBusy: number | null;
  selectedCategory: string | null;
  selectedMemoryId: string | null;
  memoryActionMode: "actions" | "edit" | null;
};

type UiAction =
  | { type: "SET_TAB"; tab: MemoriesTab }
  | { type: "SET_SEARCH"; value: string }
  | { type: "CLEAR_SEARCH" }
  | { type: "START_EDIT"; index: number; text: string; category?: string | null }
  | { type: "SET_EDIT_VALUE"; value: string }
  | { type: "SET_EDIT_CATEGORY"; value: string }
  | { type: "CANCEL_EDIT" }
  | { type: "SET_NEW_MEMORY"; value: string }
  | { type: "SET_NEW_MEMORY_CATEGORY"; value: string }
  | { type: "SET_IS_ADDING"; value: boolean }
  | { type: "SET_SUMMARY_DRAFT"; value: string }
  | { type: "SYNC_SUMMARY_FROM_SESSION"; value: string }
  | { type: "SET_IS_SAVING_SUMMARY"; value: boolean }
  | { type: "MARK_SUMMARY_SAVED" }
  | { type: "SET_RETRY_STATUS"; value: RetryStatus }
  | { type: "SET_ACTION_ERROR"; value: string | null }
  | { type: "SET_MEMORY_STATUS"; value: MemoryStatus }
  | { type: "TOGGLE_EXPANDED"; index: number }
  | { type: "SHIFT_EXPANDED_AFTER_DELETE"; index: number }
  | { type: "SET_MEMORY_TEMP_BUSY"; value: number | null }
  | { type: "SET_CATEGORY"; value: string | null }
  | { type: "OPEN_MEMORY_ACTIONS"; id: string }
  | { type: "SET_MEMORY_ACTION_MODE"; mode: "actions" | "edit" }
  | { type: "CLOSE_MEMORY_ACTIONS" };

function initUi(errorParam: string | null): UiState {
  return {
    activeTab: "memories",
    searchTerm: "",
    editingIndex: null,
    editingValue: "",
    editingCategory: "",
    newMemory: "",
    newMemoryCategory: "",
    isAdding: false,
    summaryDraft: "",
    summaryDirty: false,
    isSavingSummary: false,
    retryStatus: "idle",
    actionError: errorParam,
    memoryStatus: "idle",
    expandedMemories: new Set<number>(),
    memoryTempBusy: null,
    selectedCategory: null,
    selectedMemoryId: null,
    memoryActionMode: null,
  };
}

function uiReducer(state: UiState, action: UiAction): UiState {
  switch (action.type) {
    case "SET_TAB":
      return { ...state, activeTab: action.tab };
    case "SET_SEARCH":
      return { ...state, searchTerm: action.value };
    case "CLEAR_SEARCH":
      return { ...state, searchTerm: "" };
    case "START_EDIT":
      const category =
        action.category && isValidMemoryCategory(action.category) ? action.category : "";
      return {
        ...state,
        editingIndex: action.index,
        editingValue: action.text,
        editingCategory: category,
      };
    case "SET_EDIT_VALUE":
      return { ...state, editingValue: action.value };
    case "SET_EDIT_CATEGORY":
      return { ...state, editingCategory: action.value };
    case "CANCEL_EDIT":
      return { ...state, editingIndex: null, editingValue: "", editingCategory: "" };
    case "SET_NEW_MEMORY":
      return { ...state, newMemory: action.value };
    case "SET_NEW_MEMORY_CATEGORY":
      return { ...state, newMemoryCategory: action.value };
    case "SET_IS_ADDING":
      return { ...state, isAdding: action.value };
    case "SET_SUMMARY_DRAFT":
      return { ...state, summaryDraft: action.value, summaryDirty: true };
    case "SYNC_SUMMARY_FROM_SESSION":
      if (state.summaryDirty) return state;
      return { ...state, summaryDraft: action.value };
    case "SET_IS_SAVING_SUMMARY":
      return { ...state, isSavingSummary: action.value };
    case "MARK_SUMMARY_SAVED":
      return { ...state, summaryDirty: false, isSavingSummary: false };
    case "SET_RETRY_STATUS":
      return { ...state, retryStatus: action.value };
    case "SET_ACTION_ERROR":
      return { ...state, actionError: action.value };
    case "SET_MEMORY_STATUS":
      return { ...state, memoryStatus: action.value };
    case "TOGGLE_EXPANDED": {
      const next = new Set(state.expandedMemories);
      if (next.has(action.index)) next.delete(action.index);
      else next.add(action.index);
      return { ...state, expandedMemories: next };
    }
    case "SHIFT_EXPANDED_AFTER_DELETE": {
      if (state.expandedMemories.size === 0) return state;
      const next = new Set<number>();
      for (const idx of state.expandedMemories) {
        if (idx === action.index) continue;
        next.add(idx > action.index ? idx - 1 : idx);
      }
      return { ...state, expandedMemories: next };
    }
    case "SET_MEMORY_TEMP_BUSY":
      return { ...state, memoryTempBusy: action.value };

    case "SET_CATEGORY":
      return { ...state, selectedCategory: action.value };
    case "OPEN_MEMORY_ACTIONS":
      return { ...state, selectedMemoryId: action.id, memoryActionMode: "actions" };
    case "SET_MEMORY_ACTION_MODE":
      return { ...state, memoryActionMode: action.mode };
    case "CLOSE_MEMORY_ACTIONS":
      return {
        ...state,
        selectedMemoryId: null,
        memoryActionMode: null,
        editingIndex: null,
        editingValue: "",
        editingCategory: "",
      };
    default:
      return state;
  }
}

function SectionHeader({
  icon: Icon,
  title,
  subtitle,
  right,
}: {
  icon?: ComponentType<{ size?: string | number; className?: string }>;
  title: string;
  subtitle?: string;
  right?: ReactNode;
}) {
  return (
    <div className="flex items-end justify-between gap-3 mb-3">
      <div className="min-w-0">
        <div className="flex items-center gap-2 min-w-0">
          {Icon ? <Icon size={16} className="text-white/40" /> : null}
          <h2
            className={cn(
              typography.h2.size,
              typography.h2.weight,
              colors.text.primary,
              "truncate",
            )}
          >
            {title}
          </h2>
        </div>
        {subtitle ? (
          <p className={cn(typography.bodySmall.size, colors.text.tertiary, "mt-0.5 truncate")}>
            {subtitle}
          </p>
        ) : null}
      </div>
      {right ? <div className="shrink-0">{right}</div> : null}
    </div>
  );
}

function MemoryActionRow({
  icon: Icon,
  label,
  onClick,
  disabled = false,
  variant = "default",
  iconBg,
}: {
  icon: ComponentType<any>;
  label: string;
  onClick: () => void;
  disabled?: boolean;
  variant?: "default" | "danger";
  iconBg?: string;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "flex w-full items-center gap-3 px-1 py-2.5 transition-all rounded-lg",
        "hover:bg-white/5 active:bg-white/10",
        "disabled:opacity-40 disabled:pointer-events-none",
        variant === "danger" && "hover:bg-red-500/10",
      )}
    >
      <div
        className={cn(
          "flex items-center justify-center w-8 h-8 rounded-lg",
          iconBg || "bg-white/10",
        )}
      >
        <Icon size={16} className={cn(variant === "danger" ? "text-red-400" : "text-white")} />
      </div>
      <span
        className={cn(
          "text-[15px] text-left",
          variant === "danger" ? "text-red-400" : "text-white/90",
        )}
      >
        {label}
      </span>
    </button>
  );
}

function useSessionData(characterId?: string, requestedSessionId?: string | null) {
  const [session, setSession] = useState<Session | null>(null);
  const [character, setCharacter] = useState<Character | null>(null);
  const [pinnedMessages, setPinnedMessages] = useState<StoredMessage[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!characterId) {
      setError("Missing characterId");
      setLoading(false);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const chars = await listCharacters();
      const foundChar = chars.find((c) => c.id === characterId) ?? null;
      setCharacter(foundChar);

      let targetSession: Session | null = null;
      if (requestedSessionId) {
        targetSession = await getSessionMeta(requestedSessionId).catch(() => null);
      }

      if (!targetSession) {
        const previews = await listSessionPreviews(characterId, 1).catch(() => []);
        const latestId = previews[0]?.id;
        targetSession = latestId ? await getSessionMeta(latestId).catch(() => null) : null;
      }

      if (targetSession) {
        setSession(targetSession);
        if (foundChar?.memoryType === "dynamic") {
          const pinned = await listPinnedMessages(targetSession.id).catch(
            () => [] as StoredMessage[],
          );
          setPinnedMessages(pinned);
        } else {
          setPinnedMessages([]);
        }
      } else {
        setError("Session not found");
        setPinnedMessages([]);
      }
    } catch (err: any) {
      setError(err?.message || "Failed to load session");
      setPinnedMessages([]);
    } finally {
      setLoading(false);
    }
  }, [characterId, requestedSessionId]);

  useEffect(() => {
    void load();
  }, [load]);

  return {
    session,
    setSession,
    pinnedMessages,
    setPinnedMessages,
    character,
    loading,
    error,
    reload: load,
  };
}

function useMemoryActions(session: Session | null, setSession: (s: Session) => void) {
  const handleAdd = useCallback(
    async (memory: string, category?: string) => {
      if (!session) return;

      try {
        const updated = await addMemory(session.id, memory, category);
        if (updated) setSession(updated);
      } catch (err: any) {
        throw err;
      }
    },
    [session, setSession],
  );

  const handleRemove = useCallback(
    async (index: number) => {
      if (!session) return;

      try {
        const updated = await removeMemory(session.id, index);
        if (updated) setSession(updated);
      } catch (err: any) {
        throw err;
      }
    },
    [session, setSession],
  );

  const handleUpdate = useCallback(
    async (index: number, memory: string, category?: string) => {
      if (!session) return;

      try {
        const updated = await updateMemory(session.id, index, memory, category);
        if (updated) setSession(updated);
      } catch (err: any) {
        throw err;
      }
    },
    [session, setSession],
  );

  const handleSaveSummary = useCallback(
    async (summary: string) => {
      if (!session) return;

      try {
        const updated: Session = { ...session, memorySummary: summary };
        await saveSession(updated);
        setSession(updated);
      } catch (err: any) {
        throw err;
      }
    },
    [session, setSession],
  );

  const handleTogglePin = useCallback(
    async (index: number) => {
      if (!session) return;

      try {
        const updated = await toggleMemoryPin(session.id, index);
        if (updated) setSession(updated);
      } catch (err: any) {
        throw err;
      }
    },
    [session, setSession],
  );

  return { handleAdd, handleRemove, handleUpdate, handleSaveSummary, handleTogglePin };
}

function relativeTime(ts: number): string {
  const now = Date.now();
  const diff = now - ts;
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return "just now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days === 1) return "yesterday";
  if (days < 7) return `${days}d ago`;
  return new Date(ts).toLocaleDateString();
}

const ACTION_STYLES: Record<
  string,
  {
    icon: ComponentType<{ size?: string | number; className?: string }>;
    color: string;
    label: string;
    bg: string;
    border: string;
  }
> = {
  create_memory: {
    icon: Plus,
    color: "text-emerald-300",
    label: "Created",
    bg: "bg-emerald-400/10",
    border: "border-emerald-400/20",
  },
  delete_memory: {
    icon: Trash2,
    color: "text-red-300",
    label: "Deleted",
    bg: "bg-red-400/10",
    border: "border-red-400/20",
  },
  pin_memory: {
    icon: Pin,
    color: "text-amber-300",
    label: "Pinned",
    bg: "bg-amber-400/10",
    border: "border-amber-400/20",
  },
  unpin_memory: {
    icon: Pin,
    color: "text-amber-300/60",
    label: "Unpinned",
    bg: "bg-amber-400/10",
    border: "border-amber-400/20",
  },
  done: {
    icon: Check,
    color: "text-blue-300",
    label: "Done",
    bg: "bg-blue-400/10",
    border: "border-blue-400/20",
  },
};

function ActionCard({ action }: { action: MemoryToolEvent["actions"][number] }) {
  const style = ACTION_STYLES[action.name] || {
    icon: Cpu,
    color: "text-zinc-300",
    label: action.name,
    bg: "bg-white/5",
    border: "border-white/10",
  };
  const Icon = style.icon;
  const args = action.arguments as Record<string, unknown> | undefined;
  const memoryText = args?.text as string | undefined;
  const category = args?.category as string | undefined;
  const important = args?.important as boolean | undefined;
  const confidence = args?.confidence as number | undefined;
  const id = args?.id as string | undefined;

  return (
    <div
      className={cn(
        radius.md,
        "border px-3 py-2.5 flex items-start gap-2.5",
        style.bg,
        style.border,
      )}
    >
      <Icon size={14} className={cn(style.color, "mt-0.5 shrink-0")} />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className={cn("text-[11px] font-semibold", style.color)}>{style.label}</span>
          {category && (
            <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-white/5 text-white/40 border border-white/8">
              {category.replace(/_/g, " ")}
            </span>
          )}
          {important && (
            <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-amber-500/20 text-amber-300 border border-amber-500/30">
              pinned
            </span>
          )}
          {confidence != null && (
            <span
              className={cn(
                "text-[10px] px-1.5 py-0.5 rounded-full border",
                confidence < 0.7
                  ? "bg-amber-500/20 text-amber-300 border-amber-500/30"
                  : "bg-red-500/20 text-red-300 border-red-500/30",
              )}
            >
              {confidence < 0.7 ? "soft-delete" : `${Math.round(confidence * 100)}%`}
            </span>
          )}
        </div>
        {memoryText && (
          <p className={cn(typography.caption.size, colors.text.secondary, "mt-1 leading-relaxed")}>
            {memoryText}
          </p>
        )}
        {id && !memoryText && (
          <p className={cn(typography.caption.size, colors.text.tertiary, "mt-1 font-mono")}>
            #{id}
          </p>
        )}
      </div>
    </div>
  );
}

function summarizeActions(actions: MemoryToolEvent["actions"]): string {
  const counts: Record<string, number> = {};
  for (const a of actions) {
    const label = ACTION_STYLES[a.name]?.label || a.name;
    counts[label] = (counts[label] || 0) + 1;
  }
  return Object.entries(counts)
    .map(([label, count]) => `${count} ${label.toLowerCase()}`)
    .join(", ");
}

function CycleCard({ event, defaultOpen }: { event: MemoryToolEvent; defaultOpen: boolean }) {
  const [isOpen, setIsOpen] = useState(defaultOpen);
  const hasError = !!event.error;
  const actionSummary = event.actions?.length ? summarizeActions(event.actions) : null;

  return (
    <div className={cn(components.card.base, "overflow-hidden", hasError && "border-red-400/20")}>
      {/* Collapsed header */}
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className={cn(
          "w-full flex items-center gap-3 px-4 py-3 text-left",
          interactive.hover.brightness,
        )}
      >
        {/* Timeline dot */}
        <div
          className={cn(
            "h-2 w-2 rounded-full shrink-0",
            hasError ? "bg-red-400" : "bg-emerald-400",
          )}
        />

        <div className="flex-1 min-w-0">
          {/* Top line: relative time + action counts */}
          <div className="flex items-center gap-2">
            <span className={cn(typography.caption.size, colors.text.secondary, "font-medium")}>
              {relativeTime(event.createdAt || 0)}
            </span>
            {actionSummary && (
              <span className={cn(typography.caption.size, colors.text.tertiary)}>
                — {actionSummary}
              </span>
            )}
            {hasError && <AlertTriangle size={12} className="text-red-400 shrink-0" />}
          </div>

          {/* Truncated summary */}
          {event.summary && !isOpen && (
            <p className={cn("text-[11px] mt-0.5 truncate", colors.text.tertiary)}>
              {event.summary}
            </p>
          )}
        </div>

        <ChevronDown
          size={14}
          className={cn(
            colors.text.tertiary,
            "shrink-0 transition-transform",
            isOpen && "rotate-180",
          )}
        />
      </button>

      {/* Expanded content */}
      {isOpen && (
        <div className="px-4 pb-4 space-y-3">
          {/* Summary */}
          {event.summary && (
            <div className={cn(radius.md, "border border-blue-400/20 bg-blue-400/10 px-3 py-2.5")}>
              <p className={cn("text-[12px] leading-relaxed text-blue-200/90")}>{event.summary}</p>
            </div>
          )}

          {/* Error */}
          {event.error && (
            <div className={cn(radius.md, "border border-red-400/20 bg-red-400/10 px-3 py-2.5")}>
              <p className={cn("text-[12px] text-red-200/90")}>{event.error}</p>
              {event.stage && (
                <p className={cn("text-[11px] mt-1 text-red-200/60")}>Failed at: {event.stage}</p>
              )}
            </div>
          )}

          {/* Action cards */}
          {event.actions && event.actions.length > 0 && (
            <div className="space-y-2">
              {event.actions
                .filter((a) => a.name !== "done")
                .map((action, idx) => (
                  <ActionCard key={idx} action={action} />
                ))}
            </div>
          )}

          {/* Footer meta */}
          <div
            className={cn(
              "flex items-center gap-3 pt-1",
              typography.caption.size,
              colors.text.disabled,
            )}
          >
            <span>
              Window {event.windowStart}–{event.windowEnd}
            </span>
            <span>{new Date(event.createdAt || 0).toLocaleString()}</span>
          </div>
        </div>
      )}
    </div>
  );
}

function ToolLog({ events }: { events: MemoryToolEvent[] }) {
  if (!events.length) {
    return (
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3, ease: "easeOut" }}
        className="flex flex-col items-center justify-center py-16"
      >
        <div className="flex h-16 w-16 items-center justify-center rounded-2xl border border-white/10 bg-white/5 mb-4">
          <Clock className="h-7 w-7 text-white/20" />
        </div>
        <h3 className="mb-1 text-base font-semibold text-white">No activity yet</h3>
        <p className={cn("text-center text-sm max-w-60", colors.text.tertiary)}>
          Tool calls appear when AI manages memories in dynamic mode
        </p>
      </motion.div>
    );
  }

  return (
    <div className={cn(spacing.item, "space-y-2")}>
      {events.map((event, idx) => (
        <CycleCard key={event.id} event={event} defaultOpen={idx === events.length - 1} />
      ))}
    </div>
  );
}

export function ChatMemoriesPage() {
  const { go, backOrReplace } = useNavigationManager();
  const { characterId } = useParams();
  const [searchParams] = useSearchParams();
  const sessionId = searchParams.get("sessionId");
  const {
    session,
    setSession,
    pinnedMessages,
    setPinnedMessages,
    character,
    loading,
    error,
    reload,
  } = useSessionData(characterId, sessionId);
  const { handleAdd, handleRemove, handleUpdate, handleSaveSummary, handleTogglePin } =
    useMemoryActions(session, (s) => setSession(s));
  const [ui, dispatch] = useReducer(uiReducer, searchParams.get("error"), initUi);
  const isDynamic = useMemo(() => {
    return character?.memoryType === "dynamic";
  }, [character?.memoryType]);
  const [modelSearchQuery, setModelSearchQuery] = useState("");
  const [allModels, setAllModels] = useState<Model[]>([]);
  const [loadingModels, setLoadingModels] = useState(false);

  const handleSetColdState = useCallback(
    async (memoryIndex: number, isCold: boolean) => {
      if (!session?.id || !isDynamic) return;
      dispatch({ type: "SET_MEMORY_TEMP_BUSY", value: memoryIndex });
      try {
        const updated = await setMemoryColdState(session.id, memoryIndex, isCold);
        if (updated) setSession(updated);
        dispatch({ type: "SET_ACTION_ERROR", value: null });
      } catch (err: any) {
        console.error("Failed to update memory temperature:", err);
        dispatch({
          type: "SET_ACTION_ERROR",
          value: err?.message || "Failed to update memory temperature",
        });
      } finally {
        dispatch({ type: "SET_MEMORY_TEMP_BUSY", value: null });
      }
    },
    [session?.id, isDynamic, setSession],
  );

  useEffect(() => {
    if (!session?.id || !isDynamic) return;
    let unlisteners: (() => void)[] = [];

    const setup = async () => {
      try {
        const u1 = await listen("dynamic-memory:processing", (e: any) => {
          if (e.payload?.sessionId === session.id) {
            dispatch({ type: "SET_MEMORY_STATUS", value: "processing" });
            void reload();
          }
        });
        const u2 = await listen("dynamic-memory:success", (e: any) => {
          if (e.payload?.sessionId === session.id) {
            dispatch({ type: "SET_MEMORY_STATUS", value: "idle" });
            void reload();
          }
        });
        const u3 = await listen("dynamic-memory:error", (e: any) => {
          if (e.payload?.sessionId === session.id) {
            const message = e.payload?.error || "Dynamic memory failed";
            dispatch({ type: "SET_ACTION_ERROR", value: message });
            dispatch({ type: "SET_MEMORY_STATUS", value: "failed" });
            void reload();
          }
        });
        unlisteners.push(u1, u2, u3);
      } catch (err) {
        console.error("Failed to setup memory event listeners", err);
      }
    };

    setup();
    return () => {
      unlisteners.forEach((u) => u());
    };
  }, [session?.id, isDynamic, reload]);

  useEffect(() => {
    dispatch({ type: "SYNC_SUMMARY_FROM_SESSION", value: session?.memorySummary ?? "" });
  }, [session?.memorySummary]);

  useEffect(() => {
    if (!isDynamic && ui.activeTab !== "memories") {
      dispatch({ type: "SET_TAB", tab: "memories" });
    }
  }, [isDynamic, ui.activeTab]);

  const cycleMap = useMemo(() => {
    const map = new Map<string, string>();
    const textMap = new Map<string, string>();

    if (isDynamic && session?.memoryToolEvents) {
      session.memoryToolEvents.forEach((event: any) => {
        const cycleStr = `${event.windowStart}-${event.windowEnd}`;
        if (event.actions) {
          event.actions.forEach((action: any) => {
            if (action.name === "create_memory") {
              if (action.memoryId) {
                map.set(action.memoryId, cycleStr);
              }
              const text = action.arguments?.text;
              if (text) {
                textMap.set(text, cycleStr);
              }
            }
          });
        }
      });
    }
    return { map, textMap };
  }, [isDynamic, session?.memoryToolEvents]);

  const memoryItems = useMemo(() => {
    if (isDynamic && session?.memoryEmbeddings && session.memoryEmbeddings.length > 0) {
      return session.memoryEmbeddings
        .map((emb, index) => {
          const id = emb.id || `mem-${index}`;
          const isAi = id.length <= 6;
          const tokenCount = emb.tokenCount || 0;

          let cycle = cycleMap.map.get(id);
          if (!cycle && cycleMap.textMap.has(emb.text)) {
            cycle = cycleMap.textMap.get(emb.text);
          }

          return {
            text: emb.text,
            index,
            isAi,
            id,
            tokenCount,
            isCold: emb.isCold ?? false,
            importanceScore: emb.importanceScore ?? 1.0,
            createdAt: emb.createdAt ?? 0,
            lastAccessedAt: emb.lastAccessedAt ?? 0,
            isPinned: emb.isPinned ?? false,
            cycle,
            category: ((emb as Record<string, unknown>).category as string | null) ?? null,
          };
        })
        .sort((a, b) => {
          if (a.isPinned !== b.isPinned) return a.isPinned ? -1 : 1;
          if (a.isCold !== b.isCold) return a.isCold ? 1 : -1;
          if (a.importanceScore !== b.importanceScore) {
            return b.importanceScore - a.importanceScore;
          }
          if (a.lastAccessedAt !== b.lastAccessedAt) {
            return b.lastAccessedAt - a.lastAccessedAt;
          }
          return b.createdAt - a.createdAt;
        });
    }

    if (!session?.memories) return [];
    return session.memories
      .map((text, index) => {
        const id = `mem-${index}`;
        let cycle = cycleMap.textMap.get(text);

        return {
          text,
          index,
          isAi: false,
          id,
          tokenCount: 0,
          isCold: false,
          importanceScore: 1.0,
          createdAt: 0,
          lastAccessedAt: 0,
          isPinned: false,
          cycle,
          category: null as string | null,
        };
      })
      .sort((a, b) => {
        if (a.isPinned !== b.isPinned) return a.isPinned ? -1 : 1;
        if (a.isCold !== b.isCold) return a.isCold ? 1 : -1;
        return b.importanceScore - a.importanceScore;
      });
  }, [isDynamic, session, cycleMap]);

  const categories = useMemo(() => {
    const cats = new Set(memoryItems.map((m) => m.category).filter(Boolean));
    return Array.from(cats).sort() as string[];
  }, [memoryItems]);

  const filteredMemories = useMemo(() => {
    let items = memoryItems;
    if (ui.searchTerm.trim()) {
      items = items.filter((item) => item.text.toLowerCase().includes(ui.searchTerm.toLowerCase()));
    }
    if (ui.selectedCategory) {
      items = items.filter((item) => item.category === ui.selectedCategory);
    }
    return items;
  }, [memoryItems, ui.searchTerm, ui.selectedCategory]);

  const stats = useMemo(() => {
    const total = memoryItems.length;
    const ai = memoryItems.filter((m) => m.isAi).length;
    const user = total - ai;
    const totalMemoryTokens = memoryItems.reduce((sum, m) => sum + m.tokenCount, 0);
    const summaryTokens = isDynamic ? session?.memorySummaryTokenCount || 0 : 0;
    const totalTokens = totalMemoryTokens + summaryTokens;
    return { total, ai, user, totalMemoryTokens, summaryTokens, totalTokens };
  }, [isDynamic, memoryItems, session?.memorySummaryTokenCount]);

  const refreshPinnedMessages = useCallback(async () => {
    if (!session?.id || !isDynamic) return;
    const pinned = await listPinnedMessages(session.id).catch(() => [] as StoredMessage[]);
    setPinnedMessages(pinned);
  }, [isDynamic, session?.id, setPinnedMessages]);

  const handleUnpin = useCallback(
    async (messageId: string) => {
      if (!session || !isDynamic) return;
      try {
        await toggleMessagePin(session.id, messageId);
        await refreshPinnedMessages();
        dispatch({ type: "SET_ACTION_ERROR", value: null });
      } catch (err: any) {
        console.error("Failed to unpin message:", err);
        dispatch({ type: "SET_ACTION_ERROR", value: err?.message || "Failed to unpin message" });
      }
    },
    [session, isDynamic, refreshPinnedMessages],
  );

  const handleScrollToMessage = useCallback(
    (messageId: string) => {
      const extra = messageId ? { jumpToMessage: messageId } : undefined;
      go(Routes.chatSession(characterId!, session?.id || undefined, extra));
    },
    [go, characterId, session?.id],
  );

  const handleAddNew = useCallback(
    async (categoryOverride?: string) => {
      const trimmed = ui.newMemory.trim();
      if (!trimmed) return;
      const category = (categoryOverride ?? ui.newMemoryCategory).trim();
      const normalizedCategory = isValidMemoryCategory(category) ? category : undefined;

      dispatch({ type: "SET_IS_ADDING", value: true });
      try {
        await handleAdd(trimmed, normalizedCategory);
        dispatch({ type: "SET_NEW_MEMORY", value: "" });
        dispatch({ type: "SET_NEW_MEMORY_CATEGORY", value: "" });
        dispatch({ type: "SET_ACTION_ERROR", value: null });
      } catch (err: any) {
        console.error("Failed to add memory:", err);
        dispatch({ type: "SET_ACTION_ERROR", value: err?.message || "Failed to add memory" });
      } finally {
        dispatch({ type: "SET_IS_ADDING", value: false });
      }
    },
    [handleAdd, ui.newMemory, ui.newMemoryCategory],
  );

  const saveEdit = useCallback(
    async (index: number) => {
      const trimmed = ui.editingValue.trim();
      const category = ui.editingCategory.trim();
      const normalizedCategory = isValidMemoryCategory(category) ? category : "";
      const previousCategory = (memoryItems[index]?.category ?? "").trim();
      if (
        !trimmed ||
        (trimmed === memoryItems[index]?.text && normalizedCategory === previousCategory)
      ) {
        dispatch({ type: "CANCEL_EDIT" });
        return;
      }
      try {
        await handleUpdate(index, trimmed, normalizedCategory || undefined);
        dispatch({ type: "CANCEL_EDIT" });
        dispatch({ type: "SET_ACTION_ERROR", value: null });
      } catch (err: any) {
        console.error("Failed to update memory:", err);
        dispatch({ type: "SET_ACTION_ERROR", value: err?.message || "Failed to update memory" });
      }
    },
    [handleUpdate, memoryItems, ui.editingCategory, ui.editingValue],
  );

  const handleSaveSummaryClick = useCallback(async () => {
    if (ui.summaryDraft === session?.memorySummary) return;
    dispatch({ type: "SET_IS_SAVING_SUMMARY", value: true });
    try {
      await handleSaveSummary(ui.summaryDraft);
      dispatch({ type: "MARK_SUMMARY_SAVED" });
    } catch (err: any) {
      console.error("Failed to save summary:", err);
      dispatch({ type: "SET_ACTION_ERROR", value: err?.message || "Failed to save summary" });
      dispatch({ type: "SET_IS_SAVING_SUMMARY", value: false });
    }
  }, [handleSaveSummary, session?.memorySummary, ui.summaryDraft]);

  // Add memory category picker
  const [showAddCategoryMenu, setShowAddCategoryMenu] = useState(false);
  const [showSummaryEditor, setShowSummaryEditor] = useState(false);

  // Model selection for retry
  const [showModelSelector, setShowModelSelector] = useState(false);
  const [retryingWithModel, setRetryingWithModel] = useState(false);

  useEffect(() => {
    if (!isDynamic) return;
    // Load available models when component mounts or menu opens
    const loadModels = async () => {
      setLoadingModels(true);
      try {
        const settings = await readSettings();
        setAllModels(settings.models);
      } catch (err) {
        console.error("Failed to load models:", err);
      } finally {
        setLoadingModels(false);
      }
    };
    void loadModels();
  }, [isDynamic]);

  const filteredModels = useMemo(() => {
    if (!modelSearchQuery) return allModels;
    const query = modelSearchQuery.toLowerCase();
    return allModels.filter(
      (m) => m.name.toLowerCase().includes(query) || m.displayName.toLowerCase().includes(query),
    );
  }, [allModels, modelSearchQuery]);

  const tabs = useMemo(() => {
    if (!isDynamic) {
      return [{ id: "memories" as const, icon: Bot, label: "Memories" }];
    }
    return [
      { id: "memories" as const, icon: Bot, label: "Memories" },
      { id: "pinned" as const, icon: Pin, label: "Pinned" },
      { id: "tools" as const, icon: Clock, label: "Activity" },
    ];
  }, [isDynamic]);

  const handleRetryWithModel = useCallback(
    async (modelId?: string) => {
      if (!session?.id || !isDynamic) return;
      setRetryingWithModel(true);
      setShowModelSelector(false);
      dispatch({ type: "SET_RETRY_STATUS", value: "retrying" });
      try {
        // If modelId is provided, we use it and update default
        // If not, it's a simple retry with existing settings
        await storageBridge.retryDynamicMemory(session.id, modelId, modelId ? true : undefined);
        dispatch({ type: "SET_RETRY_STATUS", value: "success" });
        window.setTimeout(() => {
          dispatch({ type: "SET_RETRY_STATUS", value: "idle" });
        }, 3000);
      } catch (err: any) {
        console.error("Failed to retry memory processing:", err);
        dispatch({ type: "SET_RETRY_STATUS", value: "idle" });
        void reload();
      } finally {
        setRetryingWithModel(false);
      }
    },
    [session?.id, isDynamic, reload],
  );

  const handleDismissError = useCallback(async () => {
    if (!session?.id || !session) return;
    try {
      await saveSession({ ...session, memoryStatus: "idle", memoryError: null });
      void reload();
      dispatch({ type: "SET_ACTION_ERROR", value: null });
    } catch (err) {
      console.error("Failed to dismiss error:", err);
    }
  }, [session, reload]);

  const handleRetry = useCallback(async () => {
    await handleRetryWithModel();
  }, [handleRetryWithModel]);

  const handleTriggerManual = useCallback(async () => {
    if (!sessionId || !isDynamic) return;
    try {
      await storageBridge.triggerDynamicMemory(sessionId);
      dispatch({ type: "SET_ACTION_ERROR", value: null });
    } catch (err: any) {
      dispatch({
        type: "SET_ACTION_ERROR",
        value: err?.message || "Failed to trigger memory processing",
      });
    }
  }, [sessionId, isDynamic]);

  if (loading) {
    return (
      <div className={cn("flex h-screen items-center justify-center", colors.surface.base)}>
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-white/10 border-t-white/60" />
      </div>
    );
  }

  if (error || !session || !character) {
    return (
      <div
        className={cn(
          "flex min-h-screen flex-col items-center justify-center gap-4 px-4 text-center",
          colors.surface.base,
        )}
      >
        <p className={cn("text-sm", colors.text.secondary)}>{error || "Session not found"}</p>
        <button
          onClick={() =>
            backOrReplace(characterId ? Routes.chatSession(characterId, sessionId) : Routes.chat)
          }
          className={cn(
            components.button.primary,
            components.button.sizes.md,
            "bg-white/5 text-white hover:bg-white/10",
          )}
        >
          Go back
        </button>
      </div>
    );
  }

  return (
    <div className={cn("flex min-h-screen flex-col", colors.surface.base, colors.text.primary)}>
      {/* Header */}
      <header
        className={cn(
          "sticky top-0 z-20 border-b border-white/10 px-4",
          "pt-[calc(env(safe-area-inset-top)+24px)] pb-3",
          colors.glass.strong,
        )}
      >
        <div className="flex items-center gap-3">
          <div className="flex flex-1 items-center min-w-0">
            <button
              onClick={() =>
                backOrReplace(
                  characterId ? Routes.chatSession(characterId, sessionId) : Routes.chat,
                )
              }
              className={cn(
                "flex shrink-0 items-center justify-center -ml-2 px-2 py-1",
                colors.text.primary,
                interactive.transition.fast,
                "hover:text-white/80",
              )}
              aria-label="Go back"
            >
              <ArrowLeft size={14} strokeWidth={2.5} />
            </button>
            <div className="min-w-0 flex-1 flex items-baseline gap-2 text-left">
              <span
                className={cn(
                  "shrink-0",
                  typography.h1.size,
                  typography.h1.weight,
                  colors.text.primary,
                )}
              >
                Memories
              </span>
              <span className={cn("truncate text-sm font-medium", colors.text.tertiary)}>
                {character.name}
              </span>
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-2 ml-auto">
            {isDynamic && session.memoryStatus === "processing" && (
              <div
                className={cn(
                  radius.full,
                  "border px-2 py-1",
                  typography.overline.size,
                  typography.overline.weight,
                  typography.overline.tracking,
                  typography.overline.transform,
                  "border-blue-500/30 bg-blue-500/15 text-blue-200",
                )}
              >
                Processing
              </div>
            )}
          </div>
        </div>

        {/* Segmented Tab Control */}
        {isDynamic && (
          <div className="mt-3 flex bg-white/5 border border-white/8 rounded-xl p-1">
            {tabs.map(({ id, icon: Icon, label }) => (
              <button
                key={id}
                onClick={() => dispatch({ type: "SET_TAB", tab: id })}
                className={cn(
                  "relative flex-1 flex items-center justify-center gap-1.5 rounded-lg py-2 text-xs font-semibold transition-colors",
                  ui.activeTab === id ? "text-white" : "text-white/40 hover:text-white/60",
                )}
                aria-label={label}
              >
                {ui.activeTab === id && (
                  <motion.div
                    layoutId="memoryTabIndicator"
                    className="absolute inset-0 rounded-lg bg-white/10 border border-white/10"
                    transition={{ type: "spring", stiffness: 320, damping: 28 }}
                  />
                )}
                <Icon size={14} className="relative z-10" />
                <span className="relative z-10">{label}</span>
              </button>
            ))}
          </div>
        )}
      </header>

      <main className="flex-1 overflow-y-auto pb-[calc(env(safe-area-inset-bottom)+16px)]">
        {/* Error Banner */}
        {(ui.actionError ||
          (isDynamic &&
            (session.memoryError ||
              ui.retryStatus !== "idle" ||
              session.memoryStatus === "processing"))) && (
          <div className="px-3 pt-3">
            {isDynamic &&
            (ui.retryStatus === "retrying" || session.memoryStatus === "processing") ? (
              <div
                className={cn(
                  radius.md,
                  "bg-blue-500/10 border border-blue-500/20 p-3 flex items-center gap-3 animate-pulse",
                )}
              >
                <RefreshCw className="h-5 w-5 text-blue-400 shrink-0 animate-spin" />
                <div className={cn("flex-1", typography.body.size, "text-blue-200")}>
                  <p className="font-semibold">
                    {session.memoryStatus === "processing"
                      ? "AI is organizing memories..."
                      : "Retrying Memory Cycle..."}
                  </p>
                </div>
              </div>
            ) : isDynamic && ui.retryStatus === "success" ? (
              <div
                className={cn(
                  radius.md,
                  "bg-emerald-500/10 border border-emerald-500/20 p-3 flex items-center gap-3",
                )}
              >
                <Check className="h-5 w-5 text-emerald-400 shrink-0" />
                <div className={cn("flex-1", typography.body.size, "text-emerald-200")}>
                  <p className="font-semibold">Memory Cycle Processed Successfully!</p>
                </div>
                <button
                  onClick={() => dispatch({ type: "SET_RETRY_STATUS", value: "idle" })}
                  className="text-emerald-400 hover:text-emerald-300"
                >
                  <X size={16} />
                </button>
              </div>
            ) : ui.actionError || (isDynamic && session.memoryError) ? (
              <div
                className={cn(
                  radius.md,
                  "bg-red-500/10 border border-red-500/20 p-3 flex items-start gap-3",
                )}
              >
                <AlertTriangle className="h-5 w-5 text-red-400 shrink-0" />
                <div className={cn("flex-1", typography.body.size, "text-red-200")}>
                  <div className="flex items-start justify-between">
                    <p className="font-semibold mb-1">
                      {isDynamic ? "Memory System Error" : "Error"}
                    </p>
                    {isDynamic && (
                      <button
                        onClick={handleDismissError}
                        className="text-red-400/60 hover:text-red-400 transition-colors"
                        title="Dismiss error"
                      >
                        <X size={16} />
                      </button>
                    )}
                  </div>
                  <p className="opacity-90">
                    {ui.actionError || (isDynamic ? session.memoryError : null)}
                  </p>
                  {isDynamic && (ui.actionError || session.memoryError) && (
                    <div className="mt-2 flex flex-wrap gap-2">
                      <button
                        onClick={handleRetry}
                        className={cn(
                          "flex items-center gap-1.5 px-2.5 py-1.5",
                          radius.md,
                          typography.bodySmall.size,
                          "font-semibold bg-red-500/20 text-red-200",
                          interactive.transition.fast,
                          "hover:bg-red-500/30",
                          interactive.active.scale,
                        )}
                      >
                        <RefreshCw size={12} />
                        Try Again
                      </button>
                      <button
                        onClick={() => setShowModelSelector(true)}
                        className={cn(
                          "flex items-center gap-1.5 px-2.5 py-1.5",
                          radius.md,
                          typography.bodySmall.size,
                          "font-semibold bg-blue-500/20 text-blue-200",
                          interactive.transition.fast,
                          "hover:bg-blue-500/30",
                          interactive.active.scale,
                        )}
                      >
                        <Cpu size={12} />
                        Try Different Model
                      </button>
                    </div>
                  )}
                </div>
                {!isDynamic && (
                  <button
                    onClick={() => dispatch({ type: "SET_ACTION_ERROR", value: null })}
                    className="text-red-400 hover:text-red-300"
                  >
                    <X size={16} />
                  </button>
                )}
              </div>
            ) : null}
          </div>
        )}

        <AnimatePresence mode="wait">
          {ui.activeTab === "memories" ? (
            <motion.div
              key="memories"
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -8 }}
              transition={{ duration: 0.15, ease: "easeOut" }}
              className={cn("px-3 py-4", "space-y-5")}
            >
              {/* Context Summary */}
              {isDynamic && (
                <button
                  type="button"
                  onClick={() => setShowSummaryEditor(true)}
                  className={cn(
                    "w-full rounded-xl border border-emerald-400/15 bg-emerald-400/3 px-4 py-3 text-left",
                    "transition-all hover:border-emerald-400/25 hover:bg-emerald-400/5 active:scale-[0.99]",
                  )}
                >
                  <div className="flex items-center gap-2 mb-1.5">
                    <Sparkles size={13} className="text-emerald-400/70 shrink-0" />
                    <span className="text-[11px] font-semibold text-emerald-300/80 uppercase tracking-wider">
                      Context Summary
                    </span>
                    {session?.memorySummaryTokenCount && session.memorySummaryTokenCount > 0 ? (
                      <span className="text-[10px] text-white/30 ml-auto">
                        {session.memorySummaryTokenCount.toLocaleString()} tokens
                      </span>
                    ) : null}
                  </div>
                  <p
                    className={cn(
                      typography.bodySmall.size,
                      "leading-relaxed line-clamp-4 min-h-14",
                      ui.summaryDraft ? "text-emerald-50/70" : "text-emerald-200/25 italic",
                    )}
                  >
                    {ui.summaryDraft || "Tap to add a context summary..."}
                  </p>
                </button>
              )}

              {/* Memories Section */}
              <div>
                <div className="flex items-center gap-2 mb-2">
                  <span
                    className={cn(
                      "text-[12px] font-semibold uppercase tracking-wider text-white/50",
                    )}
                  >
                    {ui.searchTerm.trim()
                      ? `Results (${filteredMemories.length})`
                      : "Saved Memories"}
                  </span>
                  <span className={cn("text-[10px] text-white/30 ml-auto")}>
                    {stats.ai} AI · {stats.user} You
                  </span>
                </div>

                {/* Search + Add row */}
                <div className="flex items-center gap-2 mb-3">
                  <div className="relative flex-1 min-w-0">
                    <Search
                      className={cn(
                        "absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4",
                        colors.text.tertiary,
                      )}
                    />
                    <input
                      type="text"
                      value={ui.searchTerm}
                      onChange={(e) => dispatch({ type: "SET_SEARCH", value: e.target.value })}
                      placeholder="Search memories..."
                      className={cn(
                        "w-full pl-10 pr-10 py-2.5",
                        components.input.base,
                        radius.lg,
                        "text-sm text-white placeholder-white/40",
                      )}
                    />
                    {ui.searchTerm.trim().length > 0 && (
                      <button
                        type="button"
                        onClick={() => dispatch({ type: "CLEAR_SEARCH" })}
                        className={cn(
                          "absolute right-3 top-1/2 -translate-y-1/2",
                          colors.text.tertiary,
                          "hover:text-white",
                          interactive.transition.fast,
                        )}
                        aria-label="Clear search"
                      >
                        <X size={16} />
                      </button>
                    )}
                  </div>
                  <button
                    onClick={() => setShowAddCategoryMenu(true)}
                    className={cn(
                      "flex items-center justify-center shrink-0",
                      "h-10.5 w-10.5 rounded-lg",
                      "border border-white/10 bg-white/5",
                      "text-white/50",
                      "hover:bg-white/8 hover:text-white/70",
                      "transition-all active:scale-95",
                    )}
                    aria-label="Add memory"
                  >
                    <Plus size={18} />
                  </button>
                </div>

                {/* Category Filter Chips */}
                {isDynamic && categories.length > 0 && (
                  <div className="flex flex-wrap gap-1.5 mb-3">
                    <button
                      type="button"
                      onClick={() => dispatch({ type: "SET_CATEGORY", value: null })}
                      className={cn(
                        "px-2 py-0.5 rounded-full text-[11px] font-medium border transition-colors",
                        !ui.selectedCategory
                          ? "bg-white/12 text-white/80 border-white/20"
                          : "bg-white/4 text-white/35 border-white/8 hover:bg-white/8",
                      )}
                    >
                      All
                    </button>
                    {categories.map((cat) => (
                      <button
                        key={cat}
                        type="button"
                        onClick={() =>
                          dispatch({
                            type: "SET_CATEGORY",
                            value: ui.selectedCategory === cat ? null : cat,
                          })
                        }
                        className={cn(
                          "px-2 py-0.5 rounded-full text-[11px] font-medium border transition-colors",
                          ui.selectedCategory === cat
                            ? "bg-white/12 text-white/80 border-white/20"
                            : "bg-white/4 text-white/35 border-white/8 hover:bg-white/8",
                        )}
                      >
                        {cat.replace(/_/g, " ")}
                      </button>
                    ))}
                  </div>
                )}

                {/* Memory List */}
                {filteredMemories.length === 0 ? (
                  <motion.div
                    initial={{ opacity: 0, y: 20 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.3, ease: "easeOut" }}
                    className="flex flex-col items-center justify-center py-16"
                  >
                    <div className="flex h-16 w-16 items-center justify-center rounded-2xl border border-white/10 bg-white/5 mb-4">
                      {ui.searchTerm ? (
                        <Search className="h-7 w-7 text-white/20" />
                      ) : (
                        <Bot className="h-7 w-7 text-white/20" />
                      )}
                    </div>
                    <h3 className="mb-1 text-base font-semibold text-white">
                      {ui.searchTerm ? "No matching memories" : "No memories yet"}
                    </h3>
                    <p className="text-center text-sm text-white/40 max-w-60">
                      {ui.searchTerm
                        ? "Try a different search term"
                        : "Tap the Add button above to create one"}
                    </p>
                  </motion.div>
                ) : (
                  <motion.div
                    className="space-y-3"
                    initial="hidden"
                    animate="visible"
                    variants={{ visible: { transition: { staggerChildren: 0.03 } } }}
                  >
                    <AnimatePresence>
                      {filteredMemories.map((item) => {
                        const expanded = ui.expandedMemories.has(item.index);

                        return (
                          <motion.div
                            key={item.id}
                            layout
                            variants={{
                              hidden: { opacity: 0, y: 12 },
                              visible: { opacity: 1, y: 0 },
                            }}
                            exit={{ opacity: 0, scale: 0.95 }}
                            transition={{ duration: 0.15 }}
                            className={cn(
                              "group relative overflow-hidden rounded-xl",
                              "border",
                              expanded
                                ? "border-white/10 bg-white/2"
                                : "border-white/6 bg-white/2 hover:border-white/10 hover:bg-white/3",
                            )}
                          >
                            <div
                              className={cn("px-4 py-3 cursor-pointer")}
                              onClick={() =>
                                dispatch({ type: "TOGGLE_EXPANDED", index: item.index })
                              }
                              role="button"
                              tabIndex={0}
                              onKeyDown={(e) => {
                                if (e.key === "Enter" || e.key === " ") {
                                  e.preventDefault();
                                  dispatch({ type: "TOGGLE_EXPANDED", index: item.index });
                                }
                              }}
                            >
                              {/* Top row: source icon + text + overflow */}
                              <div className="flex items-start gap-2">
                                <div className="shrink-0 mt-0.5">
                                  {item.isAi ? (
                                    <Bot size={14} className="text-blue-400" />
                                  ) : (
                                    <User size={14} className="text-emerald-400" />
                                  )}
                                </div>
                                <motion.div className="flex-1 min-w-0" layout>
                                  <p
                                    className={cn(
                                      typography.bodySmall.size,
                                      colors.text.secondary,
                                      "leading-relaxed",
                                      expanded ? "whitespace-pre-wrap" : "line-clamp-3",
                                    )}
                                  >
                                    {item.text}
                                  </p>
                                </motion.div>
                                {/* Overflow Button */}
                                <button
                                  type="button"
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    dispatch({ type: "OPEN_MEMORY_ACTIONS", id: item.id });
                                  }}
                                  className={cn(
                                    "flex items-center justify-center shrink-0 p-2.5 -m-2 -mr-1",
                                    "rounded-lg text-white/30",
                                    "transition-all hover:bg-white/5 hover:text-white/60",
                                    "active:scale-95",
                                  )}
                                  aria-label="Memory actions"
                                >
                                  <EllipsisVertical size={16} />
                                </button>
                              </div>

                              {/* Bottom row: category + pin */}
                              {(item.category || item.isPinned) && (
                                <div className="flex items-center justify-between mt-2">
                                  <div className="flex items-center gap-1.5">
                                    {item.category && (
                                      <span
                                        className={cn(
                                          "inline-flex items-center px-1.5 py-0.5",
                                          radius.md,
                                          "text-[10px] font-medium",
                                          "bg-white/5 text-white/40 border border-white/8",
                                        )}
                                      >
                                        {item.category.replace(/_/g, " ")}
                                      </span>
                                    )}
                                  </div>
                                  {item.isPinned && <Pin size={12} className="text-amber-400/60" />}
                                </div>
                              )}

                              {/* Expanded metadata */}
                              <AnimatePresence>
                                {expanded && (
                                  <motion.div
                                    initial={{ opacity: 0, height: 0 }}
                                    animate={{ opacity: 1, height: "auto" }}
                                    exit={{ opacity: 0, height: 0 }}
                                    transition={{ duration: 0.15 }}
                                    className="overflow-hidden"
                                  >
                                    <div
                                      className={cn(
                                        "flex items-center gap-3 mt-2 pt-2 border-t border-white/5",
                                        "text-[10px] text-white/30",
                                      )}
                                    >
                                      {item.tokenCount > 0 && (
                                        <span>{item.tokenCount.toLocaleString()} tokens</span>
                                      )}
                                      {item.cycle && <span>Cycle {item.cycle}</span>}
                                      {item.lastAccessedAt > 0 && (
                                        <span>
                                          Accessed{" "}
                                          {new Date(item.lastAccessedAt).toLocaleDateString()}
                                        </span>
                                      )}
                                      {isDynamic && (
                                        <span
                                          className={
                                            item.isCold ? "text-blue-400/50" : "text-amber-400/50"
                                          }
                                        >
                                          {item.isCold
                                            ? "Cold"
                                            : `Hot ${item.importanceScore.toFixed(1)}`}
                                        </span>
                                      )}
                                    </div>
                                  </motion.div>
                                )}
                              </AnimatePresence>
                            </div>
                          </motion.div>
                        );
                      })}
                    </AnimatePresence>
                  </motion.div>
                )}
              </div>
            </motion.div>
          ) : isDynamic && ui.activeTab === "tools" ? (
            <motion.div
              key="tools"
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -8 }}
              transition={{ duration: 0.15, ease: "easeOut" }}
              className={cn("px-3 py-4", "space-y-5")}
            >
              <div className="flex items-center gap-2 mb-3">
                <span className="text-[12px] font-semibold uppercase tracking-wider text-white/50">
                  Activity Log
                </span>
                <span className="text-[10px] text-white/20 ml-auto">
                  {(session.memoryToolEvents?.length ?? 0).toLocaleString()} events
                </span>
                <button
                  onClick={handleTriggerManual}
                  disabled={session?.memoryStatus === "processing"}
                  className={cn(
                    "flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg",
                    "border border-white/10 bg-white/5",
                    "text-[11px] font-semibold text-white/50",
                    "hover:bg-white/8 hover:text-white/70",
                    "disabled:opacity-40 disabled:pointer-events-none",
                    "transition-all active:scale-95",
                  )}
                >
                  <Cpu
                    size={12}
                    className={cn(session?.memoryStatus === "processing" && "animate-pulse")}
                  />
                  Run
                </button>
              </div>
              <ToolLog events={(session.memoryToolEvents as MemoryToolEvent[]) || []} />
            </motion.div>
          ) : isDynamic && ui.activeTab === "pinned" ? (
            <motion.div
              key="pinned"
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -8 }}
              transition={{ duration: 0.15, ease: "easeOut" }}
              className={cn("px-3 py-4", "space-y-5")}
            >
              <SectionHeader
                icon={Pin}
                title="Pinned Messages"
                subtitle="Always included in context"
                right={
                  <span
                    className={cn(
                      typography.caption.size,
                      "inline-flex items-center gap-1 px-2 py-0.5",
                      radius.full,
                      "border bg-white/5",
                      colors.border.subtle,
                      colors.text.secondary,
                    )}
                  >
                    {pinnedMessages.length.toLocaleString()}
                  </span>
                }
              />
              {pinnedMessages.length === 0 ? (
                <motion.div
                  initial={{ opacity: 0, y: 20 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ duration: 0.3, ease: "easeOut" }}
                  className="flex flex-col items-center justify-center py-16"
                >
                  <div className="flex h-16 w-16 items-center justify-center rounded-2xl border border-white/10 bg-white/5 mb-4">
                    <Pin className="h-7 w-7 text-white/20" />
                  </div>
                  <h3 className="mb-1 text-base font-semibold text-white">No pinned messages</h3>
                  <p className="text-center text-sm text-white/40 max-w-60">
                    Pin important messages from the chat to always include them in context
                  </p>
                </motion.div>
              ) : (
                <div className={cn(spacing.field)}>
                  {pinnedMessages.map((msg) => {
                    const isUser = msg.role === "user";
                    const isAssistant = msg.role === "assistant";
                    const timestamp = new Date(msg.createdAt).toLocaleString();

                    return (
                      <div
                        key={msg.id}
                        className={cn(
                          components.card.base,
                          components.card.interactive,
                          "w-full p-4",
                          isUser
                            ? "border-emerald-400/30"
                            : isAssistant
                              ? "border-blue-400/30"
                              : "border-white/10",
                        )}
                      >
                        <div className="flex items-start gap-3">
                          <div
                            className={cn(
                              "flex h-8 w-8 shrink-0 items-center justify-center",
                              radius.full,
                              "border text-white/70",
                              interactive.transition.default,
                              isUser
                                ? "border-emerald-400/30 bg-emerald-400/10"
                                : isAssistant
                                  ? "border-blue-400/30 bg-blue-400/10"
                                  : "border-white/10 bg-white/5",
                            )}
                          >
                            {isUser ? (
                              <User className="h-4 w-4 text-emerald-400" />
                            ) : isAssistant ? (
                              <Bot className="h-4 w-4 text-blue-400" />
                            ) : (
                              <MessageSquare className="h-4 w-4 text-white/60" />
                            )}
                          </div>
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-2 mb-1">
                              <span
                                className={cn(
                                  typography.caption.size,
                                  "font-semibold uppercase tracking-wide",
                                  isUser
                                    ? "text-emerald-400"
                                    : isAssistant
                                      ? "text-blue-400"
                                      : colors.text.tertiary,
                                )}
                              >
                                {isUser ? "User" : isAssistant ? "Assistant" : msg.role}
                              </span>
                              <span className={cn(typography.caption.size, colors.text.disabled)}>
                                {timestamp}
                              </span>
                            </div>
                            <p
                              className={cn(
                                typography.bodySmall.size,
                                colors.text.secondary,
                                "leading-relaxed whitespace-pre-wrap wrap-break-word",
                              )}
                            >
                              {msg.content}
                            </p>
                          </div>
                        </div>
                        <div className="flex items-center gap-4 mt-3 pl-11">
                          <button
                            onClick={() => handleScrollToMessage(msg.id)}
                            className={cn(
                              typography.caption.size,
                              "font-medium flex items-center gap-1.5",
                              colors.text.tertiary,
                              "hover:text-white transition-colors",
                            )}
                          >
                            <MessageSquare size={12} />
                            Scroll to message
                          </button>
                          <button
                            onClick={() => handleUnpin(msg.id)}
                            className={cn(
                              typography.caption.size,
                              "font-medium flex items-center gap-1.5",
                              colors.text.tertiary,
                              "hover:text-amber-400 transition-colors",
                            )}
                          >
                            <Pin size={12} />
                            Unpin
                          </button>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </motion.div>
          ) : null}
        </AnimatePresence>
      </main>

      {/* Summary Editor BottomMenu */}
      <BottomMenu
        isOpen={showSummaryEditor}
        onClose={() => setShowSummaryEditor(false)}
        title="Context Summary"
      >
        <div className="space-y-4 text-white">
          <textarea
            value={ui.summaryDraft}
            onChange={(e) => dispatch({ type: "SET_SUMMARY_DRAFT", value: e.target.value })}
            rows={6}
            className={cn(
              "w-full p-3",
              radius.lg,
              "border border-white/10 bg-black/30",
              "text-sm text-white/90 resize-none leading-relaxed",
              "focus:border-white/20 focus:outline-none focus:ring-1 focus:ring-white/10",
              "placeholder:text-white/30",
            )}
            placeholder="Short recap used to keep context consistent across messages..."
            autoFocus
          />
          {session?.memorySummaryTokenCount && session.memorySummaryTokenCount > 0 ? (
            <p className="text-[10px] text-white/30">
              {session.memorySummaryTokenCount.toLocaleString()} tokens
            </p>
          ) : null}
          <div className="flex gap-2">
            <button
              onClick={() => {
                dispatch({
                  type: "SYNC_SUMMARY_FROM_SESSION",
                  value: session?.memorySummary ?? "",
                });
                setShowSummaryEditor(false);
              }}
              className={cn(
                "flex-1 px-4 py-2.5",
                radius.lg,
                "border border-white/10 bg-white/5",
                "text-sm font-medium text-white/60",
                "transition-all hover:border-white/15 hover:bg-white/8 hover:text-white/80",
                "active:scale-[0.98]",
              )}
            >
              Cancel
            </button>
            <button
              onClick={async () => {
                await handleSaveSummaryClick();
                setShowSummaryEditor(false);
              }}
              disabled={ui.isSavingSummary || ui.summaryDraft === session?.memorySummary}
              className={cn(
                "flex-1 px-4 py-2.5 flex items-center justify-center gap-2",
                radius.lg,
                "border border-emerald-400/30 bg-emerald-500/15",
                "text-sm font-semibold text-emerald-200",
                "transition-all hover:border-emerald-400/50 hover:bg-emerald-500/25",
                "active:scale-[0.98]",
                "disabled:opacity-40 disabled:pointer-events-none",
              )}
            >
              {ui.isSavingSummary ? "Saving..." : "Save"}
            </button>
          </div>
        </div>
      </BottomMenu>

      {/* Add Memory BottomMenu */}
      <BottomMenu
        isOpen={showAddCategoryMenu}
        onClose={() => setShowAddCategoryMenu(false)}
        title="Add Memory"
      >
        <div className="space-y-4 text-white">
          <textarea
            value={ui.newMemory}
            onChange={(e) => dispatch({ type: "SET_NEW_MEMORY", value: e.target.value })}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey && ui.newMemory.trim()) {
                e.preventDefault();
                setShowAddCategoryMenu(false);
                void handleAddNew(ui.newMemoryCategory || undefined);
              }
            }}
            rows={3}
            className={cn(
              "w-full p-3",
              radius.lg,
              "border border-white/10 bg-black/30",
              "text-sm text-white/90 resize-none leading-relaxed",
              "focus:border-white/20 focus:outline-none focus:ring-1 focus:ring-white/10",
              "placeholder:text-white/30",
            )}
            placeholder="What should be remembered?"
            autoFocus
          />
          {isDynamic && (
            <div>
              <p className="text-[11px] font-medium text-white/30 uppercase tracking-wider mb-2">
                Category
              </p>
              <div className="flex flex-wrap gap-1.5">
                <button
                  type="button"
                  onClick={() => dispatch({ type: "SET_NEW_MEMORY_CATEGORY", value: "" })}
                  className={cn(
                    "px-2.5 py-1 rounded-full text-[11px] font-medium border transition-colors",
                    !ui.newMemoryCategory
                      ? "bg-white/12 text-white/80 border-white/20"
                      : "bg-white/4 text-white/35 border-white/8 hover:bg-white/8",
                  )}
                >
                  None
                </button>
                {MEMORY_CATEGORY_OPTIONS.map((category) => (
                  <button
                    key={category}
                    type="button"
                    onClick={() => dispatch({ type: "SET_NEW_MEMORY_CATEGORY", value: category })}
                    className={cn(
                      "px-2.5 py-1 rounded-full text-[11px] font-medium border transition-colors",
                      ui.newMemoryCategory === category
                        ? "bg-white/12 text-white/80 border-white/20"
                        : "bg-white/4 text-white/35 border-white/8 hover:bg-white/8",
                    )}
                  >
                    {formatMemoryCategoryLabel(category)}
                  </button>
                ))}
              </div>
            </div>
          )}
          <button
            onClick={() => {
              setShowAddCategoryMenu(false);
              void handleAddNew(ui.newMemoryCategory || undefined);
            }}
            disabled={!ui.newMemory.trim() || ui.isAdding}
            className={cn(
              "w-full px-4 py-2.5 flex items-center justify-center gap-2",
              radius.lg,
              "border border-emerald-400/30 bg-emerald-500/15",
              "text-sm font-semibold text-emerald-200",
              "transition-all hover:border-emerald-400/50 hover:bg-emerald-500/25",
              "active:scale-[0.98]",
              "disabled:opacity-40 disabled:pointer-events-none",
            )}
          >
            {ui.isAdding ? (
              <div className="h-4 w-4 animate-spin rounded-full border-2 border-emerald-400/30 border-t-emerald-400" />
            ) : (
              <>
                <Plus size={14} />
                Save Memory
              </>
            )}
          </button>
        </div>
      </BottomMenu>

      {/* Memory Actions BottomMenu */}
      <BottomMenu
        isOpen={ui.selectedMemoryId !== null}
        onClose={() => dispatch({ type: "CLOSE_MEMORY_ACTIONS" })}
        title={
          ui.memoryActionMode === "edit"
            ? "Edit Memory"
            : (() => {
                const mem = memoryItems.find((m) => m.id === ui.selectedMemoryId);
                const preview = mem?.text ?? "";
                return preview.length > 60 ? preview.slice(0, 60) + "..." : preview || "Memory";
              })()
        }
      >
        {(() => {
          const selectedItem = memoryItems.find((m) => m.id === ui.selectedMemoryId);
          if (!selectedItem) return null;

          if (ui.memoryActionMode === "edit") {
            return (
              <div className="space-y-4 text-white">
                <textarea
                  value={ui.editingValue}
                  onChange={(e) => dispatch({ type: "SET_EDIT_VALUE", value: e.target.value })}
                  rows={4}
                  className={cn(
                    "w-full p-3",
                    radius.lg,
                    "border border-white/10 bg-black/30",
                    "text-sm text-white/90 resize-none leading-relaxed",
                    "focus:border-white/20 focus:outline-none focus:ring-1 focus:ring-white/10",
                    "placeholder:text-white/30",
                  )}
                  placeholder="Enter memory content..."
                  autoFocus
                />
                {isDynamic && (
                  <div className="flex flex-wrap gap-1.5">
                    <button
                      type="button"
                      onClick={() => dispatch({ type: "SET_EDIT_CATEGORY", value: "" })}
                      className={cn(
                        "px-2.5 py-1 rounded-full text-[11px] font-medium border transition-colors",
                        !ui.editingCategory
                          ? "bg-white/12 text-white/80 border-white/20"
                          : "bg-white/4 text-white/35 border-white/8 hover:bg-white/8",
                      )}
                    >
                      No tag
                    </button>
                    {MEMORY_CATEGORY_OPTIONS.map((category) => (
                      <button
                        key={category}
                        type="button"
                        onClick={() => dispatch({ type: "SET_EDIT_CATEGORY", value: category })}
                        className={cn(
                          "px-2.5 py-1 rounded-full text-[11px] font-medium border transition-colors",
                          ui.editingCategory === category
                            ? "bg-white/12 text-white/80 border-white/20"
                            : "bg-white/4 text-white/35 border-white/8 hover:bg-white/8",
                        )}
                      >
                        {formatMemoryCategoryLabel(category)}
                      </button>
                    ))}
                  </div>
                )}
                <div className="flex gap-2">
                  <button
                    onClick={() => dispatch({ type: "SET_MEMORY_ACTION_MODE", mode: "actions" })}
                    className={cn(
                      "flex-1 px-4 py-2.5",
                      radius.lg,
                      "border border-white/10 bg-white/5",
                      "text-sm font-medium text-white/60",
                      "transition-all hover:border-white/15 hover:bg-white/8 hover:text-white/80",
                      "active:scale-[0.98]",
                    )}
                  >
                    Cancel
                  </button>
                  <button
                    onClick={async () => {
                      await saveEdit(selectedItem.index);
                      dispatch({ type: "CLOSE_MEMORY_ACTIONS" });
                    }}
                    className={cn(
                      "flex-1 px-4 py-2.5 flex items-center justify-center gap-2",
                      radius.lg,
                      "border border-emerald-400/30 bg-emerald-500/15",
                      "text-sm font-semibold text-emerald-200",
                      "transition-all hover:border-emerald-400/50 hover:bg-emerald-500/25",
                      "active:scale-[0.98]",
                    )}
                  >
                    <Check size={14} />
                    Save
                  </button>
                </div>
              </div>
            );
          }

          return (
            <div className="space-y-1 text-white">
              <MemoryActionRow
                icon={Edit2}
                label="Edit"
                iconBg="bg-blue-500/20"
                onClick={() => {
                  dispatch({
                    type: "START_EDIT",
                    index: selectedItem.index,
                    text: selectedItem.text,
                    category: selectedItem.category,
                  });
                  dispatch({ type: "SET_MEMORY_ACTION_MODE", mode: "edit" });
                }}
              />
              {isDynamic && (
                <MemoryActionRow
                  icon={selectedItem.isPinned ? PinOff : Pin}
                  label={selectedItem.isPinned ? "Unpin" : "Pin"}
                  iconBg="bg-amber-500/20"
                  onClick={async () => {
                    try {
                      await handleTogglePin(selectedItem.index);
                      dispatch({ type: "SET_ACTION_ERROR", value: null });
                    } catch (err: any) {
                      dispatch({
                        type: "SET_ACTION_ERROR",
                        value: err?.message || "Failed to toggle pin",
                      });
                    }
                    dispatch({ type: "CLOSE_MEMORY_ACTIONS" });
                  }}
                />
              )}
              {isDynamic && (
                <MemoryActionRow
                  icon={selectedItem.isCold ? Flame : Snowflake}
                  label={selectedItem.isCold ? "Set Hot" : "Set Cold"}
                  iconBg={selectedItem.isCold ? "bg-amber-500/20" : "bg-blue-500/20"}
                  disabled={ui.memoryTempBusy === selectedItem.index}
                  onClick={async () => {
                    await handleSetColdState(selectedItem.index, !selectedItem.isCold);
                    dispatch({ type: "CLOSE_MEMORY_ACTIONS" });
                  }}
                />
              )}

              <div className="h-px bg-white/5 my-2" />

              <MemoryActionRow
                icon={Trash2}
                label="Delete"
                variant="danger"
                onClick={async () => {
                  try {
                    await handleRemove(selectedItem.index);
                    dispatch({ type: "SET_ACTION_ERROR", value: null });
                    dispatch({ type: "SHIFT_EXPANDED_AFTER_DELETE", index: selectedItem.index });
                  } catch (err: any) {
                    dispatch({
                      type: "SET_ACTION_ERROR",
                      value: err?.message || "Failed to remove memory",
                    });
                  }
                  dispatch({ type: "CLOSE_MEMORY_ACTIONS" });
                }}
              />
            </div>
          );
        })()}
      </BottomMenu>

      {/* Model Selection BottomMenu */}
      <BottomMenu
        isOpen={showModelSelector}
        onClose={() => {
          setShowModelSelector(false);
          setModelSearchQuery("");
        }}
        title="Select Model for Retry"
      >
        <MenuSection className="pb-0">
          <div className="px-4 py-3">
            <div
              className={cn(
                "relative flex items-center",
                radius.md,
                "bg-white/5 border",
                colors.border.subtle,
              )}
            >
              <Search className="absolute left-3 h-4 w-4 text-white/30" />
              <input
                type="text"
                placeholder="Search models..."
                value={modelSearchQuery}
                onChange={(e) => setModelSearchQuery(e.target.value)}
                className="w-full bg-transparent py-2.5 pl-9 pr-4 text-sm text-white placeholder:text-white/20 focus:outline-none"
              />
              {modelSearchQuery && (
                <button
                  onClick={() => setModelSearchQuery("")}
                  className="absolute right-3 text-white/30 hover:text-white/50"
                >
                  <X size={14} />
                </button>
              )}
            </div>
          </div>
        </MenuSection>

        <div className="max-h-[60vh] overflow-y-auto pb-6">
          {loadingModels ? (
            <div className="flex flex-col items-center justify-center py-12">
              <RefreshCw className="h-6 w-6 animate-spin text-white/20" />
              <p className={cn(typography.bodySmall.size, colors.text.tertiary, "mt-3")}>
                Loading available models...
              </p>
            </div>
          ) : filteredModels.length === 0 ? (
            <div className="px-4 py-12 text-center">
              <p className={cn(typography.body.size, colors.text.secondary)}>
                No models found matching "{modelSearchQuery}"
              </p>
            </div>
          ) : (
            <div className="flex flex-col">
              {filteredModels.map((model) => {
                const isSelected = model.id === character.defaultModelId;
                return (
                  <button
                    key={model.id}
                    onClick={() => handleRetryWithModel(model.id)}
                    disabled={retryingWithModel}
                    className={cn(
                      "group relative flex w-full flex-col gap-1 px-4 py-3 transition",
                      isSelected ? "bg-white/5" : "hover:bg-white/5",
                    )}
                  >
                    <div className="flex items-center gap-3">
                      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-white/5">
                        {getProviderIcon(model.providerId)}
                      </div>
                      <div className="min-w-0 flex-1 text-left">
                        <div className="flex items-center gap-2">
                          <span
                            className={cn(
                              typography.body.size,
                              "font-semibold text-white truncate",
                            )}
                          >
                            {model.displayName}
                          </span>
                          {isSelected && (
                            <span className="flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-emerald-500/20">
                              <Check size={10} className="text-emerald-400" />
                            </span>
                          )}
                        </div>
                        <p
                          className={cn(typography.caption.size, colors.text.tertiary, "truncate")}
                        >
                          {model.providerLabel} • {model.name}
                        </p>
                      </div>
                    </div>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </BottomMenu>
    </div>
  );
}
