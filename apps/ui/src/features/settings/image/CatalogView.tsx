import { useCallback, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Check, Download, FolderSearch, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Alert, Badge, Button, confirm, icon, KeyValue, ListGroup, ListRow, ListRowButton, LoadingState, Sheet, Text } from "@lettuceai/crisp";
import type { JobView, SdCatalogProfile, SdInstalledModel, SdRunnability, SdRuntimeRef } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { useSettingsApi } from "../data";
import { ImageJob } from "./ImageJob";
import { installedQuery, inventoryQuery, localImageKeys, useByteSize, useCleanupWarning, useCopyText, useImageAction } from "./local-data";

export function CatalogView({ fitEpoch, onJob, onEngine, onBundle }: { fitEpoch: number; onJob: (id: string, title: string) => void; onEngine: () => void; onBundle: () => void }) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const size = useByteSize();
  const inventory = useQuery(inventoryQuery(api));
  const installed = useQuery(installedQuery(api));
  const catalog = useQuery({ queryKey: localImageKeys.catalog, queryFn: () => api.call("sdCatalog") });
  const [open, setOpen] = useState<string | null>(null);
  const { shown } = useClosingValue(open);
  const [variants, setVariants] = useState<Record<string, string>>({});
  const runtime = inventory.data?.active ?? null;
  const families = new Map<string, SdCatalogProfile[]>();
  for (const profile of catalog.data?.profiles ?? []) families.set(profile.family, [...(families.get(profile.family) ?? []), profile]);
  const profile = catalog.data?.profiles.find((item) => item.id === shown);
  return (
    <>
      {!runtime && inventory.data && <Alert tone="warning" title={t("image.local.catalog.engineFirst")} description={t("image.local.catalog.engineBody")} action={<Button size="sm" variant="secondary" onClick={onEngine}>{t("image.local.engine.setup")}</Button>} />}
      {inventory.isError && <FailureState error={inventory.error} onRetry={() => void inventory.refetch()} />}
      {installed.isError && <FailureState error={installed.error} onRetry={() => void installed.refetch()} />}
      {catalog.isError && <FailureState error={catalog.error} onRetry={() => void catalog.refetch()} />}
      {!catalog.data ? !catalog.isError && <LoadingState /> : !catalog.data.runtime_supported ? <Alert tone="warning" title={t("image.local.catalog.unsupported")} /> : [...families].map(([family, profiles]) => <ListGroup key={family} label={family}>
        {profiles.map((item) => {
          const mine = installed.data?.models.find((model) => model.profile_id === item.id);
          const variant = item.variants.find((entry) => entry.id === (variants[item.id] ?? mine?.variant_id)) ?? item.variants.find((entry) => entry.recommended) ?? item.variants[0];
          const tooOld = runtime && item.minimum_runtime_build !== null && Number(/^master-(\d+)-/.exec(runtime.release)?.[1] ?? 0) < item.minimum_runtime_build;
          return <ListRowButton key={item.id} title={item.display_name} description={[variant?.label, mine ? t("image.local.catalog.installed") : size(variant?.download_bytes ?? 0), t(item.supports_image_edit ? "image.local.catalog.drawsEdits" : "image.local.catalog.draws"), item.requires_reference_image ? t("image.local.catalog.reference") : null].filter(Boolean).join(" · ")} trailing={mine ? <Badge tone={mine.model_id ? "accent" : "warning"}>{t(mine.model_id ? "image.local.catalog.downloaded" : "image.local.catalog.notListed")}</Badge> : tooOld ? <Badge tone="warning">{t("image.local.catalog.newEngine")}</Badge> : undefined} onClick={() => setOpen(item.id)} onContextMenu={(event) => { event.preventDefault(); setOpen(item.id); }} />;
        })}
      </ListGroup>)}
      <ListGroup label={t("image.local.catalog.other")}>
        <ListRowButton icon={<FolderSearch size={icon.lg} />} title={t("image.local.bundle.title")} description={t("image.local.bundle.meta")} onClick={onBundle} />
      </ListGroup>
      <Text variant="hint">{t("image.local.catalog.editorHint")}</Text>
      {profile && <CatalogModel key={profile.id} open={open !== null} onClose={() => setOpen(null)} profile={profile} runtime={runtime} installed={installed.data?.models.filter((model) => model.profile_id === profile.id)} selected={variants[profile.id] ?? null} onVariant={(id) => setVariants((current) => ({ ...current, [profile.id]: id }))} fitEpoch={fitEpoch} onJob={onJob} />}
    </>
  );
}

