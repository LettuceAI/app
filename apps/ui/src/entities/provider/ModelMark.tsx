import { cn } from "@lettuceai/crisp";
import { providerLogo } from "./logos";
import { vendorOf } from "./vendors";

export interface ModelMarkProps {
  modelId: string;
  providerKind: string;
  name: string;
  className?: string;
}

/** The logo of the company behind a model, else its provider's, else the name's first letter. */
export function ModelMark({ modelId, providerKind, name, className }: ModelMarkProps) {
  const src = vendorOf(modelId)?.logo ?? providerLogo(providerKind);
  return src ? (
    <img src={src} alt="" className={cn("h-5 w-5 object-contain", className)} />
  ) : (
    <span className={cn("text-sm font-semibold text-fg-2", className)}>{name.slice(0, 1)}</span>
  );
}
