import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import type { LlamaContextInfoRequest, LlamaSettingsDraft, SettingsModelSettingsLayer } from "@/api/generated/bindings";
import { useSettingsApi } from "./data";
import { effectiveField, resolvedKvSettings, type ModelConfig } from "@/entities/model/model-config";

function llamaDraft(config: ModelConfig, defaults: SettingsModelSettingsLayer | undefined): LlamaSettingsDraft {
  const s = { ...defaults?.llama_cpp, ...Object.fromEntries(Object.entries(config.llama_cpp).filter(([, value]) => value != null)) } as ModelConfig["llama_cpp"];
  Object.assign(s, resolvedKvSettings(config, defaults));
  return {
    offload_kqv: s.offload_kqv ?? null, kv_type: s.kv_type ?? null, kv_type_k: s.kv_type_k ?? null, kv_type_v: s.kv_type_v ?? null,
    gpu_layers: s.gpu_layers ?? null, multi_gpu_enabled: s.multi_gpu_enabled ?? null, gpu_device_ids: s.gpu_device_ids ?? null,
    gpu_distribution: s.gpu_distribution_mode ?? null, gpu_manual_layers: s.gpu_manual_layers ?? null, single_gpu_device_id: s.single_gpu_device_id ?? null,
    kv_placement: s.kv_placement ?? null, priority_vram_limit_bytes: s.priority_vram_limit_bytes ?? null,
    mmproj: s.mmproj_path ? { uri: s.mmproj_path } : null, mtp_enabled: s.mtp_enabled ?? null, mtp_placement: s.mtp_placement ?? null,
    mtp_model: s.mtp_model_path ? { uri: s.mtp_model_path } : null, dflash_enabled: s.dflash_enabled ?? null, dflash_model: s.dflash_model_path ? { uri: s.dflash_model_path } : null,
  };
}

export function useLlamaContext(path: string, config: ModelConfig, defaults: SettingsModelSettingsLayer | undefined, enabled = true) {
  const api = useSettingsApi();
  const settings = llamaDraft(config, defaults);
  const request = {
    target: { type: "draft" as const, model: { uri: path }, settings },
    context_length: effectiveField(config, defaults, "chat", { store: "chat", key: "context_length" }) as number | undefined ?? null,
    batch_size: Math.min(config.llama_cpp.ubatch_size ?? defaults?.llama_cpp.ubatch_size ?? config.llama_cpp.batch_size ?? defaults?.llama_cpp.batch_size ?? 512, config.llama_cpp.batch_size ?? defaults?.llama_cpp.batch_size ?? 512),
    flash_attention: config.llama_cpp.flash_attention ?? defaults?.llama_cpp.flash_attention ?? null,
    swa_full: config.llama_cpp.swa_full ?? defaults?.llama_cpp.swa_full ?? null,
  };
  const signature = JSON.stringify(request);
  const currentRequest = useMemo(() => JSON.parse(signature) as LlamaContextInfoRequest, [signature]);
  const [settledRequest, setSettledRequest] = useState(currentRequest);
  useEffect(() => {
    const timer = setTimeout(() => setSettledRequest(currentRequest), 120);
    return () => clearTimeout(timer);
  }, [currentRequest]);
  const settledPath = settledRequest.target.type === "draft" ? settledRequest.target.model.uri : null;
  const activeRequest = settledPath === path ? settledRequest : currentRequest;
  return useQuery({ queryKey: ["llama", "context", path, activeRequest], queryFn: () => api.call("llamaContextInfo", activeRequest), enabled: enabled && Boolean(path), placeholderData: (data, previous) => previous?.queryKey[2] === path ? keepPreviousData(data) : undefined });
}
