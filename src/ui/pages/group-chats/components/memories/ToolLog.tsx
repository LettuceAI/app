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
} from "lucide-react";

import type { GroupSession } from "../../../../../core/storage/schemas";
import {
  components,
  colors,
  radius,
  spacing,
  typography,
  cn,
  interactive,
} from "../../../../design-tokens";

type MemoryToolEvent = NonNullable<GroupSession["memoryToolEvents"]>[number];

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
}: {
  action: NonNullable<MemoryToolEvent["actions"]>[number];
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
          <span className={cn("text-[11px] font-semibold", style.color)}>
            {style.label}
          </span>
          {category && (
            <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-fg/5 text-fg/40 border border-fg/8">
              {category.replace(/_/g, " ")}
            </span>
          )}
          {important && (
            <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-warning/20 text-warning border border-warning/30">
              pinned
            </span>
          )}
          {confidence != null && (
            <span
              className={cn(
                "text-[10px] px-1.5 py-0.5 rounded-full border",
                confidence < 0.7
                  ? "bg-warning/20 text-warning border-warning/30"
                  : "bg-danger/20 text-danger border-danger/30",
              )}
            >
              {confidence < 0.7
                ? "soft-delete"
                : `${Math.round(confidence * 100)}%`}
            </span>
          )}
        </div>
        {memoryText && (
          <p
            className={cn(
              typography.caption.size,
              colors.text.secondary,
              "mt-1 leading-relaxed",
            )}
          >
            {memoryText}
          </p>
        )}
        {id && !memoryText && (
          <p
            className={cn(
              typography.caption.size,
              colors.text.tertiary,
              "mt-1 font-mono",
            )}
          >
            #{id}
          </p>
        )}
      </div>
    </div>
  );
}

function summarizeActions(
  actions: NonNullable<MemoryToolEvent["actions"]>,
): string {
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
}: {
  event: MemoryToolEvent;
  defaultOpen: boolean;
}) {
  const [isOpen, setIsOpen] = useState(defaultOpen);
  const hasError = !!event.error;
  const actions = event.actions || [];
  const actionSummary = actions.length ? summarizeActions(actions) : null;
  const eventTime = event.createdAt || event.timestamp || 0;
  const windowStart = event.windowStart ?? 0;
  const windowEnd = event.windowEnd ?? 0;

  return (
    <div
      className={cn(
        components.card.base,
        "overflow-hidden",
        hasError && "border-danger/20",
      )}
    >
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className={cn(
          "w-full flex items-center gap-3 px-4 py-3 text-left",
          interactive.hover.brightness,
        )}
      >
        <div
          className={cn(
            "h-2 w-2 rounded-full shrink-0",
            hasError ? "bg-danger" : "bg-accent",
          )}
        />

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span
              className={cn(
                typography.caption.size,
                colors.text.secondary,
                "font-medium",
              )}
            >
              {eventTime ? relativeTime(eventTime) : "Memory Cycle"}
            </span>
            {actionSummary && (
              <span className={cn(typography.caption.size, colors.text.tertiary)}>
                — {actionSummary}
              </span>
            )}
            {hasError && (
              <AlertTriangle size={12} className="text-danger shrink-0" />
            )}
          </div>

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

      {isOpen && (
        <div className="px-4 pb-4 space-y-3">
          {event.summary && (
            <div
              className={cn(
                radius.md,
                "border border-info/20 bg-info/10 px-3 py-2.5",
              )}
            >
              <p className="text-[12px] leading-relaxed text-info/90">
                {event.summary}
              </p>
            </div>
          )}

          {event.error && (
            <div
              className={cn(
                radius.md,
                "border border-danger/20 bg-danger/10 px-3 py-2.5",
              )}
            >
              <p className="text-[12px] text-danger/90">{event.error}</p>
              {event.stage && (
                <p className="text-[11px] mt-1 text-danger/60">
                  Failed at: {event.stage}
                </p>
              )}
            </div>
          )}

          {actions.length > 0 && (
            <div className="space-y-2">
              {actions
                .filter((a) => a.name !== "done")
                .map((action, idx) => (
                  <ActionCard key={idx} action={action} />
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

export function ToolLog({ events }: { events: MemoryToolEvent[] }) {
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
        <h3 className="mb-1 text-base font-semibold text-fg">No activity yet</h3>
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
        />
      ))}
    </div>
  );
}
