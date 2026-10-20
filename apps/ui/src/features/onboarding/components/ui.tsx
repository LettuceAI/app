import { cn } from "@lettuceai/crisp";
import { useOnboarding } from "../flow/context";

export { Row, Rows, type RowProps } from "@/shared/ui/SelectionRows";

export { ActionButton, type ActionButtonProps } from "@/shared/ui/ActionButton";

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
