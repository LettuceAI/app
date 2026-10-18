import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Alert, Button, Checkbox, Field, ListGroup, ListRow, Sheet, Switch, Text, Textarea, useCompactLayout } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import type { ApiClient } from "@/api/client";
import type { CompanionEmotionVector, CompanionRegulationStyle, CompanionRelationshipDefaults, CompanionSoulConfigView, CompanionSoulDraft } from "@/api/generated/bindings";
import { watchJob } from "@/entities/job";
import { useModelName } from "../components/use-model-name";
import { soulWriterModelQuery } from "../data/queries";
import { applySoulSelection, openingContext, soulChanges, soulDraftOf, type FeelingChange, type SoulTextKey } from "../data/soul";
import type { CharacterForm } from "../data/form";

type Step = "ask" | "running" | "review";

function Review({ config, proposal, direction, onDirection, onRegenerate, onDiscard, onApply }: {
  config: CompanionSoulConfigView;
  proposal: CompanionSoulDraft;
  direction: string;
  onDirection: (value: string) => void;
  onRegenerate: () => void;
  onDiscard: () => void;
  onApply: (next: CompanionSoulConfigView) => void;
}) {
  const { t, i18n } = useTranslation("characters");
  const changes = soulChanges(config, proposal);
  const [values, setValues] = useState<Partial<Record<SoulTextKey, string>>>(() => Object.fromEntries(changes.texts.map((change) => [change.key, change.proposed])));
  const [used, setUsed] = useState<Partial<Record<SoulTextKey, boolean>>>(() => Object.fromEntries(changes.texts.map((change) => [change.key, true])));
  const [feelings, setFeelings] = useState(true);
  const total = changes.texts.length + changes.feelings.length;
  const nothing = total === 0 && !changes.factsChanged;
  const feelingLabel = (change: FeelingChange) => {
    if (change.group === "affect") return t(`soul.affect.${change.key as keyof CompanionEmotionVector}`);
    if (change.group === "regulation") return t(`soul.regulation.${change.key as keyof CompanionRegulationStyle}`);
    return t(`soul.relationship.${change.key as keyof CompanionRelationshipDefaults}`);
  };
  const percent = new Intl.NumberFormat(i18n.language, { style: "percent", maximumFractionDigits: 0 });
  const apply = () => {
    const texts = Object.fromEntries(changes.texts.filter((change) => used[change.key]).map((change) => [change.key, values[change.key] ?? change.proposed]));
    onApply(applySoulSelection(config, proposal, { texts, feelings: feelings && changes.feelings.length > 0, facts: changes.factsChanged && (Object.keys(texts).length > 0 || changes.texts.length === 0) }));
  };
  return <div className="space-y-5 pb-1">
    <Text variant="secondary">{nothing ? t("generateSoul.noDifferences") : t("generateSoul.changes", { count: total })}</Text>
    <Field label={t("generateSoul.direction")} hint={t("generateSoul.directionApplyHint")}>
      <Textarea rows={2} autoGrow maxRows={6} value={direction} placeholder={t("generateSoul.directionPlaceholder")} onChange={(event) => onDirection(event.target.value)} />
    </Field>
    {changes.texts.map((change) => <div key={change.key} className="space-y-1.5">
      <div className="flex items-center justify-between gap-3">
        <span className="min-w-0 truncate text-sm font-medium text-fg">{t(`soul.fields.${change.key}.label`)}</span>
        <label className="flex shrink-0 items-center gap-2 text-sm text-fg-2">
          <Checkbox checked={used[change.key] ?? false} onChange={(on) => setUsed((current) => ({ ...current, [change.key]: on }))} aria-label={t("generateSoul.useField", { field: t(`soul.fields.${change.key}.label`) })} />
          {t("generateSoul.use")}
        </label>
      </div>
      <Textarea rows={3} autoGrow maxRows={10} value={values[change.key] ?? ""} aria-label={t(`soul.fields.${change.key}.label`)} disabled={!used[change.key]} onChange={(event) => { const value = event.target.value; setValues((current) => ({ ...current, [change.key]: value })); }} />
      <p className="truncate text-xs text-fg-3">{t("generateSoul.now", { value: change.now.trim() || t("generateSoul.empty") })}</p>
    </div>)}
    {changes.feelings.length > 0 && <ListGroup label={t("generateSoul.feelings")}>
      {changes.feelings.map((change) => <ListRow key={`${change.group}.${change.key}`} title={feelingLabel(change)} trailing={<span className="text-sm tabular-nums text-fg-2">{t("generateSoul.shift", { from: percent.format(change.now), to: percent.format(change.proposed) })}</span>} />)}
      <ListRow title={t("generateSoul.useFeelings")} trailing={<Switch checked={feelings} onChange={setFeelings} aria-label={t("generateSoul.useFeelings")} />} />
    </ListGroup>}
    {changes.factsChanged && <Text variant="secondary">{changes.facts > 0 ? t("generateSoul.facts", { count: changes.facts }) : t("generateSoul.factsCleared")}</Text>}
    <div className="flex gap-2">
      <Button variant="ghost" className="flex-1" onClick={onDiscard}>{t("generateSoul.discard")}</Button>
      <Button variant="secondary" className="flex-1" onClick={onRegenerate}>{t("generateSoul.regenerate")}</Button>
      <Button variant="primary" className="flex-1" disabled={nothing} onClick={apply}>{t("generateSoul.apply")}</Button>
    </div>
  </div>;
}

