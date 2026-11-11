import { useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useNavigate, useSearch } from "@tanstack/react-router";
import { AudioLines, Cpu, HardDrive, Pencil, Play, Plus, Trash2, Volume2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, confirm, fileSize, icon, KeyValue, ListGroup, ListRow, ListRowButton, Page, Sheet, Skeleton, Text, toast } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { AudioProviderView, AudioVoiceView, UserVoiceView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { AssetAudio } from "@/shared/ui/AssetAudio";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi, useSettingsLayout } from "../data";
import { ActionRow } from "./ActionRow";
import { KokoroBlendEditor } from "./KokoroBlendEditor";
import { KokoroSetupSheet } from "./KokoroSetupSheet";
import { KokoroStudio } from "./KokoroStudio";
import { VoiceEditorSheet, type VoicePrefill } from "./VoiceEditorSheet";
import { audioProvidersQuery, cacheStatsQuery, isCustomVoice, isKokoro, kokoroInventoryQuery, kokoroInstalled, kokoroVariantOf, kokoroVariantsQuery, kokoroVoiceName, parseBlend, providerVoiceLimit, useInvalidate, useProviderVoices, useSynthesis, useVoicesNavigation, userVoicesQuery } from "./data";
import { VoicesSkeleton } from "./VoicesSkeleton";

const promptPreviewLength = 40;

