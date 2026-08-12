import { Reorder, useDragControls } from "framer-motion";
import { Check, GripVertical, Plus, RotateCcw, Save, Trash2, X } from "lucide-react";
import {
  type FormEvent,
  useCallback,
  useEffect,
  useMemo,
  useState,
} from "react";

import {
  DEFAULT_LLAMA_SAMPLER_ORDER,
  LLAMA_SAMPLER_ORDER_PRESETS,
  LLAMA_SAMPLER_ORDER_STAGE_VALUES,
  normalizeLlamaSamplerOrder,
  type LlamaSamplerOrderPreset,
  type LlamaSamplerOrderStage,
  type LlamaSamplerPreset,
} from "../../core/storage/schemas";
import {
  readSettings,
  saveAdvancedSettings,
  SETTINGS_UPDATED_EVENT,
} from "../../core/storage/repo";
import { cn } from "../design-tokens";
import { useI18n, type TranslationKey } from "../../core/i18n/context";
import { toast } from "./toast";

const STAGE_META: Record<
  LlamaSamplerOrderStage,
  { labelKey: TranslationKey; descriptionKey: TranslationKey }
> = {
  penalties: {
    labelKey: "samplerOrder.stages.penalties.label",
    descriptionKey: "samplerOrder.stages.penalties.desc",
  },
  grammar: {
    labelKey: "samplerOrder.stages.grammar.label",
    descriptionKey: "samplerOrder.stages.grammar.desc",
  },
  top_k: {
    labelKey: "samplerOrder.stages.topK.label",
    descriptionKey: "samplerOrder.stages.topK.desc",
  },
  top_p: {
    labelKey: "samplerOrder.stages.topP.label",
    descriptionKey: "samplerOrder.stages.topP.desc",
  },
  min_p: {
    labelKey: "samplerOrder.stages.minP.label",
    descriptionKey: "samplerOrder.stages.minP.desc",
  },
  dry: {
    labelKey: "samplerOrder.stages.dry.label",
    descriptionKey: "samplerOrder.stages.dry.description",
  },
  typical: {
    labelKey: "samplerOrder.stages.typical.label",
    descriptionKey: "samplerOrder.stages.typical.desc",
  },
  xtc: {
    labelKey: "samplerOrder.stages.xtc.label",
    descriptionKey: "samplerOrder.stages.xtc.desc",
  },
  temp: {
    labelKey: "samplerOrder.stages.temp.label",
    descriptionKey: "samplerOrder.stages.temp.desc",
  },
};

const PRESET_META: Record<
  LlamaSamplerOrderPreset,
  { labelKey: TranslationKey; hintKey: TranslationKey }
> = {
  default: {
    labelKey: "samplerOrder.presets.default.label",
    hintKey: "samplerOrder.presets.default.hint",
  },
  unsloth: {
    labelKey: "samplerOrder.presets.unsloth.label",
    hintKey: "samplerOrder.presets.unsloth.hint",
  },
  focused: {
    labelKey: "samplerOrder.presets.focused.label",
    hintKey: "samplerOrder.presets.focused.hint",
  },
  creative: {
    labelKey: "samplerOrder.presets.creative.label",
    hintKey: "samplerOrder.presets.creative.hint",
  },
};

function ordersEqual(left: readonly string[], right: readonly string[]) {
  return left.length === right.length && left.every((stage, index) => stage === right[index]);
}

