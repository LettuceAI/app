import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Badge, ListGroup, ListRow, Segmented, Switch } from "@lettuceai/crisp";
import type { SettingsDynamicMemorySettings, SettingsMemoryRetrievalStrategy, SettingsMemoryRunMode, SettingsMemoryStructuredFallbackFormat } from "@/api/generated/bindings";
import { SettingsControlRow } from "@/shared/ui/SettingsControlRow";
import { useSettingsLayout } from "../data";
import { CommitNumber } from "./CommitNumber";
import { basisPointsToDecimal, customThresholdBasisPoints, decimalToBasisPoints, presetIds, presetOf, presets, runModes } from "./presets";

type Value = SettingsDynamicMemorySettings;
type Change = (change: (current: Value) => Value) => Promise<unknown>;
type WholeKey = "summary_message_interval" | "max_entries" | "hot_memory_token_budget" | "retrieval_limit" | "summary_token_budget" | "summary_chapter_size" | "recursive_memory_loop_hard_cap";
type RatioKey = "decay_rate_basis_points" | "cold_threshold_basis_points" | "delete_confidence_basis_points" | "max_hard_delete_ratio_basis_points" | "duplicate_threshold_basis_points";

export type MemorySection = "saved" | "recall" | "summary" | "advanced";

export function ProfileRows({
  value,
  onChange,
  tour = false,
}: {
  value: Value;
  onChange: Change;
  tour?: boolean;
}) {
  const { t } = useTranslation("settings");
  const { phone } = useSettingsLayout();
  const preset = presetOf(value);
  const set = (patch: Partial<Value>) => onChange((current) => ({ ...current, ...patch }));
  const modeRow = (
    <SettingsControlRow
      phone={phone}
      title={t("memory.updates.title")}
      description={t(`memory.updates.${value.run_mode}Body`)}
      control={
        <Segmented<SettingsMemoryRunMode>
          size="sm"
          block={phone}
          aria-label={t("memory.updates.title")}
          value={value.run_mode}
          onChange={(next) => void set({ run_mode: next })}
          options={runModes.map((mode) => ({ value: mode, label: t(`memory.updates.${mode}`) }))}
        />
      }
    />
  );
  const presetRow = (
    <SettingsControlRow
      phone={phone}
      title={t("memory.profile.title")}
      description={preset === "custom" ? t("memory.profile.customBody") : t(`memory.profile.${preset}Body`)}
      control={
        <Segmented
          size="sm"
          block={phone}
          aria-label={t("memory.profile.title")}
          value={preset as (typeof presetIds)[number]}
          onChange={(next) => void onChange((current) => ({ ...current, ...presets[next] }))}
          options={presetIds.map((id) => ({ value: id, label: t(`memory.profile.${id}`) }))}
        />
      }
    />
  );
  return (
    <>
      {tour ? <div data-tour-id="dynmem-mode">{modeRow}</div> : modeRow}
      {tour ? <div data-tour-id="dynmem-preset">{presetRow}</div> : presetRow}
    </>
  );
}

