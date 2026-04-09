import { ArrowLeft, Trash2, MessageCircle, AlertCircle, Search, X } from "lucide-react";
import { useNavigate } from "react-router-dom";

import { useI18n } from "../../../core/i18n/context";
import { typography, radius, cn, colors, interactive } from "../../design-tokens";
import { BottomMenu, MenuButton, MenuButtonGroup, MenuDivider } from "../../components";
import { Routes, useNavigationManager } from "../../navigation";
import { SessionCard } from "./components/history/SessionCard";
import { useGroupChatHistoryController } from "./hooks/useGroupChatHistoryController";
import { formatTimeAgo } from "./utils/formatTimeAgo";

export function GroupChatHistoryPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const { backOrReplace } = useNavigationManager();
  const {
    sessions,
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
  } = useGroupChatHistoryController({
    onOpenSession: (sessionId) => navigate(Routes.groupChat(sessionId)),
  });

  if (isLoading) {
    return (
      <div className="min-h-screen bg-surface flex items-center justify-center">
        <div className="animate-spin rounded-full h-8 w-8 border border-fg/10 border-t-fg/60" />
      </div>
    );
  }

  return (
    <div className={cn("flex h-full flex-col", colors.surface.base, colors.text.primary)}>
      {/* Header */}
      <header
        className={cn(
          "z-20 shrink-0 border-b border-fg/10 px-4 pb-3 pt-[calc(env(safe-area-inset-top)+12px)] shrink-0",
          "bg-surface",
        )}
      >
        <div className="flex items-center gap-3">
          <div className="flex flex-1 items-center min-w-0">
            <button
              onClick={() => backOrReplace(Routes.groupChats)}
              className="flex shrink-0 items-center justify-center -ml-2 text-fg transition hover:text-fg/80"
              aria-label="Back to group chats"
            >
              <ArrowLeft size={14} strokeWidth={2.5} />
            </button>
            <div className="min-w-0 flex-1 text-left">
              <p className="truncate text-xl font-bold text-fg/90">{t("groupChats.history.title")}</p>
              <p className="mt-0.5 truncate text-xs text-fg/50">{t("groupChats.history.subtitle")}</p>
            </div>
          </div>
        </div>
      </header>

      {/* Content */}
      <main className="flex-1 overflow-y-auto px-3 pt-4">
        {error && (
          <div
            className={cn("mb-4 p-4 border border-danger/30 bg-danger/10 text-center", radius.lg)}
          >
            <AlertCircle className="mx-auto mb-2 h-5 w-5 text-danger" />
            <p className={cn(typography.bodySmall.size, "text-danger")}>{error}</p>
            <button
              onClick={() => window.location.reload()}
              className={cn(
                "mt-3 px-4 py-2 border border-danger/40 bg-danger/20 text-danger",
                radius.md,
                "active:scale-95 transition-transform",
              )}
            >
              {t("common.buttons.retry")}
            </button>
          </div>
        )}

        {sessions.length > 0 && (
          <div className="mb-4">
            <div className={cn("relative")}>
              <Search
                className={cn(
                  "absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4",
                  colors.text.tertiary,
                )}
              />
              <input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder={t("groupChats.history.searchPlaceholder")}
                className={cn(
                  "w-full pl-10 pr-10 py-2.5",
                  "border bg-fg/5",
                  colors.border.subtle,
                  radius.lg,
                  typography.bodySmall.size,
                  "text-fg placeholder-fg/40",
                  "focus:outline-none focus:ring-2 focus:ring-fg/10 focus:border-fg/20",
                )}
              />
              {query.trim().length > 0 && (
                <button
                  type="button"
                  onClick={() => setQuery("")}
                  className={cn(
                    "absolute right-2 top-1/2 -translate-y-1/2",
                    "flex items-center justify-center",
                    radius.full,
                    colors.text.tertiary,
                    "hover:text-fg",
                    interactive.transition.fast,
                    interactive.active.scale,
                  )}
                  aria-label="Clear search"
                >
                  <X size={16} />
                </button>
              )}
            </div>
            {query.trim() ? (
              <p className={cn(typography.caption.size, colors.text.tertiary, "mt-2")}>
                {filteredSessions.length.toLocaleString()} result
                {filteredSessions.length === 1 ? "" : "s"}
              </p>
            ) : null}
          </div>
        )}

        {sessions.length === 0 ? (
          <div className="text-center py-20">
            <MessageCircle className="mx-auto mb-4 h-12 w-12 text-fg/30" />
            <h3 className={cn(typography.h3.size, typography.h3.weight, "text-fg/70 mb-2")}>
              {t("groupChats.history.noChatsYet")}
            </h3>
            <p className={cn(typography.bodySmall.size, "text-fg/40 mb-6")}>
              {t("groupChats.history.noChatsDesc")}
            </p>
          </div>
        ) : filteredSessions.length === 0 ? (
          <div className="text-center py-20">
            <MessageCircle className="mx-auto mb-4 h-12 w-12 text-fg/30" />
            <h3 className={cn(typography.h3.size, typography.h3.weight, "text-fg/70 mb-2")}>
              {t("groupChats.history.noMatchingChats")}
            </h3>
            <p className={cn(typography.bodySmall.size, "text-fg/40 mb-6")}>
              {t("groupChats.history.noMatchingDesc")}
            </p>
            <button
              type="button"
              onClick={() => setQuery("")}
              className={cn(
                "px-4 py-2 border bg-fg/5 text-fg/80",
                colors.border.subtle,
                radius.md,
                typography.bodySmall.size,
                interactive.active.scale,
                interactive.hover.brightness,
                interactive.transition.fast,
              )}
            >
              {t("common.buttons.clearSearch")}
            </button>
          </div>
        ) : (
          <div className="space-y-6 pb-24">
            {/* Active Sessions */}
            {activeSessions.length > 0 && (
              <div>
                <h3
                  className={cn(typography.bodySmall.size, "font-medium text-fg/50 mb-3 px-1")}
                >
                  {t("groupChats.history.active", { count: activeSessions.length })}
                </h3>
                <div className="space-y-3">
                  {activeSessions.map((session) => (
                    <SessionCard
                      key={session.id}
                      session={session}
                      characters={characters}
                      onSelect={() => navigate(Routes.groupChat(session.id))}
                      onDelete={() => setDeleteTarget(session)}
                      onRename={(newTitle) => handleRename(session.id, newTitle)}
                      onArchive={() => handleArchive(session.id, true)}
                      onDuplicate={() => handleDuplicate(session)}
                      isBusy={busyIds.has(session.id)}
                    />
                  ))}
                </div>
              </div>
            )}

            {/* Archived Sessions */}
            {archivedSessions.length > 0 && (
              <div>
                <h3
                  className={cn(typography.bodySmall.size, "font-medium text-fg/50 mb-3 px-1")}
                >
                  {t("groupChats.history.archived", { count: archivedSessions.length })}
                </h3>
                <div className="space-y-3">
                  {archivedSessions.map((session) => (
                    <SessionCard
                      key={session.id}
                      session={session}
                      characters={characters}
                      onSelect={() => navigate(Routes.groupChat(session.id))}
                      onDelete={() => setDeleteTarget(session)}
                      onRename={(newTitle) => handleRename(session.id, newTitle)}
                      onUnarchive={() => handleArchive(session.id, false)}
                      onDuplicate={() => handleDuplicate(session)}
                      isBusy={busyIds.has(session.id)}
                      isArchived
                    />
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
      </main>

      <BottomMenu
        isOpen={deleteTarget != null}
        onClose={() => setDeleteTarget(null)}
        title={t("groupChats.history.deleteSessionTitle")}
        includeExitIcon={false}
      >
        <div className="rounded-xl border border-fg/10 bg-fg/4 p-3">
          <p className={cn(typography.bodySmall.size, "font-semibold text-fg/90 truncate")}>
            {deleteTarget?.name || "Untitled Chat"}
          </p>
          {deleteTarget ? (
            <p className={cn(typography.caption.size, "text-fg/45 mt-0.5")}>
              {formatTimeAgo(deleteTarget.updatedAt)}
            </p>
          ) : null}
          {deleteTarget?.lastMessage ? (
            <p className={cn(typography.bodySmall.size, "text-fg/60 mt-2 line-clamp-2")}>
              {deleteTarget.lastMessage}
            </p>
          ) : null}
        </div>

        <MenuDivider />

        <MenuButtonGroup>
          <MenuButton
            icon={Trash2}
            title={deleteTarget && busyIds.has(deleteTarget.id) ? t("common.buttons.deleting") : t("groupChats.history.deleteSessionButton")}
            description={t("groupChats.history.deleteSessionDesc")}
            color="from-danger to-danger/80"
            disabled={!deleteTarget || busyIds.has(deleteTarget.id)}
            onClick={() => {
              if (!deleteTarget) return;
              void (async () => {
                await handleDelete(deleteTarget.id);
                setDeleteTarget(null);
              })();
            }}
          />
          <MenuButton
            icon={X}
            title={t("common.buttons.cancel")}
            description={t("groupChats.history.keepChat")}
            color="from-info to-info/80"
            disabled={!!deleteTarget && busyIds.has(deleteTarget.id)}
            onClick={() => setDeleteTarget(null)}
          />
        </MenuButtonGroup>
      </BottomMenu>
    </div>
  );
}

export default GroupChatHistoryPage;
