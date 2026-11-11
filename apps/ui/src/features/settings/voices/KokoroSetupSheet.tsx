import { useCallback, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Check, Download } from "lucide-react";
import { Badge, Button, icon, ListGroup, ListRowButton, Sheet } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import type { ApiClient } from "@/api/client";
import type { AudioProviderView, JobView, KokoroVariantView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { DownloadJob } from "@/entities/download";
import { terminalJobStates } from "@/entities/job";
import { FailureState } from "@/shared/ui/FailureState";
import { kokoroInstalled, kokoroInventoryQuery, kokoroVariantOf, kokoroVariantsQuery, useInvalidate } from "./data";

/** Picks the Kokoro quality and downloads the engine. Stays mounted while closed so a running download keeps its progress. */
export function KokoroSetupSheet({ api, provider, open, phone, onClose }: { api: ApiClient; provider: AudioProviderView; open: boolean; phone: boolean; onClose: () => void }) {
  const { t } = useTranslation("settings");
  const invalidate = useInvalidate();
  const status = useQuery(appStatusQuery(api));
  const variants = useQuery(kokoroVariantsQuery(api));
  const variant = kokoroVariantOf(provider);
  const inventory = useQuery(kokoroInventoryQuery(api, variant));
  const installed = kokoroInstalled(inventory.data);
  const [download, setDownload] = useState<{ id: string; name: string; job: JobView | null } | null>(null);
  const mobile = status.data?.platform === "android" || status.data?.platform === "ios";
  const list = (variants.data ?? []).filter((entry) => !mobile || entry.mobile_supported);
  const downloading = download !== null && (download.job === null || !terminalJobStates.has(download.job.state));
  const select = useMutation({
    mutationFn: async (next: string) => {
      await api.call("audioProviderUpdate", { provider_id: provider.id, expected_revision: provider.revision, label: provider.label, configuration: { type: "kokoro", variant: next } });
      await invalidate(queryKeys.audioProviders.all);
    },
  });
  const start = useMutation({
    mutationFn: async () => {
      if (variant === null) return;
      const accepted = await api.call("kokoroInstallModel", { variant });
      setDownload({ id: accepted.job_id, name: list.find((entry) => entry.id === variant)?.label ?? variant, job: null });
      void invalidate(queryKeys.downloads.list());
    },
  });
  const update = useCallback((job: JobView) => setDownload((previous) => previous && previous.id === job.id ? { ...previous, job } : previous), []);
  const settled = useCallback(async () => { await invalidate(queryKeys.voices.kokoroInventory(variant ?? ""), queryKeys.voices.kokoroAvailable()); }, [invalidate, variant]);
  const busy = downloading || start.isPending || select.isPending;
  const failure = select.error ?? start.error;
  return (
    <Sheet open={open} onClose={onClose} placement={phone ? "bottom" : "side"} {...(phone ? { detents: [0.7, 0.95] } : {})}
      title={t("voices.kokoro.setup")} description={t("voices.kokoro.setupIntro")}
      footer={installed && !downloading
        ? <Button variant="primary" size="lg" block onClick={onClose}>{t("voices.kokoro.done")}</Button>
        : <Button variant="primary" size="lg" block leading={<Download size={icon.md} />} loading={busy} disabled={variant === null || busy} onClick={() => start.mutate()}>{t(downloading ? "voices.kokoro.downloading" : "voices.kokoro.download")}</Button>}>
      <div className="space-y-5">
        {variants.isError && <FailureState error={variants.error} size="sm" onRetry={() => void variants.refetch()} />}
        <ListGroup label={t("voices.kokoro.variant")}>
          <VariantRows variants={list} selected={variant} recommended={mobile ? "int8" : "fp16"} busy={busy} onSelect={(id) => select.mutate(id)} />
        </ListGroup>
        {download && <DownloadJob api={api} id={download.id} name={download.name} onUpdate={update} onDone={settled} />}
        {failure !== null && <FailureState error={failure} size="sm" />}
      </div>
    </Sheet>
  );
}

function VariantRows({ variants, selected, recommended, busy, onSelect }: { variants: readonly KokoroVariantView[]; selected: string | null; recommended: string; busy: boolean; onSelect: (id: string) => void }) {
  const { t, i18n } = useTranslation("settings");
  const describe = (entry: KokoroVariantView) => {
    const key = `voices.kokoro.variants.${entry.id}`;
    return [i18n.exists(key, { ns: "settings" }) ? t(key as "voices.kokoro.variants.int8") : null, entry.size_mb === null ? null : `${entry.size_mb} MB`].filter(Boolean).join(" · ");
  };
  return (
    <>
      {variants.map((entry) => (
        <ListRowButton key={entry.id} title={entry.label} description={describe(entry)} chevron={false} aria-pressed={entry.id === selected} disabled={busy}
          trailing={entry.id === recommended || entry.id === selected ? <span className="flex items-center gap-2">{entry.id === recommended && <Badge>{t("voices.kokoro.recommended")}</Badge>}{entry.id === selected && <Check size={icon.md} className="text-accent" />}</span> : undefined}
          onClick={() => onSelect(entry.id)} />
      ))}
    </>
  );
}
