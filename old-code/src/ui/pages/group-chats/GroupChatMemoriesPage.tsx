import { useCallback, useState } from "react";
import type { ComponentType } from "react";
import { useParams } from "react-router-dom";
import { useGroupChatLayoutContext } from "./GroupChatLayout";
import { motion, AnimatePresence } from "framer-motion";
import {
  ArrowLeft,
  Sparkles,
  Clock,
  Search,
  Bot,
  User,
  Trash2,
  Edit2,
  Check,
  Plus,
  Pin,
  AlertTriangle,
  X,
  RefreshCw,
  Snowflake,
  Flame,
  EllipsisVertical,
  PinOff,
} from "lucide-react";

import { typography, radius, cn, interactive, colors, components } from "../../design-tokens";
import { Routes, useNavigationManager } from "../../navigation";
import { BottomMenu } from "../../components/BottomMenu";
import { MemoryCycleHub } from "../../components/MemoryCycleHub";
import { useI18n } from "../../../core/i18n/context";
import { ToolLog } from "./components/memories/ToolLog";
import { useGroupChatMemoriesController } from "./hooks/useGroupChatMemoriesController";

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
        "hover:bg-fg/5 active:bg-fg/10",
        "disabled:opacity-40 disabled:pointer-events-none",
        variant === "danger" && "hover:bg-danger/10",
      )}
    >
      <div
        className={cn("flex items-center justify-center w-8 h-8 rounded-lg", iconBg || "bg-fg/10")}
      >
        <Icon size={16} className={cn(variant === "danger" ? "text-danger" : "text-fg")} />
      </div>
      <span
        className={cn("text-[15px] text-left", variant === "danger" ? "text-danger" : "text-fg/90")}
      >
        {label}
      </span>
    </button>
  );
}

