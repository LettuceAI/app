import { ModelMemoryFit } from "./ModelMemoryFit";
import { ModelLayerPlacement } from "./ModelLayerPlacement";
import { useLlamaContext } from "./model-editor-runtime-data";
import { SelectMenuField } from "@/shared/ui/SelectMenuField";
import { useState } from "react";
import { Trash2 } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Button, IconButton, Input, ListGroup, ListRowButton, LoadingState, Sheet, Skeleton, Slider, Switch, Text, fileSize, icon, useCompactLayout } from "@lettuceai/crisp";
import type { SettingsModelSettingsLayer } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { EditorRow } from "@/shared/ui/EditorRow";
import { SliderValue } from "@/shared/ui/SliderValue";
import { useSettingsApi } from "./data";
import { effectiveField, readField, writeField, type ModelConfig, type ModelLora } from "@/entities/model/model-config";

export function LlamaRuntimeInfo({ path, config, defaults, onChange, gpu = false, contextEditor = false, splitOnly = false, devicesOnly = false, layersOnly = false, cpuLayers = false }: { path: string; config: ModelConfig; defaults: SettingsModelSettingsLayer | undefined; onChange: (value: ModelConfig) => void; gpu?: boolean; contextEditor?: boolean; splitOnly?: boolean; devicesOnly?: boolean; layersOnly?: boolean; cpuLayers?: boolean }) {
  const { t, i18n } = useTranslation("settings");
  const api = useSettingsApi();
  const info = useLlamaContext(path, config, defaults);
  const devices = useQuery({ queryKey: ["llama", "devices"], queryFn: () => api.call("llamaDevices"), enabled: gpu });
  const result = info.data;
  const selectedDevices = config.llama_cpp.gpu_device_ids ?? defaults?.llama_cpp.gpu_device_ids ?? result?.selected_gpu_device_ids ?? [];
  const write = (key: string, value: unknown) => onChange(writeField(config, "chat", "llama", key, value));
  const context = effectiveField(config, defaults, "chat", { store: "chat", key: "context_length" });
  const limit = result?.max_context_length;
  const requestedContext = typeof context === "number" ? context : result?.recommended_context_length ?? limit;
  const contextValue = requestedContext === undefined ? undefined : Math.max(1, Math.min(requestedContext, limit ?? requestedContext));
  const sliderMinimum = Math.min(2048, limit ?? 2048, contextValue ?? 2048);
  const sliderStep = limit !== undefined && limit < 2048 ? 1 : 2048;
  const sliderMaximum = limit === undefined ? undefined : sliderMinimum + Math.floor((limit - sliderMinimum) / sliderStep) * sliderStep;
  const sliderValue = contextValue === undefined || sliderMaximum === undefined ? undefined : Math.min(sliderMaximum, sliderMinimum + Math.round((contextValue - sliderMinimum) / sliderStep) * sliderStep);
  const contextOwn = readField(config, "chat", "chat", "context_length");
  const contextLabel = t("modelEditor.fields.contextLength");
  const cacheK = String(effectiveField(config, defaults, "chat", { store: "llama", key: "kv_type_k" }));
  const cacheV = String(effectiveField(config, defaults, "chat", { store: "llama", key: "kv_type_v" }));
  const cacheType = cacheK === cacheV ? cacheK : t("modelEditor.memoryCachePair", { keyType: cacheK, valueType: cacheV });
  const compactContext = (value: number) => value >= 1024 ? `${Math.round(value / 1024)}K` : new Intl.NumberFormat(i18n.language).format(value);
  const setContext = (value: number | undefined) => onChange(writeField(config, "chat", "chat", "context_length", value));
  if (layersOnly) {
    const field = { store: "llama", key: "gpu_layers" } as const;
    const own = readField(config, "chat", field.store, field.key);
    const value = effectiveField(config, defaults, "chat", field);
    const maximum = result?.max_gpu_layers;
    const disabled = result?.supports_gpu_offload === false || cpuLayers && maximum == null;
    const gpuCount = result?.supports_gpu_offload === false ? 0 : typeof value === "number" ? Math.min(maximum ?? value, Math.max(0, value)) : result?.estimated_placement?.total_gpu_layers;
    const count = cpuLayers ? maximum != null && gpuCount != null ? Math.max(0, maximum - gpuCount) : undefined : gpuCount;
    const setCount = (count: number) => write("gpu_layers", cpuLayers && maximum != null ? maximum - count : count);
    const label = t(cpuLayers ? "modelEditor.fields.llamaCpuLayers" : "modelEditor.fields.llamaGpuLayers");
    return <fieldset disabled={disabled} className={disabled ? "min-w-0 opacity-50" : "min-w-0"}><EditorRow label={label} overridden={own.overridden} onReset={() => write("gpu_layers", undefined)} resetLabel={t("modelEditor.resetField", { field: label })}>
      {maximum != null && count != null ? <div className="flex items-center gap-3"><Slider aria-label={label} min={0} max={maximum} step={1} value={count} onChange={setCount} className="min-w-0 flex-1" /><SliderValue label={t("modelEditor.valueLabel", { field: label })} min={0} max={maximum} step={1} value={count} text={count === maximum ? t("modelEditor.allLayers") : String(count)} overridden={own.overridden} onCommit={setCount} /></div> : <Input aria-label={label} type="number" min={0} max={maximum ?? 512} value={cpuLayers || own.value === undefined ? "" : String(own.value)} placeholder={t("modelEditor.automatic")} onChange={(event) => event.target.value === "" ? write("gpu_layers", undefined) : setCount(Number(event.target.value))} />}
    </EditorRow></fieldset>;
  }
  if (splitOnly && result) return <ModelLayerPlacement info={result} devices={devices.data?.devices ?? []} singleDeviceId={config.llama_cpp.single_gpu_device_id ?? defaults?.llama_cpp.single_gpu_device_id} />;
  return <div aria-busy={info.isPending} className={contextEditor ? "overflow-hidden rounded-xl border border-line bg-surface-1" : "space-y-3"}>
    {contextEditor && <div className="mb-3 border-b border-line"><EditorRow label={contextLabel} hint={limit ? t("modelEditor.contextLimit", { value: compactContext(limit) }) : info.isPending ? <Skeleton className="h-3 w-40" /> : undefined} overridden={contextOwn.overridden} onReset={() => setContext(undefined)} resetLabel={t("modelEditor.resetField", { field: contextLabel })}>
      {limit && contextValue !== undefined ? <div className="flex items-center gap-3">
        <Slider aria-label={contextLabel} aria-valuetext={compactContext(contextValue)} value={sliderValue ?? contextValue} min={sliderMinimum} max={limit} step={sliderStep} onChange={setContext} className="min-w-0 flex-1" />
        <SliderValue label={t("modelEditor.valueLabel", { field: contextLabel })} value={contextValue} text={compactContext(contextValue)} overridden={contextOwn.overridden} min={1} max={limit} step={1} onCommit={setContext} />
      </div> : info.isPending ? <div className="flex h-8 items-center gap-3"><Skeleton className="h-2 min-w-0 flex-1 rounded-full" /><Skeleton className="h-4 w-14" /></div> : <Input aria-label={contextLabel} type="number" min={1} max={4294967295} step={1} value={typeof context === "number" ? String(context) : ""} onChange={(event) => { if (event.target.value === "") setContext(undefined); else if (event.target.validity.valid) setContext(Number(event.target.value)); }} />}
    </EditorRow></div>}
    {info.isPending && !devicesOnly && <RuntimeMemorySkeleton contextEditor={contextEditor} splitOnly={splitOnly} />}
    {info.isError && <FailureState error={info.error} onRetry={() => void info.refetch()} size="sm" />}
    {!devicesOnly && result && <div className={`space-y-3 rounded-xl bg-fill px-4 py-3 ${contextEditor ? "mx-4 mb-3" : ""}`}>
      {!splitOnly && result.memory_fit?.map((fit, index) => <ModelMemoryFit key={index} fit={fit} cacheType={cacheType} />)}
      {!contextEditor && !splitOnly && !result.memory_fit?.length && <h3 className="text-sm font-medium">{t("modelEditor.memory")}</h3>}
      {!splitOnly && !result.memory_fit?.length && result.model_size_bytes !== null && <p className="text-xs text-fg-3">{t("modelEditor.modelBytes", { size: fileSize(result.model_size_bytes, i18n.language) })}</p>}
      {!splitOnly && !result.memory_fit?.length && result.available_vram_bytes !== null && <p className="text-xs text-fg-3">{t("modelEditor.vramBytes", { size: fileSize(result.available_vram_bytes, i18n.language) })}</p>}
      {!splitOnly && !result.memory_fit?.length && result.available_memory_bytes !== null && <p className="text-xs text-fg-3">{t("modelEditor.ramBytes", { size: fileSize(result.available_memory_bytes, i18n.language) })}</p>}
      {!splitOnly && result.recommended_context_length !== null && <div className="flex flex-wrap min-h-8 items-center justify-between gap-2 border-t border-line pt-3">
        <p className="text-xs text-fg-3">{contextEditor ? <><span className="text-fg-2">{t("modelEditor.recommendedCompact", { value: compactContext(result.recommended_context_length) })}</span> {t(result.memory_fit?.some((fit) => fit.gpu && fit.context_bytes > 0) ? "modelEditor.recommendedGpuHint" : "modelEditor.recommendedRamHint")}</> : t("modelEditor.recommended", { count: result.recommended_context_length })}</p>
        {contextValue !== result.recommended_context_length && <Button size="sm" variant="secondary" onClick={() => setContext(result.recommended_context_length ?? undefined)}>{contextEditor ? t("modelEditor.useContext", { value: compactContext(result.recommended_context_length) }) : t("modelEditor.useRecommended")}</Button>}
      </div>}
      {gpu && <ModelLayerPlacement info={result} devices={devices.data?.devices ?? []} singleDeviceId={config.llama_cpp.single_gpu_device_id ?? defaults?.llama_cpp.single_gpu_device_id} />}
    </div>}
    {gpu && !splitOnly && <>
      {devices.isError && <FailureState error={devices.error} onRetry={() => void devices.refetch()} size="sm" />}
      <div className="py-2">{devices.data?.devices.map((device) => <div key={device.index}>
        <EditorRow label={device.name} hint={`${fileSize(device.memory_free, i18n.language)} / ${fileSize(device.memory_total, i18n.language)} · ${device.backend}`} inline>
          <Switch disabled={result?.supports_gpu_offload === false} aria-label={device.name} checked={selectedDevices.includes(device.index)} onChange={(checked) => write("gpu_device_ids", checked ? [...selectedDevices.filter((id) => id !== device.index), device.index] : selectedDevices.filter((id) => id !== device.index))} />
        </EditorRow>
        {effectiveField(config, defaults, "chat", { store: "llama", key: "gpu_distribution_mode" }) === "manual" && <EditorRow label={t("modelEditor.layerAssignment", { name: device.name })}>
          <Input disabled={result?.supports_gpu_offload === false} type="number" min={0} max={512} step={1} aria-label={t("modelEditor.layerAssignment", { name: device.name })} value={config.llama_cpp.gpu_manual_layers?.find((assignment) => assignment.device_id === device.index)?.layers ?? ""} onChange={(event) => {
            const rest = (config.llama_cpp.gpu_manual_layers ?? []).filter((assignment) => assignment.device_id !== device.index);
            write("gpu_manual_layers", event.target.value === "" ? rest : [...rest, { device_id: device.index, layers: Number(event.target.value) }]);
          }} />
        </EditorRow>}
      </div>)}</div>
    </>}
  </div>;
}

