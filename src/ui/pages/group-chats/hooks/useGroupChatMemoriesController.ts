import { useCallback, useEffect, useMemo, useReducer, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import type { GroupMessage, GroupSession } from "../../../../core/storage/schemas";
import {
  SESSION_UPDATED_EVENT,
  groupSessionAddMemory,
  groupSessionRemoveMemory,
  groupSessionUpdateMemory,
  groupSessionToggleMemoryPin,
  groupSessionSetMemoryColdState,
  getGroupSession,
  listPinnedGroupMessages,
  toggleGroupMessagePin,
} from "../../../../core/storage/repo";
import { storageBridge } from "../../../../core/storage/files";
import {
  markMemoryToolEventReverted,
  reconstructMemoryStateFromEvents,
  summarizeRevertImpact,
} from "../../../../core/storage/memoryToolEvents";
import { confirmBottomMenu } from "../../../components/ConfirmBottomMenu";
import { initUi, uiReducer } from "../reducers/groupChatMemoriesReducer";

type MemoryItem = {
  text: string;
  index: number;
  isAi: boolean;
  id: string;
  tokenCount: number;
  isCold: boolean;
  importanceScore: number;
  createdAt: number;
  lastAccessedAt: number;
  isPinned: boolean;
  cycle?: string;
};

type MemoryStats = {
  total: number;
  ai: number;
  user: number;
  totalMemoryTokens: number;
  summaryTokens: number;
  totalTokens: number;
};

const isAbortError = (value: unknown) => {
  const message =
    typeof value === "string"
      ? value
      : typeof value === "object" && value && "message" in value
        ? String((value as { message?: unknown }).message ?? "")
        : "";
  const normalized = message.toLowerCase();
  return (
    normalized.includes("aborted") ||
    normalized.includes("cancelled") ||
    normalized.includes("canceled")
  );
};

function useGroupSessionData(sessionId?: string) {
  const [session, setSession] = useState<GroupSession | null>(null);
  const [pinnedMessages, setPinnedMessages] = useState<GroupMessage[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!sessionId) {
      setError("Missing sessionId");
      setLoading(false);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const [targetSession, pinned] = await Promise.all([
        getGroupSession(sessionId),
        listPinnedGroupMessages(sessionId).catch(() => [] as GroupMessage[]),
      ]);
      if (targetSession) {
        setSession(targetSession);
        setPinnedMessages(pinned);
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
  }, [sessionId]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    const handleSessionUpdated = () => {
      void load();
    };

    window.addEventListener(SESSION_UPDATED_EVENT, handleSessionUpdated);
    return () => {
      window.removeEventListener(SESSION_UPDATED_EVENT, handleSessionUpdated);
    };
  }, [load]);

  return { session, setSession, pinnedMessages, setPinnedMessages, loading, error, reload: load };
}

function useGroupMemoryActions(
  session: GroupSession | null,
  setSession: (s: GroupSession) => void,
) {
  const handleAdd = useCallback(
    async (memory: string) => {
      if (!session) return;
      try {
        const updated = await groupSessionAddMemory(session.id, memory);
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
        const updated = await groupSessionRemoveMemory(session.id, index);
        if (updated) setSession(updated);
      } catch (err: any) {
        throw err;
      }
    },
    [session, setSession],
  );

  const handleUpdate = useCallback(
    async (index: number, memory: string) => {
      if (!session) return;
      try {
        const updated = await groupSessionUpdateMemory(session.id, index, memory);
        if (updated) setSession(updated);
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
        const updated = await groupSessionToggleMemoryPin(session.id, index);
        if (updated) setSession(updated);
      } catch (err: any) {
        throw err;
      }
    },
    [session, setSession],
  );

  return { handleAdd, handleRemove, handleUpdate, handleTogglePin };
}

