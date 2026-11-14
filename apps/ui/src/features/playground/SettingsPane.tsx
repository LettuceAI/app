import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Dices } from "lucide-react";
import { icon, IconButton, Input, ListGroup, ListRowButton, Skeleton, Slider, Switch } from "@lettuceai/crisp";
import type { ImageCapabilities } from "@/api/generated/bindings";
import type { ModelChoice } from "@/entities/model";
import { editorFields } from "@/entities/model/model-editor-fields";
import { ModelEditorReveal } from "@/entities/model/ModelEditorReveal";
import { ModelMark } from "@/entities/provider";
import { EditorRow } from "@/shared/ui/EditorRow";
import { FailureState } from "@/shared/ui/FailureState";
import { SelectMenuField } from "@/shared/ui/SelectMenuField";
import { SliderValue } from "@/shared/ui/SliderValue";
import { LoraSection } from "./LoraSection";
import type { PlaygroundDraft } from "./playground-data";

interface Range {
  min: number;
  max: number;
  step: number;
}

const customSize = "custom";

function randomSeed(): number {
  return (crypto.getRandomValues(new Uint32Array(1))[0] ?? 0) % (editorFields.sdSeed.max + 1);
}

function formatted(value: number, step: number): string {
  return step >= 1 ? String(value) : value.toFixed(step === 0.5 ? 1 : 2);
}

