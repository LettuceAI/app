import { ArrowRight, Loader2 } from "lucide-react";
import { Button, cn, icon } from "@lettuceai/crisp";
import { useOnboarding } from "../flow/context";

export { Row, Rows, type RowProps } from "@/shared/ui/SelectionRows";

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
