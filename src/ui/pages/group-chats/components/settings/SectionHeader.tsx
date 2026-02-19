import { typography, cn } from "../../../../design-tokens";

export function SectionHeader({ title, subtitle }: { title: string; subtitle?: string }) {
  return (
    <div className="flex items-end justify-between gap-3 mb-3">
      <div className="min-w-0">
        <h2 className={cn(typography.h2.size, typography.h2.weight, "text-fg truncate")}>{title}</h2>
        {subtitle ? (
          <p className={cn(typography.bodySmall.size, "text-fg/50 mt-0.5 truncate")}>{subtitle}</p>
        ) : null}
      </div>
    </div>
  );
}