export function LlamaSamplerOrderEditor({
  value,
  onChange,
  className,
}: {
  value: LlamaSamplerOrderStage[] | null | undefined;
  onChange: (value: LlamaSamplerOrderStage[]) => void;
  className?: string;
}) {
  const { t } = useI18n();
  const order = normalizeLlamaSamplerOrder(value) ?? [...DEFAULT_LLAMA_SAMPLER_ORDER];
  const [customPresets, setCustomPresets] = useState<LlamaSamplerPreset[]>([]);
  const [newPresetName, setNewPresetName] = useState("");
  const [isNamingPreset, setIsNamingPreset] = useState(false);
  const [savingPresets, setSavingPresets] = useState(false);

  const loadCustomPresets = useCallback(async () => {
    try {
      const settings = await readSettings();
      setCustomPresets(settings.advancedSettings?.llamaSamplerPresets ?? []);
    } catch {
      setCustomPresets([]);
    }
  }, []);

  useEffect(() => {
    void loadCustomPresets();
    window.addEventListener(SETTINGS_UPDATED_EVENT, loadCustomPresets);
    return () => window.removeEventListener(SETTINGS_UPDATED_EVENT, loadCustomPresets);
  }, [loadCustomPresets]);

  const activeCustomPreset = useMemo(
    () => customPresets.find((preset) => ordersEqual(order, preset.stages)) ?? null,
    [customPresets, order],
  );
  const activeBuiltInPreset = useMemo(
    () =>
      ((Object.entries(LLAMA_SAMPLER_ORDER_PRESETS).find(([, presetOrder]) =>
        ordersEqual(order, presetOrder),
      )?.[0] as LlamaSamplerOrderPreset | undefined) ?? null),
    [order],
  );
  const selectedPreset = activeCustomPreset
    ? `custom:${activeCustomPreset.id}`
    : activeBuiltInPreset
      ? `built-in:${activeBuiltInPreset}`
      : "custom-chain";
  const isDefault = activeBuiltInPreset === "default" && !activeCustomPreset;
  const availableStages = LLAMA_SAMPLER_ORDER_STAGE_VALUES.filter(
    (stage) => !order.includes(stage),
  );

  const persistCustomPresets = async (next: LlamaSamplerPreset[]) => {
    setSavingPresets(true);
    try {
      const settings = await readSettings();
      await saveAdvancedSettings({
        ...(settings.advancedSettings ?? {}),
        llamaSamplerPresets: next,
      });
      setCustomPresets(next);
    } finally {
      setSavingPresets(false);
    }
  };

  const handlePresetChange = (next: string) => {
    if (next.startsWith("built-in:")) {
      const preset = next.slice("built-in:".length) as LlamaSamplerOrderPreset;
      onChange([...LLAMA_SAMPLER_ORDER_PRESETS[preset]]);
      return;
    }
    if (next.startsWith("custom:")) {
      const preset = customPresets.find((item) => item.id === next.slice("custom:".length));
      if (preset) onChange([...preset.stages]);
    }
  };

  const handleSavePreset = async (event: FormEvent) => {
    event.preventDefault();
    const name = newPresetName.trim();
    if (!name || savingPresets) return;

    const existing = customPresets.find(
      (preset) => preset.name.trim().toLocaleLowerCase() === name.toLocaleLowerCase(),
    );
    const next = existing
      ? customPresets.map((preset) =>
          preset.id === existing.id ? { ...preset, name, stages: [...order] } : preset,
        )
      : [
          ...customPresets,
          {
            id: crypto.randomUUID(),
            name,
            stages: [...order],
          },
        ];

    try {
      await persistCustomPresets(next);
      setNewPresetName("");
      setIsNamingPreset(false);
      toast.success(t("samplerOrder.presetSaved"));
    } catch (error) {
      toast.error(
        t("samplerOrder.presetSaveFailed"),
        error instanceof Error ? error.message : String(error),
      );
    }
  };

  const handleDeletePreset = async () => {
    if (!activeCustomPreset || savingPresets) return;
    try {
      await persistCustomPresets(
        customPresets.filter((preset) => preset.id !== activeCustomPreset.id),
      );
      toast.success(t("samplerOrder.presetDeleted"));
    } catch (error) {
      toast.error(
        t("samplerOrder.presetDeleteFailed"),
        error instanceof Error ? error.message : String(error),
      );
    }
  };

  return (
    <div className={cn("space-y-3", className)}>
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 space-y-0.5">
          <span className="block text-[13px] font-medium text-fg/70">
            {t("samplerOrder.title")}
          </span>
          <span className="block text-[13px] text-fg/40">{t("samplerOrder.description")}</span>
        </div>
        <button
          type="button"
          onClick={() => onChange([...DEFAULT_LLAMA_SAMPLER_ORDER])}
          disabled={isDefault}
          className={cn(
            "inline-flex shrink-0 items-center gap-1.5 rounded-md border px-2.5 py-1.5 text-[11px] font-medium transition-colors",
            isDefault
              ? "cursor-default border-fg/5 bg-transparent text-fg/25"
              : "border-fg/10 bg-fg/5 text-fg/65 hover:border-fg/20 hover:bg-fg/8 hover:text-fg",
          )}
          aria-label={t("samplerOrder.resetAria")}
        >
          <RotateCcw size={12} />
          {t("samplerOrder.reset")}
        </button>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <select
          value={selectedPreset}
          onChange={(event) => handlePresetChange(event.target.value)}
          className="min-w-48 flex-1 rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-[12px] text-fg outline-none transition-colors focus:border-fg/25"
          aria-label={t("samplerOrder.preset")}
        >
          {!activeBuiltInPreset && !activeCustomPreset && (
            <option value="custom-chain">{t("samplerOrder.customChain")}</option>
          )}
          <optgroup label={t("samplerOrder.builtInPresets")}>
            {(Object.keys(LLAMA_SAMPLER_ORDER_PRESETS) as LlamaSamplerOrderPreset[]).map(
              (preset) => (
                <option key={preset} value={`built-in:${preset}`}>
                  {t(PRESET_META[preset].labelKey)} ({t(PRESET_META[preset].hintKey)})
                </option>
              ),
            )}
          </optgroup>
          {customPresets.length > 0 && (
            <optgroup label={t("samplerOrder.customPresets")}>
              {customPresets.map((preset) => (
                <option key={preset.id} value={`custom:${preset.id}`}>
                  {preset.name}
                </option>
              ))}
            </optgroup>
          )}
        </select>
        {!isNamingPreset && (
          <button
            type="button"
            onClick={() => {
              setNewPresetName(activeCustomPreset?.name ?? "");
              setIsNamingPreset(true);
            }}
            className="inline-flex items-center gap-1.5 rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-[12px] font-medium text-fg/70 transition-colors hover:border-fg/20 hover:bg-fg/8 hover:text-fg"
          >
            <Save size={13} />
            {t("samplerOrder.savePreset")}
          </button>
        )}
        {activeCustomPreset && (
          <button
            type="button"
            onClick={() => void handleDeletePreset()}
            disabled={savingPresets}
            className="flex h-8 w-8 items-center justify-center rounded-lg border border-fg/10 text-fg/45 transition-colors hover:border-danger/30 hover:bg-danger/10 hover:text-danger disabled:opacity-40"
            aria-label={t("samplerOrder.deletePreset", { name: activeCustomPreset.name })}
          >
            <Trash2 size={13} />
          </button>
        )}
      </div>

      {isNamingPreset && (
        <form
          onSubmit={(event) => void handleSavePreset(event)}
          className="flex items-center gap-2 border-l-2 border-accent/50 pl-3"
        >
          <input
            autoFocus
            value={newPresetName}
            onChange={(event) => setNewPresetName(event.target.value)}
            maxLength={64}
            placeholder={t("samplerOrder.presetNamePlaceholder")}
            className="min-w-0 flex-1 rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-[12px] text-fg outline-none placeholder:text-fg/30 focus:border-fg/25"
          />
          <button
            type="submit"
            disabled={!newPresetName.trim() || savingPresets}
            className="flex h-8 w-8 items-center justify-center rounded-lg bg-accent text-black transition-colors hover:bg-accent/90 disabled:cursor-not-allowed disabled:opacity-40"
            aria-label={t("common.buttons.save")}
          >
            <Check size={14} />
          </button>
          <button
            type="button"
            onClick={() => {
              setIsNamingPreset(false);
              setNewPresetName("");
            }}
            className="flex h-8 w-8 items-center justify-center rounded-lg border border-fg/10 text-fg/50 transition-colors hover:bg-fg/8 hover:text-fg"
            aria-label={t("common.buttons.cancel")}
          >
            <X size={14} />
          </button>
        </form>
      )}

      <div className="overflow-hidden rounded-xl border border-fg/10 bg-fg/[0.03]">
        <div className="flex items-center justify-between border-b border-fg/8 px-3 py-2 text-[11px] text-fg/45">
          <span>{t("samplerOrder.activeStages")}</span>
          <span className="font-mono tabular-nums">
            {order.length} / {LLAMA_SAMPLER_ORDER_STAGE_VALUES.length}
          </span>
        </div>
        {order.length > 0 ? (
          <Reorder.Group
            axis="y"
            values={order}
            onReorder={(nextOrder) => onChange(nextOrder)}
            className="divide-y divide-fg/8"
          >
            {order.map((stage, index) => (
              <SamplerOrderItem
                key={stage}
                stage={stage}
                index={index}
                onRemove={() => onChange(order.filter((item) => item !== stage))}
              />
            ))}
          </Reorder.Group>
        ) : (
          <div className="px-4 py-5 text-center text-[12px] text-fg/40">
            {t("samplerOrder.emptyChain")}
          </div>
        )}
      </div>

      {availableStages.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="text-[11px] text-fg/45">{t("samplerOrder.addSampler")}</span>
          {availableStages.map((stage) => (
            <button
              key={stage}
              type="button"
              onClick={() => onChange([...order, stage])}
              className="inline-flex items-center gap-1 rounded-full border border-dashed border-fg/15 px-2.5 py-1 text-[11px] font-medium text-fg/60 transition-colors hover:border-fg/30 hover:bg-fg/5 hover:text-fg"
            >
              <Plus size={11} />
              {t(STAGE_META[stage].labelKey)}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function SamplerOrderItem({
  stage,
  index,
  onRemove,
}: {
  stage: LlamaSamplerOrderStage;
  index: number;
  onRemove: () => void;
}) {
  const { t } = useI18n();
  const controls = useDragControls();
  const meta = STAGE_META[stage];
  const label = t(meta.labelKey);
  const description = t(meta.descriptionKey);

  return (
    <Reorder.Item
      value={stage}
      dragListener={false}
      dragControls={controls}
      dragMomentum={false}
      dragElastic={0}
      layout="position"
      whileDrag={{
        zIndex: 20,
        boxShadow: "0 8px 20px rgba(0,0,0,0.2), 0 0 0 1px rgba(255,255,255,0.06)",
      }}
      transition={{ layout: { duration: 0.16, ease: "easeOut" } }}
      className="flex items-center gap-3 bg-transparent px-3 py-2.5"
      style={{ position: "relative" }}
    >
      <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full border border-fg/10 bg-fg/5 text-[10px] font-semibold tabular-nums text-fg/55">
        {index + 1}
      </div>
      <div className="min-w-0 flex-1">
        <div className="text-[13px] font-medium text-fg">{label}</div>
        <div className="text-[11px] leading-relaxed text-fg/45">{description}</div>
      </div>
      <button
        type="button"
        onClick={onRemove}
        className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-fg/35 transition-colors hover:bg-danger/10 hover:text-danger"
        aria-label={t("samplerOrder.removeSampler", { label })}
      >
        <X size={13} />
      </button>
      <button
        type="button"
        onPointerDown={(event) => controls.start(event)}
        className="flex h-8 w-8 shrink-0 cursor-grab items-center justify-center rounded-md text-fg/40 transition-colors hover:bg-fg/8 hover:text-fg/80 active:cursor-grabbing"
        aria-label={t("samplerOrder.dragToReorder", { label })}
      >
        <GripVertical size={14} />
      </button>
    </Reorder.Item>
  );
}
