import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { typography, radius, interactive, cn } from "../../../design-tokens";

export function FieldLabel({ children }: { children: ReactNode }) {
  const { label } = typography;
  return (
    <label className={cn(label.size, label.weight, label.tracking, "uppercase text-fg/70")}>
      {children}
    </label>
  );
}

interface OptionRowProps {
  selected: boolean;
  onSelect: () => void;
  icon?: LucideIcon;
  label: string;
  description: string;
  disabled?: boolean;
}

export function OptionRow({
  selected,
  onSelect,
  icon: Icon,
  label,
  description,
  disabled = false,
}: OptionRowProps) {
  return (
    <button
      onClick={onSelect}
      disabled={disabled}
      aria-pressed={selected}
      className={cn(
        "flex w-full items-center gap-3 p-3 text-left",
        radius.lg,
        "border",
        interactive.transition.fast,
        selected
          ? "border-accent/40 bg-accent/10"
          : "border-fg/10 bg-surface-el/85 hover:border-fg/20",
        "disabled:cursor-not-allowed disabled:opacity-50",
      )}
    >
      {Icon && (
        <Icon className={cn("h-5 w-5 shrink-0", selected ? "text-accent/80" : "text-fg/50")} />
      )}
      <div className="min-w-0 flex-1">
        <div className={cn("text-sm font-semibold", selected ? "text-accent" : "text-fg/80")}>
          {label}
        </div>
        <div className="mt-0.5 text-xs text-fg/40">{description}</div>
      </div>
      <span
        className={cn(
          "flex h-4 w-4 shrink-0 items-center justify-center rounded-full border",
          interactive.transition.fast,
          selected ? "border-accent" : "border-fg/25",
        )}
      >
        {selected && <span className="h-2 w-2 rounded-full bg-accent" />}
      </span>
    </button>
  );
}
