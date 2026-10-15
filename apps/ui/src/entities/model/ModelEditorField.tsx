import { SelectMenuField } from "@/shared/ui/SelectMenuField";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { FileUp, RotateCcw } from "lucide-react";
import { Button, Input, Slider, Switch, icon } from "@lettuceai/crisp";
import { EditorRow } from "@/shared/ui/EditorRow";
import { SliderValue } from "@/shared/ui/SliderValue";
import type { SettingsModelSettingsLayer } from "@/api/generated/bindings";
import { effectiveField, readField, writeField, type Layer, type ModelConfig } from "./model-config";
import { SamplerOrder } from "./SamplerOrder";
import { ModelEditorReveal } from "./ModelEditorReveal";
import { formatSequences, parseSequences } from "./model-editor-sequences";
import type { EditorKind } from "./model-editor-sections";
import { editorFields, type EditorField, type FieldId } from "./model-editor-fields";

export function ModelEditorField({ id, config, defaults, layer, onChange, onFile, onInvalid, options, kind, runtimeDefault, compact = false, disabled = false, disabledOptions = [], loading = false, multiGpuAvailable = true }: {
  kind: EditorKind; runtimeDefault: unknown; compact?: boolean;
  id: FieldId; config: ModelConfig; defaults: SettingsModelSettingsLayer | undefined; layer: Layer;
  options?: { value: string; label: string; disabled?: boolean }[] | undefined;
  disabled?: boolean; disabledOptions?: readonly string[]; loading?: boolean; multiGpuAvailable?: boolean;
  onChange: (config: ModelConfig) => void; onFile: (id: FieldId) => void; onInvalid: (key: string, invalid: boolean) => void;
}) {
  const { t, i18n } = useTranslation("settings");
  const [resetVersion, setResetVersion] = useState(0);
  const spec: EditorField = editorFields[id];
  const multiGpu = multiGpuAvailable && effectiveField(config, defaults, layer, editorFields.llamaMultiGpuEnabled) === true;
  const own = id === "llamaKvType" && (config.llama_cpp.kv_type_k != null || config.llama_cpp.kv_type_v != null) ? { value: "split", overridden: true } : readField(config, layer, spec.store, spec.key);
  const value = id === "llamaSingleGpuDeviceId" && disabled && options?.length === 1 ? options[0]?.value : effectiveField(config, defaults, layer, spec) ?? runtimeDefault ?? (id === "frequencyPenalty" || id === "presencePenalty" ? 0 : id === "temperature" ? 0.8 : id === "topP" ? 1 : undefined);
  const placeholderKey = `modelEditor.placeholders.${id}`;
  const emptyLabel = i18n.exists(placeholderKey, { ns: "settings" }) ? t(placeholderKey as never) : t(id === "topK" ? "modelEditor.off" : id === "llamaSeed" || id === "sdSeed" || id === "ollamaSeed" ? "modelEditor.random" : kind === "cloud" || kind === "ollama" ? "modelEditor.providerDefault" : "modelEditor.automatic");
  const label = t(`modelEditor.fields.${id}`);
  const scale = id === "llamaPriorityVramLimitBytes" ? 1024 ** 3 : 1;
  const unit = id === "llamaPriorityVramLimitBytes" ? "gib" : ["maxOutputTokens", "contextLength", "llamaNPenRange", "llamaDryAllowedLength", "llamaDryPenaltyLastN", "llamaBatchSize", "llamaUbatchSize", "ollamaNumCtx", "reasoningBudgetTokens"].includes(id) ? "tokens" : id === "llamaCpuLayers" ? "layers" : ["sdHiresWidth", "sdHiresHeight", "sdVaeTileSizeX", "sdVaeTileSizeY"].includes(id) ? "pixels" : undefined;
  const set = (next: unknown) => { if (!disabled) onChange(writeField(config, layer, spec.store, spec.key, next, defaults)); };
  const fallback = value === undefined ? emptyLabel : typeof value === "boolean" ? t(value ? "modelEditor.on" : "modelEditor.off") : String(value);
  const hintKey = `modelEditor.hints.${id}`;
  const autoKvOffload = id === "llamaKvPlacement" && (value === undefined || value === "auto") ? effectiveField(config, defaults, layer, { store: "llama", key: "offload_kqv" }) : undefined;
  const hint = id === "llamaKvPlacement" && disabledOptions.includes("pin") ? t("modelEditor.gpuUnavailable") : typeof autoKvOffload === "boolean" ? t(autoKvOffload ? "modelEditor.automaticKvGpu" : "modelEditor.automaticKvRam") : i18n.exists(hintKey, { ns: "settings" }) ? t(hintKey as never) : undefined;
  const optionLabel = (option: string) => t(`modelEditor.options.${id}.${option}` as never);
  if (spec.type === "caching") {
    const enabled = value !== null && typeof value === "object" && "enabled" in value;
    const retention = enabled ? (value as { enabled: { retention: string } }).enabled.retention : "five_minutes";
    return <>
      <EditorRow compact={compact} label={label} hint={hint} inline overridden={own.overridden} onReset={() => set(undefined)} resetLabel={t("modelEditor.resetField", { field: label })}>
        <Switch aria-label={label} checked={enabled} onChange={(checked) => set(checked ? { enabled: { retention } } : "disabled")} />
      </EditorRow>
      <ModelEditorReveal open={enabled}><EditorRow compact={compact} label={t("modelEditor.keepFor")}>
        <SelectMenuField aria-label={t("modelEditor.keepFor")} value={retention} options={(spec.options ?? []).filter((option) => option && option !== "disabled").map((option) => ({ value: option, label: optionLabel(option) }))} onChange={(next) => { if (next) set({ enabled: { retention: next } }); }} />
      </EditorRow></ModelEditorReveal>
    </>;
  }
  let control;
  if (options) {
    const current = value === undefined ? "" : String(value);
    const choices = options.some((option) => option.value === current) || !current ? options : [...options, { value: current, label: current }];
    control = <SelectMenuField disabled={disabled} placeholder={emptyLabel} aria-label={label} value={current} options={[{ value: "", label: emptyLabel }, ...choices]} onChange={(value) => set(!value ? undefined : spec.type === "number" ? Number(value) : value)} />;
  } else if (spec.type === "slider") {
    const number = typeof value === "number" ? value : undefined;
    control = <div className="flex items-center gap-3">
      <Slider value={number ?? spec.min ?? 0} onChange={set} min={spec.min ?? 0} max={spec.max ?? 1} step={spec.step ?? 1} aria-label={label} aria-valuetext={number === undefined ? fallback : undefined} className="min-w-0 flex-1" />
      <SliderValue value={number} text={number === undefined ? fallback : spec.step !== undefined && spec.step < 1 ? number.toFixed(spec.step === 0.1 || spec.step === 0.5 ? 1 : 2) : String(number)} overridden={own.overridden} min={spec.min ?? 0} max={spec.max ?? 1} step={spec.step ?? 1} label={t("modelEditor.valueLabel", { field: label })} onCommit={set} />
    </div>;
  } else if (spec.type === "number") {
    const numberText = typeof value === "number" ? `${new Intl.NumberFormat(i18n.language).format(value / scale)}${unit ? ` ${t(`modelEditor.units.${unit}`)}` : ""}` : fallback;
    control = <Input aria-label={label} type="number" step={scale === 1 ? spec.step : "any"} min={spec.min === undefined ? undefined : spec.min / scale} max={scale === 1 ? spec.max : Number.MAX_SAFE_INTEGER / scale} value={typeof own.value === "number" ? String(own.value / scale) : ""} placeholder={numberText} trailing={own.overridden && unit ? <span className="text-xs text-fg-3">{t(`modelEditor.units.${unit}`)}</span> : undefined} className="[appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
      onChange={(event) => { const raw = event.target.value; const n = Number(raw); onInvalid(`${layer}.${id}`, !event.target.validity.valid); if (raw === "") set(undefined); else if (Number.isFinite(n)) set(scale === 1 ? n : Math.round(n * scale)); }} />;
  } else if (spec.type === "switch" || spec.type === "reasoning") {
    control = <div className="flex justify-end"><Switch checked={spec.type === "reasoning" ? value === "enabled" : value === true} onChange={(checked) => set(spec.type === "reasoning" ? checked ? "enabled" : "disabled" : checked)} aria-label={label} /></div>;
  } else if (spec.type === "select") {
    const values = id === "llamaKvPlacement" ? multiGpu ? ["auto", "system_ram", "pin", "split"] : ["auto", "system_ram", "pin"] : [...(spec.options ?? [])];
    const hasAuto = values.includes("auto");
    const selected = id === "llamaKvPlacement" && !multiGpu && value === "split" ? "pin" : value === undefined ? hasAuto ? "auto" : "" : String(value);
    if (id === "llamaKvType") values.push("split");
    if (selected && !values.includes(selected)) values.push(selected);
    control = <SelectMenuField disabled={disabled} placeholder={emptyLabel} aria-label={label} value={selected} options={[...(hasAuto ? [] : [{ value: "", label: emptyLabel }]), ...values.filter(Boolean).map((v) => ({ value: v, label: id === "llamaKvPlacement" && !multiGpu && v === "pin" ? t("modelEditor.kvGpu") : id === "llamaKvType" && v === "split" ? t("modelEditor.separateKv") : spec.options?.includes(v) ? optionLabel(v) : v, disabled: disabledOptions.includes(v) }))]} onChange={(value) => {
      const v = value ?? "";
      if (disabledOptions.includes(v)) return;
      set(v === "" ? undefined : id === "ollamaMirostat" ? Number(v) : v);
    }} />;
  } else if (spec.type === "file") {
    control = <Button variant="secondary" loading={loading} disabled={disabled || loading} className="w-full justify-start" leading={<FileUp size={icon.sm} />} onClick={() => onFile(id)}><span className="truncate font-mono text-xs">{typeof value === "string" ? value.split(/[\\/]/).at(-1) : i18n.exists(placeholderKey, { ns: "settings" }) ? emptyLabel : t("modelEditor.chooseFile")}</span></Button>;
  } else if (spec.type === "order") {
    return <div className="space-y-2 px-4 py-3"><div className="flex flex-wrap items-center justify-between gap-2"><span className="flex items-center gap-1.5 text-sm font-medium">{label}{own.overridden && <button type="button" className="rounded p-0.5 text-accent hover:bg-fill" aria-label={t("modelEditor.resetField", { field: label })} onClick={() => set(undefined)}><RotateCcw size={12} /></button>}</span><span className="text-xs text-fg-3">{t("modelEditor.orderHint")}</span></div><SamplerOrder value={Array.isArray(value) ? value as string[] : []} onChange={set} options={spec.options ?? []} /></div>;
  } else if (spec.type === "strings") {
    control = <StringArray key={`${resetVersion}-${layer}-${id}`} value={Array.isArray(value) ? value as string[] : undefined} label={label} placeholder={emptyLabel} onChange={set} onInvalid={(invalid) => onInvalid(`${layer}.${id}`, invalid)} />;
  } else {
    const props = { "aria-label": label, value: typeof own.value === "string" ? own.value : "", placeholder: i18n.exists(placeholderKey, { ns: "settings" }) ? emptyLabel : typeof value === "string" ? value : "", onChange: (event: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => set(event.target.value || undefined) };
    control = spec.multiline ? <textarea {...props} className="min-h-24 w-full resize-y rounded-xl border border-line bg-fill px-3.5 py-2.5 text-sm" /> : <Input {...props} />;
  }
  return <fieldset disabled={disabled} className={disabled ? "min-w-0 opacity-50" : "min-w-0"}><EditorRow compact={compact} label={label} hint={hint} overridden={own.overridden} onReset={() => { set(undefined); setResetVersion((version) => version + 1); onInvalid(`${layer}.${id}`, false); }} resetLabel={t("modelEditor.resetField", { field: label })} stack={spec.multiline || spec.type === "order"} inline={spec.type === "switch" || spec.type === "reasoning"}>{control}</EditorRow></fieldset>;
}

function StringArray({ value, label, placeholder, onChange, onInvalid }: { value: string[] | undefined; label: string; placeholder: string; onChange: (value: unknown) => void; onInvalid: (invalid: boolean) => void }) {
  const [draft, setDraft] = useState<string | null>(null);
  return <Input aria-label={label} value={draft ?? (value ? formatSequences(value) : "")} placeholder={placeholder} onChange={(event) => {
    const raw = event.target.value;
    setDraft(raw);
    if (!raw.trim()) { onInvalid(false); onChange(undefined); return; }
    try {
      const parsed = parseSequences(raw);
      onInvalid(false); onChange(parsed);
    } catch { onInvalid(true); }
  }} />;
}
