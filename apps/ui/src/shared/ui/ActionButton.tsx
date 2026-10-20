import { ArrowRight, Loader2 } from "lucide-react";
import { Button, icon } from "@lettuceai/crisp";

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