export function GenerateSoulSheet({ api, open, form, onClose, onApply }: { api: ApiClient; open: boolean; form: CharacterForm; onClose: () => void; onApply: (next: CompanionSoulConfigView) => void }) {
  const { t } = useTranslation("characters");
  const { t: common, i18n } = useTranslation("common");
  const phone = useCompactLayout();
  const config = form.draft.defaults.companion_soul;
  const { data: writerModelId = null } = useQuery(soulWriterModelQuery(api));
  const modelName = useModelName(api, writerModelId);
  const [step, setStep] = useState<Step>("ask");
  const [direction, setDirection] = useState("");
  const [failure, setFailure] = useState<string | null>(null);
  const [streamed, setStreamed] = useState("");
  const [proposal, setProposal] = useState<{ draft: CompanionSoulDraft; run: number } | null>(null);
  const controller = useRef<AbortController | null>(null);
  const jobId = useRef<string | null>(null);
  const runs = useRef(0);

  const cancelJob = () => {
    const id = jobId.current;
    controller.current?.abort();
    controller.current = null;
    jobId.current = null;
    if (id) void api.call("jobCancel", { job_id: id }).catch(() => undefined);
  };

  const cancelRef = useRef(cancelJob);
  useLayoutEffect(() => {
    cancelRef.current = cancelJob;
  });
  useEffect(() => () => cancelRef.current(), []);

  const describe = (error: unknown) => common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`);

  const run = async () => {
    if (!config || controller.current) return;
    const abort = new AbortController();
    controller.current = abort;
    setStep("running");
    setFailure(null);
    setStreamed("");
    try {
      const accepted = await api.call("companionSoulWriterRun", {
        character_name: form.draft.profile.name.trim(),
        character_definition: form.draft.profile.definition,
        character_description: form.draft.profile.description,
        opening_context: openingContext(form.draft.scenes),
        current_soul: soulDraftOf(config),
        user_notes: direction.trim() || null,
        model_profile_id: null,
        client_operation_id: crypto.randomUUID(),
      });
      if (abort.signal.aborted) {
        void api.call("jobCancel", { job_id: accepted.job_id }).catch(() => undefined);
        return;
      }
      jobId.current = accepted.job_id;
      const job = await watchJob(api, accepted.job_id, () => undefined, abort.signal, (text) => {
        if (text) setStreamed((current) => current + text);
      });
      if (abort.signal.aborted) return;
      if (job.state === "succeeded" && job.result?.type === "companion_soul_draft") {
        runs.current += 1;
        setProposal({ draft: job.result.draft, run: runs.current });
        setStep("review");
      } else if (job.state === "failed") {
        const code = job.failure?.code;
        setFailure(code && i18n.exists(`errors.code.${code}`, { ns: "common" }) ? common(`errors.code.${code}` as never) : t("generateSoul.failed"));
        setStep("ask");
      } else {
        setStep("ask");
      }
    } catch (error) {
      if (abort.signal.aborted) return;
      setFailure(isApiFailure(error) ? describe(error) : t("generateSoul.failed"));
      setStep("ask");
    } finally {
      if (controller.current === abort) {
        controller.current = null;
        jobId.current = null;
      }
    }
  };

  const close = () => {
    cancelJob();
    setStep("ask");
    setFailure(null);
    setProposal(null);
    onClose();
  };

  return (
    <Sheet open={open} onClose={close} placement={phone ? "bottom" : "side"} size="md" title={t(step === "review" ? "generateSoul.reviewTitle" : "generateSoul.title")}>
      {step === "ask" && <div className="space-y-4 pb-1">
        <Field label={t("generateSoul.direction")} hint={t("generateSoul.directionHint")}>
          <Textarea rows={3} autoGrow maxRows={8} value={direction} placeholder={t("generateSoul.directionPlaceholder")} onChange={(event) => setDirection(event.target.value)} />
        </Field>
        <Text variant="secondary">{t("generateSoul.usingModel", { model: modelName })}</Text>
        {failure && <Alert tone="danger" title={failure} action={<Button size="sm" variant="ghost" onClick={() => void run()}>{common("actions.retry")}</Button>} />}
        <Button variant="primary" block onClick={() => void run()}>{t("generateSoul.generate")}</Button>
      </div>}
      {step === "running" && <div className="space-y-4 pb-1" role="status">
        <Text>{t("generateSoul.generating")}</Text>
        {streamed && <p className="max-h-40 overflow-y-auto whitespace-pre-wrap text-xs text-fg-3">{streamed}</p>}
        <Button variant="secondary" block onClick={() => { cancelJob(); setStep("ask"); }}>{t("generateSoul.stop")}</Button>
      </div>}
      {step === "review" && proposal && config && <Review key={proposal.run} config={config} proposal={proposal.draft} direction={direction} onDirection={setDirection} onRegenerate={() => void run()} onDiscard={close} onApply={(next) => { onApply(next); close(); }} />}
    </Sheet>
  );
}
