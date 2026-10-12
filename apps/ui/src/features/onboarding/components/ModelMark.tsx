import { Globe } from "lucide-react";
import { icon } from "@lettuceai/crisp";
import { providerLogo, vendorOf } from "@/entities/provider";

export function ModelMark({ modelId, providerKind }: { modelId: string; providerKind: string | null }) {
  const src = vendorOf(modelId)?.logo ?? (providerKind ? providerLogo(providerKind) : null);
  return (
    <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-fill-2">
      {src ? <img src={src} alt="" className="h-5 w-5 object-contain" /> : <Globe size={icon.md} className="text-fg-3" />}
    </span>
  );
}