export function VoicesPage() {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation();
  const api = useSettingsApi();
  const { back, phone } = useSettingsLayout();
  const { open, close } = useVoicesNavigation();
  const navigate = useNavigate();
  const invalidate = useInvalidate();
  const search = useSearch({ from: "/settings/$section" });
  const providers = useQuery(audioProvidersQuery(api));
  const voices = useQuery(userVoicesQuery(api));
  const providerVoices = useProviderVoices(api, providers.data ?? []);
  const synthesis = useSynthesis(api);
  const [previewName, setPreviewName] = useState("");
  const [prefill, setPrefill] = useState<VoicePrefill | null>(null);
  const [setup, setSetup] = useState<{ id: string; open: boolean } | null>(null);
  const openSetup = (id: string) => setSetup({ id, open: true });
  const remove = useMutation({ mutationFn: (run: () => Promise<unknown>) => run() });
  const setupKokoro = useMutation({
    mutationFn: async () => {
      const created = await api.call("audioProviderCreate", { client_operation_id: crypto.randomUUID(), label: t("providers.audioKinds.kokoro"), draft: { configuration: { type: "kokoro", variant: null }, api_key: null } });
      await invalidate(queryKeys.audioProviders.all);
      return created;
    },
    onSuccess: (created) => openSetup(created.id),
  });
  const failed = [providers, voices].find((query) => query.isError);
  const ready = providers.data && voices.data;
  const providerList = providers.data ?? [];
  const cloudProviders = providerList.filter((provider) => !isKokoro(provider));
  const kokoroProviders = providerList.filter(isKokoro);
  const providerOf = (id: string) => providerList.find((provider) => provider.id === id);
  const cloudVoices = (voices.data ?? []).filter((voice) => { const provider = providerOf(voice.provider_id); return provider && !isKokoro(provider); });
  const kokoroVoices = (voices.data ?? []).filter((voice) => { const provider = providerOf(voice.provider_id); return provider && isKokoro(provider); });
  const customVoices = providerVoices.filter(({ provider }) => provider.configuration.type !== "gemini").flatMap(({ provider, query }) => (query.data ?? []).filter(isCustomVoice).map((voice) => ({ provider, voice })));
  const studio = search.kokoro ? kokoroProviders.find((provider) => provider.id === search.kokoro) : undefined;
  const editing = search.voiceEditor && search.voiceEditor !== "new" ? cloudVoices.find((voice) => voice.id === search.voiceEditor) : undefined;
  const sheetVoice = search.voiceProvider && search.voiceItem ? providerVoices.find(({ provider }) => provider.id === search.voiceProvider)?.query.data?.find((voice) => voice.voice_id === search.voiceItem) : undefined;
  const sheetProvider = providerOf(search.voiceProvider ?? "");
  const cut = (prompt: string) => prompt.length > promptPreviewLength ? `${prompt.slice(0, promptPreviewLength).trimEnd()}…` : prompt;
  const preview = (voice: UserVoiceView) => {
    setPreviewName(voice.name);
    void synthesis.run({ provider_id: voice.provider_id, model_id: voice.model_id, voice_id: voice.voice_id, prompt: voice.prompt, text: t("voices.editor.defaultSample") });
  };
  const askDelete = async (voice: UserVoiceView, title: string, message: string) => {
    if (!await confirm({ title, message, confirmLabel: title, cancelLabel: common("actions.cancel"), destructive: true })) return;
    remove.mutate(async () => { await api.call("userVoiceDelete", { voice_id: voice.id }); await invalidate(queryKeys.voices.user()); });
  };
  const create = () => {
    const firstKokoro = kokoroProviders[0];
    if (cloudProviders.length === 0) {
      if (firstKokoro) open({ kokoro: firstKokoro.id, kokoroBlend: "new" });
      else setupKokoro.mutate();
    }
    else open({ voiceEditor: "new" });
  };
  const closeEditor = () => { setPrefill(null); close(); };
  const noRows = cloudVoices.length + kokoroVoices.length + customVoices.length === 0;
  return (
    <>
      <div className={studio ? "hidden" : "h-full min-h-0"}>
        <Page title={t("sections.voices.title")} {...back}>
          {remove.isError && <FailureState error={remove.error} />}
          {failed && <FailureState error={failed.error} onRetry={() => void failed.refetch()} />}
          {!ready ? !failed && (
            <VoicesSkeleton columns={[[3], [1, 2]]} />
          ) : (
            <div className="@container">
              {providerList.length === 0 && (
                <ListGroup className="mb-7">
                  <ListRowButton icon={<AudioLines size={icon.lg} />} title={t("voices.noProviders.title")} description={t("voices.noProviders.body")} onClick={() => void navigate({ to: "/settings/$section", params: { section: "providers" }, search: { providersTab: "audio" } })} />
                </ListGroup>
              )}
              <div className="grid grid-cols-1 gap-7 @4xl:grid-cols-2 @4xl:items-start @4xl:gap-x-10">
                {providerList.length > 0 && (
                  <div className="min-w-0 space-y-7">
                    <ListGroup label={t("voices.mine.group")}>
                      {cloudVoices.map((voice) => {
                        const provider = providerOf(voice.provider_id);
                        return <ActionRow key={voice.id} icon={<Volume2 size={icon.lg} />} title={voice.name}
                          description={t("voices.mine.description", { provider: provider?.label ?? "", prompt: voice.prompt?.trim() ? cut(voice.prompt.trim()) : t("voices.mine.noPrompt") })}
                          onClick={() => open({ voiceEditor: voice.id })} menuLabel={t("voices.actions.more")}
                          actions={[
                            { label: t("voices.actions.preview"), icon: <Play size={icon.md} />, onSelect: () => preview(voice) },
                            { label: t("voices.actions.edit"), icon: <Pencil size={icon.md} />, onSelect: () => open({ voiceEditor: voice.id }) },
                            { label: t("voices.actions.delete"), icon: <Trash2 size={icon.md} />, danger: true, onSelect: () => void askDelete(voice, t("voices.editor.delete"), t("voices.editor.deleteConfirm")) },
                          ]} />;
                      })}
                      {kokoroVoices.map((voice) => {
                        const blend = parseBlend(voice.voice_id);
                        const edit = () => open({ kokoro: voice.provider_id, kokoroBlend: voice.id });
                        return <ActionRow key={voice.id} icon={<Cpu size={icon.lg} />} title={voice.name}
                          description={blend.length > 1 ? t("voices.mine.kokoroBlend", { count: blend.length }) : t("voices.mine.kokoroSingle", { voice: kokoroVoiceName(blend[0]?.id ?? voice.voice_id) })}
                          onClick={edit} menuLabel={t("voices.actions.more")}
                          actions={[
                            { label: t("voices.actions.preview"), icon: <Play size={icon.md} />, onSelect: () => preview(voice) },
                            { label: t("voices.actions.edit"), icon: <Pencil size={icon.md} />, onSelect: edit },
                            { label: t("voices.actions.delete"), icon: <Trash2 size={icon.md} />, danger: true, onSelect: () => void askDelete(voice, t("voices.blend.delete"), t("voices.blend.deleteConfirm")) },
                          ]} />;
                      })}
                      {customVoices.map(({ provider, voice }) => (
                        <ListRowButton key={`${provider.id}:${voice.voice_id}`} icon={<Volume2 size={icon.lg} />} title={voice.name}
                          description={t("voices.mine.customDescription", { provider: provider.label, category: voice.labels.category ?? "" })}
                          onClick={() => open({ voiceProvider: provider.id, voiceItem: voice.voice_id })} />
                      ))}
                      <ListRowButton icon={<Plus size={icon.lg} />} title={t("voices.mine.create")} description={t(noRows ? "voices.mine.emptyBody" : "voices.mine.createBody")} chevron={false} onClick={create} />
                    </ListGroup>
                    {(synthesis.busy || synthesis.asset || synthesis.error !== null) && (
                      <div className="space-y-2">
                        {synthesis.error !== null && <FailureState error={synthesis.error} size="sm" />}
                        {synthesis.busy ? <Text variant="hint" className="px-1">{t("voices.editor.previewing")}</Text> : synthesis.asset && <AssetAudio src={synthesis.asset.url} title={previewName} />}
                      </div>
                    )}
                  </div>
                )}
                <div className="min-w-0 space-y-7">
                  {kokoroProviders.map((provider) => <KokoroEntry key={provider.id} api={api} provider={provider} onOpen={(installed) => installed ? open({ kokoro: provider.id }) : openSetup(provider.id)} />)}
                  {kokoroProviders.length === 0 && (
                    <ListGroup label={t("providers.audioKinds.kokoro")}>
                      <ListRowButton icon={<Cpu size={icon.lg} />} title={t("voices.kokoro.studio")} description={t("voices.kokoro.onDevice")} disabled={setupKokoro.isPending} onClick={() => setupKokoro.mutate()} />
                      {setupKokoro.isError && <div className="p-3"><FailureState error={setupKokoro.error} size="sm" onRetry={() => setupKokoro.mutate()} /></div>}
                    </ListGroup>
                  )}
                  {providerVoices.map(({ provider, query }) => <ProviderVoices key={provider.id} api={api} provider={provider} query={query} />)}
                  <CacheGroup api={api} />
                </div>
              </div>
            </div>
          )}
        </Page>
      </div>
      {studio && <div className={search.kokoroBlend ? "hidden" : "h-full min-h-0"}><KokoroStudio api={api} provider={studio} onSetup={() => openSetup(studio.id)} /></div>}
      {studio && search.kokoroBlend && <KokoroBlendEditor key={search.kokoroBlend} api={api} provider={studio} blendId={search.kokoroBlend} />}
      {ready && search.voiceEditor && (search.voiceEditor === "new" || editing) && cloudProviders.length > 0 && (
        <VoiceEditorSheet key={`${search.voiceEditor}:${prefill?.voiceId ?? ""}`} api={api} providers={cloudProviders} voice={editing ?? null} prefill={prefill} onClose={closeEditor} />
      )}
      {setup && kokoroProviders.some((provider) => provider.id === setup.id) && <KokoroSetupSheet key={setup.id} api={api} provider={kokoroProviders.find((provider) => provider.id === setup.id)!} open={setup.open} phone={phone} onClose={() => setSetup((current) => current && { ...current, open: false })} />}
      {sheetVoice && sheetProvider && <ProviderVoiceSheet provider={sheetProvider} voice={sheetVoice} onClose={() => close()} onUse={() => { setPrefill({ providerId: sheetProvider.id, name: sheetVoice.name, voiceId: sheetVoice.voice_id }); open({ voiceEditor: "new" }, true); }} />}
    </>
  );
}

