import { cn } from "@lettuceai/crisp";
import { providerLogo } from "./logos";

export interface ProviderMarkProps {
  kind: string;
  name: string;
  className?: string;
}

/** A provider's logo, or its first letter when the catalog has no logo for it. */
export function ProviderMark({ kind, name, className }: ProviderMarkProps) {
  const src = providerLogo(kind);
  return src ? (
    <img src={src} alt="" className={cn("h-5 w-5 object-contain", className)} />
  ) : (
    <span className={cn("text-sm font-semibold text-fg-2", className)}>{name.slice(0, 1)}</span>
  );
}
