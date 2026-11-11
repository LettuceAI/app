import { useCallback, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useSearch } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { AudioLines, Cpu, Download, Pencil, Play, Plus, Trash2 } from "lucide-react";
import { Button, confirm, fileSize, icon, KeyValue, ListGroup, ListRow, ListRowButton, Page, SelectMenu, Sheet, Skeleton, Slider, Textarea, Field } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import type { ApiClient } from "@/api/client";
import type { AudioProviderView, JobAccepted, JobView, KokoroInventory } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { DownloadJob } from "@/entities/download";
import { terminalJobStates } from "@/entities/job";
import { AssetAudio } from "@/shared/ui/AssetAudio";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsLayout } from "../data";
import { ActionRow } from "./ActionRow";
import { KokoroLibrary } from "./KokoroLibrary";
import { kokoroAvailableQuery, kokoroInstalled, kokoroInventoryQuery, kokoroVariantOf, kokoroVariantsQuery, kokoroVoiceName, parseBlend, parseSpeed, serializeSpeed, speedRange, speedText, useInvalidate, useSynthesis, useVoicesNavigation, userVoicesQuery } from "./data";
import { LibrarySkeleton, VoicesSkeleton } from "./VoicesSkeleton";

interface Tracked { name: string; voices: readonly string[]; model: boolean; job: JobView | null }

