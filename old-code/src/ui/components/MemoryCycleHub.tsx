import type { ComponentType } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { AlertTriangle, Check, Clock, Cpu, Hourglass, Play, RefreshCw, ScrollText, X } from "lucide-react";

import { cn, interactive, radius } from "../design-tokens";
import { useI18n } from "../../core/i18n/context";
import type { TranslationKey } from "../../core/i18n/context";

export type DynamicMemoryCycleStatus = {
  runMode: "auto" | "askFirst" | "manual";
  interval: number;
  messagesSinceLastCycle: number;
  messagesUntilNextCycle: number;
  totalConversationMessages: number;
  pendingApprovalCount: number | null;
  skipped: boolean;
  latestCycleStatus: string | null;
};

const MEMORY_PROGRESS_TOTAL = 4;
const MEMORY_STEP_LABELS: Record<number, TranslationKey> = {
  1: "chats.memories.stepSummarizing",
  2: "chats.memories.stepAnalyzing",
  3: "chats.memories.stepApplying",
  4: "chats.memories.stepOrganizing",
};

type CycleHubState = "collecting" | "ready" | "approval" | "running" | "success" | "failed";

const CYCLE_HUB_ACCENTS: Record<
  CycleHubState,
  { section: string; chip: string; bar: string; track: string; subtitle: string }
> = {
  collecting: {
    section: "bg-transparent",
    chip: "bg-fg/5 text-fg/55",
    bar: "bg-fg/35",
    track: "bg-fg/10",
    subtitle: "text-fg/55",
  },
  ready: {
    section: "bg-accent/[0.04]",
    chip: "bg-accent/10 text-accent",
    bar: "bg-accent/70",
    track: "bg-accent/15",
    subtitle: "text-accent/80",
  },
  approval: {
    section: "bg-warning/[0.04]",
    chip: "bg-warning/10 text-warning",
    bar: "bg-warning/70",
    track: "bg-warning/15",
    subtitle: "text-warning/80",
  },
  running: {
    section: "bg-info/[0.05]",
    chip: "bg-info/10 text-info",
    bar: "bg-info/70",
    track: "bg-info/15",
    subtitle: "text-info/80",
  },
  success: {
    section: "bg-accent/[0.05]",
    chip: "bg-accent/10 text-accent",
    bar: "bg-accent/70",
    track: "bg-accent/15",
    subtitle: "text-accent/85",
  },
  failed: {
    section: "bg-danger/[0.05]",
    chip: "bg-danger/10 text-danger",
    bar: "bg-danger/70",
    track: "bg-danger/15",
    subtitle: "text-danger/85",
  },
};

const CYCLE_HUB_ICONS: Record<CycleHubState, ComponentType<{ size?: number; className?: string }>> =
  {
    collecting: Clock,
    ready: Play,
    approval: Hourglass,
    running: RefreshCw,
    success: Check,
    failed: AlertTriangle,
  };