export function ScopedSettings({ section, value, onChange }: { section: MemorySection; value: Value; onChange: Change }) {
  const { t } = useTranslation("settings");
  const set = (patch: Partial<Value>) => onChange((current) => ({ ...current, ...patch }));
  const whole = (title: string, description: string, key: WholeKey, min: number, max: number, step: number) => (
    <ListRow title={title} description={description} trailing={<CommitNumber value={value[key]} min={min} max={max} step={step} onCommit={(next) => set({ [key]: next })} />} />
  );
  const ratio = (title: string, description: string, key: RatioKey, step = 0.05) => (
    <ListRow title={title} description={description} trailing={<CommitNumber value={basisPointsToDecimal(value[key])} min={0} max={1} step={step} precision={2} onCommit={(next) => set({ [key]: decimalToBasisPoints(next) })} />} />
  );
  const toggle = (title: ReactNode, label: string, description: string, checked: boolean, patch: (next: boolean) => Partial<Value>) => (
    <ListRow title={title} description={description} trailing={<Switch checked={checked} onChange={(next) => void set(patch(next))} aria-label={label} />} />
  );
  const experimental = (label: string) => (
    <span className="flex items-center gap-2">
      {label} <Badge>{t("memory.advanced.experimental")}</Badge>
    </span>
  );
  const similarity = value.min_similarity_basis_points;

  if (section === "saved") {
    return (
      <ListGroup label={t("memory.groups.saved")}>
        {whole(t("memory.profile.interval"), t("memory.profile.intervalBody"), "summary_message_interval", 5, 200, 1)}
        {whole(t("memory.profile.maxEntries"), t("memory.profile.maxEntriesBody"), "max_entries", 10, 65536, 10)}
        {toggle(t("memory.advanced.duplicateCheck"), t("memory.advanced.duplicateCheck"), t("memory.advanced.duplicateCheckBody"), value.duplicate_check_enabled, (next) => ({ duplicate_check_enabled: next }))}
        {value.duplicate_check_enabled && ratio(t("memory.advanced.duplicate"), t("memory.advanced.duplicateBody"), "duplicate_threshold_basis_points", 0.01)}
        {toggle(t("memory.advanced.modelPinning"), t("memory.advanced.modelPinning"), t("memory.advanced.modelPinningBody"), value.model_pinning_enabled, (next) => ({ model_pinning_enabled: next }))}
      </ListGroup>
    );
  }

  if (section === "recall") {
    return (
      <ListGroup label={t("memory.groups.reply")}>
        {whole(t("memory.profile.hotBudget"), t("memory.profile.hotBudgetBody"), "hot_memory_token_budget", 512, 262144, 512)}
        {whole(t("memory.profile.retrievalLimit"), t("memory.profile.retrievalLimitBody"), "retrieval_limit", 1, 4096, 1)}
        <ListRow
          title={t("memory.profile.mode")}
          description={t(value.retrieval_strategy === "smart" ? "memory.profile.modeSmart" : "memory.profile.modeCosine")}
          trailing={<Segmented<SettingsMemoryRetrievalStrategy> size="sm" aria-label={t("memory.profile.mode")} value={value.retrieval_strategy} onChange={(next) => void set({ retrieval_strategy: next })} options={[{ value: "smart", label: t("memory.profile.smart") }, { value: "cosine", label: t("memory.profile.cosine") }]} />}
        />
        <ListRow
          title={t("memory.profile.similarity")}
          description={similarity === null ? t("memory.profile.similarityDefault") : t("memory.profile.similarityBody")}
          trailing={
            <Segmented
              size="sm"
              aria-label={t("memory.profile.similarity")}
              value={similarity === null ? "auto" : "custom"}
              onChange={(next) => void set({ min_similarity_basis_points: next === "auto" ? null : customThresholdBasisPoints })}
              options={[{ value: "auto", label: t("memory.profile.similarityAuto") }, { value: "custom", label: t("memory.profile.similarityCustom") }]}
            />
          }
        />
        {similarity !== null && (
          <ListRow
            title={t("memory.profile.similarityValue")}
            description={t("memory.profile.similarityValueBody")}
            trailing={<CommitNumber value={basisPointsToDecimal(similarity)} min={0} max={1} step={0.05} precision={2} onCommit={(next) => set({ min_similarity_basis_points: decimalToBasisPoints(next) })} />}
          />
        )}
        {toggle(experimental(t("memory.advanced.enrichment")), t("memory.advanced.enrichment"), t("memory.advanced.enrichmentBody"), value.context_enrichment_enabled, (next) => ({ context_enrichment_enabled: next }))}
      </ListGroup>
    );
  }

  if (section === "summary") {
    return (
      <ListGroup label={t("memory.groups.summary")}>
        {toggle(experimental(t("memory.advanced.chapterSummaries")), t("memory.advanced.chapterSummaries"), t("memory.advanced.chapterSummariesBody"), value.chapter_summaries_enabled, (next) => ({ chapter_summaries_enabled: next }))}
        {value.chapter_summaries_enabled && whole(t("memory.advanced.summaryBudget"), t("memory.advanced.summaryBudgetBody"), "summary_token_budget", 1, 1048576, 128)}
        {value.chapter_summaries_enabled && whole(t("memory.advanced.chapterSize"), t("memory.advanced.chapterSizeBody"), "summary_chapter_size", 2, 100, 1)}
      </ListGroup>
    );
  }

  return (
    <>
      <ListGroup label={t("memory.groups.forgetting")}>
        {ratio(t("memory.profile.decay"), t("memory.profile.decayBody"), "decay_rate_basis_points")}
        {ratio(t("memory.profile.cold"), t("memory.profile.coldBody"), "cold_threshold_basis_points")}
        {ratio(t("memory.advanced.deleteConfidence"), t("memory.advanced.deleteConfidenceBody"), "delete_confidence_basis_points")}
        {ratio(t("memory.advanced.deleteRatio"), t("memory.advanced.deleteRatioBody"), "max_hard_delete_ratio_basis_points")}
      </ListGroup>
      <ListGroup label={t("memory.groups.manager")}>
        {toggle(t("memory.advanced.loops"), t("memory.advanced.loops"), t("memory.advanced.loopsBody"), value.recursive_memory_loops, (next) => ({ recursive_memory_loops: next }))}
        {value.recursive_memory_loops && whole(t("memory.advanced.loopCap"), t("memory.advanced.loopCapBody"), "recursive_memory_loop_hard_cap", 1, 100, 1)}
        <ListRow
          title={t("memory.advanced.fallback")}
          description={t("memory.advanced.fallbackBody")}
          trailing={<Segmented<SettingsMemoryStructuredFallbackFormat> size="sm" aria-label={t("memory.advanced.fallback")} value={value.structured_fallback_format} onChange={(next) => void set({ structured_fallback_format: next })} options={[{ value: "xml", label: "XML" }, { value: "json", label: "JSON" }]} />}
        />
      </ListGroup>
    </>
  );
}