export function KokoroStudio({ api, provider, onSetup }: { api: ApiClient; provider: AudioProviderView; onSetup: () => void }) {
  const { t, i18n } = useTranslation("settings");
  const { t: common } = useTranslation();
  const { phone } = useSettingsLayout();
  const { open, close } = useVoicesNavigation();
  const invalidate = useInvalidate();
  const engineOpen = useSearch({ from: "/settings/$section", select: (search) => search.kokoroEngine === true });
  const status = useQuery(appStatusQuery(api));
  const variant = kokoroVariantOf(provider);
  const variants = useQuery(kokoroVariantsQuery(api));
  const inventory = useQuery(kokoroInventoryQuery(api, variant));
  const userVoices = useQuery(userVoicesQuery(api));
  const installed = kokoroInstalled(inventory.data);
  const available = useQuery(kokoroAvailableQuery(api, variant !== null));
  const synthesis = useSynthesis(api);
  const [tracked, setTracked] = useState<Record<string, Tracked>>({});
  const [text, setText] = useState(() => t("voices.kokoro.tryText"));
  const [speed, setSpeed] = useState(1);
  const [choice, setChoice] = useState<string | null>(null);
  const mobile = status.data?.platform === "android" || status.data?.platform === "ios";
  const variantList = (variants.data ?? []).filter((entry) => !mobile || entry.mobile_supported);
  const blends = (userVoices.data ?? []).filter((voice) => voice.provider_id === provider.id);
  const installedIds = inventory.data?.installed_voices.map((entry) => entry.id) ?? [];
  const failed = [variants, status, ...(variant === null ? [] : [inventory]), userVoices].find((query) => query.isError);
  const loading = !failed && (variants.isPending || status.isPending || userVoices.isPending);
  const checking = variant !== null && inventory.isPending;
  const active = Object.values(tracked).filter((entry) => !entry.job || !terminalJobStates.has(entry.job.state));
  const installingVoices = new Set(active.flatMap((entry) => entry.voices));
  const updateJob = useCallback((job: JobView) => setTracked((previous) => {
    const known = previous[job.id];
    return known && known.job !== job ? { ...previous, [job.id]: { ...known, job } } : previous;
  }), []);
  const settled = useCallback(async (job: JobView) => {
    await invalidate(queryKeys.voices.kokoroInventory(variant ?? ""), queryKeys.voices.kokoroAvailable());
    if (job.state === "succeeded") setTracked((previous) => { const { [job.id]: _done, ...rest } = previous; return rest; });
  }, [invalidate, variant]);
  const start = useMutation({
    mutationFn: async (job: { accept: () => Promise<JobAccepted> } & Omit<Tracked, "job">) => {
      const accepted = await job.accept();
      setTracked((previous) => ({ ...previous, [accepted.job_id]: { name: job.name, voices: job.voices, model: job.model, job: null } }));
      void invalidate(queryKeys.downloads.list());
    },
  });
  const installVoices = (ids: readonly string[]) => start.mutate({ accept: () => api.call("kokoroInstallVoices", { voice_ids: [...ids] }), name: ids.map(kokoroVoiceName).join(", "), voices: ids, model: false });
  const removeBlend = useMutation({ mutationFn: async (id: string) => { await api.call("userVoiceDelete", { voice_id: id }); await invalidate(queryKeys.voices.user()); } });
  const askRemoveBlend = async (id: string) => {
    if (await confirm({ title: t("voices.blend.delete"), message: t("voices.blend.deleteConfirm"), confirmLabel: t("voices.blend.delete"), cancelLabel: common("actions.cancel"), destructive: true })) removeBlend.mutate(id);
  };
  const options = [
    ...installedIds.map((id) => ({ value: id, label: kokoroVoiceName(id) })),
    ...blends.map((voice) => ({ value: voice.id, label: voice.name })),
  ];
  const selectedChoice = options.some((entry) => entry.value === choice) ? choice : options[0]?.value ?? null;
  const speak = () => {
    if (variant === null || selectedChoice === null) return;
    const blend = blends.find((voice) => voice.id === selectedChoice);
    void synthesis.run({ provider_id: provider.id, model_id: variant, voice_id: blend ? blend.voice_id : selectedChoice, prompt: serializeSpeed(speed), text });
  };
  const previewBlend = (voice: { voice_id: string; prompt: string | null }) => {
    if (variant === null) return;
    void synthesis.run({ provider_id: provider.id, model_id: variant, voice_id: voice.voice_id, prompt: voice.prompt, text: t("voices.kokoro.tryText") });
  };
  const closeEngine = () => close({ kokoro: provider.id });
  const jobCard = ([id, entry]: [string, Tracked]) => <DownloadJob key={id} api={api} id={id} name={entry.name} onUpdate={updateJob} onDone={settled} />;
  const ready = t("voices.kokoro.ready", { variant: variantList.find((entry) => entry.id === variant)?.label ?? variant ?? "", count: installedIds.length });
  return (
    <>
      <Page title={provider.label} meta={phone ? undefined : t("voices.kokoro.studio")} onBack={() => close()} backLabel={t("sections.voices.title")}>
        {failed && <FailureState error={failed.error} onRetry={() => void failed.refetch()} />}
        {start.isError && <FailureState error={start.error} size="sm" />}
        {removeBlend.isError && <FailureState error={removeBlend.error} size="sm" />}
        {loading ? (
          <VoicesSkeleton top={1} columns={[[4, 2]]} aside={<LibrarySkeleton />} />
        ) : !failed && (
          <div className="@container space-y-7">
            <ListGroup label={t("voices.kokoro.engine")}>
              {checking
                ? <ListRowButton icon={<Cpu size={icon.lg} />} title={t("voices.kokoro.engineTitle")} description={<Skeleton className="h-4 w-40 max-w-full" />} disabled />
                : installed
                ? <ListRowButton icon={<Cpu size={icon.lg} />} title={t("voices.kokoro.engineTitle")} description={ready} onClick={() => open({ kokoro: provider.id, kokoroEngine: true })} />
                : <ListRowButton icon={<Download size={icon.lg} />} title={t("voices.kokoro.setup")} description={t("voices.kokoro.setupBody")} onClick={onSetup} />}
            </ListGroup>
            {Object.entries(tracked).length > 0 && <div className="space-y-3">{Object.entries(tracked).map(jobCard)}</div>}
            {checking && <VoicesSkeleton columns={[[4, 2]]} aside={<LibrarySkeleton />} />}
            {installed && (
              <div className="grid grid-cols-1 gap-7 @4xl:grid-cols-2 @4xl:items-start @4xl:gap-x-10">
                <div className="min-w-0 space-y-7">
                  <ListGroup label={t("voices.kokoro.tryIt")}>
                    {options.length === 0 ? <ListRow title={t("voices.kokoro.needVoice")} /> : (
                      <>
                        <div className="p-4">
                          <Field label={<span className="sr-only">{t("voices.kokoro.tryIt")}</span>}>
                            <Textarea rows={3} value={text} onChange={(event) => setText(event.target.value)} />
                          </Field>
                        </div>
                        <ListRow title={t("voices.kokoro.voice")} trailing={
                          <Field label={<span className="sr-only">{t("voices.kokoro.voice")}</span>} className="w-52 shrink-0 space-y-0">
                            <SelectMenu options={options} value={selectedChoice} onChange={setChoice} />
                          </Field>
                        } />
                        <ListRow title={t("voices.kokoro.speed")} trailing={
                          <span className="w-52 shrink-0"><Slider aria-label={t("voices.kokoro.speed")} min={speedRange.min} max={speedRange.max} step={speedRange.step} value={speed} onChange={setSpeed} showValue={speedText} /></span>
                        } />
                        <div className="p-4">
                          <Button variant="primary" leading={<Play size={icon.sm} />} loading={synthesis.busy} disabled={!text.trim()} onClick={speak}>{t("voices.kokoro.speak")}</Button>
                        </div>
                      </>
                    )}
                  </ListGroup>
                  {synthesis.error !== null && <FailureState error={synthesis.error} size="sm" />}
                  {synthesis.asset && <AssetAudio src={synthesis.asset.url} title={t("voices.kokoro.tryIt")} />}
                  <ListGroup label={t("voices.kokoro.blends")}>
                    {blends.map((voice) => {
                      const edit = () => open({ kokoro: provider.id, kokoroBlend: voice.id });
                      return <ActionRow key={voice.id} icon={<AudioLines size={icon.lg} />} title={voice.name}
                        description={t("voices.kokoro.blendDescription", { count: parseBlend(voice.voice_id).length, speed: Number(parseSpeed(voice.prompt).toFixed(2)) })}
                        onClick={edit} menuLabel={t("voices.actions.more")}
                        actions={[
                          { label: t("voices.actions.preview"), icon: <Play size={icon.md} />, onSelect: () => previewBlend(voice) },
                          { label: t("voices.actions.edit"), icon: <Pencil size={icon.md} />, onSelect: edit },
                          { label: t("voices.actions.delete"), icon: <Trash2 size={icon.md} />, danger: true, onSelect: () => void askRemoveBlend(voice.id) },
                        ]} />;
                    })}
                    <ListRowButton icon={<Plus size={icon.lg} />} title={t("voices.kokoro.newBlend")} chevron={false} disabled={installedIds.length === 0} onClick={() => open({ kokoro: provider.id, kokoroBlend: "new" })} />
                  </ListGroup>
                </div>
                <div className="min-w-0">
                  {variant !== null && <KokoroLibrary api={api} providerId={provider.id} variant={variant} available={available} installing={installingVoices} onInstall={installVoices} />}
                </div>
              </div>
            )}
          </div>
        )}
      </Page>
      <Sheet open={engineOpen && installed} onClose={closeEngine} placement={phone ? "bottom" : "side"} {...(phone ? { detents: [0.7, 0.95] } : {})} title={t("voices.kokoro.engineTitle")}
        {...(variant !== null ? { footer: <EngineDelete api={api} variant={variant} onDeleted={closeEngine} /> } : {})}>
        <div className="space-y-5">
          <EngineDetails inventory={inventory.data} label={variantList.find((entry) => entry.id === variant)?.label ?? variant ?? ""} language={i18n.language} />
          <ListGroup>
            <ListRowButton icon={<Cpu size={icon.lg} />} title={t("voices.kokoro.changeQuality")} description={t("voices.kokoro.changeQualityBody")} onClick={() => { closeEngine(); onSetup(); }} />
          </ListGroup>
        </div>
      </Sheet>
    </>
  );
}

