import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useLocation, useNavigate, useRouter, useSearch } from "@tanstack/react-router";
import { Check, Cpu, Download, Gauge, Maximize2, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge, Button, confirm, icon, ListGroup, ListRow, ListRowButton, Sheet, Skeleton, useLongPress } from "@lettuceai/crisp";
import type { SdInstalledRuntime, SdRuntimeRef } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { EngineInstallSheet } from "./EngineInstallSheet";
import { inventoryQuery, localImageKeys, useByteSize, useImageAction, type ImageInstallDetail } from "./local-data";

export function EngineView({ open, onJob, onPolicy, activeInstalls, downloadsReady }: { open: boolean; onJob: (id: string, title: string, detail: ImageInstallDetail) => void; onPolicy: (runtime: SdRuntimeRef) => void; activeInstalls: ImageInstallDetail[]; downloadsReady: boolean }) {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation("common");
  const api = useSettingsApi();
  const action = useImageAction();
  const router = useRouter();
  const navigate = useNavigate();
  const installOpen = useSearch({ from: "/settings/$section", select: (search) => search.engineInstall === true });
  const installPushed = useLocation({ select: (location) => location.state.engineInstallPushed });
  const size = useByteSize();
  const inventory = useQuery(inventoryQuery(api));
  const pairing = inventory.data?.active;
  const policy = useQuery({ queryKey: localImageKeys.policy(pairing?.release ?? "", pairing?.asset ?? ""), queryFn: () => api.call("sdComputePolicyGet", pairing!), enabled: !!pairing });
  const upscaler = useQuery({ queryKey: localImageKeys.upscaler, queryFn: () => api.call("sdUpscalersList") });
  const [selected, setSelected] = useState<SdInstalledRuntime | null>(null);
  const [wasOpen, setWasOpen] = useState(open);
  if (wasOpen !== open) {
    setWasOpen(open);
    if (!open) setSelected(null);
  }
  const installingUpscaler = activeInstalls.some((detail) => detail.type === "sd_upscaler_install" && detail.filename === upscaler.data?.recommended_filename);
  const openInstall = () => {
    void navigate({ to: "/settings/$section", params: { section: "imageGeneration" }, search: { localImageView: "engine", engineInstall: true }, state: { engineInstallPushed: true }, resetScroll: false });
  };
  const closeInstall = () => {
    if (!router.state.location.search.engineInstall) return;
    if (installPushed) router.history.back();
    else void navigate({ to: "/settings/$section", params: { section: "imageGeneration" }, search: { localImageView: "engine" }, replace: true, resetScroll: false });
  };
  const remove = async (build: SdInstalledRuntime) => {
    const sure = await confirm({ title: t("image.local.engine.deleteTitle", { version: build.release }), message: t(build.active ? "image.local.engine.deleteActive" : "image.local.engine.deleteBody"), confirmLabel: t("manage.delete"), cancelLabel: common("actions.cancel"), destructive: true });
    if (sure) action.mutate(async () => { await api.call("sdRuntimeDelete", { release: build.release, asset: build.asset }); setSelected(null); });
  };
  return (
    <div className="@container space-y-5">
      {action.isError && <FailureState error={action.error} size="sm" />}
      {inventory.isError && <FailureState error={inventory.error} onRetry={() => void inventory.refetch()} />}
      {!inventory.data ? !inventory.isError && <EngineSkeleton label={t("image.local.engine.installed")} loadingLabel={t("image.local.engine.loadingInventory")} /> : <ListGroup label={t("image.local.engine.installed")}>
        {inventory.data.installed.length === 0 && <ListRow title={t("image.local.engine.missing")} description={t("image.local.engine.installBody")} />}
        {inventory.data.installed.map((build) => <EngineBuildRow key={`${build.release}/${build.asset}`} build={build} onOpen={() => setSelected(build)} />)}
      </ListGroup>}
      {inventory.data?.active && <ListGroup label={t("image.local.policy.title")}>
        <ListRowButton icon={<Gauge size={icon.lg} />} title={t("image.local.policy.gpus")} description={policy.data ? t("image.local.policy.summary", { mode: t(policy.data.policy.multi_gpu_enabled ? "image.local.policy.multi" : policy.data.policy.single_gpu_device_id !== null ? "image.local.policy.single" : "image.local.policy.auto"), count: policy.data.devices.length, split: t(policy.data.policy.split_mode === "row" ? "image.local.policy.row" : "image.local.policy.layer"), backend: policy.data.backend }) : policy.isError ? t("image.local.readFailed") : t("image.loading")} onClick={() => onPolicy(inventory.data!.active!)} />
      </ListGroup>}
      <ListGroup>
        <ListRowButton icon={<Download size={icon.lg} />} title={t("image.local.engine.install")} description={t("image.local.engine.chooseBuild")} onClick={openInstall} />
      </ListGroup>
      <EngineInstallSheet open={open && installOpen} installed={inventory.data?.installed} activeInstalls={activeInstalls} downloadsReady={downloadsReady} onClose={closeInstall} onJob={onJob} />
      {upscaler.isError ? <FailureState error={upscaler.error} onRetry={() => void upscaler.refetch()} /> : !upscaler.data ? <EngineSkeleton label={t("image.local.upscaler.title")} loadingLabel={t("image.local.engine.loadingUpscaler")} /> : <ListGroup label={t("image.local.upscaler.title")}>
        <ListRow icon={<Maximize2 size={icon.lg} />} title={upscaler.data.recommended_filename} description={t("image.local.upscaler.body", { size: size(upscaler.data.recommended_bytes) })} trailing={<Button size="sm" variant={upscaler.data.recommended_installed ? "ghost" : "secondary"} loading={installingUpscaler} disabled={action.isPending || installingUpscaler || !downloadsReady} onClick={() => action.mutate(async () => {
          if (upscaler.data.recommended_installed) await api.call("sdUpscalersRemove", { filename: upscaler.data.recommended_filename });
          else { const accepted = await api.call("sdUpscalersInstall"); onJob(accepted.job_id, upscaler.data.recommended_filename, { type: "sd_upscaler_install", filename: upscaler.data.recommended_filename }); }
        })}>{t(upscaler.data.recommended_installed ? "image.local.remove" : installingUpscaler ? "image.local.installing" : "image.local.install")}</Button>} />
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

function EngineSkeleton({ label, loadingLabel }: { label?: string; loadingLabel: string }) {
  return <div role="status" aria-label={loadingLabel}><ListGroup label={label}>
    <ListRow title={<Skeleton className="h-4 w-24 max-w-full" />} description={<Skeleton className="h-3 w-40 max-w-full" />} />
  </ListGroup></div>;
}

function EngineBuildRow({ build, onOpen }: { build: SdInstalledRuntime; onOpen: () => void }) {
  const { t } = useTranslation("settings");
  const size = useByteSize();
  const press = useLongPress(onOpen);
  return <ListRowButton icon={<Cpu size={icon.lg} />} tone={build.active ? "accent" : "neutral"} title={<span className="font-mono text-sm">{build.release}</span>} description={`${build.backend} · ${size(build.size_bytes)}`} trailing={build.active ? <Badge tone="accent" dot>{t("image.local.engine.inUse")}</Badge> : undefined} chevron={!build.active} {...press.handlers} onClick={press.guardClick(onOpen)} />;
}
