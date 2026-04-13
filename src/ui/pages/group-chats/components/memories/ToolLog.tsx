import { useState } from "react";
import type { ComponentType } from "react";
import { motion } from "framer-motion";
import {
  Plus,
  Trash2,
  Pin,
  Check,
  Cpu,
  Clock,
  ChevronDown,
  AlertTriangle,
  RefreshCw,
  RotateCcw,
} from "lucide-react";

import type { GroupSession } from "../../../../../core/storage/schemas";
import { isDevelopmentMode } from "../../../../../core/utils/env";
import {
  components,
  colors,
  radius,
  spacing,
  typography,
  cn,
  interactive,
} from "../../../../design-tokens";
import { useI18n } from "../../../../../core/i18n/context";

type MemoryToolEvent = NonNullable<GroupSession["memoryToolEvents"]>[number];

function getDebugSteps(event: MemoryToolEvent): unknown[] {
  const raw = (event as Record<string, unknown>).debugSteps;
  return Array.isArray(raw) ? raw : [];
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
    color: "text-accent/80",
    label: "Created",
    bg: "bg-accent/10",
    border: "border-accent/20",
  },
  delete_memory: {
    icon: Trash2,
    color: "text-danger",
    label: "Deleted",
    bg: "bg-danger/10",
    border: "border-danger/20",
  },
  pin_memory: {
    icon: Pin,
    color: "text-warning",
    label: "Pinned",
    bg: "bg-warning/10",
    border: "border-warning/20",
  },
  unpin_memory: {
    icon: Pin,
    color: "text-warning/60",
    label: "Unpinned",
    bg: "bg-warning/10",
    border: "border-warning/20",
  },
  done: {
    icon: Check,
    color: "text-info",
    label: "Done",
    bg: "bg-info/10",
    border: "border-info/20",
  },
};

function ActionCard({
  action,
  isReverted,
}: {
  action: NonNullable<MemoryToolEvent["actions"]>[number];
  isReverted?: boolean;
}) {
  const style = ACTION_STYLES[action.name] || {
    icon: Cpu,
    color: "text-zinc-300",
    label: action.name,
    bg: "bg-fg/5",
    border: "border-fg/10",
  };
  const Icon = style.icon;
  const args = action.arguments as Record<string, unknown> | undefined;
  const rawAction = action as Record<string, unknown>;
  const memoryText =
    (args?.text as string | undefined) ??
    (rawAction.deletedText as string | undefined) ??
    (rawAction.text as string | undefined);
  const category =
    (args?.category as string | undefined) ?? (rawAction.category as string | undefined);
  const important =
    (args?.important as boolean | undefined) ?? (rawAction.important as boolean | undefined);
  const confidence = args?.confidence as number | undefined;
  const id =
    (args?.id as string | undefined) ??
    (rawAction.deletedMemoryId as string | undefined) ??
    (args?.text as string | undefined);

  return (
    <div
      className={cn(
        radius.md,
        "border px-3 py-2.5 flex items-start gap-2.5 transition-colors",
        isReverted ? "bg-fg/3 border-fg/8" : style.bg,
        isReverted ? "border-fg/8" : style.border,
      )}
    >
      <Icon
        size={14}
        className={cn(isReverted ? "text-fg/30" : style.color, "mt-0.5 shrink-0")}
      />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span
            className={cn(
              "text-[11px] font-semibold",
              isReverted ? "text-fg/40 line-through" : style.color,
            )}
          >
            {style.label}
          </span>
          {isReverted && (
            <span className="rounded-md border border-fg/10 bg-fg/5 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-fg/40">
              undone
            </span>
          )}
          {category && (
            <span
              className={cn(
                "text-[10px] px-1.5 py-0.5 rounded-full bg-fg/5 border border-fg/8",
                isReverted ? "text-fg/30" : "text-fg/40",
              )}
            >
              {category.replace(/_/g, " ")}
            </span>
          )}
          {important && !isReverted && (
            <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-warning/20 text-warning border border-warning/30">
              pinned
            </span>
          )}
          {confidence != null && !isReverted && (
            <span
              className={cn(
                "text-[10px] px-1.5 py-0.5 rounded-full border",
                confidence < 0.7
                  ? "bg-warning/20 text-warning border-warning/30"
                  : "bg-danger/20 text-danger border-danger/30",
              )}
            >
              {confidence < 0.7 ? "soft-delete" : `${Math.round(confidence * 100)}%`}
            </span>
          )}
        </div>
        {memoryText && (
          <p
            className={cn(
              typography.caption.size,
              "mt-1 leading-relaxed",
              isReverted ? "text-fg/35 line-through decoration-fg/20" : colors.text.secondary,
            )}
          >
            {memoryText}
          </p>
        )}
        {id && !memoryText && (
          <p
            className={cn(
              typography.caption.size,
              "mt-1 font-mono",
              isReverted ? "text-fg/30 line-through" : colors.text.tertiary,
            )}
          >
            #{id}
          </p>
        )}
      </div>
    </div>
  );
}