function RuntimeMemorySkeleton({ contextEditor, splitOnly }: { contextEditor: boolean; splitOnly: boolean }) {
  const { t } = useTranslation("settings");
  return <div role="status" aria-label={t("modelEditor.loadingMemory")} className={`space-y-3 rounded-xl bg-fill px-4 py-3 ${contextEditor ? "mx-4 mb-3" : ""}`}>
    <span className="sr-only">{t("modelEditor.loadingMemory")}</span>
    {splitOnly ? <Skeleton shape="block" className="h-8 w-full" /> : <>
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1 space-y-2"><Skeleton className="h-3 w-40 max-w-full" /><Skeleton className="h-6 w-20" /></div>
        <Skeleton className="h-3 w-28 shrink-0" />
      </div>
      <Skeleton className="h-3 w-48 max-w-full" />
      <Skeleton className="h-2 w-full rounded-full" />
      <div className="grid grid-cols-2 gap-x-4 gap-y-2 @[36rem]:grid-cols-3">
        {Array.from({ length: 5 }, (_, index) => <div key={index} className="flex items-start gap-1.5"><Skeleton shape="circle" className="mt-1 h-2 w-2 shrink-0" /><div className="min-w-0 space-y-1.5"><Skeleton className="h-3 w-20 max-w-full" /><Skeleton className="h-3 w-12" /></div></div>)}
      </div>
      <div className="flex min-h-8 items-center border-t border-line pt-3"><Skeleton className="h-3 w-3/4" /></div>
    </>}
  </div>;
}