function EngineDetails({ inventory, label, language }: { inventory: KokoroInventory | undefined; label: string; language: string }) {
  const { t } = useTranslation("settings");
  const model = [inventory?.model, inventory?.config, inventory?.tokenizer, inventory?.tokenizer_config].reduce((sum, artifact) => sum + (artifact?.byte_size ?? 0), 0);
  const voices = (inventory?.installed_voices ?? []).reduce((sum, voice) => sum + voice.artifact.byte_size, 0);
  return (
    <KeyValue items={[
      { label: t("voices.kokoro.variant"), value: label },
      { label: t("voices.kokoro.status"), value: t(kokoroInstalled(inventory) ? "voices.kokoro.installed" : "voices.kokoro.notInstalled") },
      { label: t("voices.kokoro.modelSize"), value: fileSize(model, language) },
      { label: t("voices.kokoro.voicesSize"), value: fileSize(voices, language) },
      { label: t("voices.kokoro.total"), value: fileSize(model + voices, language) },
    ]} />
  );
}

function EngineDelete({ api, variant, onDeleted }: { api: ApiClient; variant: string; onDeleted: () => void }) {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation();
  const invalidate = useInvalidate();
  const remove = useMutation({
    mutationFn: async () => { await api.call("kokoroUninstallModel", { variant }); await invalidate(queryKeys.voices.kokoroInventory(variant), queryKeys.voices.kokoroAvailable()); },
    onSuccess: onDeleted,
  });
  const ask = async () => {
    if (await confirm({ title: t("voices.kokoro.deleteModel"), message: t("voices.kokoro.deleteModelConfirm"), confirmLabel: t("voices.kokoro.deleteModel"), cancelLabel: common("actions.cancel"), destructive: true })) remove.mutate();
  };
  return (
    <div className="space-y-3">
      {remove.isError && <FailureState error={remove.error} size="sm" />}
      <Button variant="danger" loading={remove.isPending} onClick={() => void ask()}>{t("voices.kokoro.deleteModel")}</Button>
    </div>
  );
}
