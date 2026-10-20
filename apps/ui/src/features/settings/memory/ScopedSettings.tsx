import { useTranslation } from "react-i18next";
import { Accordion, AccordionItem, Badge, ListGroup, ListRow, Segmented, Switch, Text } from "@lettuceai/crisp";
import type { SettingsDynamicMemorySettings, SettingsMemoryRetrievalStrategy, SettingsMemoryRunMode, SettingsMemoryStructuredFallbackFormat } from "@/api/generated/bindings";
import { CommitNumber } from "./CommitNumber";
import { basisPointsToDecimal, customThresholdBasisPoints, decimalToBasisPoints, presetIds, presetOf, presets, runModes } from "./presets";

type Value = SettingsDynamicMemorySettings;
type Change = (change: (current: Value) => Value) => Promise<unknown>;

function Profile({ value, onChange }: { value: Value; onChange: Change }) {
  const { t } = useTranslation("settings");
  const preset = presetOf(value);
  const set = (patch: Partial<Value>) => onChange((current) => ({ ...current, ...patch }));
  const whole = (title: string, description: string, key: "summary_message_interval" | "max_entries" | "hot_memory_token_budget" | "retrieval_limit", min: number, max: number, step: number) => (
    <ListRow title={title} description={description} trailing={<CommitNumber value={value[key]} min={min} max={max} step={step} onCommit={(next) => set({ [key]: next })} />} />
  );
  const ratio = (title: string, description: string, key: "decay_rate_basis_points" | "cold_threshold_basis_points") => (
    <ListRow title={title} description={description} trailing={<CommitNumber value={basisPointsToDecimal(value[key])} min={0} max={1} step={0.05} precision={2} onCommit={(next) => set({ [key]: decimalToBasisPoints(next) })} />} />
  );
  const similarity = value.min_similarity_basis_points;
  return (
    <div className="space-y-3">
      <Segmented
        block
        aria-label={t("memory.profile.title")}
        value={preset as (typeof presetIds)[number]}
        onChange={(next) => void onChange((current) => ({ ...current, ...presets[next] }))}
        options={presetIds.map((id) => ({ value: id, label: t(`memory.profile.${id}`) }))}
      />
      <Text variant="hint">{preset === "custom" ? t("memory.profile.customBody") : t(`memory.profile.${preset}Body`)}</Text>
      <Accordion>
        <AccordionItem value="tune" title={t("memory.profile.advanced")} description={t("memory.profile.advancedBody")}>
          <ListGroup>
            {whole(t("memory.profile.interval"), t("memory.profile.intervalBody"), "summary_message_interval", 5, 200, 1)}
            {whole(t("memory.profile.maxEntries"), t("memory.profile.maxEntriesBody"), "max_entries", 10, 2000, 10)}
            {whole(t("memory.profile.hotBudget"), t("memory.profile.hotBudgetBody"), "hot_memory_token_budget", 512, 32768, 512)}
            {whole(t("memory.profile.retrievalLimit"), t("memory.profile.retrievalLimitBody"), "retrieval_limit", 1, 20, 1)}
            <ListRow
              title={t("memory.profile.similarity")}
              description={similarity === null ? t("memory.profile.similarityDefault") : t("memory.profile.similarityBody")}
              trailing={
                <Segmented
                  size="sm"
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
            <ListRow
              title={t("memory.profile.mode")}
              description={t(value.retrieval_strategy === "smart" ? "memory.profile.modeSmart" : "memory.profile.modeCosine")}
              trailing={<Segmented<SettingsMemoryRetrievalStrategy> size="sm" aria-label={t("memory.profile.mode")} value={value.retrieval_strategy} onChange={(next) => void set({ retrieval_strategy: next })} options={[{ value: "smart", label: t("memory.profile.smart") }, { value: "cosine", label: t("memory.profile.cosine") }]} />}
            />
            {ratio(t("memory.profile.decay"), t("memory.profile.decayBody"), "decay_rate_basis_points")}
            {ratio(t("memory.profile.cold"), t("memory.profile.coldBody"), "cold_threshold_basis_points")}
          </ListGroup>
        </AccordionItem>
      </Accordion>
    </div>
  );
}

export function ScopedSettings({ value, onChange }: { value: Value; onChange: Change }) {
  const { t } = useTranslation("settings");
  const set = (patch: Partial<Value>) => onChange((current) => ({ ...current, ...patch }));
  const ratio = (title: string, description: string, key: "delete_confidence_basis_points" | "max_hard_delete_ratio_basis_points" | "duplicate_threshold_basis_points", step = 0.05) => (
    <ListRow title={title} description={description} trailing={<CommitNumber value={basisPointsToDecimal(value[key])} min={0} max={1} step={step} precision={2} onCommit={(next) => set({ [key]: decimalToBasisPoints(next) })} />} />
  );
  return (
    <>
      <section className="space-y-2">
        <Text variant="section">{t("memory.updates.title")}</Text>
        <Segmented<SettingsMemoryRunMode> block aria-label={t("memory.updates.title")} value={value.run_mode} onChange={(next) => void set({ run_mode: next })} options={runModes.map((mode) => ({ value: mode, label: t(`memory.updates.${mode}`) }))} />
        <Text variant="hint">{t(`memory.updates.${value.run_mode}Body`)}</Text>
      </section>

      <section className="space-y-2">
        <Text variant="section">{t("memory.profile.title")}</Text>
        <Profile value={value} onChange={onChange} />
      </section>

      <Accordion>
        <AccordionItem value="advanced" title={t("memory.advanced.title")} description={t("memory.advanced.body")}>
          <ListGroup>
            <ListRow
              title={<span className="flex items-center gap-2">{t("memory.advanced.enrichment")} <Badge>{t("memory.advanced.experimental")}</Badge></span>}
              description={t("memory.advanced.enrichmentBody")}
              trailing={<Switch checked={value.context_enrichment_enabled} onChange={(next) => void set({ context_enrichment_enabled: next })} aria-label={t("memory.advanced.enrichment")} />}
            />
            <ListRow
              title={t("memory.advanced.loops")}
              description={t("memory.advanced.loopsBody")}
              trailing={<Switch checked={value.recursive_memory_loops} onChange={(next) => void set({ recursive_memory_loops: next })} aria-label={t("memory.advanced.loops")} />}
            />
            {value.recursive_memory_loops && (
              <ListRow title={t("memory.advanced.loopCap")} description={t("memory.advanced.loopCapBody")} trailing={<CommitNumber value={value.recursive_memory_loop_hard_cap} min={1} max={100} onCommit={(next) => set({ recursive_memory_loop_hard_cap: next })} />} />
            )}
            {ratio(t("memory.advanced.deleteConfidence"), t("memory.advanced.deleteConfidenceBody"), "delete_confidence_basis_points")}
            {ratio(t("memory.advanced.deleteRatio"), t("memory.advanced.deleteRatioBody"), "max_hard_delete_ratio_basis_points")}
            {ratio(t("memory.advanced.duplicate"), t("memory.advanced.duplicateBody"), "duplicate_threshold_basis_points", 0.01)}
            <ListRow
              title={t("memory.advanced.fallback")}
              description={t("memory.advanced.fallbackBody")}
              trailing={<Segmented<SettingsMemoryStructuredFallbackFormat> size="sm" aria-label={t("memory.advanced.fallback")} value={value.structured_fallback_format} onChange={(next) => void set({ structured_fallback_format: next })} options={[{ value: "xml", label: "XML" }, { value: "json", label: "JSON" }]} />}
            />
          </ListGroup>
        </AccordionItem>
      </Accordion>
    </>
  );
}
