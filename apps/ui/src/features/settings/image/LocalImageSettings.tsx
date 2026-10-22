import { useCallback, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useLocation, useNavigate, useRouter } from "@tanstack/react-router";
import { Cpu, Download, HardDrive, Layers, RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge, Button, icon, IconButton, ListGroup, ListRow, ListRowButton, Page, Sheet } from "@lettuceai/crisp";
import type { JobView, SdRuntimeRef } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { terminalJobStates } from "@/entities/job";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi, useSettingsLayout } from "../data";
import { activeImageDownloads, installedQuery, inventoryQuery, localImageKeys, useByteSize, type ImageInstallDetail } from "./local-data";
import { CatalogView } from "./CatalogView";
import { ComputePolicyView } from "./ComputePolicyView";
import { EngineView } from "./EngineView";
import { BundleView } from "./BundleView";
import { ImageJob } from "./ImageJob";
import { LoraLibraryView } from "./LoraLibraryView";

type View = "engine" | "models" | "bundle" | "loras";
const titleKeys = { engine: "image.local.engine.title", models: "image.local.catalog.title", bundle: "image.local.bundle.title", loras: "models.loras" } as const;
const metaKeys = { engine: "image.local.engine.meta", models: "image.local.catalog.meta", bundle: "image.local.bundle.meta", loras: "image.local.lora.meta" } as const;

function useLocalNavigation() {
  const navigate = useNavigate();
  return (view: View | null, replace = false) => {
    void navigate({ to: "/settings/$section", params: { section: "imageGeneration" }, search: view ? { localImageView: view } : {}, state: { localImagesPushed: view !== null }, replace, resetScroll: false });
  };
}

export function LocalImageSettings() {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const size = useByteSize();
  const open = useLocalNavigation();
  const inventory = useQuery(inventoryQuery(api));
  const installed = useQuery(installedQuery(api));
  const loras = useQuery({ queryKey: queryKeys.loras.all, queryFn: () => api.call("lorasList", { profile_id: null }) });
  const disk = useQuery({ queryKey: localImageKeys.disk, queryFn: () => api.call("sdDiskUsage") });
  const active = inventory.data?.installed.find((build) => build.active);
  return (
    <ListGroup label={t("image.local.group")}>
      <ListRowButton icon={<Cpu size={icon.lg} />} tone={inventory.data && !active ? "warning" : "neutral"} title={t("image.local.engine.title")} description={inventory.isError ? t("image.local.readFailed") : active ? `${active.release} · ${active.backend}` : inventory.isPending ? t("image.loading") : t("image.local.engine.missing")} trailing={inventory.data && !active ? <Badge tone="warning" dot>{t("image.local.engine.setup")}</Badge> : undefined} onClick={() => open("engine")} />
      <ListRowButton icon={<Download size={icon.lg} />} title={t("image.local.catalog.title")} description={installed.isError ? t("image.local.readFailed") : installed.data?.models.length ? t("image.local.catalog.count", { count: installed.data.models.length }) : t("image.local.catalog.downloadBody")} onClick={() => open("models")} />
      <ListRowButton icon={<Layers size={icon.lg} />} title={t("models.loras")} description={loras.data?.loras.length ? t("image.local.lora.count", { count: loras.data.loras.length }) : t("image.local.lora.summary")} onClick={() => open("loras")} />
      {disk.data && <ListRow icon={<HardDrive size={icon.lg} />} title={t("image.local.disk")} description={t("image.local.diskBody", { models: size(disk.data.components_bytes), engines: size(disk.data.runtimes_bytes), loras: size(disk.data.loras_bytes) })} trailing={size(disk.data.total_bytes)} />}
      {disk.isError && <FailureState error={disk.error} onRetry={() => void disk.refetch()} size="sm" />}
    </ListGroup>
  );
}

