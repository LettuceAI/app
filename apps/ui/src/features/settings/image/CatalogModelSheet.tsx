import { useNavigate } from "@tanstack/react-router";
import { AlertTriangle, Check, Download, Gauge, Trash2, XCircle } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Alert, Badge, Button, Collapsible, confirm, icon, KeyValue, ListGroup, ListRow, ListRowButton, Sheet, Spinner, Text } from "@lettuceai/crisp";
import type { SdCatalogProfile, SdFitEstimate, SdInstalledModel, SdRuntimeRef } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { useVariantFits } from "./fit-data";
import { fitKind, fitMemory, pickVariant, variantName, type FitKind } from "./fit";
import { useByteSize, useCleanupWarning, useCopyText, useImageAction, type ImageInstallDetail } from "./local-data";

const warnings: ReadonlySet<FitKind> = new Set(["cpu", "inconclusive", "update"]);

export function CatalogModelSheet({ open, onClose, profile, runtime, installed, selected, onVariant, fitEpoch, onJob, activeInstalls, downloadsReady }: {
  open: boolean;
  onClose: () => void;
  profile: SdCatalogProfile;
  runtime: SdRuntimeRef | null;
  installed: SdInstalledModel[] | undefined;
  selected: string | null;
  onVariant: (id: string) => void;
  fitEpoch: number;
  onJob: (id: string, title: string, detail: ImageInstallDetail) => void;
  activeInstalls: ImageInstallDetail[];
  downloadsReady: boolean;
}) {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation("common");
  const api = useSettingsApi();
  const navigate = useNavigate();
  const action = useImageAction();
  const size = useByteSize();
  const warn = useCleanupWarning();
  const copy = useCopyText();
  const installedIds = new Set((installed ?? []).map((model) => model.variant_id));
  const build = runtime ? /^master-(\d+)-/.exec(runtime.release) : null;
  const supported = profile.minimum_runtime_build === null || (build !== null && Number(build[1]) >= profile.minimum_runtime_build);
  const checkable = runtime !== null && supported;
  const { fits, run } = useVariantFits({ profile, runtime: checkable ? runtime : null, enabled: open, epoch: fitEpoch, installed: installedIds });
  const pick = checkable ? pickVariant(profile.variants.filter((item) => !installedIds.has(item.id)), fits) : null;
  const variant = profile.variants.find((item) => item.id === (selected ?? installed?.[0]?.variant_id)) ?? profile.variants.find((item) => item.id === pick?.id) ?? profile.variants.find((item) => item.recommended) ?? profile.variants[0];
  const local = installed?.find((item) => item.variant_id === variant?.id);
  const fit = variant ? fits.get(variant.id) : undefined;
  const kind = fit?.state === "ready" ? fitKind(fit.verdict) : null;
  const installing = activeInstalls.some((detail) => detail.type === "sd_model_install" && detail.profile_id === profile.id && detail.variant_id === variant?.id && detail.runtime_release === runtime?.release && detail.runtime_asset === runtime?.asset);
  const blocked = !runtime ? t("image.local.catalog.engineRequired") : !supported ? t("image.local.catalog.updateRequired", { build: profile.minimum_runtime_build }) : null;
  const name = variant ? `${profile.display_name} · ${variantName(variant.label)}` : profile.display_name;

  const install = async () => {
    if (!variant || !runtime || installing || !downloadsReady) return;
    if (kind === "cpu" && !await confirm({ title: t("image.local.catalog.cpuTitle"), message: t("image.local.catalog.cpuBody"), confirmLabel: t("image.local.catalog.download"), cancelLabel: common("actions.cancel") })) return;
    action.mutate(async () => {
      const result = await api.call("sdModelInstall", { profile_id: profile.id, variant_id: variant.id, release: runtime.release, asset: runtime.asset });
      if (result.job_id) onJob(result.job_id, name, { type: "sd_model_install", profile_id: profile.id, variant_id: variant.id, runtime_release: runtime.release, runtime_asset: runtime.asset, display_name: name });
      onClose();
    });
  };
  const remove = async () => {
    if (!variant || !await confirm({ title: t("image.local.catalog.deleteTitle", { name: profile.display_name }), message: t("image.local.catalog.deleteBody"), confirmLabel: t("manage.delete"), cancelLabel: common("actions.cancel"), destructive: true })) return;
    action.mutate(async () => { const result = await api.call("sdModelUninstall", { profile_id: profile.id, variant_id: variant.id, also_remove_engine_if_unused: false }); warn(result.left_behind); onClose(); });
  };

  const verdictBody = (value: FitKind, estimate: SdFitEstimate | null) => {
    const memory = estimate ? fitMemory(estimate) : null;
    if (value === "gpu") return memory ? t("image.local.fit.kindBody.gpu", { need: size(memory.gpuNeed), budget: size(memory.gpuBudget) }) : undefined;
    if (value === "cpu") return memory ? t("image.local.fit.kindBody.cpu", { size: size(memory.ramWeights) }) : undefined;
    if (value === "phased" || value === "split" || value === "verified" || value === "failed") return t(`image.local.fit.kindBody.${value}`);
    return undefined;
  };

  const hardware = () => {
    if (!variant) return null;
    const retest = <Button variant="ghost" size="sm" onClick={() => run(variant.id)}>{t(local ? "image.local.fit.testAgain" : "image.local.fit.retry")}</Button>;
    if (fit?.state === "error") return <ListRow icon={<AlertTriangle size={icon.lg} />} tone="warning" title={t("image.local.fit.checkFailed")} trailing={retest} />;
    if (fit?.state === "checking" || (!fit && !local)) return <ListRow title={t("image.local.fit.checking")} trailing={<Spinner size="sm" />} />;
    if (fit?.state !== "ready") return <ListRowButton icon={<Gauge size={icon.lg} />} title={t("image.local.fit.test")} description={t("image.local.fit.testBody")} chevron={false} onClick={() => run(variant.id)} />;
    const estimate = fit.verdict.estimate;
    const tone = kind === "failed" ? "danger" : kind && warnings.has(kind) ? "warning" : "neutral";
    const Glyph = kind === "failed" ? XCircle : kind && warnings.has(kind) ? AlertTriangle : Check;
    return <>
      <ListRow icon={<Glyph size={icon.lg} />} tone={tone} title={t(`image.local.fit.kind.${kind!}`)} description={verdictBody(kind!, estimate)} trailing={local ? retest : undefined} />
      {estimate && <MemoryDetails estimate={estimate} />}
    </>;
  };

  return (
    <Sheet open={open} onClose={onClose} placement="auto" title={profile.display_name} description={`${profile.family} · ${profile.license}`} footer={<div className="flex flex-wrap items-center justify-end gap-2">
      {local ? <>
        <Button variant="danger" leading={<Trash2 size={icon.sm} />} className="mr-auto" disabled={action.isPending} onClick={() => void remove()}>{t("manage.delete")}</Button>
        <Button disabled={action.isPending} onClick={() => local.model_id ? void navigate({ to: "/settings/models/$modelId", params: { modelId: local.model_id } }) : action.mutate(async () => { await api.call("sdModelRepair", { profile_id: profile.id, variant_id: local.variant_id }); onClose(); })}>{t(local.model_id ? "image.local.catalog.openEditor" : "image.local.catalog.addBack")}</Button>
      </> : <>
        {blocked && <span className="mr-auto text-sm text-fg-3">{blocked}</span>}
        <Button leading={<Download size={icon.sm} />} disabled={!!blocked || !variant || installed === undefined || action.isPending || installing || !downloadsReady} loading={action.isPending || installing} onClick={() => void install()}>{installing ? t("image.local.installing") : t("image.local.catalog.downloadSize", { size: size(variant?.download_bytes ?? 0) })}</Button>
      </>}
    </div>}>
      <div className="space-y-6 pb-1">
        {action.isError && <FailureState error={action.error} size="sm" />}
        <Text variant="secondary">{profile.description}</Text>
        {local && !local.model_id && <Alert tone="warning" title={t("image.local.catalog.notListed")} description={t("image.local.catalog.needsRepair")} />}
        <div className="space-y-2">
          <KeyValue items={[{ label: t("image.local.catalog.makes"), value: t(profile.supports_image_edit ? "image.local.catalog.drawsEdits" : "image.local.catalog.draws") }, { label: t("image.local.catalog.defaults"), value: t("image.local.catalog.defaultValues", { width: profile.default_width, height: profile.default_height, steps: profile.default_steps }) }]} />
          {profile.requires_reference_image && <Text variant="hint">{t("image.local.catalog.reference")}</Text>}
          <Button variant="ghost" size="sm" onClick={() => copy(profile.source_url)}>{t("image.local.catalog.modelCard")}</Button>
        </div>
        <ListGroup label={t("image.local.catalog.variant")}>
          {profile.variants.map((item) => {
            const isInstalled = installedIds.has(item.id);
            const state = fits.get(item.id);
            const best = pick?.fits === true && pick.id === item.id && !isInstalled;
            const note = isInstalled || !runtime || !supported
              ? (!isInstalled && item.recommended ? t("image.local.fit.suggested") : null)
              : state?.state === "ready" ? t(`image.local.fit.short.${fitKind(state.verdict)}`) : state?.state === "error" ? t("image.local.fit.rowFailed") : t("image.local.fit.rowChecking");
            const chosen = variant?.id === item.id;
            return <ListRowButton key={item.id} title={variantName(item.label)} description={[size(item.download_bytes), note, isInstalled ? t("image.local.catalog.downloaded") : null].filter(Boolean).join(" · ")} chevron={false} aria-pressed={chosen}
              trailing={best || chosen ? <span className="flex items-center gap-2">{best && <Badge tone="accent">{t("image.local.fit.bestFit")}</Badge>}{chosen && <Check size={icon.md} className="text-accent" />}</span> : undefined} onClick={() => onVariant(item.id)} />;
          })}
        </ListGroup>
        {checkable && variant && <ListGroup label={t("image.local.catalog.hardware")}>{hardware()}</ListGroup>}
      </div>
    </Sheet>
  );
}

