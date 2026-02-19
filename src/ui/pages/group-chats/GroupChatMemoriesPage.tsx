import { useState } from "react";
import type { ComponentType } from "react";
import { useParams } from "react-router-dom";
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
  Cpu,
  EllipsisVertical,
  PinOff,
} from "lucide-react";

import { typography, radius, cn, interactive, colors, components } from "../../design-tokens";
import { Routes, useNavigationManager } from "../../navigation";
import { BottomMenu } from "../../components/BottomMenu";
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
        className={cn(
          "flex items-center justify-center w-8 h-8 rounded-lg",
          iconBg || "bg-fg/10",
        )}
      >
        <Icon size={16} className={cn(variant === "danger" ? "text-danger" : "text-fg")} />
      </div>
      <span
        className={cn(
          "text-[15px] text-left",
          variant === "danger" ? "text-danger" : "text-fg/90",
        )}
      >
        {label}
      </span>
    </button>
  );
}

export function GroupChatMemoriesPage() {
  const { backOrReplace } = useNavigationManager();
  const { groupSessionId } = useParams();

  const {
    session,
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
    handleRefresh,
    handleSaveSummaryClick,
  } = useGroupChatMemoriesController(groupSessionId);

  const [showAddMemoryMenu, setShowAddMemoryMenu] = useState(false);
  const [showSummaryEditor, setShowSummaryEditor] = useState(false);

  const tabs = [
    { id: "memories" as const, icon: Bot, label: "Memories" },
    { id: "tools" as const, icon: Clock, label: "Activity" },
  ];

  if (loading) {
    return (
      <div className={cn("flex h-screen items-center justify-center", colors.surface.base)}>
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-fg/10 border-t-white/60" />
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
        <p className={cn("text-sm", colors.text.secondary)}>{error || "Session not found"}</p>
        <button
          onClick={() => backOrReplace(Routes.groupChats)}
          className={cn(
            components.button.primary,
            components.button.sizes.md,
            "bg-fg/5 text-fg hover:bg-fg/10",
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
          "sticky top-0 z-20 border-b border-fg/10 px-4",
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
              aria-label="Go back"
            >
              <ArrowLeft size={14} strokeWidth={2.5} />
            </button>
            <div className="min-w-0 flex-1 flex items-baseline gap-2 text-left">
              <span className={cn("shrink-0", typography.h1.size, typography.h1.weight, colors.text.primary)}>
                Memories
              </span>
              <span className={cn("truncate text-sm font-medium", colors.text.tertiary)}>
                {session.name}
              </span>
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-2 ml-auto">
            {(ui.retryStatus === "retrying" || ui.memoryStatus === "processing") && (
              <div
                className={cn(
                  radius.full,
                  "border px-2 py-1",
                  typography.overline.size,
                  typography.overline.weight,
                  typography.overline.tracking,
                  typography.overline.transform,
                  "border-info/30 bg-info/15 text-info",
                )}
              >
                Processing
              </div>
            )}
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
        {/* Error / Status Banners */}
        {(ui.actionError || ui.retryStatus !== "idle" || ui.memoryStatus === "processing") && (
          <div className="px-3 pt-3">
            {(ui.retryStatus === "retrying" || ui.memoryStatus === "processing") ? (
              <div
                className={cn(
                  radius.md,
                  "bg-info/10 border border-info/20 p-3 flex items-center gap-3 animate-pulse",
                )}
              >
                <RefreshCw className="h-5 w-5 text-info shrink-0 animate-spin" />
                <div className={cn("flex-1", typography.body.size, "text-info")}>
                  <p className="font-semibold">AI is organizing group memories...</p>
                </div>
              </div>
            ) : ui.retryStatus === "success" ? (
              <div
                className={cn(
                  radius.md,
                  "bg-accent/10 border border-accent/20 p-3 flex items-center gap-3",
                )}
              >
                <Check className="h-5 w-5 text-accent shrink-0" />
                <div className={cn("flex-1", typography.body.size, "text-accent/80")}>
                  <p className="font-semibold">Memory cycle processed successfully!</p>
                </div>
                <button
                  onClick={() => dispatch({ type: "SET_RETRY_STATUS", value: "idle" })}
                  className="text-accent hover:text-accent/80"
                >
                  <X size={16} />
                </button>
              </div>
            ) : ui.actionError ? (
              <div
                className={cn(
                  radius.md,
                  "bg-danger/10 border border-danger/20 p-3 flex items-start gap-3",
                )}
              >
                <AlertTriangle className="h-5 w-5 text-danger shrink-0" />
                <div className={cn("flex-1", typography.body.size, "text-danger")}>
                  <p className="font-semibold mb-1">Memory action failed</p>
                  <p className="opacity-90">{ui.actionError}</p>
                </div>
                <button
                  onClick={() => dispatch({ type: "SET_ACTION_ERROR", value: null })}
                  className="text-danger hover:text-danger"
                >
                  <X size={16} />
                </button>
              </div>
            ) : null}
          </div>
        )}

        {ui.pendingRefresh && ui.memoryStatus !== "processing" && (
          <div className="px-3 pt-3">
            <div
              className={cn(
                radius.md,
                "bg-fg/5 border border-fg/10 p-3 flex items-center justify-between gap-3",
              )}
            >
              <div className={cn(typography.bodySmall.size, colors.text.secondary)}>
                New memory updates available
              </div>
              <button
                type="button"
                onClick={handleRefresh}
                className={cn(
                  typography.caption.size,
                  "font-semibold px-3 py-1",
                  radius.full,
                  "border border-fg/15 bg-fg/10 text-fg/80",
                  interactive.transition.fast,
                  interactive.active.scale,
                )}
              >
                Refresh
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
                  Context Summary
                </span>
                {session?.memorySummaryTokenCount && session.memorySummaryTokenCount > 0 ? (
                  <span className="text-[10px] text-fg/30 ml-auto">
                    {session.memorySummaryTokenCount.toLocaleString()} tokens
                  </span>
                ) : null}
              </div>
              <p
                className={cn(
                  typography.bodySmall.size,
                  "leading-relaxed line-clamp-4 min-h-14",
                  ui.summaryDraft
                    ? "text-accent/70"
                    : "text-accent/25 italic",
                )}
              >
                {ui.summaryDraft || "Tap to add a context summary..."}
              </p>
            </button>

            {/* Memories Section */}
            <div>
              <div className="flex items-center gap-2 mb-2">
                <span className="text-[12px] font-semibold uppercase tracking-wider text-fg/50">
                  {ui.searchTerm.trim() ? `Results (${filteredMemories.length})` : "Saved Memories"}
                </span>
                <span className="text-[10px] text-fg/30 ml-auto">
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
                      aria-label="Clear search"
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
                  aria-label="Add memory"
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
                    {ui.searchTerm ? "No matching memories" : "No memories yet"}
                  </h3>
                  <p className="text-center text-sm text-fg/40 max-w-60">
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
                            ? "border-fg/10 bg-fg/2"
                            : "border-fg/6 bg-fg/2 hover:border-fg/10 hover:bg-fg/3",
                        )}
                      >
                        <div
                          className="px-4 py-3 cursor-pointer"
                          onClick={() => dispatch({ type: "TOGGLE_EXPANDED", index: item.index })}
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
                              aria-label="Memory actions"
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
                                    <span>{item.tokenCount.toLocaleString()} tokens</span>
                                  )}
                                  {item.cycle && <span>Cycle {item.cycle}</span>}
                                  {item.lastAccessedAt > 0 && (
                                    <span>
                                      Accessed {new Date(item.lastAccessedAt).toLocaleDateString()}
                                    </span>
                                  )}
                                  <span className={item.isCold ? "text-info/50" : "text-warning/50"}>
                                    {item.isCold ? "Cold" : `Hot ${item.importanceScore.toFixed(1)}`}
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
              <button
                onClick={handleRunMemoryCycle}
                disabled={ui.retryStatus === "retrying" || ui.memoryStatus === "processing"}
                className={cn(
                  "flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg",
                  "border border-fg/10 bg-fg/5",
                  "text-[11px] font-semibold text-fg/50",
                  "hover:bg-fg/8 hover:text-fg/70",
                  "disabled:opacity-40 disabled:pointer-events-none",
                  "transition-all active:scale-95",
                )}
              >
                <Cpu
                  size={12}
                  className={cn(
                    (ui.retryStatus === "retrying" || ui.memoryStatus === "processing") && "animate-pulse",
                  )}
                />
                Run
              </button>
            </div>
            <ToolLog events={session.memoryToolEvents || []} />
          </motion.div>
        )}
        </AnimatePresence>
      </main>

      {/* Summary Editor BottomMenu */}
      <BottomMenu
        isOpen={showSummaryEditor}
        onClose={() => setShowSummaryEditor(false)}
        title="Context Summary"
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
              "focus:border-fg/20 focus:outline-none focus:ring-1 focus:ring-white/10",
              "placeholder:text-fg/30",
            )}
            placeholder="Short recap used to keep context consistent across messages..."
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
                dispatch({ type: "SYNC_SUMMARY_FROM_SESSION", value: session?.memorySummary ?? "" });
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
                "border border-accent/30 bg-accent/15",
                "text-sm font-semibold text-accent/80",
                "transition-all hover:border-accent/50 hover:bg-accent/25",
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
        isOpen={showAddMemoryMenu}
        onClose={() => setShowAddMemoryMenu(false)}
        title="Add Memory"
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
              "focus:border-fg/20 focus:outline-none focus:ring-1 focus:ring-white/10",
              "placeholder:text-fg/30",
            )}
            placeholder="What should be remembered?"
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
              <div className="space-y-4 text-fg">
                <textarea
                  value={ui.editingValue}
                  onChange={(e) => dispatch({ type: "SET_EDIT_VALUE", value: e.target.value })}
                  rows={4}
                  className={cn(
                    "w-full p-3",
                    radius.lg,
                    "border border-fg/10 bg-surface-el/30",
                    "text-sm text-fg/90 resize-none leading-relaxed",
                    "focus:border-fg/20 focus:outline-none focus:ring-1 focus:ring-white/10",
                    "placeholder:text-fg/30",
                  )}
                  placeholder="Enter memory content..."
                  autoFocus
                />
                <div className="flex gap-2">
                  <button
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
                      "border border-accent/30 bg-accent/15",
                      "text-sm font-semibold text-accent/80",
                      "transition-all hover:border-accent/50 hover:bg-accent/25",
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
            <div className="space-y-1 text-fg">
              <MemoryActionRow
                icon={Edit2}
                label="Edit"
                iconBg="bg-info/20"
                onClick={() => {
                  startEdit(selectedItem.index, selectedItem.text);
                  dispatch({ type: "SET_MEMORY_ACTION_MODE", mode: "edit" });
                }}
              />
              <MemoryActionRow
                icon={selectedItem.isPinned ? PinOff : Pin}
                label={selectedItem.isPinned ? "Unpin" : "Pin"}
                iconBg="bg-warning/20"
                onClick={async () => {
                  try {
                    await handleTogglePin(selectedItem.index);
                    dispatch({ type: "SET_ACTION_ERROR", value: null });
                  } catch (err: any) {
                    dispatch({ type: "SET_ACTION_ERROR", value: err?.message || "Failed to toggle pin" });
                  }
                  dispatch({ type: "CLOSE_MEMORY_ACTIONS" });
                }}
              />
              <MemoryActionRow
                icon={selectedItem.isCold ? Flame : Snowflake}
                label={selectedItem.isCold ? "Set Hot" : "Set Cold"}
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
                label="Delete"
                variant="danger"
                onClick={async () => {
                  try {
                    await handleRemove(selectedItem.index);
                    dispatch({ type: "SET_ACTION_ERROR", value: null });
                    dispatch({ type: "SHIFT_EXPANDED_AFTER_DELETE", index: selectedItem.index });
                  } catch (err: any) {
                    dispatch({ type: "SET_ACTION_ERROR", value: err?.message || "Failed to remove memory" });
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
