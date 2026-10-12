import type { ReactNode } from "react";
import { ArrowRight, Check, Loader2 } from "lucide-react";
import { Button, cn, icon } from "@lettuceai/crisp";
import { useOnboarding } from "../flow/context";

export function Rows({ children }: { children: ReactNode }) {
  return <div className="divide-y divide-line border-y border-line">{children}</div>;
}

export interface RowProps {
  selected?: boolean;
  onClick?: () => void;
  leading?: ReactNode;
  title: ReactNode;
  description?: ReactNode;
  trailing?: ReactNode;
  indicator?: "check" | "arrow" | "none";
  disabled?: boolean;
}

export function Row({ selected, onClick, leading, title, description, trailing, indicator = "check", disabled }: RowProps) {
  return (
    <div className={cn(selected && "bg-fill")}>
      <button
        type="button"
        aria-pressed={indicator === "check" ? selected : undefined}
        disabled={disabled ?? false}
        onClick={onClick}
        className="flex w-full items-center gap-4 px-2 py-4 text-left transition-colors hover:bg-fill disabled:opacity-60 @xl:px-3"
      >
        {leading && (
          <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-fill-2 text-fg-2">{leading}</span>
        )}
        <span className="min-w-0 flex-1">
          <span className="block text-lg font-medium text-fg">{title}</span>
          {description && <span className="mt-0.5 block text-base leading-snug text-fg-3">{description}</span>}
        </span>
        {trailing}
        {indicator === "arrow" && <ArrowRight size={icon.lg} className="shrink-0 text-fg-3" />}
        {indicator === "check" && (
          <span
            className={cn(
              "flex h-6 w-6 shrink-0 items-center justify-center rounded-full border transition-colors",
              selected ? "border-accent bg-accent text-on-accent" : "border-line-3",
            )}
          >
            {selected && <Check size={icon.sm} strokeWidth={3} />}
          </span>
        )}
      </button>
    </div>
  );
}

export interface ActionButtonProps {
  label: string;
  disabled?: boolean;
  loading?: boolean;
  block?: boolean;
  onClick: () => void;
}

export function ActionButton({ label, disabled, loading, block, onClick }: ActionButtonProps) {
  return (
    <Button
      variant="primary"
      size="lg"
      {...(block ? { block } : {})}
      disabled={disabled ?? false}
      aria-busy={loading || undefined}
      className="disabled:bg-primary-disabled disabled:text-white/55 disabled:opacity-100"
      leading={loading ? <Loader2 size={icon.md} className="animate-spin" /> : undefined}
      trailing={loading ? undefined : <ArrowRight size={icon.md} />}
      onClick={onClick}
    >
      {label}
    </Button>
  );
}

export interface ScreenTitleProps {
  title: string;
  lead?: string;
  kicker?: string;
}

export function ScreenTitle({ title, lead, kicker }: ScreenTitleProps) {
  const { phone } = useOnboarding();
  return (
    <div className="mb-8">
      {kicker && <p className="mb-3 text-sm font-medium text-accent">{kicker}</p>}
      <h1 className={cn("font-bold tracking-[-0.02em] text-fg", phone ? "text-3xl" : "text-4xl")}>{title}</h1>
      {lead && <p className="mt-3 max-w-[58ch] text-lg leading-relaxed text-fg-2">{lead}</p>}
    </div>
  );
}
