import { useState } from "react";
import { useMutation, type UseQueryResult } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Check, Download, Loader2, Play, Plus, Trash2 } from "lucide-react";
import { Button, Chip, confirm, fileSize, icon, ListGroup, ListRow, Menu, MenuContent, MenuItem, MenuLabel, MenuTrigger, SearchField, Skeleton } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { KokoroAvailableVoiceView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { AssetAudio } from "@/shared/ui/AssetAudio";
import { FailureState } from "@/shared/ui/FailureState";
import { kokoroGenderOf, kokoroLanguageName, kokoroLanguageOf, kokoroLanguages, kokoroVoiceName, type KokoroLanguage, serializeSpeed, useInvalidate, useSynthesis, useVoicesNavigation } from "./data";
import { LibrarySkeleton } from "./VoicesSkeleton";

/** The voice catalog as wrapping chips, one block per language split by gender; each chip opens its own actions. */
export function KokoroLibrary({ api, providerId, variant, available, installing, onInstall }: { api: ApiClient; providerId: string; variant: string; available: UseQueryResult<KokoroAvailableVoiceView[]>; installing: ReadonlySet<string>; onInstall: (ids: readonly string[]) => void }) {
  const { t, i18n } = useTranslation("settings");
  const { t: common } = useTranslation();
  const { open } = useVoicesNavigation();
  const invalidate = useInvalidate();
  const synthesis = useSynthesis(api);
  const [query, setQuery] = useState("");
  const [previewName, setPreviewName] = useState("");
  const remove = useMutation({
    mutationFn: async (id: string) => { await api.call("kokoroUninstallVoice", { voice_id: id }); await invalidate(queryKeys.voices.kokoroInventory(variant), queryKeys.voices.kokoroAvailable()); },
  });
  const languageLabel = (language: KokoroLanguage) => kokoroLanguageName(language, i18n.language) ?? t("voices.kokoro.otherLanguage");
  const needle = query.trim().toLowerCase();
  const matching = (available.data ?? []).filter((voice) => !needle || kokoroVoiceName(voice.id).toLowerCase().includes(needle) || voice.id.toLowerCase().includes(needle) || languageLabel(kokoroLanguageOf(voice.id)).toLowerCase().includes(needle));
  const missing = matching.filter((voice) => !voice.installed && !installing.has(voice.id));
  const languages = [...kokoroLanguages, "other" as const].map((language) => ({ language, voices: matching.filter((voice) => kokoroLanguageOf(voice.id) === language) })).filter((entry) => entry.voices.length > 0);
  const preview = (id: string) => {
    setPreviewName(kokoroVoiceName(id));
    void synthesis.run({ provider_id: providerId, model_id: variant, voice_id: id, prompt: serializeSpeed(1), text: t("voices.kokoro.tryText") });
  };
  const askRemove = async (id: string) => {
    if (await confirm({ title: t("voices.kokoro.removeVoice"), message: t("voices.kokoro.removeVoiceConfirm"), confirmLabel: t("voices.kokoro.removeVoice"), cancelLabel: common("actions.cancel"), destructive: true })) remove.mutate(id);
  };
  const downloadAll = async () => {
    const size = fileSize(missing.reduce((sum, voice) => sum + voice.byte_size, 0), i18n.language);
    if (await confirm({ title: t("voices.kokoro.downloadAll", { count: missing.length }), message: t("voices.kokoro.downloadAllConfirm", { count: missing.length, size }), confirmLabel: t("voices.kokoro.downloadAll", { count: missing.length }), cancelLabel: common("actions.cancel") })) onInstall(missing.map((voice) => voice.id));
  };
  const chip = (voice: KokoroAvailableVoiceView) => {
    const name = kokoroVoiceName(voice.id);
    const busy = installing.has(voice.id);
    return (
      <Menu key={voice.id}>
        <MenuTrigger>
          <Chip selected={voice.installed} disabled={busy} aria-label={t(voice.installed ? "voices.kokoro.chipInstalled" : "voices.kokoro.chipAvailable", { name })}
            icon={busy ? <Loader2 size={icon.sm} className="animate-spin" /> : voice.installed ? <Check size={icon.sm} /> : <Download size={icon.sm} className="text-fg-3" />}>
            {name}
          </Chip>
        </MenuTrigger>
        <MenuContent align="start">
          <MenuLabel>{t("voices.kokoro.chipMenuLabel", { name, size: fileSize(voice.byte_size, i18n.language) })}</MenuLabel>
          {voice.installed ? (
            <>
              <MenuItem icon={<Play size={icon.md} />} onSelect={() => preview(voice.id)}>{t("voices.actions.preview")}</MenuItem>
              <MenuItem icon={<Plus size={icon.md} />} onSelect={() => open({ kokoro: providerId, kokoroBlend: "new", kokoroBlendVoice: voice.id })}>{t("voices.kokoro.addToBlend")}</MenuItem>
              <MenuItem icon={<Trash2 size={icon.md} />} danger onSelect={() => void askRemove(voice.id)}>{t("voices.kokoro.removeVoice")}</MenuItem>
            </>
          ) : (
            <MenuItem icon={<Download size={icon.md} />} onSelect={() => onInstall([voice.id])}>{t("voices.kokoro.downloadVoice")}</MenuItem>
          )}
        </MenuContent>
      </Menu>
    );
  };
  if (available.isPending) return <LibrarySkeleton />;
  return (
    <div className="space-y-3">
      <ListGroup label={t("voices.kokoro.library")}>
        <ListRow title={<SearchField value={query} onChange={setQuery} placeholder={t("voices.kokoro.search")} aria-label={t("voices.kokoro.search")} />}
          trailing={missing.length > 0 ? <Button size="sm" variant="ghost" leading={<Download size={icon.sm} />} onClick={() => void downloadAll()}>{t("voices.kokoro.downloadAll", { count: missing.length })}</Button> : undefined} />
        {available.isError && <div className="p-3"><FailureState error={available.error} size="sm" onRetry={() => void available.refetch()} /></div>}
        {available.isSuccess && available.data.length === 0 && <ListRow title={t("voices.kokoro.emptyCatalog")} />}
        {available.isSuccess && available.data.length > 0 && matching.length === 0 && <ListRow title={t("voices.kokoro.noMatch")} />}
        {languages.map(({ language, voices }) => {
          const installedCount = voices.filter((voice) => voice.installed).length;
          return (
            <section key={language} className="space-y-2 px-4 py-3" aria-label={languageLabel(language)}>
              <div className="flex items-baseline justify-between gap-3">
                <span className="text-base font-medium text-fg">{languageLabel(language)}</span>
                <span className="text-sm text-fg-3">{t("voices.kokoro.languageCount", { count: voices.length, installed: installedCount })}</span>
              </div>
              {(["female", "male", "other"] as const).map((gender) => {
                const members = voices.filter((voice) => kokoroGenderOf(voice.id) === gender);
                if (members.length === 0) return null;
                return (
                  <div key={gender} className="flex gap-3">
                    {gender !== "other" && <span className="w-14 shrink-0 pt-1.5 text-sm text-fg-3">{t(`voices.kokoro.${gender}`)}</span>}
                    <div className="flex min-w-0 flex-1 flex-wrap gap-1.5">{members.map(chip)}</div>
                  </div>
                );
              })}
            </section>
          );
        })}
      </ListGroup>
      {remove.isError && <FailureState error={remove.error} size="sm" />}
      {synthesis.error !== null && <FailureState error={synthesis.error} size="sm" />}
      {synthesis.busy ? <Skeleton className="h-16 w-full" /> : synthesis.asset && <AssetAudio src={synthesis.asset.url} title={previewName} />}
    </div>
  );
}