function CatalogModel({ open, onClose, profile, runtime, installed, selected, onVariant, fitEpoch, onJob }: { open: boolean; onClose: () => void; profile: SdCatalogProfile; runtime: SdRuntimeRef | null; installed: SdInstalledModel[] | undefined; selected: string | null; onVariant: (id: string) => void; fitEpoch: number; onJob: (id: string, title: string) => void }) {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation("common");
  const api = useSettingsApi();
  const navigate = useNavigate();
  const action = useImageAction();
  const size = useByteSize();
  const warn = useCleanupWarning();
  const copy = useCopyText();
  const variant = profile.variants.find((item) => item.id === (selected ?? installed?.[0]?.variant_id)) ?? profile.variants.find((item) => item.recommended) ?? profile.variants[0];
  const local = installed?.find((item) => item.variant_id === variant?.id);
  const [verdict, setVerdict] = useState<{ key: string; value: SdRunnability } | null>(null);
  const fitKey = `${variant?.id}/${runtime?.release}/${runtime?.asset}/${fitEpoch}`;
  const fit = verdict?.key === fitKey ? verdict.value : null;
  const build = runtime ? /^master-(\d+)-/.exec(runtime.release) : null;
  const supported = profile.minimum_runtime_build === null || (build !== null && Number(build[1]) >= profile.minimum_runtime_build);
  const blocked = !runtime ? t("image.local.catalog.engineRequired") : !supported ? t("image.local.catalog.updateRequired", { build: profile.minimum_runtime_build }) : null;
  const install = async () => {
    if (!variant || !runtime) return;
    if (fit?.status === "cpu_fallback" && !await confirm({ title: t("image.local.catalog.cpuTitle"), message: t("image.local.catalog.cpuBody"), confirmLabel: t("image.local.install"), cancelLabel: common("actions.cancel") })) return;
    action.mutate(async () => {
      const result = await api.call("sdModelInstall", { profile_id: profile.id, variant_id: variant.id, release: runtime.release, asset: runtime.asset });
      if (result.job_id) onJob(result.job_id, `${profile.display_name} · ${variant.label}`);
      onClose();
    });
  };
  const remove = async () => {
    if (!variant || !await confirm({ title: t("image.local.catalog.deleteTitle", { name: profile.display_name }), message: t("image.local.catalog.deleteBody"), confirmLabel: t("manage.delete"), cancelLabel: common("actions.cancel"), destructive: true })) return;
    action.mutate(async () => { const result = await api.call("sdModelUninstall", { profile_id: profile.id, variant_id: variant.id, also_remove_engine_if_unused: false }); warn(result.left_behind); onClose(); });
  };
  const onFit = useCallback((value: SdRunnability) => { setVerdict({ key: fitKey, value }); }, [fitKey]);
  return (
    <Sheet open={open} onClose={onClose} placement="auto" title={profile.display_name} description={`${profile.family} · ${profile.license}`} footer={<div className="flex flex-wrap items-center justify-end gap-2">
      {local ? <>
        <Button variant="danger" leading={<Trash2 size={icon.sm} />} className="mr-auto" disabled={action.isPending} onClick={() => void remove()}>{t("manage.delete")}</Button>
        <Button disabled={action.isPending} onClick={() => local.model_id ? void navigate({ to: "/settings/models/$modelId", params: { modelId: local.model_id } }) : action.mutate(async () => { await api.call("sdModelRepair", { profile_id: profile.id, variant_id: local.variant_id }); onClose(); })}>{t(local.model_id ? "image.local.catalog.openEditor" : "image.local.catalog.addBack")}</Button>
      </> : <>
        {blocked && <span className="mr-auto text-sm text-fg-3">{blocked}</span>}
        <Button leading={<Download size={icon.sm} />} disabled={!!blocked || !variant || installed === undefined || action.isPending} loading={action.isPending} onClick={() => void install()}>{t("image.local.catalog.downloadSize", { size: size(variant?.download_bytes ?? 0) })}</Button>
      </>}
    </div>}>
      <div className="space-y-5">
        {action.isError && <FailureState error={action.error} size="sm" />}
        <Text variant="secondary">{profile.description}</Text>
        {local && !local.model_id && <Alert tone="warning" title={t("image.local.catalog.notListed")} description={t("image.local.catalog.needsRepair")} />}
        <KeyValue items={[{ label: t("image.local.catalog.makes"), value: t(profile.supports_image_edit ? "image.local.catalog.drawsEdits" : "image.local.catalog.draws") }, { label: t("image.local.catalog.defaults"), value: t("image.local.catalog.defaultValues", { width: profile.default_width, height: profile.default_height, steps: profile.default_steps }) }, { label: t("image.local.catalog.license"), value: profile.license }]} />
        <Button variant="ghost" size="sm" onClick={() => copy(profile.source_url)}>{t("image.local.catalog.modelCard")}</Button>
        <ListGroup label={t("image.local.catalog.variant")}>
          {profile.variants.map((item) => <ListRowButton key={item.id} title={item.label} description={[size(item.download_bytes), item.recommended ? t("image.local.engine.recommended") : null, installed?.some((model) => model.variant_id === item.id) ? t("image.local.catalog.downloaded") : null].filter(Boolean).join(" · ")} chevron={false} trailing={variant?.id === item.id ? <Check size={icon.md} className="text-accent" /> : undefined} onClick={() => onVariant(item.id)} />)}
        </ListGroup>
        {variant && runtime && supported && open && <ListGroup label={t("image.local.catalog.hardware")}><FitProbe key={fitKey} profile={profile.id} variant={variant.id} runtime={runtime} onResult={onFit} /></ListGroup>}
      </div>
    </Sheet>
  );
}

