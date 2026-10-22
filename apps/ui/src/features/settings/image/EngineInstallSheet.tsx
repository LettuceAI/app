import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Check } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, EmptyState, icon, ListGroup, ListRow, ListRowButton, Select, Sheet, Skeleton, Text } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import type { SdInstalledRuntime, SdRuntimeAsset } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { localImageKeys, recommendedAsset, useByteSize, useImageAction, type ImageInstallDetail } from "./local-data";

const assetBytes = (asset: SdRuntimeAsset) => asset.bytes + asset.dependencies.reduce((total, dependency) => total + dependency.bytes, 0);
const supportingBytes = (asset: SdRuntimeAsset) => asset.dependencies.reduce((total, dependency) => total + dependency.bytes, 0);

export function EngineInstallSheet({ open, installed, activeInstalls, downloadsReady, onClose, onJob }: {
  open: boolean;
  installed: SdInstalledRuntime[] | undefined;
  activeInstalls: ImageInstallDetail[];
  downloadsReady: boolean;
  onClose: () => void;
  onJob: (id: string, title: string, detail: ImageInstallDetail) => void;
}) {
  const { t, i18n } = useTranslation("settings");
  const api = useSettingsApi();
  const action = useImageAction();
  const size = useByteSize();
  const releases = useQuery({ queryKey: localImageKeys.releases, queryFn: () => api.call("sdRuntimeReleases") });
  const devices = useQuery({ queryKey: [...localImageKeys.releases, "devices"], queryFn: () => api.call("llamaDevices") });
  const status = useQuery(appStatusQuery(api));
  const [releaseTag, setRelease] = useState<string | null>(null);
  const [assetName, setAsset] = useState<string | null>(null);
  const [wasOpen, setWasOpen] = useState(open);
  if (wasOpen !== open) {
    setWasOpen(open);
    if (open) {
      setRelease(null);
      setAsset(null);
    }
  }
  const list = releases.data?.releases ?? [];
  const release = list.find((item) => item.tag === releaseTag) ?? list[0];
  const recommended = recommendedAsset(status.data?.platform ?? "other", devices.data?.devices ?? [], release?.assets ?? []);
  const asset = release?.assets.find((item) => item.name === assetName) ?? recommended;
  const builds = release ? [...release.assets].sort((a, b) => Number(b.name === recommended?.name) - Number(a.name === recommended?.name)) : [];
  const isInstalled = (candidate: SdRuntimeAsset) => installed?.some((item) => item.release === release?.tag && item.asset === candidate.name) ?? false;
  const installing = activeInstalls.some((detail) => detail.type === "sd_runtime_install" && detail.release === release?.tag && detail.asset === asset?.name);
  const published = release?.published_at ? new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" }).format(new Date(release.published_at)) : null;
  const versionNote = release ? [release === list[0] ? t("image.local.engine.newest") : null, release.prerelease ? t("image.local.engine.prereleaseTag") : null, published ? t("image.local.engine.published", { date: published }) : null].filter(Boolean).join(" · ") : "";

  const install = () => {
    if (!release || !asset) return;
    action.mutate(async () => {
      const accepted = await api.call("sdRuntimeInstall", { release: release.tag, asset: asset.name, then_register: null });
      onJob(accepted.job_id, `${release.tag} · ${asset.backend}`, { type: "sd_runtime_install", release: release.tag, asset: asset.name, backend: asset.backend });
      onClose();
    });
  };

  const ready = release !== undefined && asset !== null && asset !== undefined;
  const footer = ready ? (
    <Button variant="primary" className="w-full" loading={installing} disabled={action.isPending || isInstalled(asset) || installing || installed === undefined || !downloadsReady} onClick={install}>
      {t(isInstalled(asset) ? "image.local.engine.alreadyInstalled" : installing ? "image.local.installing" : "image.local.install")}
    </Button>
  ) : undefined;

  return (
    <Sheet open={open} onClose={onClose} placement="auto" title={t("image.local.engine.install")} description={t("image.local.engine.chooseBuild")} {...(footer ? { footer } : {})}>
      <div className="space-y-6 pb-1">
        {releases.isError ? <FailureState error={releases.error} onRetry={() => void releases.refetch()} /> : !releases.data ? (
          <div role="status" aria-label={t("image.local.engine.loadingReleases")} className="space-y-6">
            <ListGroup label={t("image.local.engine.version")}>
              <ListRow title={<Skeleton className="h-4 w-24 max-w-full" />} description={<Skeleton className="h-3 w-40 max-w-full" />} trailing={<Skeleton shape="block" className="control-md w-32" />} />
            </ListGroup>
            <ListGroup label={t("image.local.engine.build")}>
              {[0, 1, 2].map((row) => <ListRow key={row} title={<Skeleton className="h-4 w-20 max-w-full" />} description={<Skeleton className="h-3 w-44 max-w-full" />} />)}
            </ListGroup>
          </div>
        ) : !release ? (
          <EmptyState title={t("image.local.engine.noBuilds")} />
        ) : (
          <>
            <ListGroup label={t("image.local.engine.version")}>
              <ListRow
                title={release.tag}
                description={versionNote}
                trailing={<Select aria-label={t("image.local.engine.version")} className="w-40 max-w-full" value={release.tag} options={list.map((item) => ({ value: item.tag, label: item.prerelease ? t("image.local.engine.prerelease", { version: item.tag }) : item.tag }))} onChange={(event) => { setRelease(event.target.value); setAsset(null); }} />}
              />
            </ListGroup>
            <ListGroup label={t("image.local.engine.build")}>
              {builds.length === 0 && <ListRow title={t("image.local.engine.noBuilds")} />}
              {builds.map((item) => {
                const extra = supportingBytes(item);
                const notes = [size(assetBytes(item)), extra > 0 ? t("image.local.engine.supporting", { size: size(extra) }) : null, item.name === recommended?.name && devices.data ? t("image.local.engine.recommended") : null, isInstalled(item) ? t("image.local.engine.alreadyInstalled") : null].filter(Boolean).join(" · ");
                return <ListRowButton key={item.name} title={item.backend} description={notes} chevron={false} aria-pressed={item.name === asset?.name} trailing={item.name === asset?.name ? <Check size={icon.md} /> : undefined} onClick={() => setAsset(item.name)} />;
              })}
            </ListGroup>
            {asset && <Text variant="hint"><span className="break-all font-mono">{asset.name}</span></Text>}
          </>
        )}
        {devices.isError && <FailureState error={devices.error} onRetry={() => void devices.refetch()} size="sm" />}
        {action.isError && <FailureState error={action.error} size="sm" />}
      </div>
    </Sheet>
  );
}