export function LocalImagePage({ view }: { view: View }) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const { phone } = useSettingsLayout();
  const client = useQueryClient();
  const router = useRouter();
  const open = useLocalNavigation();
  const pushed = useLocation({ select: (location) => location.state.localImagesPushed });
  const [policy, setPolicy] = useState<SdRuntimeRef | null>(null);
  const [loraActionTarget, setLoraActionTarget] = useState<HTMLDivElement | null>(null);
  const [policyFooter, setPolicyFooter] = useState<HTMLDivElement | null>(null);
  const [fitEpoch, setFitEpoch] = useState(0);
  const [jobs, setJobs] = useState<Record<string, { title: string; detail: ImageInstallDetail | null }>>({});
  const [observed, setObserved] = useState<Record<string, JobView>>({});
  const downloads = useQuery({ queryKey: [...queryKeys.jobs.lists(), "image-installs"], queryFn: () => activeImageDownloads(api), enabled: view !== "loras" });
  const onJob = useCallback((id: string, title: string, detail: ImageInstallDetail | null = null) => { setJobs((current) => ({ ...current, [id]: { title, detail } })); }, []);
  const onUpdate = useCallback((job: JobView) => { setObserved((current) => ({ ...current, [job.id]: job })); }, []);
  const listed = (downloads.data ?? []).filter((job) => view !== "engine" || job.subject_detail?.type === "sd_runtime_install" || job.subject_detail?.type === "sd_upscaler_install");
  const jobIds = [...new Set([...Object.keys(jobs), ...Object.keys(observed), ...listed.map((job) => job.id)])];
  const activeInstalls = jobIds.flatMap((id): ImageInstallDetail[] => {
    const job = observed[id] ?? listed.find((item) => item.id === id);
    if (job && terminalJobStates.has(job.state)) return [];
    const detail = job?.subject_detail ?? jobs[id]?.detail;
    return detail?.type === "sd_runtime_install" || detail?.type === "sd_upscaler_install" || detail?.type === "sd_model_install" ? [detail] : [];
  });
  const back = () => {
    if (pushed) router.history.back();
    else open(view === "bundle" ? "models" : null, true);
  };
  const refresh = () => { setFitEpoch((current) => current + 1); void client.invalidateQueries({ queryKey: queryKeys.localImages.all }); };
  const actions = view === "engine" ? <IconButton label={t("image.local.refresh")} variant="ghost" onClick={refresh}><RefreshCw size={icon.md} /></IconButton>
    : view === "models" ? <Button variant="ghost" size="sm" leading={<RefreshCw size={icon.sm} />} onClick={refresh}>{t("image.local.fit.again")}</Button> : view === "loras" ? <div ref={setLoraActionTarget} /> : undefined;
  return (
    <>
      <Page key={view} title={t(titleKeys[view])} meta={phone ? undefined : t(metaKeys[view])} onBack={back} backLabel={t(view === "bundle" ? "image.local.catalog.title" : "sections.imageGeneration.title")} actions={actions} contentClassName={view === "loras" ? "max-w-4xl space-y-7" : "max-w-3xl space-y-7"}>
        {downloads.isError && view !== "loras" && <FailureState error={downloads.error} onRetry={() => void downloads.refetch()} size="sm" />}
        {view !== "loras" && jobIds.map((id) => <ImageJob key={id} id={id} title={jobs[id]?.title} download onUpdate={onUpdate} />)}
        {view === "engine" ? <EngineView open onJob={onJob} onPolicy={setPolicy} activeInstalls={activeInstalls} downloadsReady={downloads.data !== undefined && !downloads.isError} /> : view === "models" ? <CatalogView fitEpoch={fitEpoch} onJob={onJob} activeInstalls={activeInstalls} downloadsReady={downloads.data !== undefined && !downloads.isError} onEngine={() => open("engine")} onBundle={() => open("bundle")} /> : view === "bundle" ? <BundleView onJob={onJob} onEngine={() => open("engine")} /> : <LoraLibraryView open actionTarget={loraActionTarget} />}
      </Page>
      <Sheet open={policy !== null} onClose={() => setPolicy(null)} title={t("image.local.policy.title")} description={t("image.local.policy.body")} placement="auto" footer={<div ref={setPolicyFooter} />}>
        {policy && <ComputePolicyView key={`${policy.release}/${policy.asset}`} runtime={policy} footerTarget={policyFooter} onClose={() => setPolicy(null)} onDone={() => { setPolicy(null); setFitEpoch((current) => current + 1); }} />}
      </Sheet>
    </>
  );
}
