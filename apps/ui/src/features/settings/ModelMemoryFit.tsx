import { useTranslation } from "react-i18next";
import type { LlamaMemoryFit } from "@/api/generated/bindings";

export function ModelMemoryFit({ fit, cacheType }: { fit: LlamaMemoryFit; cacheType: string }) {
  const { t, i18n } = useTranslation("settings");
  const modelParts = [
    { label: t("modelEditor.memoryParts.model"), bytes: fit.model_bytes, color: "bg-accent", detail: undefined },
    { label: t("modelEditor.memoryParts.context"), bytes: fit.context_bytes, color: "bg-info", detail: t("modelEditor.memoryCacheType", { type: cacheType }) },
    { label: t("modelEditor.memoryParts.working"), bytes: fit.working_bytes, color: "bg-fg-3", detail: undefined },
  ];
  const needed = modelParts.reduce((sum, part) => sum + part.bytes, 0);
  const other = Math.max(0, fit.capacity_bytes - fit.available_bytes);
  const remaining = fit.available_bytes - needed;
  const scale = Math.max(fit.capacity_bytes, needed + other, 1);
  const parts = [{ label: t("modelEditor.memoryAlreadyUsed"), bytes: other, color: "bg-fg-4", detail: undefined }, ...modelParts];
  const size = (value: number) => t("modelEditor.memoryGigabytes", { value: new Intl.NumberFormat(i18n.language, { minimumFractionDigits: 1, maximumFractionDigits: 1 }).format(value / 1_000_000_000) });
  return <div className="space-y-3" aria-label={t("modelEditor.memoryFit")}>
    <div className="flex flex-wrap items-start justify-between gap-3">
      <div role="group" aria-label={t("modelEditor.memoryModelEstimate")}>
        <p className="text-xs text-fg-3">{t("modelEditor.memoryModelEstimate")}</p>
        <p className="mt-1 text-lg font-medium tabular-nums text-fg">{size(needed)}</p>
      </div>
      <p className={`pt-0.5 text-xs tabular-nums ${remaining < 0 ? "text-warning" : "text-accent"}`}>{t(remaining < 0 ? "modelEditor.memoryShortfall" : "modelEditor.memoryAfterLoading", { size: size(Math.abs(remaining)) })}</p>
    </div>
    <p className="text-xs text-fg-3">{t("modelEditor.memoryDeviceTotal", { device: fit.gpu ? fit.devices.join(", ") : t("modelEditor.systemMemory"), size: size(fit.capacity_bytes) })}</p>
    <div className="relative flex h-2 overflow-hidden rounded-full bg-fill-2" role="img" aria-label={t("modelEditor.memoryBreakdown", { alreadyUsed: size(other), model: size(fit.model_bytes), contextSize: size(fit.context_bytes), working: size(fit.working_bytes), left: size(Math.max(0, remaining)) })}>
      {parts.map((part) => <div key={part.label} className={part.color} style={{ width: `${part.bytes / scale * 100}%` }} />)}
      {remaining < 0 && <div className="absolute inset-y-0 w-px bg-fg" style={{ left: `${fit.capacity_bytes / scale * 100}%` }} />}
    </div>
    <div className="grid grid-cols-2 gap-x-4 gap-y-2 text-xs @[36rem]:grid-cols-3">
      {parts.map((part) => <div key={part.label} className="flex items-start gap-1.5"><span className={`mt-1 h-2 w-2 shrink-0 rounded-full ${part.color}`} /><span className="min-w-0"><span className="block text-fg-3">{part.label}</span><span className="block tabular-nums text-fg-2">{size(part.bytes)}</span>{part.detail && <span className="block text-fg-3">{part.detail}</span>}</span></div>)}
      <div className="flex items-start gap-1.5"><span className="mt-1 h-2 w-2 shrink-0 rounded-full border border-fg-4 bg-fill-2" /><span><span className="block text-fg-3">{t("modelEditor.memoryLeft")}</span><span className="block tabular-nums text-fg-2">{size(Math.max(0, remaining))}</span></span></div>
    </div>
  </div>;
}