export function GroupChatMemoriesPage() {
  const { backOrReplace } = useNavigationManager();
  const { t } = useI18n();
  const { groupSessionId } = useParams();
  const { backgroundImageData } = useGroupChatLayoutContext();

  const {
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
    startEdit,
    saveEdit,
    handleRunMemoryCycle,
    handleAbortMemoryCycle,
    handleRevertMemoryEvent,
    handleRefresh,
    handleDismissError,
    handleTogglePinnedMessage,
    handleSaveSummaryClick,
    revertingEventId,
    cycleStatus,
  } = useGroupChatMemoriesController(groupSessionId);

  const [showAddMemoryMenu, setShowAddMemoryMenu] = useState(false);
  const [showSummaryEditor, setShowSummaryEditor] = useState(false);

  const isMemoryCycleActive =
    session?.memoryStatus === "processing" || ui.retryStatus === "retrying";
  const cycleErrorMessage =
    session?.memoryError ?? (ui.memoryStatus === "failed" ? ui.actionError : null) ?? null;

  const handleSaveEdit = useCallback(
    async (index: number) => {
      const didSave = await saveEdit(index);
      if (didSave) {
        dispatch({ type: "CLOSE_MEMORY_ACTIONS" });
      }
    },
    [dispatch, saveEdit],
  );

  const tabs = [
    { id: "memories" as const, icon: Bot, label: t("common.nav.dynamicMemory") },
    { id: "pinned" as const, icon: Pin, label: t("groupChats.memories.tabPinned") },
    { id: "tools" as const, icon: Clock, label: t("groupChats.memoriesPageExtra.activityTabLabel") },
  ];

  if (loading) {
    return (
      <div className={cn("flex h-screen items-center justify-center", colors.surface.base)}>
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-fg/10 border-t-fg/60" />
      </div>
    );
  }

  if (error || !session) {
    return (
      <div
        className={cn(
          "flex min-h-screen flex-col items-center justify-center gap-4 px-4 text-center",
          colors.surface.base,
        )}
      >
        <p className={cn("text-sm", colors.text.secondary)}>{error || t("groupChats.memoriesPageExtra.sessionNotFound")}</p>
        <button
          onClick={() => backOrReplace(Routes.groupChats)}
          className={cn(
            components.button.primary,
            components.button.sizes.md,
            "bg-fg/5 text-fg hover:bg-fg/10",
          )}
        >
          {t("common.buttons.goBack")}
        </button>
      </div>
    );
  }

  return (
    <div
      className={cn(
        "relative flex h-full flex-col",
        !backgroundImageData && colors.surface.base,
        colors.text.primary,
      )}
    >
      {backgroundImageData && (
        <div
          className="pointer-events-none absolute inset-0 z-0 bg-surface/70 backdrop-blur-xl"
          aria-hidden
        />
      )}
      <div className="relative z-10 flex h-full flex-col">
      {/* Header */}
      <header
        className={cn(
          "z-20 shrink-0 border-b border-fg/10 px-4",
          "pt-[calc(env(safe-area-inset-top)+24px)] pb-3",
          colors.glass.strong,
        )}
      >
        <div className="flex items-center gap-3">
          <div className="flex flex-1 items-center min-w-0">
            <button
              onClick={() => backOrReplace(Routes.groupChat(groupSessionId!))}
              className={cn(
                "flex shrink-0 items-center justify-center -ml-2 px-2 py-1",
                colors.text.primary,
                interactive.transition.fast,
                "hover:text-fg/80",
              )}
              aria-label={t("common.buttons.goBack")}
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
                {t("common.nav.dynamicMemory")}
              </span>
              <span className={cn("truncate text-sm font-medium", colors.text.tertiary)}>
                {session.name}
              </span>
            </div>
          </div>
        </div>

        {/* Segmented Tab Control */}
        <div className="mt-3 flex bg-fg/5 border border-fg/8 rounded-xl p-1">
          {tabs.map(({ id, icon: Icon, label }) => (
            <button
              key={id}
              onClick={() => dispatch({ type: "SET_TAB", tab: id })}
              className={cn(
                "relative flex-1 flex items-center justify-center gap-1.5 rounded-lg py-2 text-xs font-semibold transition-colors",
                ui.activeTab === id ? "text-fg" : "text-fg/40 hover:text-fg/60",
              )}
              aria-label={label}
            >
              {ui.activeTab === id && (
                <motion.div
                  layoutId="groupMemoryTabIndicator"
                  className="absolute inset-0 rounded-lg bg-fg/10 border border-fg/10"
                  transition={{ type: "spring", stiffness: 320, damping: 28 }}
                />
              )}
              <Icon size={14} className="relative z-10" />
              <span className="relative z-10">{label}</span>
            </button>
          ))}
        </div>
      </header>

      <main className="flex-1 overflow-y-auto pb-[calc(env(safe-area-inset-bottom)+16px)]">
        {session.memoryType === "dynamic" &&
          (cycleStatus ||
            isMemoryCycleActive ||
            cycleErrorMessage ||
            ui.retryStatus === "success") && (
            <MemoryCycleHub
              status={cycleStatus}
              running={isMemoryCycleActive}
              retrying={ui.retryStatus === "retrying"}
              retrySuccess={ui.retryStatus === "success"}
              errorMessage={cycleErrorMessage}
              step={ui.memoryProgressStep ?? session.memoryProgressStep ?? null}
              onRun={handleRunMemoryCycle}
              onCancel={handleAbortMemoryCycle}
              onRetry={handleRunMemoryCycle}
              onDismissError={handleDismissError}
              onDismissSuccess={() => dispatch({ type: "SET_RETRY_STATUS", value: "idle" })}
            />
          )}
        {ui.actionError && !isMemoryCycleActive && ui.actionError !== cycleErrorMessage && (
          <div className="px-3 pt-3">
            <div
              className={cn(
                radius.md,
                "bg-danger/10 border border-danger/20 p-3 flex items-start gap-3",
              )}
            >
              <AlertTriangle className="h-5 w-5 text-danger shrink-0" />
              <div className={cn("flex-1", typography.body.size, "text-danger")}>
                <p className="font-semibold mb-1">{t("groupChats.memoriesPageExtra.memorySystemError")}</p>
                <p className="opacity-90">{ui.actionError}</p>
              </div>
              <button
                onClick={() => dispatch({ type: "SET_ACTION_ERROR", value: null })}
                className="text-danger/70 hover:text-danger"
              >
                <X size={16} />
              </button>
            </div>
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
              {/* Context Summary - Tappable preview */}
              <button
                type="button"
                onClick={() => setShowSummaryEditor(true)}
                className={cn(
                  "w-full rounded-xl border border-accent/15 bg-accent/3 px-4 py-3 text-left",
                  "transition-all hover:border-accent/25 hover:bg-accent/5 active:scale-[0.99]",
                )}
              >
                <div className="flex items-center gap-2 mb-1.5">
                  <Sparkles size={13} className="text-accent/70 shrink-0" />
                  <span className="text-[11px] font-semibold text-accent/60 uppercase tracking-wider">
                    {t("groupChats.memoriesPageExtra.contextSummaryTitle")}
                  </span>
                  {session?.memorySummaryTokenCount && session.memorySummaryTokenCount > 0 ? (
                    <span className="text-[10px] text-fg/30 ml-auto">
                      {session.memorySummaryTokenCount.toLocaleString()} {t("groupChats.memoriesPage.tokens")}
                    </span>
                  ) : null}
                </div>
                <p
                  className={cn(
                    typography.bodySmall.size,
                    "leading-relaxed line-clamp-4 min-h-14",
                    ui.summaryDraft ? "text-accent/70" : "text-accent/25 italic",
                  )}
                >
                  {ui.summaryDraft || t("groupChats.memoriesPage.contextSummaryPrompt")}
                </p>
              </button>

              {/* Memories Section */}
              <div>
                <div className="flex items-center gap-2 mb-2">
                  <span className="text-[12px] font-semibold uppercase tracking-wider text-fg/50">
                    {ui.searchTerm.trim()
                      ? t("groupChats.memoriesPage.resultsCount", { count: filteredMemories.length })
                      : t("groupChats.memoriesPage.savedMemories")}
                  </span>
                  <span className="text-[10px] text-fg/30 ml-auto">
                    {stats.ai} {t("groupChats.memoriesPage.statAi")} · {stats.user}{" "}
                    {t("groupChats.memoriesPage.statYou")}
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
                      placeholder={t("groupChats.memoriesPage.searchMemoriesPlaceholder")}
                      className={cn(
                        "w-full pl-10 pr-10 py-2.5",
                        components.input.base,
                        radius.lg,
                        "text-sm text-fg placeholder-fg/40",
                      )}
                    />
                    {ui.searchTerm.trim().length > 0 && (
                      <button
                        type="button"
                        onClick={() => dispatch({ type: "CLEAR_SEARCH" })}
                        className={cn(
                          "absolute right-3 top-1/2 -translate-y-1/2",
                          colors.text.tertiary,
                          "hover:text-fg",
                          interactive.transition.fast,
                        )}
                        aria-label={t("groupChats.memoriesPage.clearSearch")}
                      >
                        <X size={16} />
                      </button>
                    )}
                  </div>
                  <button
                    onClick={() => setShowAddMemoryMenu(true)}
                    className={cn(
                      "flex items-center justify-center shrink-0",
                      "h-10.5 w-10.5 rounded-lg",
                      "border border-fg/10 bg-fg/5",
                      "text-fg/50",
                      "hover:bg-fg/8 hover:text-fg/70",
                      "transition-all active:scale-95",
                    )}
                    aria-label={t("groupChats.memoriesPage.addMemory")}
                  >
                    <Plus size={18} />
                  </button>
                </div>

                {/* Memory List */}
                {filteredMemories.length === 0 ? (
                  <motion.div
                    initial={{ opacity: 0, y: 20 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.3, ease: "easeOut" }}
                    className="flex flex-col items-center justify-center py-16"
                  >
                    <div className="flex h-16 w-16 items-center justify-center rounded-2xl border border-fg/10 bg-fg/5 mb-4">
                      {ui.searchTerm ? (
                        <Search className="h-7 w-7 text-fg/20" />
                      ) : (
                        <Bot className="h-7 w-7 text-fg/20" />
                      )}
                    </div>
                    <h3 className="mb-1 text-base font-semibold text-fg">
                      {ui.searchTerm
                        ? t("groupChats.memoriesPage.noMatchingMemories")
                        : t("groupChats.memoriesPage.noMemoriesYet")}
                    </h3>
                    <p className="text-center text-sm text-fg/40 max-w-60">
                      {ui.searchTerm
                        ? t("groupChats.memoriesPage.tryDifferentSearch")
                        : t("groupChats.memoriesPage.tapAddToCreate")}
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
                                ? "border-fg/10 bg-fg/2"
                                : "border-fg/6 bg-fg/2 hover:border-fg/10 hover:bg-fg/3",
                            )}
                          >
                            <div
                              className="px-4 py-3 cursor-pointer"
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
                                    <Bot size={14} className="text-info" />
                                  ) : (
                                    <User size={14} className="text-accent" />
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
                                    "rounded-lg text-fg/30",
                                    "transition-all hover:bg-fg/5 hover:text-fg/60",
                                    "active:scale-95",
                                  )}
                                  aria-label={t("groupChats.memoriesPage.memoryActions")}
                                >
                                  <EllipsisVertical size={16} />
                                </button>
                              </div>

                              {/* Pin indicator */}
                              {item.isPinned && (
                                <div className="flex items-center justify-end mt-2">
                                  <Pin size={12} className="text-warning/60" />
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
                                        "flex items-center gap-3 mt-2 pt-2 border-t border-fg/5",
                                        "text-[10px] text-fg/30",
                                      )}
                                    >
                                      {item.tokenCount > 0 && (
                                        <span>{item.tokenCount.toLocaleString()} {t("groupChats.memoriesPage.tokens")}</span>
                                      )}
                                      {item.cycle && <span>{t("groupChats.memoriesPage.cycle")} {item.cycle}</span>}
                                      {item.lastAccessedAt > 0 && (
                                        <span>
                                          {t("groupChats.memoriesPage.accessed")}{" "}
                                          {new Date(item.lastAccessedAt).toLocaleDateString()}
                                        </span>
                                      )}
                                      <span
                                        className={item.isCold ? "text-info/50" : "text-warning/50"}
                                      >
                                        {item.isCold
                                          ? t("groupChats.memoriesPage.cold")
                                          : `${t("groupChats.memoriesPage.hot")} ${item.importanceScore.toFixed(1)}`}
                                      </span>
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
          ) : ui.activeTab === "pinned" ? (
            <motion.div
              key="pinned"
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -8 }}
              transition={{ duration: 0.15, ease: "easeOut" }}
              className={cn("px-3 py-4", "space-y-5")}
            >
              <div className="flex items-center gap-2 mb-3">
                <span className="text-[12px] font-semibold uppercase tracking-wider text-fg/50">
                  Pinned Messages
                </span>
                <span className="text-[10px] text-fg/20 ml-auto">
                  {pinnedMessages.length.toLocaleString()}
                </span>
                <button
                  onClick={handleRefresh}
                  className={cn(
                    "flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg",
                    "border border-fg/10 bg-fg/5",
                    "text-[11px] font-semibold text-fg/50",
                    "hover:bg-fg/8 hover:text-fg/70",
                    "transition-all active:scale-95",
                  )}
                >
                  <RefreshCw size={12} />
                  Refresh
                </button>
              </div>

              {pinnedMessages.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-16">
                  <div className="flex h-16 w-16 items-center justify-center rounded-2xl border border-fg/10 bg-fg/5 mb-4">
                    <Pin className="h-7 w-7 text-fg/20" />
                  </div>
                  <h3 className="mb-1 text-base font-semibold text-fg">No pinned messages</h3>
                  <p className="text-center text-sm text-fg/40 max-w-60">
                    Pin important group chat messages to always keep them in context.
                  </p>
                </div>
              ) : (
                <div className="space-y-3">
                  {pinnedMessages.map((message) => (
                    <div
                      key={message.id}
                      className={cn(
                        "rounded-xl border border-fg/6 bg-fg/2 px-4 py-3",
                        "hover:border-fg/10 hover:bg-fg/3",
                      )}
                    >
                      <div className="flex items-start gap-3">
                        <div className="min-w-0 flex-1">
                          <div className="mb-1 flex items-center gap-2 text-[11px] uppercase tracking-wider text-fg/35">
                            <span>{message.role === "assistant" ? "Assistant" : "User"}</span>
                            <span>•</span>
                            <span>{new Date(message.createdAt).toLocaleDateString()}</span>
                          </div>
                          <p
                            className={cn(typography.bodySmall.size, "leading-relaxed text-fg/75")}
                          >
                            {message.content}
                          </p>
                        </div>
                        <button
                          type="button"
                          onClick={async () => {
                            try {
                              await handleTogglePinnedMessage(message.id);
                              dispatch({ type: "SET_ACTION_ERROR", value: null });
                            } catch (err: any) {
                              dispatch({
                                type: "SET_ACTION_ERROR",
                                value: err?.message || t("groupChats.memoriesPage.failedToUnpinMessage"),
                              });
                            }
                          }}
                          className={cn(
                            "shrink-0 rounded-lg border border-fg/10 bg-fg/5 px-3 py-1.5",
                            "text-[11px] font-semibold text-fg/60 hover:bg-fg/8 hover:text-fg/80",
                            "transition-all active:scale-95",
                          )}
                        >
                          Unpin
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </motion.div>
          ) : (
            <motion.div
              key="tools"
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -8 }}
              transition={{ duration: 0.15, ease: "easeOut" }}
              className={cn("px-3 py-4", "space-y-5")}
            >
              <div className="flex items-center gap-2 mb-3">
                <span className="text-[12px] font-semibold uppercase tracking-wider text-fg/50">
                  Activity Log
                </span>
                <span className="text-[10px] text-fg/20 ml-auto">
                  {(session.memoryToolEvents?.length ?? 0).toLocaleString()} events
                </span>
              </div>
              <ToolLog
                events={session.memoryToolEvents || []}
                onRevert={handleRevertMemoryEvent}
                revertingEventId={revertingEventId}
              />
            </motion.div>
          )}
        </AnimatePresence>
      </main>
      </div>

      {/* Summary Editor BottomMenu */}
      <BottomMenu
        isOpen={showSummaryEditor}
        onClose={() => setShowSummaryEditor(false)}
        title={t("groupChats.memoriesPage.contextSummaryTitle")}
      >
        <div className="space-y-4 text-fg">
          <textarea
            value={ui.summaryDraft}
            onChange={(e) => dispatch({ type: "SET_SUMMARY_DRAFT", value: e.target.value })}
            rows={6}
            className={cn(
              "w-full p-3",
              radius.lg,
              "border border-fg/10 bg-surface-el/30",
              "text-sm text-fg/90 resize-none leading-relaxed",
              "focus:border-fg/20 focus:outline-none focus:ring-1 focus:ring-fg/10",
              "placeholder:text-fg/30",
            )}
            placeholder={t("groupChats.memoriesPage.contextSummaryPlaceholder")}
            autoFocus
          />
          {session?.memorySummaryTokenCount && session.memorySummaryTokenCount > 0 ? (
            <p className="text-[10px] text-fg/30">
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
                "border border-fg/10 bg-fg/5",
                "text-sm font-medium text-fg/60",
                "transition-all hover:border-fg/15 hover:bg-fg/8 hover:text-fg/80",
                "active:scale-[0.98]",
              )}
            >
              {t("groupChats.memoriesPage.cancel")}
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
                "border border-accent/30 bg-accent/15",
                "text-sm font-semibold text-accent/80",
                "transition-all hover:border-accent/50 hover:bg-accent/25",
                "active:scale-[0.98]",
                "disabled:opacity-40 disabled:pointer-events-none",
              )}
            >
              {ui.isSavingSummary ? t("groupChats.memoriesPage.saving") : t("groupChats.memoriesPage.save")}
            </button>
          </div>
        </div>
      </BottomMenu>

      {/* Add Memory BottomMenu */}
      <BottomMenu
        isOpen={showAddMemoryMenu}
        onClose={() => setShowAddMemoryMenu(false)}
        title={t("groupChats.memoriesPage.addMemoryTitle")}
      >
        <div className="space-y-4 text-fg">
          <textarea
            value={ui.newMemory}
            onChange={(e) => dispatch({ type: "SET_NEW_MEMORY", value: e.target.value })}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey && ui.newMemory.trim()) {
                e.preventDefault();
                setShowAddMemoryMenu(false);
                void handleAddNew();
              }
            }}
            rows={3}
            className={cn(
              "w-full p-3",
              radius.lg,
              "border border-fg/10 bg-surface-el/30",
              "text-sm text-fg/90 resize-none leading-relaxed",
              "focus:border-fg/20 focus:outline-none focus:ring-1 focus:ring-fg/10",
              "placeholder:text-fg/30",
            )}
            placeholder={t("groupChats.memoriesPage.memoryPlaceholder")}
            autoFocus
          />
          <button
            onClick={() => {
              setShowAddMemoryMenu(false);
              void handleAddNew();
            }}
            disabled={!ui.newMemory.trim() || ui.isAdding}
            className={cn(
              "w-full px-4 py-2.5 flex items-center justify-center gap-2",
              radius.lg,
              "border border-accent/30 bg-accent/15",
              "text-sm font-semibold text-accent/80",
              "transition-all hover:border-accent/50 hover:bg-accent/25",
              "active:scale-[0.98]",
              "disabled:opacity-40 disabled:pointer-events-none",
            )}
          >
            {ui.isAdding ? (
              <div className="h-4 w-4 animate-spin rounded-full border-2 border-accent/30 border-t-accent" />
            ) : (
              <>
                <Plus size={14} />
                {t("groupChats.memoriesPage.saveMemory")}
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
            ? t("groupChats.memoriesPage.editMemoryTitle")
            : (() => {
                const mem = memoryItems.find((m) => m.id === ui.selectedMemoryId);
                const preview = mem?.text ?? "";
                return preview.length > 60
                  ? preview.slice(0, 60) + "..."
                  : preview || t("groupChats.memoriesPage.memoryTitle");
              })()
        }
      >
        {(() => {
          const selectedItem = memoryItems.find((m) => m.id === ui.selectedMemoryId);
          if (!selectedItem) return null;

          if (ui.memoryActionMode === "edit") {
            return (
              <div className="space-y-4 text-fg">
                <textarea
                  value={ui.editingValue}
                  onChange={(e) => dispatch({ type: "SET_EDIT_VALUE", value: e.target.value })}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && !e.shiftKey) {
                      e.preventDefault();
                      e.stopPropagation();
                      void handleSaveEdit(selectedItem.index);
                    }
                  }}
                  rows={4}
                  className={cn(
                    "w-full p-3",
                    radius.lg,
                    "border border-fg/10 bg-surface-el/30",
                    "text-sm text-fg/90 resize-none leading-relaxed",
                    "focus:border-fg/20 focus:outline-none focus:ring-1 focus:ring-fg/10",
                    "placeholder:text-fg/30",
                  )}
                  placeholder={t("groupChats.memoriesPage.editMemoryPlaceholder")}
                  autoFocus
                />
                <div className="flex gap-2">
                  <button
                    type="button"
                    onClick={() => dispatch({ type: "SET_MEMORY_ACTION_MODE", mode: "actions" })}
                    className={cn(
                      "flex-1 px-4 py-2.5",
                      radius.lg,
                      "border border-fg/10 bg-fg/5",
                      "text-sm font-medium text-fg/60",
                      "transition-all hover:border-fg/15 hover:bg-fg/8 hover:text-fg/80",
                      "active:scale-[0.98]",
                    )}
                  >
                    {t("groupChats.memoriesPage.cancel")}
                  </button>
                  <button
                    type="button"
                    onClick={() => void handleSaveEdit(selectedItem.index)}
                    className={cn(
                      "flex-1 px-4 py-2.5 flex items-center justify-center gap-2",
                      radius.lg,
                      "border border-accent/30 bg-accent/15",
                      "text-sm font-semibold text-accent/80",
                      "transition-all hover:border-accent/50 hover:bg-accent/25",
                      "active:scale-[0.98]",
                    )}
                  >
                    <Check size={14} />
                    {t("groupChats.memoriesPage.save")}
                  </button>
                </div>
              </div>
            );
          }

          return (
            <div className="space-y-1 text-fg">
              <MemoryActionRow
                icon={Edit2}
                label={t("groupChats.memoriesPage.edit")}
                iconBg="bg-info/20"
                onClick={() => {
                  startEdit(selectedItem.index, selectedItem.text);
                  dispatch({ type: "SET_MEMORY_ACTION_MODE", mode: "edit" });
                }}
              />
              <MemoryActionRow
                icon={selectedItem.isPinned ? PinOff : Pin}
                label={selectedItem.isPinned ? t("groupChats.memoriesPage.unpin") : t("groupChats.memoriesPage.pin")}
                iconBg="bg-warning/20"
                onClick={async () => {
                  try {
                    await handleTogglePin(selectedItem.index);
                    dispatch({ type: "SET_ACTION_ERROR", value: null });
                  } catch (err: any) {
                    dispatch({
                      type: "SET_ACTION_ERROR",
                      value: err?.message || t("groupChats.memoriesPage.failedToToggleMemPin"),
                    });
                  }
                  dispatch({ type: "CLOSE_MEMORY_ACTIONS" });
                }}
              />
              <MemoryActionRow
                icon={selectedItem.isCold ? Flame : Snowflake}
                label={selectedItem.isCold ? t("groupChats.memoriesPage.setHot") : t("groupChats.memoriesPage.setCold")}
                iconBg={selectedItem.isCold ? "bg-warning/20" : "bg-info/20"}
                disabled={ui.memoryTempBusy === selectedItem.index}
                onClick={async () => {
                  await handleSetColdState(selectedItem.index, !selectedItem.isCold);
                  dispatch({ type: "CLOSE_MEMORY_ACTIONS" });
                }}
              />

              <div className="h-px bg-fg/5 my-2" />

              <MemoryActionRow
                icon={Trash2}
                label={t("groupChats.memoriesPage.delete")}
                variant="danger"
                onClick={async () => {
                  try {
                    await handleRemove(selectedItem.index);
                    dispatch({ type: "SET_ACTION_ERROR", value: null });
                    dispatch({ type: "SHIFT_EXPANDED_AFTER_DELETE", index: selectedItem.index });
                  } catch (err: any) {
                    dispatch({
                      type: "SET_ACTION_ERROR",
                      value: err?.message || t("groupChats.memoriesPage.failedToRemoveMemory"),
                    });
                  }
                  dispatch({ type: "CLOSE_MEMORY_ACTIONS" });
                }}
              />
            </div>
          );
        })()}
      </BottomMenu>
    </div>
  );
}
