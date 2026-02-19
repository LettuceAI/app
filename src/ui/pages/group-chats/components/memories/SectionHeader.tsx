import type { ComponentType, ReactNode } from "react";

import { typography, colors, cn } from "../../../../design-tokens";

export function SectionHeader({
  icon: Icon,
  title,
  subtitle,
  right,
}: {
  icon?: ComponentType<{ size?: string | number; className?: string }>;
  title: string;
  subtitle?: string;
  right?: ReactNode;
}) {
  return (
    <div className="flex items-end justify-between gap-3 mb-3">
      <div className="min-w-0">
        <div className="flex items-center gap-2 min-w-0">
          {Icon ? <Icon size={16} className="text-fg/40" /> : null}
          <h2
            className={cn(
              typography.h2.size,
              typography.h2.weight,
              colors.text.primary,
              "truncate",
            )}
          >
            {title}
          </h2>
        </div>
        {subtitle ? (
          <p className={cn(typography.bodySmall.size, colors.text.tertiary, "mt-0.5 truncate")}>
            {subtitle}
          </p>
        ) : null}
      </div>
      {right ? <div className="shrink-0">{right}</div> : null}
    </div>
  );
}
