import { useEffect, useState, type ReactNode } from "react";
import { Check, ChevronDown, Dices, SlidersHorizontal, Sparkles } from "lucide-react";

import { cn } from "../../design-tokens";
import { BottomMenu, MenuButton, MenuSection } from "../../components/BottomMenu";
import { ModelSelectionBottomMenu } from "../../components/ModelSelectionBottomMenu";
import { NumberInput } from "../../components/NumberInput";
import { Switch } from "../../components/Switch";
import {
  ADVANCED_SD_CFG_SCALE_RANGE,
  ADVANCED_SD_HIRES_DENOISING_RANGE,
  ADVANCED_SD_HIRES_SCALE_RANGE,
  ADVANCED_SD_HIRES_STEPS_RANGE,
  ADVANCED_SD_SEED_RANGE,
  ADVANCED_SD_STEPS_RANGE,
} from "../../components/AdvancedModelSettingsForm";
import { useI18n } from "../../../core/i18n/context";
import { getModelSizes, getSdcppUpscalerInventory } from "../../../core/image-generation";
import { SDCPP_SAMPLERS, SDCPP_SCHEDULERS } from "../../../core/image-generation/sdcpp-options";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import { randomPlaygroundSeed } from "../../../core/image-generation/playground";
import { PlaygroundLoraSection } from "./PlaygroundLoraSection";
import type { PlaygroundSettingsController } from "./usePlaygroundSettings";

function FieldRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="text-[12px] text-fg/55">{label}</span>
      {children}
    </div>
  );
}

function OptionPickerMenu({
  isOpen,
  onClose,
  title,
  defaultLabel,
  options,
  selected,
  onSelect,
  onClear,
}: {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  defaultLabel: string;
  options: readonly string[];
  selected: string | null;
  onSelect: (value: string) => void;
  onClear: () => void;
}) {
  return (
    <BottomMenu isOpen={isOpen} onClose={onClose} title={title}>
      <div className="max-h-[60vh] overflow-y-auto">
        <MenuSection>
          <MenuButton
            icon={Sparkles}
            title={defaultLabel}
            color="from-accent to-accent/80"
            rightElement={selected === null ? <Check className="h-4 w-4 text-accent" /> : null}
            onClick={() => {
              onClear();
              onClose();
            }}
          />
          {options.map((option) => (
            <MenuButton
              key={option}
              icon={SlidersHorizontal}
              title={option}
              color="from-white/10 to-white/5"
              rightElement={
                selected === option ? <Check className="h-4 w-4 text-accent" /> : null
              }
              onClick={() => {
                onSelect(option);
                onClose();
              }}
            />
          ))}
        </MenuSection>
      </div>
    </BottomMenu>
  );
}

const NUMBER_INPUT_CLASS =
  "h-8 w-24 rounded-lg border border-fg/10 bg-fg/5 px-2 text-center text-[12.5px] text-fg transition-all focus:border-fg/20 focus:bg-fg/[0.07] focus:outline-none";