function MemoryDetails({ estimate }: { estimate: SdFitEstimate }) {
  const { t } = useTranslation("settings");
  const size = useByteSize();
  const memory = fitMemory(estimate);
  return (
    <div className="p-3">
      <Collapsible title={t("image.local.fit.details")}>
        <div className="space-y-4">
          <KeyValue items={[
            { label: t("image.local.fit.weights"), value: size(estimate.model_bytes) },
            ...(memory.gpuWeights > 0 ? [{ label: t("image.local.fit.onGpu"), value: size(memory.gpuWeights) }] : []),
            ...(memory.ramWeights > 0 ? [{ label: t("image.local.fit.inRam"), value: size(memory.ramWeights) }] : []),
            ...(estimate.available_ram_bytes !== null ? [{ label: t("image.local.fit.ram"), value: size(estimate.available_ram_bytes) }] : []),
          ]} />
          {estimate.devices.length > 0 && <KeyValue items={estimate.devices.map((device) => ({ label: device.description || device.name, value: t("image.local.policy.memory", { free: size(device.free_bytes), total: size(device.total_bytes) }) }))} />}
          <KeyValue items={estimate.placements.map((placement) => ({ label: t(`image.local.fit.${placement.component}`), value: `${placement.targets.join(" + ")} · ${size(placement.params_bytes + placement.compute_reserve_bytes)}` }))} />
        </div>
      </Collapsible>
    </div>
  );
}