function KokoroEntry({ api, provider, onOpen }: { api: ApiClient; provider: AudioProviderView; onOpen: (installed: boolean) => void }) {
  const { t } = useTranslation("settings");
  const variant = kokoroVariantOf(provider);
  const variants = useQuery(kokoroVariantsQuery(api));
  const inventory = useQuery(kokoroInventoryQuery(api, variant));
  const pending = variant !== null && (inventory.isPending || variants.isPending);
  const label = variants.data?.find((item) => item.id === variant)?.label ?? variant ?? "";
  const installed = kokoroInstalled(inventory.data);
  return (
    <ListGroup label={provider.label}>
      <ListRowButton icon={<Cpu size={icon.lg} />} title={t("voices.kokoro.studio")}
        description={pending ? <Skeleton className="h-4 w-40 max-w-full" /> : installed ? t("voices.kokoro.ready", { variant: label, count: inventory.data?.installed_voices.length ?? 0 }) : t("voices.kokoro.notSetUp")}
        onClick={() => onOpen(installed)} disabled={pending} />
    </ListGroup>
  );
}

function ProviderVoices({ api, provider, query }: { api: ApiClient; provider: AudioProviderView; query: ReturnType<typeof useProviderVoices>[number]["query"] }) {
  const { t } = useTranslation("settings");
  const invalidate = useInvalidate();
  const [showAll, setShowAll] = useState(false);
  const refresh = useMutation({ mutationFn: async () => { await api.call("audioProviderVoicesRefresh", { provider_id: provider.id }); await invalidate(queryKeys.voices.providerVoices(provider.id)); } });
  const premade = (query.data ?? []).filter((voice) => !isCustomVoice(voice));
  const shown = showAll ? premade : premade.slice(0, providerVoiceLimit);
  const detail = (voice: AudioVoiceView) => voice.labels.category || voice.labels.description || voice.labels.gender;
  return (
    <ListGroup label={t("voices.providerVoices.group")}>
      <ListRow title={provider.label} description={query.isPending ? <Skeleton className="h-4 w-20" /> : t("voices.providerVoices.count", { count: premade.length })}
        trailing={provider.configuration.type === "gemini" ? undefined : <Button size="sm" variant="ghost" loading={refresh.isPending} onClick={() => refresh.mutate()}>{t("voices.providerVoices.refresh")}</Button>} />
      {query.isError && <div className="p-3"><FailureState error={query.error} size="sm" onRetry={() => void query.refetch()} /></div>}
      {refresh.isError && <div className="p-3"><FailureState error={refresh.error} size="sm" /></div>}
      {query.isSuccess && premade.length === 0 && <ListRow title={t("voices.providerVoices.empty")} />}
      {shown.map((voice) => <ListRow key={voice.voice_id} title={voice.name} {...(detail(voice) ? { description: detail(voice) } : {})} />)}
      {premade.length > providerVoiceLimit && <ListRowButton title={t(showAll ? "voices.providerVoices.showLess" : "voices.providerVoices.showAll", { count: premade.length })} chevron={false} onClick={() => setShowAll((value) => !value)} />}
    </ListGroup>
  );
}

