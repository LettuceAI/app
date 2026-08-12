import { useCallback, useEffect, useMemo, useState } from "react";
import { History, LayoutGrid, LayoutList, Plus, SearchX, Settings, Trash2 } from "lucide-react";
import { useNavigate } from "react-router-dom";

import { useI18n } from "../../../core/i18n/context";
import {
  getGroupChatsViewMode,
  getGroupChatsViewModeCached,
  setGroupChatsViewMode,
} from "../../../core/storage/appState";
import type { GroupChatsViewMode } from "../../../core/storage/schemas";
import { BottomMenu } from "../../components";
import {
  PageHeader,
  PageHeaderAction,
  useInlineHeader,
  useRailSettings,
} from "../../components/App";
import { Routes } from "../../navigation";
import { useGroupChatsListController } from "./hooks/useGroupChatsListController";
import { GroupList, GroupListSkeleton, EmptyState } from "./components/list/GroupList";

export function GroupChatsListPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [viewMode, setViewMode] = useState<GroupChatsViewMode>(
    () => getGroupChatsViewModeCached() ?? "classic",
  );

  useEffect(() => {
    getGroupChatsViewMode()
      .then((mode) => setViewMode(mode))
      .catch(() => {});
  }, []);

  useEffect(() => {
    (window as any).__groupChatsViewMode = viewMode;
    window.dispatchEvent(new CustomEvent("groupChats:viewModeChanged"));
  }, [viewMode]);

  const cycleViewMode = useCallback(() => {
    setViewMode((prev) => {
      const next: GroupChatsViewMode = prev === "classic" ? "detailed" : "classic";
      setGroupChatsViewMode(next).catch(() => {});
      return next;
    });
  }, []);

  useEffect(() => {
    const handler = () => cycleViewMode();
    window.addEventListener("groupChats:cycleViewMode", handler);
    return () => window.removeEventListener("groupChats:cycleViewMode", handler);
  }, [cycleViewMode]);
  const {
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
  } = useGroupChatsListController();

  const [query, setQuery] = useState("");
  const inlineHeader = useInlineHeader();
  const railSettings = useRailSettings();

  const visibleGroups = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return groups;
    return groups.filter((group) => group.name.toLowerCase().includes(needle));
  }, [groups, query]);

  const menuActionClass =
    "flex w-full items-center gap-3 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-left transition hover:border-fg/20 hover:bg-fg/10";
  const menuIconClass =
    "flex h-8 w-8 items-center justify-center rounded-full border border-fg/10 bg-fg/10";

  return (
    <div className="flex h-full flex-col pb-6 text-fg/80">
      <main className="flex-1 overflow-y-auto px-1 lg:px-8 pt-4 mx-auto w-full max-w-md lg:max-w-[1600px]">
        {inlineHeader && (
          <PageHeader
            title={t("common.nav.groupChats")}
            meta={
              query.trim()
                ? t("pageHeader.results", {
                    count: String(visibleGroups.length),
                    total: String(groups.length),
                  })
                : undefined
            }
            searchValue={query}
            onSearchChange={setQuery}
            searchPlaceholder={t("pageHeader.searchGroups")}
            actions={
              <>
                <PageHeaderAction
                  icon={viewMode === "detailed" ? LayoutList : LayoutGrid}
                  label={t("topNav.changeLayout")}
                  onClick={cycleViewMode}
                />
                {!railSettings && (
                  <PageHeaderAction
                    icon={Settings}
                    label={t("topNav.settings")}
                    onClick={() => navigate("/settings")}
                  />
                )}
              </>
            }
          />
        )}
        {loading ? (
          <GroupListSkeleton />
        ) : visibleGroups.length ? (
          <GroupList
            groups={visibleGroups}
            characters={characters}
            viewMode={viewMode}
            openingGroupId={openingGroupId}
            onOpenGroup={(group) => {
              void handleOpenGroup(group).then((sessionId) => {
                if (sessionId) navigate(Routes.groupChat(sessionId));
              });
            }}
            onLongPress={(group) => setSelectedGroup(group)}
          />
        ) : groups.length ? (
          <div className="rounded-2xl border border-dashed border-fg/10 bg-fg/2 p-8 text-center">
            <SearchX className="mx-auto h-6 w-6 text-fg/30" />
            <h3 className="mt-3 text-base font-semibold text-fg">
              {t("pageHeader.noResultsTitle")}
            </h3>
            <p className="mt-1 text-sm text-fg/50">
              {t("pageHeader.noResultsBody", { query })}
            </p>
          </div>
        ) : (
          <EmptyState />
        )}
      </main>

      <BottomMenu
        isOpen={Boolean(selectedGroup) && !showDeleteConfirm}
        onClose={() => setSelectedGroup(null)}
        includeExitIcon={false}
        title={selectedGroup?.name || ""}
      >
        {selectedGroup && (
          <div className="space-y-2">
            <button
              onClick={() => {
                const group = selectedGroup;
                setSelectedGroup(null);
                void handleNewChat(group.id).then((sessionId) => {
                  if (sessionId) navigate(Routes.groupChat(sessionId));
                });
              }}
              className={menuActionClass}
            >
              <div className={menuIconClass}>
                <Plus className="h-4 w-4 text-fg/70" />
              </div>
              <span className="text-sm font-medium text-fg">{t("groupChats.list.newChat")}</span>
            </button>

            <button
              onClick={() => {
                navigate(Routes.groupChatHistory(selectedGroup.id));
                setSelectedGroup(null);
              }}
              className={menuActionClass}
            >
              <div className={menuIconClass}>
                <History className="h-4 w-4 text-fg/70" />
              </div>
              <span className="text-sm font-medium text-fg">
                {t("groupChats.list.chatHistory")}
              </span>
            </button>

            <button
              onClick={() => {
                navigate(Routes.groupSettings(selectedGroup.id));
                setSelectedGroup(null);
              }}
              className={menuActionClass}
            >
              <div className={menuIconClass}>
                <Settings className="h-4 w-4 text-fg/70" />
              </div>
              <span className="text-sm font-medium text-fg">
                {t("groupChats.list.groupSettings")}
              </span>
            </button>

            <button
              onClick={() => {
                setShowDeleteConfirm(true);
              }}
              className="flex w-full items-center gap-3 rounded-xl border border-danger/30 bg-danger/10 px-4 py-3 text-left transition hover:border-danger/50 hover:bg-danger/20"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-full border border-danger/30 bg-danger/20">
                <Trash2 className="h-4 w-4 text-danger" />
              </div>
              <span className="text-sm font-medium text-danger">
                {t("groupChats.list.deleteGroup")}
              </span>
            </button>
          </div>
        )}
      </BottomMenu>

      <BottomMenu
        isOpen={showDeleteConfirm}
        onClose={() => setShowDeleteConfirm(false)}
        title={t("groupChats.list.deleteConfirmTitle")}
      >
        <div className="space-y-4">
          <p className="text-sm text-fg/70">
            {t("groupChats.list.deleteConfirmMessage", {
              name: selectedGroup?.name ?? "",
            })}
          </p>
          <div className="flex gap-3">
            <button
              onClick={() => setShowDeleteConfirm(false)}
              disabled={deleting}
              className="flex-1 rounded-xl border border-fg/10 bg-fg/5 py-3 text-sm font-medium text-fg transition hover:border-fg/20 hover:bg-fg/10 disabled:opacity-50"
            >
              {t("common.buttons.cancel")}
            </button>
            <button
              onClick={handleDelete}
              disabled={deleting}
              className="flex-1 rounded-xl border border-danger/30 bg-danger/20 py-3 text-sm font-medium text-danger transition hover:bg-danger/30 disabled:opacity-50"
            >
              {deleting ? t("common.buttons.deleting") : t("groupChats.list.deleteGroup")}
            </button>
          </div>
        </div>
      </BottomMenu>
    </div>
  );
}
