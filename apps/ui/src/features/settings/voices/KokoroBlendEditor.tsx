import { useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useSearch } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Play, X } from "lucide-react";
import { ActionBar, Button, confirm, Field, icon, IconButton, Input, ListGroup, ListRow, Page, SelectMenu, Slider, Textarea } from "@lettuceai/crisp";
import { ApiFailure } from "@/api/client";
import type { ApiClient } from "@/api/client";
import type { AudioProviderView, UserVoiceView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { AssetAudio } from "@/shared/ui/AssetAudio";
import { FailureState } from "@/shared/ui/FailureState";
import { kokoroInventoryQuery, kokoroVariantOf, kokoroVoiceName, parseBlend, parseSpeed, serializeBlend, serializeSpeed, speedRange, speedText, useInvalidate, useOperationId, useSynthesis, useVoicesNavigation, userVoicesQuery, type BlendVoice } from "./data";
import { VoicesSkeleton } from "./VoicesSkeleton";

export function KokoroBlendEditor({ api, provider, blendId }: { api: ApiClient; provider: AudioProviderView; blendId: string }) {
  const { t } = useTranslation("settings");
  const { close } = useVoicesNavigation();
  const variant = kokoroVariantOf(provider);
  const voices = useQuery(userVoicesQuery(api));
  const inventory = useQuery(kokoroInventoryQuery(api, variant));
  const existing = blendId === "new" ? null : voices.data?.find((voice) => voice.id === blendId && voice.provider_id === provider.id) ?? null;
  const failed = [voices, ...(variant === null ? [] : [inventory])].find((query) => query.isError);
  const ready = voices.data && (variant === null || inventory.data);
  const title = t(blendId === "new" ? "voices.kokoro.newBlend" : "voices.kokoro.editBlend");
  if (ready && !failed && blendId !== "new" && !existing) {
    return <Page title={title} onBack={() => close({ kokoro: provider.id })} backLabel={t("voices.kokoro.studio")}><FailureState error={new ApiFailure("not_found", "blend not found", null)} /></Page>;
  }
  if (!ready || failed) {
    return (
      <Page title={title} onBack={() => close({ kokoro: provider.id })} backLabel={t("voices.kokoro.studio")}>
        {failed ? <FailureState error={failed.error} onRetry={() => void failed.refetch()} /> : (
          <VoicesSkeleton columns={[[1, 3], [1, 2]]} />
        )}
      </Page>
    );
  }
  return <BlendForm api={api} provider={provider} variant={variant ?? ""} voice={existing} installed={inventory.data?.installed_voices.map((entry) => entry.id) ?? []} title={title} />;
}

function BlendForm({ api, provider, variant, voice, installed, title }: { api: ApiClient; provider: AudioProviderView; variant: string; voice: UserVoiceView | null; installed: readonly string[]; title: string }) {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation();
  const { close } = useVoicesNavigation();
  const invalidate = useInvalidate();
  const operationId = useOperationId();
  const synthesis = useSynthesis(api);
  const [name, setName] = useState(voice?.name ?? "");
  const startVoice = useSearch({ from: "/settings/$section", select: (search) => search.kokoroBlendVoice });
  const [blend, setBlend] = useState<BlendVoice[]>(() => {
    if (voice) return parseBlend(voice.voice_id);
    const first = startVoice && installed.includes(startVoice) ? startVoice : installed[0];
    return first ? [{ id: first, weight: 100 }] : [];
  });
  const [speed, setSpeed] = useState(() => parseSpeed(voice?.prompt ?? null));
  const [text, setText] = useState(() => t("voices.kokoro.tryText"));
  const total = blend.reduce((sum, entry) => sum + entry.weight, 0);
  const addable = installed.filter((id) => !blend.some((entry) => entry.id === id));
  const canSave = name.trim() !== "" && total > 0;
  const setWeight = (id: string, weight: number) => setBlend((previous) => previous.map((entry) => entry.id === id ? { ...entry, weight } : entry));
  const save = useMutation({
    mutationFn: async () => {
      const fields = { provider_id: provider.id, name: name.trim(), model_id: variant, voice_id: serializeBlend(blend), prompt: serializeSpeed(speed) };
      if (voice) await api.call("userVoiceUpdate", { ...fields, id: voice.id, expected_revision: voice.revision });
      else await api.call("userVoiceCreate", { ...fields, client_operation_id: operationId(fields) });
      await invalidate(queryKeys.voices.user());
    },
    onSuccess: () => close({ kokoro: provider.id }),
  });
  const remove = useMutation({
    mutationFn: async () => {
      if (!voice) return;
      await api.call("userVoiceDelete", { voice_id: voice.id });
      await invalidate(queryKeys.voices.user());
    },
    onSuccess: () => close({ kokoro: provider.id }),
  });
  const askDelete = async () => {
    if (await confirm({ title: t("voices.blend.delete"), message: t("voices.blend.deleteConfirm"), confirmLabel: t("voices.blend.delete"), cancelLabel: common("actions.cancel"), destructive: true })) remove.mutate();
  };
  const speak = () => { void synthesis.run({ provider_id: provider.id, model_id: variant, voice_id: serializeBlend(blend), prompt: serializeSpeed(speed), text }); };
  const busy = save.isPending || remove.isPending;
  return (
    <Page title={title} onBack={() => close({ kokoro: provider.id })} backLabel={t("voices.kokoro.studio")}
      footer={
        <ActionBar
          primary={<Button variant="primary" loading={save.isPending} disabled={!canSave || remove.isPending} onClick={() => save.mutate()}>{t("voices.blend.save")}</Button>}
          secondary={voice ? <Button variant="danger" disabled={busy} onClick={() => void askDelete()}>{t("voices.blend.delete")}</Button> : undefined}
        />
      }>
      {save.isError && <FailureState error={save.error} size="sm" />}
      {remove.isError && <FailureState error={remove.error} size="sm" />}
      <div className="@container">
        <div className="grid grid-cols-1 gap-7 @4xl:grid-cols-2 @4xl:items-start @4xl:gap-x-10">
          <div className="min-w-0 space-y-7">
            <ListGroup label={t("voices.blend.details")}>
              <ListRow title={t("voices.blend.name")} trailing={
                <Field label={<span className="sr-only">{t("voices.blend.name")}</span>} className="w-56 shrink-0 space-y-0">
                  <Input value={name} placeholder={t("voices.blend.namePlaceholder")} onChange={(event) => setName(event.target.value)} />
                </Field>
              } />
            </ListGroup>
            <ListGroup label={t("voices.blend.voices")}>
              {blend.map((entry) => (
                <ListRow key={entry.id} title={kokoroVoiceName(entry.id)} description={t("voices.blend.share", { percent: total > 0 ? Math.round((entry.weight / total) * 100) : 0 })}
                  trailing={
                    <span className="flex items-center gap-2">
                      <span className="w-36"><Slider aria-label={kokoroVoiceName(entry.id)} min={0} max={100} step={1} value={entry.weight} onChange={(weight) => setWeight(entry.id, weight)} /></span>
                      <IconButton label={t("voices.blend.remove")} size="sm" disabled={blend.length <= 1} onClick={() => setBlend((previous) => previous.filter((item) => item.id !== entry.id))}><X size={icon.md} /></IconButton>
                    </span>
                  } />
              ))}
              <ListRow title={t("voices.blend.add")} trailing={
                <Field label={<span className="sr-only">{t("voices.blend.add")}</span>} className="w-52 shrink-0 space-y-0">
                  <SelectMenu options={addable.map((id) => ({ value: id, label: kokoroVoiceName(id) }))} value={null} placeholder={t("voices.blend.addPlaceholder")} disabled={addable.length === 0} onChange={(id) => { if (id) setBlend((previous) => [...previous, { id, weight: 50 }]); }} />
                </Field>
              } />
            </ListGroup>
          </div>
          <div className="min-w-0 space-y-7">
            <ListGroup label={t("voices.kokoro.speed")}>
              <ListRow title={<Slider aria-label={t("voices.kokoro.speed")} min={speedRange.min} max={speedRange.max} step={speedRange.step} value={speed} onChange={setSpeed} showValue={speedText} />}
                trailing={speed !== 1 ? <Button size="sm" variant="ghost" onClick={() => setSpeed(1)}>{t("voices.blend.resetSpeed")}</Button> : undefined} />
            </ListGroup>
            <ListGroup label={t("voices.blend.preview")}>
              <div className="space-y-3 p-4">
                <Field label={<span className="sr-only">{t("voices.blend.preview")}</span>}>
                  <Textarea rows={3} value={text} onChange={(event) => setText(event.target.value)} />
                </Field>
                <Button variant="secondary" leading={<Play size={icon.sm} />} loading={synthesis.busy} disabled={!text.trim() || total === 0} onClick={speak}>{t("voices.kokoro.speak")}</Button>
                {synthesis.error !== null && <FailureState error={synthesis.error} size="sm" />}
                {synthesis.asset && <AssetAudio src={synthesis.asset.url} title={name.trim() || t("voices.blend.preview")} />}
              </div>
            </ListGroup>
          </div>
        </div>
      </div>
    </Page>
  );
}