export function useGroupChatMemoriesController(groupSessionId?: string) {
  const { session, setSession, pinnedMessages, setPinnedMessages, loading, error, reload } =
    useGroupSessionData(groupSessionId);
  const { handleAdd, handleRemove, handleUpdate, handleTogglePin } = useGroupMemoryActions(
    session,
    (s) => setSession(s),
  );
  const [ui, dispatch] = useReducer(uiReducer, undefined, initUi);
  const [revertingEventId, setRevertingEventId] = useState<string | null>(null);

  const getLatestSessionSnapshot = useCallback(async (): Promise<GroupSession> => {
    if (!session?.id) {
      throw new Error("Session not loaded");
    }
    const latest = await getGroupSession(session.id);
    if (!latest) {
      throw new Error("Session not found");
    }
    return latest;
  }, [session?.id]);

  const handleSetColdState = useCallback(
    async (memoryIndex: number, isCold: boolean) => {
      if (!session?.id) return;
      dispatch({ type: "SET_MEMORY_TEMP_BUSY", value: memoryIndex });
      try {
        const updated = await groupSessionSetMemoryColdState(session.id, memoryIndex, isCold);
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
    [session?.id, setSession],
  );

  const handleSaveSummary = useCallback(
    async (summary: string) => {
      if (!session) return;
      try {
        const latest = await getLatestSessionSnapshot();
        await storageBridge.groupSessionUpdateMemories(
          latest.id,
          latest.memoryEmbeddings ?? [],
          summary,
          latest.memorySummaryTokenCount ?? 0,
        );
        setSession({ ...latest, memorySummary: summary });
      } catch (err: any) {
        throw err;
      }
    },
    [getLatestSessionSnapshot, session, setSession],
  );

  const handleSaveSummaryClick = useCallback(async () => {
    if (ui.summaryDraft === session?.memorySummary) return;
    dispatch({ type: "SET_IS_SAVING_SUMMARY", value: true });
    try {
      await handleSaveSummary(ui.summaryDraft);
      dispatch({ type: "MARK_SUMMARY_SAVED" });
      dispatch({ type: "SET_ACTION_ERROR", value: null });
    } catch (err: any) {
      console.error("Failed to save summary:", err);
      dispatch({ type: "SET_ACTION_ERROR", value: err?.message || "Failed to save summary" });
      dispatch({ type: "SET_IS_SAVING_SUMMARY", value: false });
    }
  }, [handleSaveSummary, session?.memorySummary, ui.summaryDraft]);

  useEffect(() => {
    if (!session?.id) return;
    let unlisteners: (() => void)[] = [];

    const setup = async () => {
      try {
        const u1 = await listen("group-dynamic-memory:processing", (e: any) => {
          if (e.payload?.sessionId === session.id) {
            dispatch({ type: "SET_MEMORY_STATUS", value: "processing" });
            void reload();
          }
        });
        const u2 = await listen("group-dynamic-memory:success", (e: any) => {
          if (e.payload?.sessionId === session.id) {
            dispatch({ type: "SET_MEMORY_STATUS", value: "idle" });
            dispatch({ type: "SET_ACTION_ERROR", value: null });
            void reload();
          }
        });
        const u3 = await listen("group-dynamic-memory:error", (e: any) => {
          if (e.payload?.sessionId === session.id) {
            const message = e.payload?.error || "Group memory cycle failed";
            dispatch({ type: "SET_ACTION_ERROR", value: message });
            dispatch({ type: "SET_MEMORY_STATUS", value: "failed" });
            void reload();
          }
        });
        const u4 = await listen("group-dynamic-memory:cancelled", (e: any) => {
          if (e.payload?.sessionId === session.id) {
            dispatch({ type: "SET_ACTION_ERROR", value: null });
            dispatch({ type: "SET_MEMORY_STATUS", value: "idle" });
            dispatch({ type: "SET_RETRY_STATUS", value: "idle" });
            void reload();
          }
        });
        const u5 = await listen("group-dynamic-memory:progress", (e: any) => {
          if (e.payload?.sessionId === session.id) {
            dispatch({ type: "SET_MEMORY_PROGRESS_STEP", value: e.payload.step });
          }
        });
        unlisteners.push(u1, u2, u3, u4, u5);
      } catch (err) {
        console.error("Failed to setup memory event listeners", err);
      }
    };

    setup();
    return () => {
      unlisteners.forEach((u) => u());
    };
  }, [session?.id, reload]);

  useEffect(() => {
    dispatch({ type: "SYNC_SUMMARY_FROM_SESSION", value: session?.memorySummary ?? "" });
  }, [session?.memorySummary]);

  const cycleMap = useMemo(() => {
    const map = new Map<string, string>();
    const textMap = new Map<string, string>();

    if (session?.memoryToolEvents) {
      session.memoryToolEvents.forEach((event: any) => {
        const cycleStr = `${event.windowStart || 0}-${event.windowEnd || 0}`;
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
  }, [session?.memoryToolEvents]);

  const memoryItems: MemoryItem[] = useMemo(() => {
    if (!session?.memoryEmbeddings) return [];
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
  }, [session, cycleMap]);

  const filteredMemories = useMemo(() => {
    if (!ui.searchTerm.trim()) return memoryItems;
    return memoryItems.filter((item) =>
      item.text.toLowerCase().includes(ui.searchTerm.toLowerCase()),
    );
  }, [memoryItems, ui.searchTerm]);

  const stats: MemoryStats = useMemo(() => {
    const total = memoryItems.length;
    const ai = memoryItems.filter((m) => m.isAi).length;
    const user = total - ai;
    const totalMemoryTokens = memoryItems.reduce((sum, m) => sum + m.tokenCount, 0);
    const summaryTokens = session?.memorySummaryTokenCount || 0;
    const totalTokens = totalMemoryTokens + summaryTokens;
    return { total, ai, user, totalMemoryTokens, summaryTokens, totalTokens };
  }, [memoryItems, session?.memorySummaryTokenCount]);

  const handleAddNew = useCallback(async () => {
    const trimmed = ui.newMemory.trim();
    if (!trimmed) return;

    dispatch({ type: "SET_IS_ADDING", value: true });
    try {
      await handleAdd(trimmed);
      dispatch({ type: "SET_NEW_MEMORY", value: "" });
      dispatch({ type: "SET_ACTION_ERROR", value: null });
    } catch (err: any) {
      console.error("Failed to add memory:", err);
      dispatch({ type: "SET_ACTION_ERROR", value: err?.message || "Failed to add memory" });
    } finally {
      dispatch({ type: "SET_IS_ADDING", value: false });
    }
  }, [handleAdd, ui.newMemory]);

  const startEdit = useCallback((index: number, text: string) => {
    dispatch({ type: "START_EDIT", index, text });
  }, []);

  const cancelEdit = useCallback(() => {
    dispatch({ type: "CANCEL_EDIT" });
  }, []);

  const saveEdit = useCallback(
    async (index: number) => {
      const currentItem = memoryItems.find((item) => item.index === index);
      const trimmed = ui.editingValue.trim();
      if (!trimmed || trimmed === currentItem?.text) {
        dispatch({ type: "CANCEL_EDIT" });
        return true;
      }
      try {
        await handleUpdate(index, trimmed);
        dispatch({ type: "CANCEL_EDIT" });
        dispatch({ type: "SET_ACTION_ERROR", value: null });
        return true;
      } catch (err: any) {
        console.error("Failed to update memory:", err);
        dispatch({ type: "SET_ACTION_ERROR", value: err?.message || "Failed to update memory" });
        return false;
      }
    },
    [handleUpdate, memoryItems, ui.editingValue],
  );

  const handleRunMemoryCycle = useCallback(async () => {
    if (!session?.id) return;
    dispatch({ type: "SET_RETRY_STATUS", value: "retrying" });
    dispatch({ type: "SET_MEMORY_STATUS", value: "processing" });
    try {
      await storageBridge.groupChatRetryDynamicMemory(session.id);
      dispatch({ type: "SET_RETRY_STATUS", value: "success" });
      window.setTimeout(() => {
        dispatch({ type: "SET_RETRY_STATUS", value: "idle" });
      }, 3000);
    } catch (err: any) {
      console.error("Failed to retry memory processing:", err);
      dispatch({ type: "SET_RETRY_STATUS", value: "idle" });
      if (isAbortError(err)) {
        dispatch({ type: "SET_ACTION_ERROR", value: null });
        dispatch({ type: "SET_MEMORY_STATUS", value: "idle" });
        void reload();
        return;
      }
      dispatch({ type: "SET_MEMORY_STATUS", value: "failed" });
      dispatch({ type: "SET_ACTION_ERROR", value: err?.message || "Failed to run memory cycle" });
      void reload();
    }
  }, [session?.id, reload]);

  const handleAbortMemoryCycle = useCallback(async () => {
    if (!session?.id) return;
    try {
      if (session.memoryStatus === "processing") {
        await storageBridge.groupChatAbortDynamicMemory(session.id);
      }
      const latest = await getLatestSessionSnapshot();
      await storageBridge.groupSessionUpdateMemories(
        latest.id,
        latest.memoryEmbeddings ?? [],
        latest.memorySummary ?? "",
        latest.memorySummaryTokenCount ?? 0,
        "idle",
        null,
      );
      setSession({ ...latest, memoryStatus: "idle", memoryError: null });
      dispatch({ type: "SET_ACTION_ERROR", value: null });
      dispatch({ type: "SET_RETRY_STATUS", value: "idle" });
      dispatch({ type: "SET_MEMORY_STATUS", value: "idle" });
      await reload();
    } catch (err: any) {
      if (!isAbortError(err)) {
        console.error("Failed to abort memory processing:", err);
      }
      dispatch({ type: "SET_RETRY_STATUS", value: "idle" });
      dispatch({ type: "SET_MEMORY_STATUS", value: "idle" });
      void reload();
    }
  }, [getLatestSessionSnapshot, reload, session, setSession]);

  const handleRevertMemoryEvent = useCallback(
    async (event: NonNullable<GroupSession["memoryToolEvents"]>[number]) => {
      if (!session?.id || !event.id || !session.memoryEmbeddings) return;

      const confirmed = await confirmBottomMenu({
        title: "Revert this cycle?",
        message: summarizeRevertImpact(event),
        confirmLabel: "Revert",
        destructive: true,
      });
      if (!confirmed) return;

      setRevertingEventId(event.id);
      try {
        const nextEvents = markMemoryToolEventReverted(
          session.memoryToolEvents ?? [],
          event.id,
          Date.now(),
        );
        const nextMemoryState = reconstructMemoryStateFromEvents(
          session.memoryEmbeddings,
          session.memorySummary ?? "",
          session.memorySummaryTokenCount ?? 0,
          nextEvents,
        );
        await storageBridge.groupSessionUpdateMemoryState(
          session.id,
          nextMemoryState.memoryEmbeddings.map((memory) => memory.text),
          nextMemoryState.memoryEmbeddings,
          nextMemoryState.memorySummary,
          nextMemoryState.memorySummaryTokenCount,
          nextMemoryState.memoryToolEvents,
          nextMemoryState.memoryStatus,
          nextMemoryState.memoryError ?? null,
        );
        setSession({
          ...session,
          memories: nextMemoryState.memoryEmbeddings.map((memory) => memory.text),
          memoryEmbeddings: nextMemoryState.memoryEmbeddings,
          memorySummary: nextMemoryState.memorySummary,
          memorySummaryTokenCount: nextMemoryState.memorySummaryTokenCount,
          memoryToolEvents: nextMemoryState.memoryToolEvents,
          memoryStatus: nextMemoryState.memoryStatus,
          memoryError: nextMemoryState.memoryError ?? undefined,
          updatedAt: Date.now(),
        });
        dispatch({ type: "SET_ACTION_ERROR", value: null });
      } catch (err: any) {
        console.error("Failed to revert group memory cycle:", err);
        dispatch({ type: "SET_ACTION_ERROR", value: err?.message || "Failed to revert cycle" });
      } finally {
        setRevertingEventId(null);
      }
    },
    [session, setSession],
  );

  const handleRefresh = useCallback(async () => {
    if (!session?.id) return;
    try {
      await reload();
      dispatch({ type: "SET_ACTION_ERROR", value: null });
    } catch (err: any) {
      dispatch({ type: "SET_ACTION_ERROR", value: err?.message || "Failed to refresh" });
    }
  }, [reload, session?.id]);

  const handleDismissError = useCallback(async () => {
    if (!session?.id || !session) return;
    try {
      const latest = await getLatestSessionSnapshot();
      await storageBridge.groupSessionUpdateMemories(
        latest.id,
        latest.memoryEmbeddings ?? [],
        latest.memorySummary ?? "",
        latest.memorySummaryTokenCount ?? 0,
        "idle",
        null,
      );
      setSession({ ...latest, memoryStatus: "idle", memoryError: null });
      dispatch({ type: "SET_ACTION_ERROR", value: null });
      dispatch({ type: "SET_MEMORY_STATUS", value: "idle" });
      await reload();
    } catch (err: any) {
      dispatch({ type: "SET_ACTION_ERROR", value: err?.message || "Failed to dismiss error" });
    }
  }, [getLatestSessionSnapshot, reload, session, setSession]);

  const handleTogglePinnedMessage = useCallback(
    async (messageId: string) => {
      if (!session?.id) return;
      const nextPinned = await toggleGroupMessagePin(session.id, messageId);
      if (nextPinned === null) {
        throw new Error("Failed to toggle pinned message");
      }
      const nextPinnedMessages = nextPinned
        ? pinnedMessages
        : pinnedMessages.filter((message) => message.id !== messageId);
      setPinnedMessages(nextPinnedMessages);
      await reload();
    },
    [pinnedMessages, reload, session?.id, setPinnedMessages],
  );

  return {
    session,
    pinnedMessages,
    loading,
    error,
    ui,
    dispatch,
    memoryItems,
    filteredMemories,
    stats,
    handleAddNew,
    handleSetColdState,
    handleTogglePin,
    handleRemove,
    handleUpdate,
    startEdit,
    cancelEdit,
    saveEdit,
    handleRunMemoryCycle,
    handleAbortMemoryCycle,
    handleRevertMemoryEvent,
    handleRefresh,
    handleDismissError,
    handleTogglePinnedMessage,
    handleSaveSummaryClick,
    revertingEventId,
  } as const;
}
