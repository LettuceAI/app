import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Check, Cpu, Gauge, Maximize2, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge, Button, confirm, icon, ListGroup, ListRow, ListRowButton, LoadingState, Select, Sheet, useLongPress } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import type { SdInstalledRuntime, SdRuntimeRef } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { inventoryQuery, localImageKeys, recommendedAsset, useByteSize, useImageAction } from "./local-data";

export function EngineView({ open, onJob, onPolicy }: { open: boolean; onJob: (id: string, title: string) => void; onPolicy: (runtime: SdRuntimeRef) => void }) {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation("common");
  const api = useSettingsApi();
  const action = useImageAction();
  const size = useByteSize();
  const inventory = useQuery(inventoryQuery(api));
  const pairing = inventory.data?.active;
  const policy = useQuery({ queryKey: localImageKeys.policy(pairing?.release ?? "", pairing?.asset ?? ""), queryFn: () => api.call("sdComputePolicyGet", pairing!), enabled: !!pairing });
  const releases = useQuery({ queryKey: localImageKeys.releases, queryFn: () => api.call("sdRuntimeReleases") });
  const devices = useQuery({ queryKey: [...localImageKeys.releases, "devices"], queryFn: () => api.call("llamaDevices") });
  const status = useQuery(appStatusQuery(api));
  const upscaler = useQuery({ queryKey: localImageKeys.upscaler, queryFn: () => api.call("sdUpscalersList") });
  const [releaseTag, setRelease] = useState<string | null>(null);
  const [assetName, setAsset] = useState<string | null>(null);
  const [selected, setSelected] = useState<SdInstalledRuntime | null>(null);
  const [wasOpen, setWasOpen] = useState(open);
  if (wasOpen !== open) {
    setWasOpen(open);
    if (!open) setSelected(null);
  }
  const release = releases.data?.releases.find((item) => item.tag === releaseTag) ?? releases.data?.releases[0];
  const recommended = recommendedAsset(status.data?.platform ?? "other", devices.data?.devices ?? [], release?.assets ?? []);
  const asset = release?.assets.find((item) => item.name === assetName) ?? recommended;
  const installed = inventory.data?.installed.some((item) => item.release === release?.tag && item.asset === asset?.name);
  const remove = async (build: SdInstalledRuntime) => {
    const sure = await confirm({ title: t("image.local.engine.deleteTitle", { version: build.release }), message: t(build.active ? "image.local.engine.deleteActive" : "image.local.engine.deleteBody"), confirmLabel: t("manage.delete"), cancelLabel: common("actions.cancel"), destructive: true });
    if (sure) action.mutate(async () => { await api.call("sdRuntimeDelete", { release: build.release, asset: build.asset }); setSelected(null); });
  };
  return (
    <div className="space-y-5">
      {action.isError && <FailureState error={action.error} size="sm" />}
      {inventory.isError && <FailureState error={inventory.error} onRetry={() => void inventory.refetch()} />}
      {!inventory.data ? !inventory.isError && <LoadingState /> : <ListGroup label={t("image.local.engine.installed")}>
        {inventory.data.installed.length === 0 && <ListRow title={t("image.local.engine.missing")} description={t("image.local.engine.installBody")} />}
        {inventory.data.installed.map((build) => <EngineBuildRow key={`${build.release}/${build.asset}`} build={build} onOpen={() => setSelected(build)} />)}
      </ListGroup>}
      {inventory.data?.active && <ListGroup label={t("image.local.policy.title")}>
        <ListRowButton icon={<Gauge size={icon.lg} />} title={t("image.local.policy.gpus")} description={policy.data ? t("image.local.policy.summary", { mode: t(policy.data.policy.multi_gpu_enabled ? "image.local.policy.multi" : policy.data.policy.single_gpu_device_id !== null ? "image.local.policy.single" : "image.local.policy.auto"), count: policy.data.devices.length, split: t(policy.data.policy.split_mode === "row" ? "image.local.policy.row" : "image.local.policy.layer"), backend: policy.data.backend }) : policy.isError ? t("image.local.readFailed") : t("image.loading")} onClick={() => onPolicy(inventory.data!.active!)} />
      </ListGroup>}
      {releases.isError ? <FailureState error={releases.error} onRetry={() => void releases.refetch()} /> : !releases.data ? <LoadingState /> : <ListGroup label={t("image.local.engine.install")}>
        {release ? <>
          <ListRow title={t("image.local.engine.version")} description={t("image.local.engine.newestFirst")} trailing={<Select className="w-56 max-w-full" value={release.tag} options={releases.data.releases.map((item) => ({ value: item.tag, label: item.prerelease ? t("image.local.engine.prerelease", { version: item.tag }) : item.tag }))} onChange={(event) => { setRelease(event.target.value); setAsset(null); }} />} />
          <ListRow title={t("image.local.engine.build")} description={asset?.name === recommended?.name && devices.data ? t("image.local.engine.recommended") : undefined} trailing={<Select className="w-56 max-w-full" value={asset?.name ?? ""} options={release.assets.map((item) => ({ value: item.name, label: `${item.backend} · ${size(item.bytes + item.dependencies.reduce((total, dependency) => total + dependency.bytes, 0))}` }))} onChange={(event) => setAsset(event.target.value)} />} />
          {asset && <ListRow title={`${asset.backend} · ${size(asset.bytes + asset.dependencies.reduce((sum, item) => sum + item.bytes, 0))}`} description={<span className="font-mono text-xs">{asset.name}</span>} trailing={<Button size="sm" disabled={action.isPending || installed || !inventory.data} onClick={() => action.mutate(async () => { const accepted = await api.call("sdRuntimeInstall", { release: release.tag, asset: asset.name, then_register: null }); onJob(accepted.job_id, `${release.tag} · ${asset.backend}`); })}>{t(installed ? "image.local.engine.alreadyInstalled" : "image.local.install")}</Button>} />}
        </> : <ListRow title={t("image.local.engine.noBuilds")} />}
      </ListGroup>}
      {devices.isError && <FailureState error={devices.error} onRetry={() => void devices.refetch()} size="sm" />}
      {upscaler.isError ? <FailureState error={upscaler.error} onRetry={() => void upscaler.refetch()} /> : !upscaler.data ? <LoadingState /> : <ListGroup label={t("image.local.upscaler.title")}>
        <ListRow icon={<Maximize2 size={icon.lg} />} title={upscaler.data.recommended_filename} description={t("image.local.upscaler.body", { size: size(upscaler.data.recommended_bytes) })} trailing={<Button size="sm" variant={upscaler.data.recommended_installed ? "ghost" : "secondary"} disabled={action.isPending} onClick={() => action.mutate(async () => {
          if (upscaler.data.recommended_installed) await api.call("sdUpscalersRemove", { filename: upscaler.data.recommended_filename });
          else { const accepted = await api.call("sdUpscalersInstall"); onJob(accepted.job_id, upscaler.data.recommended_filename); }
        })}>{t(upscaler.data.recommended_installed ? "image.local.remove" : "image.local.install")}</Button>} />
      </ListGroup>}
      <Sheet open={selected !== null} onClose={() => setSelected(null)} title={selected?.release} description={selected?.backend}>
        {selected && <ListGroup>
          <ListRowButton icon={<Check size={icon.lg} />} title={t(selected.active ? "image.local.engine.inUse" : "image.local.engine.use")} disabled={selected.active || action.isPending} onClick={() => action.mutate(async () => { await api.call("sdRuntimeSwitch", { release: selected.release, asset: selected.asset }); setSelected(null); })} />
          <ListRowButton icon={<Trash2 size={icon.lg} />} title={t("manage.delete")} tone="danger" disabled={action.isPending} onClick={() => void remove(selected)} />
        </ListGroup>}
        {action.isError && <FailureState error={action.error} size="sm" />}
      </Sheet>
    </div>
  );
}

function EngineBuildRow({ build, onOpen }: { build: SdInstalledRuntime; onOpen: () => void }) {
  const { t } = useTranslation("settings");
  const size = useByteSize();
  const press = useLongPress(onOpen);
  return <ListRowButton icon={<Cpu size={icon.lg} />} tone={build.active ? "accent" : "neutral"} title={<span className="font-mono text-sm">{build.release}</span>} description={`${build.backend} · ${size(build.size_bytes)}`} trailing={build.active ? <Badge tone="accent" dot>{t("image.local.engine.inUse")}</Badge> : undefined} chevron={!build.active} {...press.handlers} onClick={press.guardClick(onOpen)} />;
}