function FitProbe({ profile, variant, runtime, onResult }: { profile: string; variant: string; runtime: SdRuntimeRef; onResult: (value: SdRunnability) => void }) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const action = useImageAction();
  const size = useByteSize();
  const [job, setJob] = useState<string | null>(null);
  const [result, setResult] = useState<SdRunnability | null>(null);
  const [running, setRunning] = useState(false);
  const completed = useCallback((view: JobView) => {
    setRunning(false);
    if (view.state === "succeeded" && view.result?.type === "runnability") { setResult(view.result.verdict); onResult(view.result.verdict); }
  }, [onResult]);
  const check = () => action.mutate(async () => {
    const accepted = await api.call("sdRunnability", { profile_id: profile, variant_id: variant, runtime_release: runtime.release, runtime_asset: runtime.asset, width: null, height: null, reference_image_count: null, loras: [], prompt: null, negative_prompt: null, sample_steps: null, cfg_scale: null, seed: null, sample_method: null, batch_count: null, full_execution: false, client_operation_id: crypto.randomUUID() });
    setResult(null); setRunning(true); setJob(accepted.job_id);
  });
  return (
    <>
      <ListRow title={t("image.local.fit.title")} description={result ? t(`image.local.fit.${result.status}`) : t("image.local.fit.notChecked")} trailing={<Button variant="ghost" size="sm" disabled={action.isPending || running} onClick={check}>{t("image.local.fit.check")}</Button>} />
      {job && <ImageJob id={job} title={t("image.local.fit.title")} onDone={completed} />}
      {result?.estimate && <>
        <ListRow title={t("image.local.fit.weights")} trailing={size(result.estimate.model_bytes)} />
        {result.estimate.available_ram_bytes !== null && <ListRow title={t("image.local.fit.ram")} trailing={size(result.estimate.available_ram_bytes)} />}
        {result.estimate.devices.map((device) => <ListRow key={device.id} title={device.description || device.name} description={t("image.local.policy.memory", { free: size(device.free_bytes), total: size(device.total_bytes) })} trailing={size(device.budget_bytes)} />)}
        {result.estimate.placements.map((placement, index) => <ListRow key={index} title={t(`image.local.fit.${placement.component}`)} description={t(placement.cpu ? "image.local.fit.cpu" : "image.local.fit.gpu")} trailing={size(placement.params_bytes + placement.compute_reserve_bytes)} />)}
      </>}
      {action.isError && <FailureState error={action.error} size="sm" />}
    </>
  );
}