function CacheGroup({ api }: { api: ApiClient }) {
  const { t, i18n } = useTranslation("settings");
  const stats = useQuery(cacheStatsQuery(api));
  const invalidate = useInvalidate();
  const clear = useMutation({
    mutationFn: async () => { await api.call("ttsCacheClear"); await invalidate(queryKeys.voices.cache()); },
    onSuccess: () => { toast.success(t("voices.cache.cleared")); },
  });
  return (
    <ListGroup label={t("voices.cache.group")}>
      <ListRow icon={<HardDrive size={icon.lg} />} title={t("voices.cache.title")} description={t("voices.cache.body")}
        trailing={stats.data ? t("voices.cache.stats", { count: stats.data.count, size: fileSize(stats.data.size_bytes, i18n.language) }) : stats.isPending ? <Skeleton className="h-4 w-16" /> : undefined} />
      {stats.isError && <div className="p-3"><FailureState error={stats.error} size="sm" onRetry={() => void stats.refetch()} /></div>}
      {clear.isError && <div className="p-3"><FailureState error={clear.error} size="sm" /></div>}
      <ListRowButton icon={<Trash2 size={icon.lg} />} tone="danger" title={t("voices.cache.clear")} chevron={false} disabled={!stats.data?.count || clear.isPending} onClick={() => clear.mutate()} />
    </ListGroup>
  );
}

function ProviderVoiceSheet({ provider, voice, onClose, onUse }: { provider: AudioProviderView; voice: AudioVoiceView; onClose: () => void; onUse: () => void }) {
  const { t } = useTranslation("settings");
  const items = [
    { label: t("voices.providerVoice.provider"), value: provider.label },
    { label: t("voices.providerVoice.category"), value: voice.labels.category ?? "" },
    ...(voice.labels.description ? [{ label: t("voices.providerVoice.description"), value: voice.labels.description }] : []),
  ];
  return (
    <Sheet open onClose={onClose} size="sm" title={voice.name} footer={<Button variant="primary" block onClick={onUse}>{t("voices.providerVoice.use")}</Button>}>
      <KeyValue items={items} />
    </Sheet>
  );
}