export function SettingsPane({ choice, loading, failure, capabilities, draft, base, update, hiresUpscalers, onPickModel }: {
  choice: ModelChoice | null;
  loading: boolean;
  failure: { error: unknown; retry: () => void } | null;
  capabilities: ImageCapabilities | undefined;
  draft: PlaygroundDraft;
  base: PlaygroundDraft;
  update: (patch: Partial<PlaygroundDraft>) => void;
  hiresUpscalers: readonly string[];
  onPickModel: () => void;
}) {
  const { t } = useTranslation("playground");
  const { t: ts } = useTranslation("settings");
  const [customizing, setCustomizing] = useState(false);
  const defaultOption = t("settings.defaultOption");
  const reset = (label: string) => ts("modelEditor.resetField", { field: label });

  const changed = <K extends keyof PlaygroundDraft>(key: K) => draft[key] !== base[key];
  const resetTo = <K extends keyof PlaygroundDraft>(key: K) => () => update({ [key]: base[key] } as Partial<PlaygroundDraft>);

  const slider = <K extends "steps" | "cfg_scale" | "hires_scale" | "hires_steps" | "hires_denoising_strength" | "count">(key: K, label: string, range: Range, fallback = defaultOption) => {
    const value = draft[key];
    return (
      <EditorRow label={label} overridden={changed(key)} onReset={resetTo(key)} resetLabel={reset(label)}>
        <div className="flex items-center gap-3">
          <Slider
            aria-label={label}
            value={value ?? range.min}
            min={range.min}
            max={range.max}
            step={range.step}
            onChange={(next) => update({ [key]: next } as Partial<PlaygroundDraft>)}
            className="min-w-0 flex-1"
          />
          <SliderValue
            value={value ?? undefined}
            text={value === null ? fallback : formatted(value, range.step)}
            overridden={value !== null}
            min={range.min}
            max={range.max}
            step={range.step}
            label={ts("modelEditor.valueLabel", { field: label })}
            onCommit={(next) => update({ [key]: next } as Partial<PlaygroundDraft>)}
          />
        </div>
      </EditorRow>
    );
  };

  const select = <K extends "sampler" | "scheduler" | "quality" | "style" | "hires_upscaler">(key: K, label: string, values: readonly string[], empty = defaultOption, disabled = false) => {
    const value = draft[key];
    const options = value && !values.includes(value) ? [...values, value] : values;
    return (
      <EditorRow label={label} compact overridden={changed(key)} onReset={resetTo(key)} resetLabel={reset(label)}>
        <SelectMenuField
          aria-label={label}
          disabled={disabled}
          value={value ?? ""}
          options={[{ value: "", label: empty }, ...options.map((option) => ({ value: option, label: option }))]}
          onChange={(next) => update({ [key]: next || null } as Partial<PlaygroundDraft>)}
        />
      </EditorRow>
    );
  };

  const sizes = capabilities?.sizes ?? [];
  const custom = customizing || (draft.size !== null && !sizes.includes(draft.size));
  const sizeLabel = t("settings.size");

  return (
    <div className="@container space-y-6">
      <ListGroup label={t("settings.model")}>
        <ListRowButton
          icon={choice ? <ModelMark modelId={choice.model.external_model_id} providerKind={choice.account.provider_kind} name={choice.model.display_name} /> : undefined}
          title={choice?.model.display_name ?? t("settings.noModel")}
          description={choice?.account.label}
          disabled={loading}
          onClick={onPickModel}
        />
      </ListGroup>

      {choice && failure && <FailureState size="sm" error={failure.error} onRetry={failure.retry} />}
      {choice && !failure && !capabilities && (
        <div role="status" aria-label={t("settings.generation")} className="space-y-3">
          <Skeleton shape="block" className="h-40 w-full" />
          <Skeleton shape="block" className="h-24 w-full" />
        </div>
      )}

      {choice && capabilities && (
        <>
          <ListGroup label={t("settings.generation")}>
            <EditorRow label={sizeLabel} compact overridden={changed("size")} onReset={() => { setCustomizing(false); update({ size: base.size }); }} resetLabel={reset(sizeLabel)}>
              <SelectMenuField
                aria-label={sizeLabel}
                value={custom ? customSize : draft.size ?? ""}
                options={[
                  { value: "", label: capabilities.default_size ? `${defaultOption} (${capabilities.default_size})` : defaultOption },
                  ...sizes.map((size) => ({ value: size, label: size })),
                  { value: customSize, label: t("settings.customSize") },
                ]}
                onChange={(next) => {
                  setCustomizing(next === customSize);
                  if (next !== customSize) update({ size: next || null });
                }}
              />
            </EditorRow>
            <ModelEditorReveal open={custom}>
              <div className="px-4 py-3">
                <Input
                  aria-label={t("settings.customSize")}
                  value={custom ? draft.size ?? "" : ""}
                  placeholder={t("settings.sizePlaceholder")}
                  className="font-mono"
                  onChange={(event) => update({ size: event.target.value.trim() || null })}
                />
              </div>
            </ModelEditorReveal>
            {capabilities.local && (
              <>
                {slider("steps", t("settings.steps"), editorFields.sdSteps)}
                {slider("cfg_scale", t("settings.cfg"), editorFields.sdCfgScale)}
                {select("sampler", t("settings.sampler"), capabilities.samplers)}
                {select("scheduler", t("settings.scheduler"), capabilities.schedulers)}
                <EditorRow label={t("settings.seed")} compact overridden={draft.seed !== null} onReset={() => update({ seed: null })} resetLabel={reset(t("settings.seed"))}>
                  <div className="flex items-center gap-1.5">
                    <Input
                      aria-label={t("settings.seed")}
                      type="number"
                      min={editorFields.sdSeed.min}
                      max={editorFields.sdSeed.max}
                      step={1}
                      value={draft.seed === null ? "" : String(draft.seed)}
                      placeholder={t("settings.seedRandom")}
                      className="min-w-0 flex-1 [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
                      onChange={(event) => {
                        const raw = event.target.value;
                        const seed = Number(raw);
                        if (raw === "") update({ seed: null });
                        else if (Number.isInteger(seed)) update({ seed: Math.min(editorFields.sdSeed.max, Math.max(editorFields.sdSeed.min, seed)) });
                      }}
                    />
                    <IconButton variant="secondary" label={t("settings.rollSeed")} onClick={() => update({ seed: randomSeed() })}>
                      <Dices size={icon.md} />
                    </IconButton>
                  </div>
                </EditorRow>
              </>
            )}
            {capabilities.max_count > 1 && slider("count", t("settings.batch"), { min: 1, max: capabilities.max_count, step: 1 }, "1")}
            {capabilities.quality.length > 0 && select("quality", t("settings.quality"), capabilities.quality)}
            {capabilities.styles.length > 0 && select("style", t("settings.style"), capabilities.styles)}
          </ListGroup>

          {capabilities.local && (
            <ListGroup label={t("settings.hires")}>
              <EditorRow label={t("settings.hires")} inline overridden={changed("hires_enabled")} onReset={resetTo("hires_enabled")} resetLabel={reset(t("settings.hires"))}>
                <Switch aria-label={t("settings.hires")} checked={draft.hires_enabled} onChange={(hires_enabled) => update({ hires_enabled })} />
              </EditorRow>
              <ModelEditorReveal open={draft.hires_enabled}>
                {select("hires_upscaler", t("settings.hiresUpscaler"), hiresUpscalers, hiresUpscalers.length === 0 ? t("settings.hiresNone") : defaultOption, hiresUpscalers.length === 0)}
                {slider("hires_scale", t("settings.hiresScale"), editorFields.sdHiresScale)}
                {slider("hires_steps", t("settings.hiresSteps"), editorFields.sdHiresSteps)}
                {slider("hires_denoising_strength", t("settings.hiresDenoising"), editorFields.sdHiresDenoisingStrength)}
              </ModelEditorReveal>
            </ListGroup>
          )}

          {capabilities.local && <LoraSection choice={choice} draft={draft} update={update} />}
        </>
      )}
    </div>
  );
}
