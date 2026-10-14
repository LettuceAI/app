import { useRef, useState } from "react";
import { cn } from "@lettuceai/crisp";

export function SliderValue({ value, text, overridden, min, max, step, label, onCommit }: {
  value: number | undefined; text: string; overridden: boolean; min: number; max: number; step: number; label: string; onCommit: (value: number) => void;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const cancelled = useRef(false);
  const edited = useRef(false);
  const commit = () => {
    const n = Number(draft?.replace(",", "."));
    if (edited.current && !cancelled.current && draft !== null && draft.trim() !== "" && Number.isFinite(n)) {
      const rounded = min + Math.round((n - min) / step) * step;
      onCommit(Number(Math.min(max, Math.max(min, rounded)).toFixed(8)));
    }
    cancelled.current = false;
    setDraft(null);
  };
  return <input aria-label={label} inputMode="decimal" value={draft ?? text}
    onFocus={(event) => { cancelled.current = false; edited.current = false; setDraft(value === undefined ? "" : String(value)); event.currentTarget.select(); }}
    onChange={(event) => { edited.current = true; setDraft(event.target.value); }} onBlur={commit}
    onKeyDown={(event) => {
      if (event.key === "Enter") { event.preventDefault(); event.currentTarget.blur(); }
      if (event.key === "Escape") { event.preventDefault(); cancelled.current = true; setDraft(null); event.currentTarget.blur(); }
    }}
    className={cn("h-8 rounded-md border border-transparent bg-transparent px-1.5 text-right text-sm tabular-nums outline-none hover:border-line hover:bg-fill focus:border-accent focus:bg-surface-1 focus:text-fg", value === undefined ? "w-24" : "w-14", overridden ? "text-fg" : "text-fg-3")} />;
}
