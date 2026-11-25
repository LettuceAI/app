import { AssetAudio } from "@/shared/ui/AssetAudio";
import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { AudioLines, Cloud, Cpu, HardDrive, Mic, Plus, Zap } from "lucide-react";
import { Badge, Button, Collapsible, Field, fileSize, icon, IconButton, Input, ListGroup, ListRow, ListRowButton, Segmented, SelectMenu, Switch, Text } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { AssetRef, MicrophoneDevices, ProviderAccountView, TranscribeOptions, WhisperInstalledModel } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { whisperLanguages } from "./languages";
import { microphoneOptions, recordingTime, modelName, runOptions, useSpeechAction } from "./data";
import { useRecording } from "./use-recording";
import { RecordingControls } from "./RecordingControls";

export function ModelTab({ api, phone, active, cloudAccounts, cloudProviderId, cloudModelId, onSelectCloud, gpu, gpuSupported, keepLoaded, onGpu, onKeepLoaded, deviceId, devices, onDevice, onChoose, onMicrophone, onSaveRecording, downloads }: {
  api: ApiClient; phone: boolean; active: WhisperInstalledModel | null; cloudAccounts: readonly ProviderAccountView[]; cloudProviderId: string | null; cloudModelId: string | null; onSelectCloud: (providerAccountId: string | null, modelId: string | null) => void; gpu: boolean; gpuSupported: boolean; keepLoaded: boolean; onGpu: (value: boolean) => void; onKeepLoaded: (value: boolean) => void; deviceId: string | null; devices: MicrophoneDevices | undefined; onDevice: (value: string | null) => void; onChoose: () => void; onMicrophone: () => void; onSaveRecording: (value: { audio: AssetRef; whisperOutput: string | null }) => void; downloads: React.ReactNode;
}) {
  const { t } = useTranslation("settings", { keyPrefix: "speech" });
  const action = useSpeechAction();
  const recording = useRecording(api, deviceId);
  const [source, setSource] = useState<"local" | "cloud">(cloudProviderId ? "cloud" : "local");
  const [pickedAccount, setPickedAccount] = useState(cloudProviderId ?? cloudAccounts[0]?.id ?? "");
  const selectedAccountId = cloudAccounts.some((account) => account.id === pickedAccount)
    ? pickedAccount
    : (cloudProviderId ?? cloudAccounts[0]?.id ?? "");
  const cloudModels = useQuery({
    queryKey: [...queryKeys.speech.all, "cloud-models", selectedAccountId],
    queryFn: () => api.call("speechCloudModels", { provider_account_id: selectedAccountId }),
    enabled: source === "cloud" && Boolean(selectedAccountId),
  });
  const [language, setLanguage] = useState("auto");
  const [extra, setExtra] = useState("");
  const useGpu = gpuSupported && gpu;
  const options: TranscribeOptions = { language: language === "auto" ? null : language, scopes: ["conversation", "global"], initial_prompt: extra.trim() || null, translate: false, detect_language: false, threads: null, run: runOptions(useGpu), keep_model_loaded: keepLoaded };
  const microphones = microphoneOptions(devices, deviceId, t("defaultMic"), t("micUnavailable"), (name) => t("micAmbiguous", { name }));
  const busy = recording.phase !== "idle" || action.isPending;
  const activeModelId = source === "cloud"
    ? (cloudProviderId === selectedAccountId ? cloudModelId : null)
    : (active?.id ?? null);
  const activeModelLabel = source === "cloud"
    ? (cloudModels.data?.find((item) => item.id === activeModelId)?.name ?? activeModelId ?? undefined)
    : (active ? modelName(active.id) : undefined);
  const switchSource = (next: "local" | "cloud") => {
    setSource(next);
    if (next === "local" && cloudProviderId) {
      onSelectCloud(null, active?.id ?? null);
    }
  };
  const model = (
    <section className="space-y-3" data-tour-id="asr-active-model">
      <Segmented<"local" | "cloud">
        block
        aria-label={t("source")}
        value={source}
        onChange={switchSource}
        options={[
          { value: "local", label: t("sourceLocal"), icon: <Cpu size={icon.sm} /> },
          { value: "cloud", label: t("sourceCloud"), icon: <Cloud size={icon.sm} /> },
        ]}
      />
      {source === "local" ? (
        <>
          <ListGroup label={t("model")}>
            <ListRowButton
              icon={<AudioLines size={icon.lg} />}
              tone={active ? "accent" : "warning"}
              title={active ? modelName(active.id) : t("noModel")}
              description={active ? <span>{t("metadata", { left: fileSize(active.size_bytes), right: t(active.english_only ? "englishOnly" : "multilingual") })}{!phone && <span className="block font-mono text-xs">{active.filename}</span>}</span> : t("noModelBody")}
              trailing={phone ? undefined : <Text variant="hint">{t(active ? "change" : "choose")}</Text>}
              onClick={onChoose}
            />
          </ListGroup>
          {downloads}
          <Text variant="hint" className="px-1">{t("privacy")}</Text>
        </>
      ) : (
        <ListGroup label={t("cloudModel")}>
          <div className="space-y-4 p-4">
            <Field label={t("cloudProvider")} hint={cloudAccounts.length === 0 ? t("cloudProviderEmpty") : undefined}>
              <SelectMenu
                options={cloudAccounts.map((account) => ({ value: account.id, label: account.label }))}
                value={selectedAccountId || null}
                disabled={cloudAccounts.length === 0 || busy}
                onChange={(value) => { if (value) setPickedAccount(value); }}
              />
            </Field>
            {selectedAccountId && (
              <Field label={t("model")}>
                <SelectMenu
                  options={(cloudModels.data ?? []).map((item) => ({ value: item.id, label: item.name }))}
                  value={activeModelId}
                  disabled={cloudModels.isPending || busy}
                  onChange={(value) => { if (value) onSelectCloud(selectedAccountId, value); }}
                />
              </Field>
            )}
            {cloudModels.isError && <FailureState error={cloudModels.error} size="sm" onRetry={() => void cloudModels.refetch()} />}
          </div>
        </ListGroup>
      )}
    </section>
  );
  const runtime = (
    <section className="space-y-2" data-tour-id="asr-runtime">
      <ListGroup label={t("runtime")}>
        {source === "local" && (
          <>
            {gpuSupported ? <ListRow icon={<Zap size={icon.lg} />} title={t("gpu")} description={t("gpuBody")} trailing={<Switch checked={gpu} onChange={onGpu} disabled={busy} aria-label={t("gpu")} />} /> : <ListRow icon={<Cpu size={icon.lg} />} title={t("cpu")} description={t("cpuBody")} trailing={<Switch checked disabled onChange={() => onGpu(false)} aria-label={t("cpu")} />} />}
            <ListRow icon={<Cpu size={icon.lg} />} title={t("keepLoaded")} description={t("keepLoadedBody")} trailing={<Switch checked={keepLoaded} onChange={onKeepLoaded} disabled={busy} aria-label={t("keepLoaded")} />} />
            <ListRow icon={<HardDrive size={icon.lg} />} title={t("loadedModel")} description={t(!active ? "pickFirst" : active.loaded ? "loadedBody" : "notLoadedBody")} trailing={<Button size="sm" className="shrink-0" variant={active?.loaded ? "ghost" : "tonal"} disabled={!active || busy} loading={action.isPending} onClick={() => action.mutate(() => active?.loaded ? api.call("whisperClearCache") : api.call("whisperPreload", { model_id: active?.id ?? null, run: runOptions(useGpu) }))}>{t(active?.loaded ? "unload" : "load")}</Button>} />
          </>
        )}
        {phone ? <ListRowButton icon={<Mic size={icon.lg} />} title={t("microphone")} description={microphones.find((item) => item.value === (deviceId ?? "default"))?.label} onClick={onMicrophone} /> : <ListRow icon={<Mic size={icon.lg} />} title={t("microphone")} description={t("microphoneBody")} trailing={<Field label={<span className="sr-only">{t("microphone")}</span>} className="w-52 shrink-0 space-y-0"><SelectMenu options={microphones} value={deviceId ?? "default"} disabled={busy || !devices?.supported} onChange={(value) => { if (value) onDevice(value === "default" ? null : value); }} /></Field>} />}
      </ListGroup>
      {source === "local" && <Text variant="hint" className="px-1">{t(gpuSupported ? "runtimeHint" : "cpuRuntimeHint")}</Text>}
      {action.isError && <FailureState error={action.error} size="sm" />}
    </section>
  );
  const result = recording.result;
  const transcribe = () => { if (recording.audio && activeModelId) void recording.transcribe(recording.audio, activeModelId, options); };
  const test = <ListGroup data-tour-id="asr-mic-test" label={t("tryIt")}><div className="space-y-4 p-4">
    <div className={phone ? "grid gap-3" : "grid grid-cols-2 gap-3"}><Field label={t("language")}><SelectMenu options={[{ value: "auto", label: t("detect") }, ...whisperLanguages.map((item) => ({ value: item.code, label: item.name }))]} value={language} onChange={(value) => setLanguage(value ?? "auto")} disabled={busy} /></Field><Field label={t("extra")} hint={t("extraHint")}><Input value={extra} onChange={(event) => setExtra(event.target.value)} placeholder={t("extraPlaceholder")} disabled={busy} /></Field></div>
    <RecordingControls recording={recording} {...(activeModelLabel ? { modelName: activeModelLabel } : {})} disabled={!activeModelId} onStop={() => { void recording.stop().then((audio) => { if (audio && activeModelId) void recording.transcribe(audio, activeModelId, options); }); }} />
    {recording.audio && !result && recording.phase === "idle" && <Button size="sm" variant="tonal" disabled={!activeModelId} onClick={transcribe}>{t("retryTranscribe")}</Button>}
    {result && <div className="space-y-3 border-t border-line pt-4"><p className="whitespace-pre-wrap text-base leading-relaxed text-fg">{result.text}</p><div className="space-y-1"><Text variant="label">{t("heard")}</Text><p className="whitespace-pre-wrap text-sm text-fg-3">{result.raw_text}</p></div><div className="flex flex-wrap items-center gap-1.5">{result.detected_language && <Badge>{whisperLanguages.find((item) => item.code === result.detected_language)?.name ?? result.detected_language}</Badge>}{recording.elapsed > 0 && <Badge>{t("seconds", { seconds: (recording.elapsed / 1000).toFixed(1) })}</Badge>}{result.applied_corrections.map((correction) => <Badge key={correction.correction_id} tone="accent">{t("pair", { left: correction.matched_text, right: correction.correct })}</Badge>)}</div><Collapsible variant="plain" title={t("segments", { count: result.segments.length })}><div className="space-y-1.5 pt-1">{result.segments.map((segment, index) => <div key={index} className="flex gap-3 text-sm"><span className="shrink-0 font-mono text-xs leading-5 text-fg-3">{t("range", { start: recordingTime(segment.start_ms), end: recordingTime(segment.end_ms) })}</span><span className="text-fg-2">{segment.text}</span></div>)}</div></Collapsible></div>}
    {recording.audio && recording.phase === "idle" && <AssetAudio src={recording.audio.url} title={t("recording")} actions={<IconButton label={t("saveExample")} onClick={() => { if (recording.audio) onSaveRecording({ audio: recording.audio, whisperOutput: result?.raw_text ?? null }); }}><Plus size={icon.md} /></IconButton>} />}
    {!recording.audio && recording.phase === "idle" && activeModelId && <Text variant="hint">{t("testHint")}</Text>}
  </div></ListGroup>;
  return phone ? <div className="space-y-7">{model}{test}{runtime}</div> : <div className="grid grid-cols-2 items-start gap-8"><div className="min-w-0 space-y-7">{model}{runtime}</div><div className="min-w-0">{test}</div></div>;
}
