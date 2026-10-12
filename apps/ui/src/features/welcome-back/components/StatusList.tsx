import type { ReactNode } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Check } from "lucide-react";
import { cn } from "@lettuceai/crisp";

export type RowStatus = "waiting" | "active" | "done" | { progress: number };

export interface StatusRow {
  id: string;
  title: ReactNode;
  detail?: ReactNode;
  status: RowStatus;
  trailing?: ReactNode;
  completion?: string;
}

const spinner = 2 * Math.PI * 9;
const pop = { type: "spring", stiffness: 500, damping: 26 } as const;

function Indicator({ status }: { status: RowStatus }) {
  return (
    <AnimatePresence mode="wait" initial={false}>
      {status === "done" ? (
        <motion.span key="done" initial={{ scale: 0.5, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} transition={pop} className="flex h-5 w-5 items-center justify-center rounded-full bg-white text-black">
          <Check size={12} strokeWidth={3} />
        </motion.span>
      ) : status === "active" ? (
        <motion.svg key="active" viewBox="0 0 24 24" className="h-5 w-5 motion-safe:animate-spin motion-reduce:animate-pulse">
          <circle cx="12" cy="12" r="9" fill="none" stroke="rgb(255 255 255 / 0.15)" strokeWidth="2.5" />
          <circle cx="12" cy="12" r="9" fill="none" stroke="#fff" strokeWidth="2.5" strokeLinecap="round" strokeDasharray={`${spinner * 0.28} ${spinner}`} />
        </motion.svg>
      ) : typeof status === "object" ? (
        <motion.svg key="progress" viewBox="0 0 24 24" className="h-5 w-5 -rotate-90" exit={{ opacity: 0 }}>
          <circle cx="12" cy="12" r="9" fill="none" stroke="rgb(255 255 255 / 0.15)" strokeWidth="2.5" />
          <circle cx="12" cy="12" r="9" fill="none" stroke="#fff" strokeWidth="2.5" strokeLinecap="round" strokeDasharray={spinner} strokeDashoffset={spinner * (1 - status.progress)} />
        </motion.svg>
      ) : (
        <motion.span key="waiting" className="h-1.5 w-1.5 rounded-full bg-white/25" />
      )}
    </AnimatePresence>
  );
}

/** A frosted list of steps, each with its own state on the right. */
export function StatusList({ rows }: { rows: readonly StatusRow[] }) {
  return (
    <div className="w-full max-w-md overflow-hidden rounded-2xl border border-white/10 bg-black/30 text-left backdrop-blur-md">
      {rows.map((row, index) => (
        <div key={row.id} className={cn("flex items-center gap-3 px-4 py-3", index > 0 && "border-t border-white/8")}>
          <span className="min-w-0 flex-1">
            <span className={cn("block text-base transition-colors motion-slow", row.status === "active" ? "font-medium text-fg" : row.status === "waiting" ? "text-fg-3" : "text-fg")}>{row.title}</span>
            {row.detail && <span className="block text-sm tabular-nums text-fg-3">{row.detail}</span>}
          </span>
          <span className="flex h-6 shrink-0 items-center justify-end gap-3">
            {row.completion !== undefined && <span className="min-w-10 text-right text-sm tabular-nums text-fg-3">{row.completion}</span>}
            <span className="flex w-6 items-center justify-end">{row.trailing ?? <Indicator status={row.status} />}</span>
          </span>
        </div>
      ))}
    </div>
  );
}
