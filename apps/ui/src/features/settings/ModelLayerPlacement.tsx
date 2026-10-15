import { useTranslation } from "react-i18next";
import type { LlamaContextInfo, LlamaDevice } from "@/api/generated/bindings";

export function ModelLayerPlacement({ info, devices, singleDeviceId }: { info: LlamaContextInfo; devices: LlamaDevice[]; singleDeviceId: number | null | undefined }) {
  const { t } = useTranslation("settings");
  const placement = info.estimated_placement;
  const total = info.max_gpu_layers ?? 0;
  if (!placement || total <= 0) return null;
  const cpuLayers = Math.max(0, total - placement.total_gpu_layers);
  const gpuName = (index: number) => {
    if (info.selected_gpu_device_ids && info.selected_gpu_device_ids.length >= 2) {
      const deviceId = info.selected_gpu_device_ids[index];
      return devices.find((device) => device.index === deviceId)?.name ?? t("modelEditor.gpuDevice", { index: deviceId });
    }
    return info.memory_fit?.find((fit) => fit.gpu)?.devices[0] ?? devices.find((device) => device.index === singleDeviceId)?.name ?? (devices.length === 1 ? devices[0]?.name : undefined) ?? t("modelEditor.kvGpu");
  };
  const parts = placement.per_device_layers.map((layers, index) => ({ name: gpuName(index), layers, gpu: true }));
  if (cpuLayers > 0) parts.push({ name: t("modelEditor.groups.cpu"), layers: cpuLayers, gpu: false });
  return <div className="space-y-2">
    <div role="img" aria-label={t("modelEditor.gpuLayerDistribution")} className="flex h-8 overflow-hidden rounded-lg text-xs font-medium">
      {parts.filter((part) => part.layers > 0).map((part, index) => <div key={index} title={t("modelEditor.layerDevice", { device: part.name, layers: t("modelEditor.layers", { count: part.layers }) })} className={`flex min-w-0 items-center overflow-hidden border-r border-line last:border-r-0 ${part.gpu ? "bg-accent/80 text-on-accent" : "bg-fill-2 text-fg-2"}`} style={{ width: `${part.layers / total * 100}%` }}>
        <span className="truncate px-2.5">{t("modelEditor.layerDevice", { device: part.name, layers: t("modelEditor.layers", { count: part.layers }) })}</span>
      </div>)}
    </div>
    <p className="text-xs text-fg-3">{placement.total_gpu_layers === 0 ? t("modelEditor.layersCpuOnly") : cpuLayers === 0 ? t("modelEditor.layersGpuOnly") : t("modelEditor.layersGpuCpu", { gpu: t("modelEditor.layers", { count: placement.total_gpu_layers }), cpu: t("modelEditor.layers", { count: cpuLayers }) })}</p>
  </div>;
}
