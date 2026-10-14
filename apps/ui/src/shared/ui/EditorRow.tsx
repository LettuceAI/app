import type { ReactNode } from "react";
import { RotateCcw } from "lucide-react";
import { cn } from "@lettuceai/crisp";

export function EditorRow({ label, hint, children, overridden, onReset, resetLabel, stack, inline }: {
  label: string; hint?: ReactNode; children: ReactNode; overridden?: boolean; onReset?: () => void; resetLabel?: string; stack?: boolean; inline?: boolean;
}) {
  return <div className={cn("flex gap-x-6 gap-y-2.5 px-4 py-3", inline ? "items-center" : stack ? "flex-col" : "flex-col @[36rem]:flex-row @[36rem]:items-center")}>
    <div className="min-w-0 flex-1">
      <div className="flex items-center gap-1.5">
        <span className="text-sm font-medium text-fg">{label}</span>
        {overridden && onReset && <button type="button" onClick={onReset} aria-label={resetLabel} title={resetLabel} className="rounded p-0.5 text-accent hover:bg-fill focus-visible:outline-2 focus-visible:outline-accent"><RotateCcw size={12} /></button>}
      </div>
      {hint && <div className="mt-0.5 text-xs text-fg-3">{hint}</div>}
    </div>
    <div className={cn("min-w-0 shrink-0", inline ? undefined : stack ? "w-full" : "w-full @[36rem]:w-64")}>{children}</div>
  </div>;
}