export function OpenRouterEndpoints({ externalId, config, defaults, onChange }: { externalId: string; config: ModelConfig; defaults: SettingsModelSettingsLayer | undefined; onChange: (value: ModelConfig) => void }) {
  const { t, i18n } = useTranslation("settings");
  const api = useSettingsApi();
  const [sort, setSort] = useState("price");
  const [choosing, setChoosing] = useState(false);
  const query = useQuery({ queryKey: ["openrouter", "endpoints", externalId], queryFn: () => api.call("providerOpenrouterEndpoints", { model: externalId }), enabled: Boolean(externalId) });
  const inheritedPin = defaults?.chat_parameters.openrouter?.pinned_provider;
  const pinned = config.chat_parameters.openrouter?.pinned_provider ?? inheritedPin;
  const rows = [...(query.data ?? [])].sort((a, b) => sort === "price" ? Number(a.prompt_price) + Number(a.completion_price) - Number(b.prompt_price) - Number(b.completion_price) : sort === "uptime" ? (b.uptime_last_30m ?? -1) - (a.uptime_last_30m ?? -1) : sort === "cache" ? Number(b.supports_prompt_caching) - Number(a.supports_prompt_caching) : a.name.localeCompare(b.name));
  const pinMode = Boolean(pinned) || choosing;
  return <div className="space-y-6">
    <ListGroup>{([false, true] as const).map((pin) => <button key={String(pin)} type="button" disabled={!pin && Boolean(inheritedPin)} title={!pin && inheritedPin ? t("modelEditor.globalPin", { name: inheritedPin }) : undefined} aria-pressed={pinMode === pin} onClick={() => { setChoosing(pin); if (!pin) onChange(writeField(config, "chat", "openrouter", "pinned_provider", undefined)); }} className="flex w-full items-center gap-3 px-4 py-3 text-left disabled:opacity-50">
      <span aria-hidden="true" className={`flex h-5 w-5 shrink-0 items-center justify-center rounded-full border-2 ${pinMode === pin ? "border-accent" : "border-fg-4"}`}>{pinMode === pin && <span className="h-2.5 w-2.5 rounded-full bg-accent" />}</span>
      <span className="min-w-0 flex-1"><span className="block text-sm font-medium">{t(pin ? "modelEditor.pinRouting" : "modelEditor.automaticRouting")}</span><span className="block text-xs text-fg-3">{t(pin ? "modelEditor.pinRoutingHint" : "modelEditor.automaticRoutingHint")}</span></span>
    </button>)}</ListGroup>
    {pinMode && <section className="space-y-2">
    <div className="flex items-center justify-between gap-3"><h3 className="text-2xs font-medium uppercase tracking-[0.18em] text-fg-3">{t("modelEditor.endpoints")}</h3><div className="w-36"><SelectMenuField aria-label={t("modelEditor.sort")} value={sort} onChange={(value) => { if (value) setSort(value); }} options={["price", "uptime", "cache", "providerName"].map((value) => ({ value, label: t(`modelEditor.${value}` as never) }))} /></div></div>
    {query.isFetching && <LoadingState />}{query.isError && <FailureState error={query.error} onRetry={() => void query.refetch()} />}
    <div className="overflow-hidden rounded-xl border border-line bg-surface-1">
      <div className="hidden grid-cols-[1fr_5rem_5rem_4rem_4.5rem] gap-3 border-b border-line px-4 py-2 text-xs text-fg-3 @[36rem]:grid">
        {["endpointProvider", "inputPrice", "outputPrice", "context", "endpointUptimeLabel"].map((key, index) => <span key={key} className={index ? "text-right" : undefined}>{t(`modelEditor.${key}` as never)}</span>)}
      </div>
      <div className="divide-y divide-line">{rows.map((endpoint) => {
        const price = (value: string | null) => value !== null && value.trim() !== "" && Number.isFinite(Number(value)) ? new Intl.NumberFormat(i18n.language, { style: "currency", currency: "USD", maximumFractionDigits: 4 }).format(Number(value) * 1_000_000) : t("modelEditor.unavailable");
        const input = price(endpoint.prompt_price); const output = price(endpoint.completion_price);
        const context = endpoint.context_length === null ? t("modelEditor.unavailable") : new Intl.NumberFormat(i18n.language, { notation: "compact" }).format(endpoint.context_length);
        const uptime = endpoint.uptime_last_30m === null ? t("modelEditor.unavailable") : t("modelEditor.percent", { value: endpoint.uptime_last_30m });
        return <button key={endpoint.id} type="button" aria-pressed={pinned === endpoint.id} onClick={() => onChange(writeField(config, "chat", "openrouter", "pinned_provider", endpoint.id))} className={`grid w-full grid-cols-1 items-center gap-3 px-4 py-3 text-left @[36rem]:grid-cols-[1fr_5rem_5rem_4rem_4.5rem] ${pinned === endpoint.id ? "bg-accent/10" : "hover:bg-fill"}`}>
          <span className="flex min-w-0 items-center gap-3">
            <span aria-hidden="true" className={`flex h-4 w-4 shrink-0 items-center justify-center rounded-full border-2 ${pinned === endpoint.id ? "border-accent" : "border-fg-4"}`}>{pinned === endpoint.id && <span className="h-2 w-2 rounded-full bg-accent" />}</span>
            <span className="min-w-0"><span className="block truncate text-sm font-medium">{endpoint.name}</span>
              <span className="block text-xs text-fg-3">{t(endpoint.supports_prompt_caching ? "modelEditor.cachingSupported" : "modelEditor.cachingUnsupported")}</span>
              <span className="block text-xs text-fg-3">{[endpoint.cache_read_price !== null ? t("modelEditor.cacheReadPrice", { price: price(endpoint.cache_read_price) }) : null, endpoint.cache_write_price !== null ? t("modelEditor.cacheWritePrice", { price: price(endpoint.cache_write_price) }) : null].filter(Boolean).join(" · ")}</span>
              <span className="mt-0.5 block text-xs tabular-nums text-fg-3 @[36rem]:hidden">{t("modelEditor.endpointSummary", { input, output, contextSize: context, uptime })}</span>
            </span>
          </span>
          {[input, output, context, uptime].map((value, index) => <span key={index} className="hidden text-right text-sm tabular-nums text-fg-2 @[36rem]:block">{value}</span>)}
        </button>;
      })}</div>
    </div>
    <p className="px-1 text-xs text-fg-3">{t("modelEditor.uptimeHint")}</p>
    </section>}
  </div>;
}