export function PlaygroundSettingsPane({
  controller,
}: {
  controller: PlaygroundSettingsController;
}) {
  const { t } = useI18n();
  const { models, selectedModel, isLocal, selectModel, draft, updateDraft } = controller;
  const [modelMenuOpen, setModelMenuOpen] = useState(false);
  const [samplerMenuOpen, setSamplerMenuOpen] = useState(false);
  const [schedulerMenuOpen, setSchedulerMenuOpen] = useState(false);
  const [hiresUpscalers, setHiresUpscalers] = useState<string[]>([]);
  const [hiresMenuOpen, setHiresMenuOpen] = useState(false);

  useEffect(() => {
    if (!isLocal) return;
    let cancelled = false;
    getSdcppUpscalerInventory()
      .then((inventory) => {
        if (!cancelled) setHiresUpscalers(inventory.hiresUpscalerNames);
      })
      .catch(() => {
        if (!cancelled) setHiresUpscalers([]);
      });
    return () => {
      cancelled = true;
    };
  }, [isLocal]);

  const presetSizes = selectedModel
    ? getModelSizes(selectedModel.providerId, selectedModel.name)
    : [];
  const isOpenAiModel = selectedModel?.providerId === "openai";

  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto p-4">
      <div data-tour-id="playground-model">
        <p className="mb-1.5 text-[11px] font-medium uppercase tracking-wide text-fg/40">
          {t("playground.settings.model")}
        </p>
        <button
          type="button"
          onClick={() => setModelMenuOpen(true)}
          className="flex w-full items-center gap-2.5 rounded-xl border border-fg/10 bg-fg/5 px-3.5 py-3 text-left transition-all hover:border-fg/15 hover:bg-fg/[0.07] active:scale-[0.99]"
        >
          {selectedModel ? (
            <>
              <span className="flex shrink-0 items-center justify-center [&_img]:h-4 [&_img]:w-4 [&_svg]:h-4 [&_svg]:w-4">
                {getProviderIcon(selectedModel.providerId)}
              </span>
              <span className="min-w-0 flex-1 truncate text-[12.5px] font-medium text-fg/85">
                {selectedModel.displayName || selectedModel.name}
              </span>
            </>
          ) : (
            <span className="min-w-0 flex-1 truncate text-[12.5px] text-fg/45">
              {t("playground.settings.noModel")}
            </span>
          )}
          <ChevronDown size={13} className="shrink-0 text-fg/40" />
        </button>
      </div>

      {selectedModel && (
        <div className="space-y-3">
          <p className="text-[11px] font-medium uppercase tracking-wide text-fg/40">
            {t("playground.settings.generation")}
          </p>
          <div className="space-y-3 rounded-2xl border border-fg/10 bg-fg/4 p-3.5">
          <div>
            <p className="mb-1.5 text-[12px] text-fg/55">{t("playground.settings.size")}</p>
            {presetSizes.length > 0 && (
              <div className="mb-1.5 flex flex-wrap gap-1.5">
                {presetSizes.map((size) => (
                  <button
                    key={size}
                    type="button"
                    onClick={() => updateDraft({ size })}
                    className={cn(
                      "rounded-full px-2.5 py-1 text-[11px] font-medium transition",
                      draft.size === size
                        ? "bg-accent/15 text-accent ring-1 ring-inset ring-accent/30"
                        : "bg-fg/5 text-fg/55 hover:text-fg/85",
                    )}
                  >
                    {size}
                  </button>
                ))}
              </div>
            )}
            <input
              value={draft.size ?? ""}
              onChange={(event) => updateDraft({ size: event.target.value || null })}
              placeholder={t("playground.settings.sizePlaceholder")}
              className="h-8 w-full rounded-lg border border-fg/10 bg-fg/5 px-2.5 font-mono text-[12px] text-fg placeholder-fg/40 transition-all focus:border-fg/20 focus:bg-fg/[0.07] focus:outline-none"
            />
          </div>

          {isLocal && (
            <>
              <FieldRow label={t("playground.settings.steps")}>
                <NumberInput
                  min={ADVANCED_SD_STEPS_RANGE.min}
                  max={ADVANCED_SD_STEPS_RANGE.max}
                  step={1}
                  value={draft.steps ?? null}
                  onChange={(value) => updateDraft({ steps: value })}
                  className={NUMBER_INPUT_CLASS}
                />
              </FieldRow>
              <FieldRow label={t("playground.settings.cfg")}>
                <NumberInput
                  min={ADVANCED_SD_CFG_SCALE_RANGE.min}
                  max={ADVANCED_SD_CFG_SCALE_RANGE.max}
                  step={0.5}
                  value={draft.cfgScale ?? null}
                  onChange={(value) => updateDraft({ cfgScale: value })}
                  className={NUMBER_INPUT_CLASS}
                />
              </FieldRow>
              <FieldRow label={t("playground.settings.sampler")}>
                <button
                  type="button"
                  onClick={() => setSamplerMenuOpen(true)}
                  className="flex h-8 items-center gap-1.5 rounded-lg border border-fg/10 bg-fg/5 px-2.5 font-mono text-[12px] text-fg/75 transition-all hover:border-fg/15 hover:bg-fg/[0.07] active:scale-[0.98]"
                >
                  {draft.sampler || t("playground.settings.defaultOption")}
                  <ChevronDown size={12} className="text-fg/40" />
                </button>
              </FieldRow>
              <FieldRow label={t("playground.settings.scheduler")}>
                <button
                  type="button"
                  onClick={() => setSchedulerMenuOpen(true)}
                  className="flex h-8 items-center gap-1.5 rounded-lg border border-fg/10 bg-fg/5 px-2.5 font-mono text-[12px] text-fg/75 transition-all hover:border-fg/15 hover:bg-fg/[0.07] active:scale-[0.98]"
                >
                  {draft.scheduler || t("playground.settings.defaultOption")}
                  <ChevronDown size={12} className="text-fg/40" />
                </button>
              </FieldRow>
              <FieldRow label={t("playground.settings.seed")}>
                <div className="flex items-center gap-1">
                  <NumberInput
                    min={ADVANCED_SD_SEED_RANGE.min}
                    max={ADVANCED_SD_SEED_RANGE.max}
                    step={1}
                    value={draft.seed ?? null}
                    onChange={(value) => updateDraft({ seed: value })}
                    placeholder={t("playground.settings.seedRandom")}
                    className="h-8 w-28 rounded-lg border border-fg/10 bg-fg/5 px-2 text-center text-[12px] text-fg placeholder-fg/40 transition-all focus:border-fg/20 focus:bg-fg/[0.07] focus:outline-none"
                  />
                  <button
                    type="button"
                    onClick={() => updateDraft({ seed: randomPlaygroundSeed() })}
                    title={t("playground.settings.rollSeed")}
                    className="flex h-8 w-8 items-center justify-center rounded-lg border border-fg/10 bg-fg/5 text-fg/50 transition-all hover:border-fg/15 hover:bg-fg/[0.07] hover:text-fg active:scale-95"
                  >
                    <Dices size={13} />
                  </button>
                </div>
              </FieldRow>
            </>
          )}

          </div>

          {isLocal && (
            <div className="space-y-3 rounded-2xl border border-fg/10 bg-fg/4 p-3.5">
              <div className="flex items-center justify-between">
                <span className="text-[12px] font-medium text-fg/70">
                  {t("playground.settings.hires")}
                </span>
                <Switch
                  checked={draft.hiresEnabled ?? false}
                  onChange={(checked) => updateDraft({ hiresEnabled: checked })}
                />
              </div>
              {draft.hiresEnabled && (
                <>
                  <FieldRow label={t("playground.settings.hiresUpscaler")}>
                    <button
                      type="button"
                      onClick={() => setHiresMenuOpen(true)}
                      disabled={hiresUpscalers.length === 0}
                      className="flex h-8 max-w-[160px] items-center gap-1.5 rounded-lg border border-fg/10 bg-fg/5 px-2.5 font-mono text-[11.5px] text-fg/75 transition-all hover:border-fg/15 hover:bg-fg/[0.07] disabled:opacity-50"
                    >
                      <span className="truncate">
                        {draft.hiresUpscaler ||
                          (hiresUpscalers.length === 0
                            ? t("playground.settings.hiresNone")
                            : t("playground.settings.defaultOption"))}
                      </span>
                      <ChevronDown size={12} className="shrink-0 text-fg/40" />
                    </button>
                  </FieldRow>
                  <FieldRow label={t("playground.settings.hiresScale")}>
                    <NumberInput
                      min={ADVANCED_SD_HIRES_SCALE_RANGE.min}
                      max={ADVANCED_SD_HIRES_SCALE_RANGE.max}
                      step={0.25}
                      value={draft.hiresScale ?? null}
                      onChange={(value) => updateDraft({ hiresScale: value })}
                      className={NUMBER_INPUT_CLASS}
                    />
                  </FieldRow>
                  <FieldRow label={t("playground.settings.hiresSteps")}>
                    <NumberInput
                      min={ADVANCED_SD_HIRES_STEPS_RANGE.min}
                      max={ADVANCED_SD_HIRES_STEPS_RANGE.max}
                      step={1}
                      value={draft.hiresSteps ?? null}
                      onChange={(value) => updateDraft({ hiresSteps: value })}
                      className={NUMBER_INPUT_CLASS}
                    />
                  </FieldRow>
                  <FieldRow label={t("playground.settings.hiresDenoising")}>
                    <NumberInput
                      min={ADVANCED_SD_HIRES_DENOISING_RANGE.min}
                      max={ADVANCED_SD_HIRES_DENOISING_RANGE.max}
                      step={0.05}
                      value={draft.hiresDenoisingStrength ?? null}
                      onChange={(value) => updateDraft({ hiresDenoisingStrength: value })}
                      className={NUMBER_INPUT_CLASS}
                    />
                  </FieldRow>
                </>
              )}
            </div>
          )}

          {isLocal && <PlaygroundLoraSection controller={controller} />}

          <div className="space-y-3 rounded-2xl border border-fg/10 bg-fg/4 p-3.5">
          <FieldRow label={t("playground.settings.batch")}>
            <NumberInput
              min={1}
              max={8}
              step={1}
              value={draft.n ?? null}
              onChange={(value) => updateDraft({ n: value })}
              placeholder="1"
              className={NUMBER_INPUT_CLASS}
            />
          </FieldRow>

          {isOpenAiModel && (
            <>
              <FieldRow label={t("playground.settings.quality")}>
                <div className="flex gap-1.5">
                  {["standard", "hd"].map((quality) => (
                    <button
                      key={quality}
                      type="button"
                      onClick={() =>
                        updateDraft({ quality: draft.quality === quality ? null : quality })
                      }
                      className={cn(
                        "rounded-full px-2.5 py-1 text-[11px] font-medium transition",
                        draft.quality === quality
                          ? "bg-accent/15 text-accent ring-1 ring-inset ring-accent/30"
                          : "bg-fg/5 text-fg/55 hover:text-fg/85",
                      )}
                    >
                      {quality}
                    </button>
                  ))}
                </div>
              </FieldRow>
              <FieldRow label={t("playground.settings.style")}>
                <div className="flex gap-1.5">
                  {["vivid", "natural"].map((style) => (
                    <button
                      key={style}
                      type="button"
                      onClick={() => updateDraft({ style: draft.style === style ? null : style })}
                      className={cn(
                        "rounded-full px-2.5 py-1 text-[11px] font-medium transition",
                        draft.style === style
                          ? "bg-accent/15 text-accent ring-1 ring-inset ring-accent/30"
                          : "bg-fg/5 text-fg/55 hover:text-fg/85",
                      )}
                    >
                      {style}
                    </button>
                  ))}
                </div>
              </FieldRow>
            </>
          )}
          </div>
        </div>
      )}

      <ModelSelectionBottomMenu
        isOpen={modelMenuOpen}
        onClose={() => setModelMenuOpen(false)}
        title={t("playground.settings.pickModel")}
        models={models}
        selectedModelIds={selectedModel ? [selectedModel.id] : []}
        onSelectModel={(modelId) => {
          selectModel(modelId);
          setModelMenuOpen(false);
        }}
      />
      <OptionPickerMenu
        isOpen={samplerMenuOpen}
        onClose={() => setSamplerMenuOpen(false)}
        title={t("playground.settings.sampler")}
        defaultLabel={t("editModel.generationAdvanced.modelDefault")}
        options={SDCPP_SAMPLERS}
        selected={draft.sampler ?? null}
        onSelect={(sampler) => updateDraft({ sampler })}
        onClear={() => updateDraft({ sampler: null })}
      />
      <OptionPickerMenu
        isOpen={hiresMenuOpen}
        onClose={() => setHiresMenuOpen(false)}
        title={t("playground.settings.hiresUpscaler")}
        defaultLabel={t("editModel.generationAdvanced.modelDefault")}
        options={hiresUpscalers}
        selected={draft.hiresUpscaler ?? null}
        onSelect={(hiresUpscaler) => updateDraft({ hiresUpscaler })}
        onClear={() => updateDraft({ hiresUpscaler: null })}
      />
      <OptionPickerMenu
        isOpen={schedulerMenuOpen}
        onClose={() => setSchedulerMenuOpen(false)}
        title={t("playground.settings.scheduler")}
        defaultLabel={t("editModel.generationAdvanced.modelDefault")}
        options={SDCPP_SCHEDULERS}
        selected={draft.scheduler ?? null}
        onSelect={(scheduler) => updateDraft({ scheduler })}
        onClear={() => updateDraft({ scheduler: null })}
      />
    </div>
  );
}