function summarizeActions(actions: NonNullable<MemoryToolEvent["actions"]>): string {
  const counts: Record<string, number> = {};
  for (const a of actions) {
    const label = ACTION_STYLES[a.name]?.label || a.name;
    counts[label] = (counts[label] || 0) + 1;
  }
  return Object.entries(counts)
    .map(([label, count]) => `${count} ${label.toLowerCase()}`)
    .join(", ");
}

function CycleCard({
  event,
  defaultOpen,
  onRevert,
  reverting,
}: {
  event: MemoryToolEvent;
  defaultOpen: boolean;
  onRevert?: (event: MemoryToolEvent) => void;
  reverting?: boolean;
}) {
  const [isOpen, setIsOpen] = useState(defaultOpen);
  const [showDebug, setShowDebug] = useState(false);
  const hasError = !!event.error;
  const isReverted = !!event.revertedAt;
  const actions = event.actions || [];
  const actionSummary = actions.length ? summarizeActions(actions) : null;
  const eventTime = event.createdAt || event.timestamp || 0;
  const windowStart = event.windowStart ?? 0;
  const windowEnd = event.windowEnd ?? 0;
  const debugSteps = getDebugSteps(event);
  const debugEnabled = isDevelopmentMode() && debugSteps.length > 0;

  return (
    <div
      className={cn(
        components.card.base,
        "overflow-hidden",
        hasError && "border-danger/20",
        isReverted && "border-fg/8 bg-fg/3",
      )}
    >
      <div
        role="button"
        tabIndex={0}
        onClick={() => setIsOpen(!isOpen)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setIsOpen(!isOpen);
          }
        }}
        className={cn(
          "w-full flex items-center gap-3 px-4 py-3 text-left cursor-pointer",
          interactive.hover.brightness,
        )}
      >
        <div
          className={cn(
            "h-2 w-2 rounded-full shrink-0",
            isReverted ? "bg-fg/20" : hasError ? "bg-danger" : "bg-accent",
          )}
        />

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span
              className={cn(
                typography.caption.size,
                "font-medium",
                isReverted ? "text-fg/40" : colors.text.secondary,
              )}
            >
              {eventTime ? relativeTime(eventTime) : "Memory Cycle"}
            </span>
            {actionSummary && (
              <span
                className={cn(
                  typography.caption.size,
                  isReverted ? "text-fg/30 line-through" : colors.text.tertiary,
                )}
              >
                — {actionSummary}
              </span>
            )}
            {isReverted && (
              <span className="rounded-md border border-warning/20 bg-warning/10 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide text-warning">
                reverted
              </span>
            )}
            {hasError && <AlertTriangle size={12} className="text-danger shrink-0" />}
          </div>

          {event.summary && !isOpen && (
            <p className={cn("text-[11px] mt-0.5 truncate", colors.text.tertiary)}>
              {event.summary}
            </p>
          )}
        </div>

        {event.id && !isReverted && onRevert && (
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              onRevert(event);
            }}
            disabled={reverting}
            title={reverting ? "Reverting..." : "Revert this cycle"}
            aria-label="Revert this cycle"
            className={cn(
              "shrink-0 inline-flex h-7 w-7 items-center justify-center rounded-md",
              "border border-fg/10 bg-fg/5 text-fg/60",
              "transition hover:border-warning/30 hover:bg-warning/10 hover:text-warning",
              "disabled:pointer-events-none disabled:opacity-50",
            )}
          >
            <RotateCcw size={13} className={cn(reverting && "animate-spin")} />
          </button>
        )}

        <ChevronDown
          size={14}
          className={cn(
            colors.text.tertiary,
            "shrink-0 transition-transform",
            isOpen && "rotate-180",
          )}
        />
      </div>

      {isOpen && (
        <div className="px-4 pb-4 space-y-3">
          {isReverted && event.revertedAt && (
            <div
              className={cn(
                "flex items-center gap-1.5 rounded-md border border-warning/15 bg-warning/5 px-3 py-1.5",
                typography.caption.size,
                "text-warning/80",
              )}
            >
              <RefreshCw size={12} className="text-warning/70" />
              <span>Reverted {relativeTime(event.revertedAt)}</span>
              <span className="text-warning/40">·</span>
              <span className="text-warning/50">
                {new Date(event.revertedAt).toLocaleString()}
              </span>
            </div>
          )}

          {event.summary && (
            <div className={cn(radius.md, "border border-info/20 bg-info/10 px-3 py-2.5")}>
              <p className="text-[12px] leading-relaxed text-info/90">{event.summary}</p>
            </div>
          )}

          {event.error && (
            <div className={cn(radius.md, "border border-danger/20 bg-danger/10 px-3 py-2.5")}>
              <p className="text-[12px] text-danger/90">{event.error}</p>
              {event.stage && (
                <p className="text-[11px] mt-1 text-danger/60">Failed at: {event.stage}</p>
              )}
            </div>
          )}

          {debugEnabled && (
            <div className="space-y-2">
              <button
                type="button"
                onClick={() => setShowDebug((value) => !value)}
                className={cn(
                  "inline-flex items-center gap-2 rounded-md border border-fg/10 bg-fg/5 px-2.5 py-1.5",
                  "text-[11px] font-medium text-fg/60 transition hover:bg-fg/8 hover:text-fg/80",
                )}
              >
                {showDebug ? "Hide Debug" : "Debug"}
              </button>
              {showDebug && (
                <pre className="max-h-96 overflow-auto rounded-md border border-fg/10 bg-black/30 p-3 text-[10px] leading-5 text-fg/75 whitespace-pre-wrap break-all">
                  {JSON.stringify(debugSteps, null, 2)}
                </pre>
              )}
            </div>
          )}

          {actions.length > 0 && (
            <div className="space-y-2">
              {actions
                .filter((a) => a.name !== "done")
                .map((action, idx) => (
                  <ActionCard key={idx} action={action} isReverted={isReverted} />
                ))}
            </div>
          )}

          <div
            className={cn(
              "flex items-center gap-3 pt-1",
              typography.caption.size,
              colors.text.disabled,
            )}
          >
            <span>
              Window {windowStart}–{windowEnd}
            </span>
            {eventTime > 0 && <span>{new Date(eventTime).toLocaleString()}</span>}
          </div>
        </div>
      )}
    </div>
  );
}

export function ToolLog({
  events,
  onRevert,
  revertingEventId,
}: {
  events: MemoryToolEvent[];
  onRevert?: (event: MemoryToolEvent) => void;
  revertingEventId?: string | null;
}) {
  const { t } = useI18n();

  if (!events.length) {
    return (
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3, ease: "easeOut" }}
        className="flex flex-col items-center justify-center py-16"
      >
        <div className="flex h-16 w-16 items-center justify-center rounded-2xl border border-fg/10 bg-fg/5 mb-4">
          <Clock className="h-7 w-7 text-fg/20" />
        </div>
        <h3 className="mb-1 text-base font-semibold text-fg">{t("common.labels.none")}</h3>
        <p className={cn("text-center text-sm max-w-[240px]", colors.text.tertiary)}>
          Tool calls appear when AI manages memories in dynamic mode
        </p>
      </motion.div>
    );
  }

  return (
    <div className={cn(spacing.item, "space-y-2")}>
      {events.map((event, idx) => (
        <CycleCard
          key={event.id ?? `event-${idx}`}
          event={event}
          defaultOpen={idx === events.length - 1}
          onRevert={onRevert}
          reverting={revertingEventId != null && event.id === revertingEventId}
        />
      ))}
    </div>
  );
}