export function MemoryCycleHub({
  status,
  running,
  retrying,
  retrySuccess,
  errorMessage,
  step,
  generationTokens = null,
  generationTps = null,
  generationStalled = false,
  generationStalledSeconds = 0,
  liveOutputAvailable = false,
  onRun,
  onCancel,
  onRetry,
  onPickModel,
  onDismissError,
  onDismissSuccess,
  onShowLiveOutput,
  conversationScope = "conversation",
}: {
  status: DynamicMemoryCycleStatus | null;
  running: boolean;
  retrying: boolean;
  retrySuccess: boolean;
  errorMessage: string | null;
  step: number | null;
  generationTokens?: number | null;
  generationTps?: number | null;
  generationStalled?: boolean;
  generationStalledSeconds?: number;
  liveOutputAvailable?: boolean;
  onRun: () => void;
  onCancel: () => void;
  onRetry: () => void;
  onPickModel?: () => void;
  onDismissError: () => void;
  onDismissSuccess: () => void;
  onShowLiveOutput?: () => void;
  conversationScope?: "conversation" | "branch";
}) {
  const { t } = useI18n();

  const state: CycleHubState = running
    ? "running"
    : retrySuccess
      ? "success"
      : errorMessage
        ? "failed"
        : status?.pendingApprovalCount != null
          ? "approval"
          : status && status.messagesUntilNextCycle === 0
            ? "ready"
            : "collecting";
  const accent = CYCLE_HUB_ACCENTS[state];
  const Icon = CYCLE_HUB_ICONS[state];

  const stepLabelKey = step != null ? MEMORY_STEP_LABELS[step] : undefined;
  const subtitle =
    state === "running"
      ? ((stepLabelKey ? t(stepLabelKey) : null) ??
        (retrying ? t("chats.memories.retryingCycle") : t("chats.memories.processingMemories")))
      : state === "success"
        ? t("chats.memories.cycleProcessedSuccess")
        : state === "failed"
          ? t("chats.memories.memorySystemError")
          : state === "approval"
            ? t("chats.memories.cycleWaitingForApproval", {
                count: status?.pendingApprovalCount ?? 0,
              })
            : state === "ready"
              ? status?.runMode === "manual"
                ? t("chats.memories.cycleManualReady", {
                    count: status?.messagesSinceLastCycle ?? 0,
                  })
                : t("chats.memories.cycleReadyAfterResponse")
              : status?.messagesUntilNextCycle === 1
                ? t("chats.memories.cycleMessageNeeded")
                : t("chats.memories.cycleMessagesNeeded", {
                    count: status?.messagesUntilNextCycle ?? 0,
                  });

  const extrasTransition = { duration: 0.22, ease: "easeOut" as const };
  const actionButton = cn(
    "flex min-w-[6.5rem] items-center justify-center gap-2 rounded-xl px-5 py-2.5 text-sm font-semibold",
    interactive.transition.default,
    interactive.active.scale,
  );
  const smallButton = cn(
    "flex items-center gap-1.5 px-2.5 py-1.5",
    radius.md,
    "text-[11px] font-semibold",
    interactive.transition.fast,
    interactive.active.scale,
  );

  return (
    <section
      className={cn(
        "border-b border-fg/10 px-3 py-3 lg:px-8 transition-colors duration-300",
        accent.section,
      )}
      aria-label={t("chats.memories.cycleStatusAria")}
    >
      <div className="flex gap-4">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-3">
            <div
              className={cn(
                "flex h-9 w-9 shrink-0 items-center justify-center rounded-lg",
                "transition-colors duration-300",
                accent.chip,
              )}
            >
              <AnimatePresence mode="wait" initial={false}>
                <motion.span
                  key={state}
                  initial={{ scale: 0.5, opacity: 0, rotate: -45 }}
                  animate={{ scale: 1, opacity: 1, rotate: 0 }}
                  exit={{ scale: 0.5, opacity: 0, rotate: 45 }}
                  transition={{ type: "spring", stiffness: 420, damping: 28 }}
                  className="flex"
                >
                  <Icon size={16} className={state === "running" ? "animate-spin" : undefined} />
                </motion.span>
              </AnimatePresence>
            </div>
            <div className="min-w-0 flex-1">
              <p className="text-sm font-semibold text-fg">
                {t("chats.memories.nextMemoryCycle")}
              </p>
              <AnimatePresence mode="wait" initial={false}>
                <motion.p
                  key={subtitle}
                  initial={{ opacity: 0, y: 4 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -4 }}
                  transition={{ duration: 0.16, ease: "easeOut" }}
                  className={cn(
                    "mt-0.5 truncate text-xs transition-colors duration-300",
                    accent.subtitle,
                  )}
                >
                  {subtitle}
                </motion.p>
              </AnimatePresence>
            </div>
          </div>

          <AnimatePresence initial={false}>
            {state === "running" && (
              <motion.div
                key="bar"
                initial={{ height: 0, opacity: 0 }}
                animate={{ height: "auto", opacity: 1 }}
                exit={{ height: 0, opacity: 0 }}
                transition={extrasTransition}
                className="overflow-hidden"
              >
                <div
                  className={cn(
                    "mt-3 h-1.5 w-full overflow-hidden rounded-full",
                    accent.track,
                  )}
                >
                  {step ? (
                    <motion.div
                      initial={false}
                      animate={{ width: `${(step / MEMORY_PROGRESS_TOTAL) * 100}%` }}
                      transition={{ type: "spring", stiffness: 170, damping: 26 }}
                      className={cn("h-full rounded-full", accent.bar)}
                    />
                  ) : (
                    <div
                      className={cn(
                        "h-full w-1/3 rounded-full animate-[indeterminate_1.5s_ease-in-out_infinite]",
                        accent.bar,
                      )}
                    />
                  )}
                </div>
              </motion.div>
            )}
            {state === "running" && generationTokens != null && (
              <motion.div
                key="generation"
                initial={{ height: 0, opacity: 0 }}
                animate={{ height: "auto", opacity: 1 }}
                exit={{ height: 0, opacity: 0 }}
                transition={extrasTransition}
                className="overflow-hidden"
              >
                <div className="flex items-center justify-between gap-3 pt-2.5">
                  {generationStalled ? (
                    <p className="flex items-center gap-1.5 text-[11px] tabular-nums text-warning/80">
                      <AlertTriangle size={11} className="shrink-0" />
                      {t("chats.memories.generationStalled", { seconds: generationStalledSeconds })}
                    </p>
                  ) : (
                    <p className="text-[11px] tabular-nums text-info/60">
                      {t("chats.memories.generatingTokens", { tokens: generationTokens })}
                      {generationTps && generationTps > 0
                        ? ` · ${generationTps.toFixed(1)} tok/s`
                        : ""}
                    </p>
                  )}
                  {liveOutputAvailable && onShowLiveOutput && (
                    <button
                      type="button"
                      onClick={onShowLiveOutput}
                      className={cn(
                        "inline-flex shrink-0 items-center gap-1.5 rounded-md px-2 py-1",
                        "bg-info/10 text-[11px] font-medium text-info/80 transition hover:bg-info/20",
                      )}
                    >
                      <ScrollText size={12} />
                      {t("chats.memories.viewLiveOutput")}
                    </button>
                  )}
                </div>
              </motion.div>
            )}
            {state === "failed" && (
              <motion.div
                key="failed"
                initial={{ height: 0, opacity: 0 }}
                animate={{ height: "auto", opacity: 1 }}
                exit={{ height: 0, opacity: 0 }}
                transition={extrasTransition}
                className="overflow-hidden"
              >
                <div className="pt-2.5">
                  <p className="text-xs leading-relaxed text-danger/80">{errorMessage}</p>
                  <div className="mt-2.5 flex flex-wrap gap-2">
                    <button
                      type="button"
                      onClick={onRetry}
                      className={cn(
                        smallButton,
                        "bg-danger/15 text-danger/90 hover:bg-danger/25",
                      )}
                    >
                      <RefreshCw size={12} />
                      {t("chats.memories.tryAgain")}
                    </button>
                    {onPickModel && (
                      <button
                        type="button"
                        onClick={onPickModel}
                        className={cn(
                          smallButton,
                          "bg-info/15 text-info/90 hover:bg-info/25",
                        )}
                      >
                        <Cpu size={12} />
                        {t("chats.memories.tryDifferentModel")}
                      </button>
                    )}
                  </div>
                </div>
              </motion.div>
            )}
            {state !== "running" && state !== "success" && state !== "failed" && status && (
              <motion.div
                key="meta"
                initial={{ height: 0, opacity: 0 }}
                animate={{ height: "auto", opacity: 1 }}
                exit={{ height: 0, opacity: 0 }}
                transition={extrasTransition}
                className="overflow-hidden"
              >
                <div className="flex flex-wrap gap-x-4 gap-y-1 pt-2.5 text-[11px] text-fg/40">
                  <span>
                    {t("chats.memories.cycleMode", {
                      mode:
                        status.runMode === "askFirst"
                          ? t("chats.memories.cycleModeAskFirst")
                          : status.runMode === "manual"
                            ? t("chats.memories.cycleModeManual")
                            : t("chats.memories.cycleModeAuto"),
                    })}
                  </span>
                  <span>
                    {t(
                      conversationScope === "branch"
                        ? "chats.memories.branchConversationMessageCount"
                        : "chats.memories.conversationMessageCount",
                      {
                        count: status.totalConversationMessages.toLocaleString(),
                      },
                    )}
                  </span>
                  {status.latestCycleStatus && (
                    <span>
                      {t("chats.memories.lastCycleStatus", {
                        status:
                          status.latestCycleStatus === "complete"
                            ? t("chats.memories.cycleStatusComplete")
                            : status.latestCycleStatus === "error"
                              ? t("chats.memories.cycleStatusError")
                              : t("chats.memories.cycleStatusUnknown"),
                      })}
                    </span>
                  )}
                </div>
                {status.skipped && status.pendingApprovalCount == null && (
                  <p className="mt-2 border-l-2 border-warning/50 pl-2 text-xs text-warning/80">
                    {t("chats.memories.skippedCycleNotice")}
                  </p>
                )}
              </motion.div>
            )}
          </AnimatePresence>
        </div>

        <div className="flex shrink-0 items-center">
          <button
            type="button"
            onClick={
              state === "running"
                ? onCancel
                : state === "failed"
                  ? onDismissError
                  : state === "success"
                    ? onDismissSuccess
                    : onRun
            }
            aria-label={
              state === "failed"
                ? t("groupChats.footer.dismissError")
                : state === "success"
                  ? t("chats.errorBanner.dismiss")
                  : undefined
            }
            className={cn(
              actionButton,
              state === "running"
                ? "bg-danger/10 text-danger hover:bg-danger/20"
                : state === "failed"
                  ? "text-danger/70 hover:bg-danger/10 hover:text-danger"
                  : state === "success"
                    ? "text-accent/70 hover:bg-accent/10 hover:text-accent"
                    : state === "ready"
                      ? "bg-accent/15 text-accent hover:bg-accent/25"
                      : "bg-fg/8 text-fg/70 hover:bg-fg/12 hover:text-fg/90",
            )}
          >
            <motion.span
              key={
                state === "running"
                  ? "cancel"
                  : state === "failed" || state === "success"
                    ? "dismiss"
                    : "run"
              }
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              transition={{ duration: 0.15, ease: "easeOut" }}
              className="flex items-center justify-center gap-2"
            >
              {state === "running" ? (
                <>
                  <X size={15} />
                  {t("common.buttons.cancel")}
                </>
              ) : state === "failed" || state === "success" ? (
                <X size={15} />
              ) : (
                <>
                  <Play size={15} />
                  {t("groupChats.memories.run")}
                </>
              )}
            </motion.span>
          </button>
        </div>
      </div>
    </section>
  );
}
