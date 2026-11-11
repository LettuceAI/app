import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Play } from "lucide-react";
import { Button, confirm, Field, icon, Input, SelectMenu, Sheet, Textarea } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { AudioProviderView, AudioVoiceView, UserVoiceView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { watchJob } from "@/entities/job";
import { AssetAudio } from "@/shared/ui/AssetAudio";
import { FailureState } from "@/shared/ui/FailureState";
import { providerVoicesQuery, ttsModelsQuery, useInvalidate, useOperationId, useSynthesis } from "./data";

export interface VoicePrefill {
  providerId: string;
  name: string;
  voiceId: string;
}

const designMinimum = 100;
type FieldKey = "name" | "model" | "voice" | "preview";

export function VoiceEditorSheet({ api, providers, voice, prefill, onClose }: { api: ApiClient; providers: readonly AudioProviderView[]; voice: UserVoiceView | null; prefill: VoicePrefill | null; onClose: () => void }) {
  const { t, i18n } = useTranslation("settings");
  const { t: common } = useTranslation();
  const invalidate = useInvalidate();
  const [draft, setDraft] = useState({ name: voice?.name ?? prefill?.name ?? "", providerId: voice?.provider_id ?? prefill?.providerId ?? providers[0]!.id, modelId: voice?.model_id ?? "", voiceId: voice?.voice_id ?? prefill?.voiceId ?? "", prompt: voice?.prompt ?? "" });
  const [sample, setSample] = useState(() => t("voices.editor.defaultSample"));
  const [problem, setProblem] = useState<{ field: FieldKey; message: string } | null>(null);
  const [design, setDesign] = useState<{ id: string; url: string } | null>(null);
  const provider = providers.find((entry) => entry.id === draft.providerId) ?? providers[0]!;
  const type = provider.configuration.type;
  const designing = type === "elevenlabs" && voice === null && prefill === null;
  const listsVoices = type === "elevenlabs" || type === "fish_tts" || type === "gemini";
  const picksModel = type !== "open_ai_compatible";
  const models = useQuery({ ...ttsModelsQuery(api, type, designing), enabled: picksModel });
  const voices = useQuery({ ...providerVoicesQuery(api, provider.id), enabled: listsVoices && !designing });
  const synthesis = useSynthesis(api);
  const controller = useRef(new AbortController());
  const designOperationId = useOperationId();
  const saveOperationId = useOperationId();
  const refreshed = useRef(new Set<string>());
  const refresh = useMutation({ mutationFn: async () => { await api.call("audioProviderVoicesRefresh", { provider_id: provider.id }); await invalidate(queryKeys.voices.providerVoices(provider.id)); } });
  const { mutate: refreshVoices } = refresh;
  useEffect(() => () => controller.current.abort(), []);
  useEffect(() => {
    if ((type === "elevenlabs" || type === "fish_tts") && !designing && voices.isSuccess && voices.data.length === 0 && !refreshed.current.has(provider.id)) {
      refreshed.current.add(provider.id);
      refreshVoices();
    }
  }, [type, designing, voices.isSuccess, voices.data, provider.id, refreshVoices]);
  const modelList = models.data ?? [];
  const model = picksModel ? modelList.some((entry) => entry.id === draft.modelId) ? draft.modelId : modelList[0]?.id ?? "" : draft.modelId;
  const voiceList = voices.data ?? [];
  const voiceId = type === "gemini" && !draft.voiceId.trim() ? voiceList[0]?.voice_id ?? "" : draft.voiceId;
  const edit = (patch: Partial<typeof draft>) => { setProblem(null); setDraft((previous) => ({ ...previous, ...patch })); };
  const changeProvider = (id: string | null) => { if (id && id !== draft.providerId) { setDesign(null); edit({ providerId: id, modelId: "", voiceId: "" }); } };
  const pickVoice = (id: string | null) => {
    const picked = voiceList.find((entry) => entry.voice_id === id);
    edit({ voiceId: id ?? "", ...(picked && !draft.name.trim() ? { name: picked.name } : {}) });
  };
  const sampleLength = sample.trim().length;
  const needsVoiceId = type === "open_ai_compatible" || type === "fish_tts" || type === "fish_speech";
  const previewBlocked = !sample.trim() || (designing ? sampleLength < designMinimum : !model || (needsVoiceId && !draft.voiceId.trim()));
  const designPreview = useMutation({
    mutationFn: async () => {
      const previews = await api.call("voiceDesignPreview", { provider_id: provider.id, text_sample: sample.trim(), voice_description: draft.prompt, model_id: model || null, num_previews: 1 });
      const first = previews[0];
      if (first) setDesign({ id: first.generated_voice_id, url: first.audio.url });
    },
  });
  const preview = () => {
    if (designing) designPreview.mutate();
    else void synthesis.run({ provider_id: provider.id, model_id: model, voice_id: voiceId, prompt: draft.prompt.trim() || null, text: sample });
  };
  const save = useMutation({
    mutationFn: async () => {
      const name = draft.name.trim();
      const prompt = draft.prompt.trim() || null;
      let stored = voiceId;
      if (designing && design) {
        const accepted = await api.call("voiceDesignCreate", { client_operation_id: designOperationId({ design: design.id, name, prompt }), provider_id: provider.id, generated_voice_id: design.id, name, description: draft.prompt });
        const job = await watchJob(api, accepted.job_id, () => undefined, controller.current.signal);
        if (job.state !== "succeeded" || job.result?.type !== "voice_created") throw new Error("voice design did not create a voice");
        stored = job.result.voice_id;
      }
      const fields = { provider_id: provider.id, name, model_id: model, voice_id: stored, prompt };
      if (voice) await api.call("userVoiceUpdate", { ...fields, id: voice.id, expected_revision: voice.revision });
      else await api.call("userVoiceCreate", { ...fields, client_operation_id: saveOperationId(fields) });
      await invalidate(queryKeys.voices.user());
    },
    onSuccess: onClose,
  });
  const submit = () => {
    const fail = (field: FieldKey, message: string) => { setProblem({ field, message }); };
    if (!draft.name.trim()) return fail("name", t("voices.editor.nameRequired"));
    if (designing && !design) return fail("preview", t("voices.editor.previewFirst"));
    if (type === "fish_tts" && !voiceId.trim()) return fail("voice", t("voices.editor.fishVoiceIdRequired"));
    if (type === "open_ai_compatible" && !model.trim()) return fail("model", t("voices.editor.openaiModelIdRequired"));
    if (type === "open_ai_compatible" && !voiceId.trim()) return fail("voice", t("voices.editor.openaiVoiceIdRequired"));
    setProblem(null);
    save.mutate();
  };
  const remove = useMutation({
    mutationFn: async () => {
      if (!voice) return;
      await api.call("userVoiceDelete", { voice_id: voice.id });
      await invalidate(queryKeys.voices.user());
    },
    onSuccess: onClose,
  });
  const askDelete = async () => {
    if (await confirm({ title: t("voices.editor.delete"), message: t("voices.editor.deleteConfirm"), confirmLabel: t("voices.editor.delete"), cancelLabel: common("actions.cancel"), destructive: true })) remove.mutate();
  };
  const busy = save.isPending || remove.isPending;
  const error = (field: FieldKey) => problem?.field === field ? problem.message : undefined;
  const voiceLabel = (entry: AudioVoiceView) => type === "gemini" ? entry.labels.description ? `${entry.name} (${entry.labels.description})` : entry.name : entry.labels.category ? `${entry.name} (${entry.labels.category})` : entry.name;
  const voiceOptions = voiceList.map((entry) => ({ value: entry.voice_id, label: voiceLabel(entry) }));
  const refreshButton = <Button size="sm" variant="ghost" type="button" loading={refresh.isPending} onClick={() => refreshVoices()}>{t("voices.providerVoices.refresh")}</Button>;
  const idCopy = type === "fish_tts" ? "fishVoiceId" : type === "fish_speech" ? "fishSpeechVoiceId" : "voiceId";
  const audioUrl = designing ? design?.url : synthesis.asset?.url;
  const previewing = designing ? designPreview.isPending : synthesis.busy;
  const previewError = designing ? designPreview.error : synthesis.error;
  return (
    <Sheet open onClose={onClose} placement="auto" title={t(voice ? "voices.editor.editTitle" : "voices.editor.createTitle")}
      footer={
        <div className="flex items-center gap-2">
          {voice && <Button variant="danger" disabled={busy} onClick={() => void askDelete()}>{t("voices.editor.delete")}</Button>}
          <div className="flex-1" />
          <Button variant="primary" loading={save.isPending} disabled={remove.isPending} onClick={submit}>{t(save.isPending && designing ? "voices.editor.creating" : "voices.editor.save")}</Button>
        </div>
      }>
      <div className="space-y-4">
        {save.isError && <FailureState error={save.error} size="sm" />}
        {remove.isError && <FailureState error={remove.error} size="sm" />}
        <Field label={t("voices.editor.name")} {...(error("name") ? { error: error("name") } : {})}>
          <Input value={draft.name} placeholder={t("voices.editor.namePlaceholder")} onChange={(event) => edit({ name: event.target.value })} />
        </Field>
        <Field label={t("voices.editor.provider")}>
          <SelectMenu options={providers.map((entry) => ({ value: entry.id, label: entry.label }))} value={provider.id} onChange={changeProvider} />
        </Field>
        {picksModel ? (
          <Field label={t("voices.editor.model")}>
            <SelectMenu options={modelList.map((entry) => ({ value: entry.id, label: entry.name }))} value={model || null} onChange={(value) => edit({ modelId: value ?? "" })} disabled={models.isPending} />
          </Field>
        ) : (
          <Field label={t("voices.editor.model")} hint={t("voices.editor.modelIdHint")} {...(error("model") ? { error: error("model") } : {})}>
            <Input value={draft.modelId} placeholder={t("voices.editor.modelIdPlaceholder")} onChange={(event) => edit({ modelId: event.target.value })} />
          </Field>
        )}
        {models.isError && <FailureState error={models.error} size="sm" onRetry={() => void models.refetch()} />}
        {(type === "elevenlabs" && !designing || type === "fish_tts") && (
          <Field label={<span className="flex items-center justify-between gap-2">{t(type === "elevenlabs" ? "voices.editor.elevenlabsVoice" : "voices.editor.fishVoice")}{refreshButton}</span>} hint={t(type === "elevenlabs" ? "voices.editor.elevenlabsVoiceHint" : "voices.editor.fishVoiceHint")}>
            <SelectMenu options={voiceOptions} value={voiceOptions.some((entry) => entry.value === draft.voiceId) ? draft.voiceId : null} onChange={pickVoice} disabled={voices.isPending} />
          </Field>
        )}
        {type === "gemini" && (
          <Field label={t("voices.editor.geminiVoice")} hint={t("voices.editor.geminiVoiceHint")}>
            <SelectMenu options={voiceOptions} value={voiceOptions.some((entry) => entry.value === voiceId) ? voiceId : null} onChange={pickVoice} disabled={voices.isPending} />
          </Field>
        )}
        {listsVoices && !designing && voices.isError && <FailureState error={voices.error} size="sm" onRetry={() => void voices.refetch()} />}
        {refresh.isError && <FailureState error={refresh.error} size="sm" />}
        {needsVoiceId && (
          <Field label={t("voices.editor.voiceId")} hint={t(`voices.editor.${idCopy}Hint`)} {...(error("voice") ? { error: error("voice") } : {})}>
            <Input value={draft.voiceId} placeholder={t(`voices.editor.${idCopy}Placeholder`)} onChange={(event) => edit({ voiceId: event.target.value })} />
          </Field>
        )}
        <Field label={t(designing ? "voices.editor.designDescription" : "voices.editor.prompt")} hint={t(designing ? "voices.editor.designDescriptionHint" : "voices.editor.promptHint")}>
          <Textarea rows={3} value={draft.prompt} placeholder={t("voices.editor.promptPlaceholder")} onChange={(event) => edit({ prompt: event.target.value })} />
        </Field>
        <Field label={t("voices.editor.sample")} {...(designing ? { hint: t("voices.editor.designChars", { current: new Intl.NumberFormat(i18n.language).format(sampleLength), minimum: designMinimum }) } : {})}>
          <Textarea rows={3} value={sample} onChange={(event) => setSample(event.target.value)} />
        </Field>
        <Field {...(error("preview") ? { error: error("preview") } : {})}>
          <div className="space-y-3">
            <Button variant="secondary" leading={<Play size={icon.sm} />} loading={previewing} disabled={previewBlocked} onClick={preview}>{t(previewing ? "voices.editor.previewing" : "voices.editor.preview")}</Button>
            {previewError !== null && <FailureState error={previewError} size="sm" />}
            {audioUrl && <AssetAudio src={audioUrl} title={draft.name.trim() || t("voices.editor.preview")} />}
          </div>
        </Field>
      </div>
    </Sheet>
  );
}
