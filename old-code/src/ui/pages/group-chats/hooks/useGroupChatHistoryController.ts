import { useCallback, useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";

import type { GroupSessionPreview, Group, Character } from "../../../../core/storage/schemas";
import { storageBridge } from "../../../../core/storage/files";
import { listCharacters } from "../../../../core/storage/repo";
import { useI18n } from "../../../../core/i18n/context";
import { toast } from "../../../components/toast";

export function useGroupChatHistoryController(options?: {
  onOpenSession?: (sessionId: string) => void;
}) {
  const { t } = useI18n();
  const [searchParams] = useSearchParams();
  const groupIdFilter = searchParams.get("groupId");
  const [filterGroup, setFilterGroup] = useState<Group | null>(null);
  const [sessions, setSessions] = useState<GroupSessionPreview[]>([]);
  const [characters, setCharacters] = useState<Character[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busyIds, setBusyIds] = useState<Set<string>>(new Set());
  const [deleteTarget, setDeleteTarget] = useState<GroupSessionPreview | null>(null);
  const [query, setQuery] = useState(() => {
    const fromStorage = sessionStorage.getItem("groupChatHistoryQuery");
    if (fromStorage != null) return fromStorage;
    return new URLSearchParams(window.location.search).get("q") ?? "";
  });

  useEffect(() => {
    let cancelled = false;
    if (!groupIdFilter) {
      setFilterGroup(null);
      return;
    }
    (async () => {
      try {
        const group = await storageBridge.groupGet(groupIdFilter);
        if (!cancelled) setFilterGroup(group);
      } catch (err) {
        console.error("Failed to load group for history filter:", err);
        if (!cancelled) setFilterGroup(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [groupIdFilter]);


  useEffect(() => {
    if (query.trim()) sessionStorage.setItem("groupChatHistoryQuery", query);
    else sessionStorage.removeItem("groupChatHistoryQuery");
  }, [query]);

  useEffect(() => {
    const handle = window.setTimeout(() => {
      const url = new URL(window.location.href);
      const next = query.trim();
      if (next) url.searchParams.set("q", next);
      else url.searchParams.delete("q");
      window.history.replaceState(window.history.state, "", url.toString());
    }, 150);
    return () => window.clearTimeout(handle);
  }, [query]);

  const loadData = useCallback(async () => {
    try {
      setIsLoading(true);
      setError(null);

      const [allSessions, chars] = await Promise.all([
        storageBridge.groupSessionsListAll(),
        listCharacters(),
      ]);

      setSessions(allSessions);
      setCharacters(chars);
    } catch (err) {
      setError(err instanceof Error ? err.message : t("groupChats.historyController.failedToLoadData"));
    } finally {
      setIsLoading(false);
    }
  }, [t]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const handleDelete = useCallback(async (sessionId: string) => {
    setBusyIds((prev) => new Set(prev).add(sessionId));
    try {
      await storageBridge.groupSessionDelete(sessionId);
      setSessions((prev) => prev.filter((s) => s.id !== sessionId));
    } catch (err) {
      setError(t("groupChats.historyController.failedToDelete", { error: String(err) }));
    } finally {
      setBusyIds((prev) => {
        const next = new Set(prev);
        next.delete(sessionId);
        return next;
      });
    }
  }, [t]);

  const handleRename = useCallback(async (sessionId: string, newTitle: string) => {
    setBusyIds((prev) => new Set(prev).add(sessionId));
    try {
      await storageBridge.groupSessionUpdateTitle(sessionId, newTitle);
      setSessions((prev) => prev.map((s) => (s.id === sessionId ? { ...s, name: newTitle } : s)));
    } catch (err) {
      setError(t("groupChats.historyController.failedToRename", { error: String(err) }));
    } finally {
      setBusyIds((prev) => {
        const next = new Set(prev);
        next.delete(sessionId);
        return next;
      });
    }
  }, [t]);

  const handleArchive = useCallback(async (sessionId: string, archived: boolean) => {
    setBusyIds((prev) => new Set(prev).add(sessionId));
    try {
      await storageBridge.groupSessionArchive(sessionId, archived);
      setSessions((prev) => prev.map((s) => (s.id === sessionId ? { ...s, archived } : s)));
    } catch (err) {
      setError(
        t(
          archived
            ? "groupChats.historyController.failedToArchive"
            : "groupChats.historyController.failedToUnarchive",
          { error: String(err) },
        ),
      );
    } finally {
      setBusyIds((prev) => {
        const next = new Set(prev);
        next.delete(sessionId);
        return next;
      });
    }
  }, [t]);

  const handleExport = useCallback(
    async (sessionId: string) => {
      setBusyIds((prev) => new Set(prev).add(sessionId));
      try {
        const path = await storageBridge.jsonlExportGroupChat(sessionId);
        toast.success(t("groupChats.chatSettingsExtra.exportSuccess", { path }));
      } catch (err) {
        console.error("Failed to export group chat:", err);
        toast.error(t("groupChats.chatSettingsExtra.failedExport"));
      } finally {
        setBusyIds((prev) => {
          const next = new Set(prev);
          next.delete(sessionId);
          return next;
        });
      }
    },
    [t],
  );

  const handleDuplicate = useCallback(
    async (session: GroupSessionPreview) => {
      setBusyIds((prev) => new Set(prev).add(session.id));
      try {
        const newSession = await storageBridge.groupSessionDuplicate(session.id);
        setSessions((prev) => [newSession, ...prev]);
        options?.onOpenSession?.(newSession.id);
      } catch (err) {
        setError(err instanceof Error ? err.message : t("groupChats.historyController.failedToDuplicate"));
      } finally {
        setBusyIds((prev) => {
          const next = new Set(prev);
          next.delete(session.id);
          return next;
        });
      }
    },
    [options?.onOpenSession, t],
  );

  const scopedSessions = useMemo(() => {
    if (!groupIdFilter) return sessions;
    return sessions.filter((s) => s.groupCharacterId === groupIdFilter);
  }, [sessions, groupIdFilter]);

  const filteredSessions = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return scopedSessions;
    return scopedSessions.filter((s) => {
      if (s.name.toLowerCase().includes(q)) return true;
      const lastMessage = s.lastMessage?.toLowerCase() ?? "";
      if (lastMessage.includes(q)) return true;
      const charNames = s.characterIds
        .map((id) => characters.find((c) => c.id === id)?.name || "")
        .join(" ")
        .toLowerCase();
      return charNames.includes(q);
    });
  }, [scopedSessions, characters, query]);

  const activeSessions = useMemo(
    () => filteredSessions.filter((s) => !s.archived),
    [filteredSessions],
  );

  const archivedSessions = useMemo(
    () => filteredSessions.filter((s) => s.archived),
    [filteredSessions],
  );

  return {
    sessions,
    scopedSessions,
    filterGroup,
    characters,
    isLoading,
    error,
    busyIds,
    deleteTarget,
    query,
    filteredSessions,
    activeSessions,
    archivedSessions,
    setQuery,
    setDeleteTarget,
    handleDelete,
    handleRename,
    handleArchive,
    handleDuplicate,
    handleExport,
  } as const;
}