export function ModelLoras({ config, onChange }: { config: ModelConfig; onChange: (value: ModelConfig) => void }) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const list = useQuery({ queryKey: ["loras", config.stable_diffusion.cpp?.profile_id ?? null], queryFn: () => api.call("lorasList", { profile_id: config.stable_diffusion.cpp?.profile_id ?? null }) });
  const loras = config.stable_diffusion.base_loras ?? [];
  const phone = useCompactLayout();
  const [picker, setPicker] = useState(false);
  const set = (values: ModelLora[]) => onChange(writeField(config, "chat", "sd", "base_loras", values));
  return <div className="space-y-3">
    {list.isError && <FailureState error={list.error} onRetry={() => void list.refetch()} />}
    <div className="space-y-3">{loras.map((lora, index) => {
      const name = lora.path.split(/[\\/]/).at(-1) ?? lora.path;
      const strength = lora.multiplier ?? 1;
      const change = (multiplier: number) => set(loras.map((item, i) => i === index ? { ...item, multiplier } : item));
      return <div key={lora.path} className="flex flex-wrap items-center gap-4">
        <div className="min-w-0 flex-1 basis-32"><p className="truncate font-mono text-xs text-fg">{name}</p>{lora.keywords.length > 0 && <p className="text-xs text-fg-3">{t("modelEditor.loraAdds", { keywords: lora.keywords.join(", ") })}</p>}</div>
        <Slider aria-label={t("modelEditor.strength", { name })} value={strength} min={0} max={2} step={0.05} onChange={change} className="w-32" />
        <SliderValue value={strength} text={strength.toFixed(2)} overridden min={0} max={2} step={0.05} label={t("modelEditor.valueLabel", { field: t("modelEditor.strength", { name }) })} onCommit={change} />
        <IconButton size="sm" label={t("modelEditor.removeLora", { name })} onClick={() => set(loras.filter((_, i) => i !== index))}><Trash2 size={icon.sm} /></IconButton>
      </div>;
    })}</div>
    {loras.length === 0 && <Text variant="hint">{t("modelEditor.noLoras")}</Text>}
    <Button variant="secondary" size="sm" onClick={() => setPicker(true)}>{t("modelEditor.addLora")}</Button>
    <Sheet open={picker} onClose={() => setPicker(false)} placement={phone ? "bottom" : "center"} size="md" title={t("modelEditor.addLora")}>
      {list.isPending && <LoadingState />}
      <ListGroup>{(list.data?.loras ?? []).filter((lora) => !loras.some((item) => item.path === lora.path)).map((lora) => <ListRowButton key={lora.path} title={lora.filename} description={lora.keywords.join(", ")} onClick={() => { set([...loras, { path: lora.path, multiplier: 1, keywords: lora.keywords, is_high_noise: false }]); setPicker(false); }} />)}</ListGroup>
    </Sheet>
  </div>;
}
