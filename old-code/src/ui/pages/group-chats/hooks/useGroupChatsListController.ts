import { useCallback, useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { storageBridge } from "../../../../core/storage/files";
import { listCharacters } from "../../../../core/storage/repo";
import type { GroupPreview, Character } from "../../../../core/storage/schemas";
import { toast } from "../../../components/toast";
import { useI18n } from "../../../../core/i18n/context";

export function useGroupChatsListController() {
  const { t } = useI18n();
  const [groups, setGroups] = useState<GroupPreview[]>([]);
  const [characters, setCharacters] = useState<Character[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedGroup, setSelectedGroup] = useState<GroupPreview | null>(null);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [openingGroupId, setOpeningGroupId] = useState<string | null>(null);
  const openingRef = useRef(false);

  const loadData = useCallback(async () => {
    try {
      const [items, chars] = await Promise.all([storageBridge.groupsList(), listCharacters()]);
      setGroups(items);
      setCharacters(chars);
    } catch (err) {
      console.error("Failed to load groups:", err);
    }
  }, []);

  useEffect(() => {
    (async () => {
      try {
        await loadData();
      } finally {
        setLoading(false);
      }
    })();

    let unlisten: UnlistenFn | null = null;
    (async () => {
      unlisten = await listen("database-reloaded", () => {
        void loadData();
      });
    })();

    return () => {
      if (unlisten) unlisten();
    };
  }, [loadData]);

  const handleDelete = useCallback(async () => {
    if (!selectedGroup) return;

    try {
      setDeleting(true);
      await storageBridge.groupDelete(selectedGroup.id);
      await loadData();
      setShowDeleteConfirm(false);
      setSelectedGroup(null);
    } catch (err) {
      console.error("Failed to delete group:", err);
      toast.error(t("groupChats.list.deleteGroupFailed"));
    } finally {
      setDeleting(false);
    }
  }, [selectedGroup, loadData, t]);

  const resolveLatestSessionId = useCallback(async (group: GroupPreview) => {
    if (!group.latestSessionId) return null;
    const existing = await storageBridge.groupSessionGet(group.latestSessionId);
    if (existing) return existing.id as string;

    const freshGroups: GroupPreview[] = await storageBridge.groupsList();
    setGroups(freshGroups);
    const fresh = freshGroups.find((item) => item.id === group.id);
    if (fresh?.latestSessionId && fresh.latestSessionId !== group.latestSessionId) {
      const freshSession = await storageBridge.groupSessionGet(fresh.latestSessionId);
      if (freshSession) return freshSession.id as string;
    }
    return null;
  }, []);

  const handleOpenGroup = useCallback(
    async (group: GroupPreview): Promise<string | null> => {
      if (openingRef.current) return null;
      openingRef.current = true;
      setOpeningGroupId(group.id);
      try {
        const existingId = await resolveLatestSessionId(group);
        if (existingId) return existingId;
        const session = await storageBridge.groupCreateSession(group.id);
        await loadData();
        return session.id as string;
      } catch (err) {
        console.error("Failed to open group chat:", err);
        toast.error(t("groupChats.list.startChatFailed"));
        await loadData();
        return null;
      } finally {
        openingRef.current = false;
        setOpeningGroupId(null);
      }
    },
    [resolveLatestSessionId, loadData, t],
  );

  const handleNewChat = useCallback(
    async (groupId: string): Promise<string | null> => {
      if (openingRef.current) return null;
      openingRef.current = true;
      setOpeningGroupId(groupId);
      try {
        const session = await storageBridge.groupCreateSession(groupId);
        await loadData();
        return session.id as string;
      } catch (err) {
        console.error("Failed to create new group chat:", err);
        toast.error(t("groupChats.list.startChatFailed"));
        return null;
      } finally {
        openingRef.current = false;
        setOpeningGroupId(null);
      }
    },
    [loadData, t],
  );

  return {
    groups,
    characters,
    loading,
    selectedGroup,
    showDeleteConfirm,
    deleting,
    openingGroupId,
    setSelectedGroup,
    setShowDeleteConfirm,
    handleDelete,
    handleOpenGroup,
    handleNewChat,
  } as const;
}
