import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Check,
  ChevronRight,
  Cpu,
  Download,
  HardDrive,
  Loader2,
  RefreshCw,
  Trash2,
} from "lucide-react";

import {
  useDownloadQueueOptional,
  type QueuedDownload,
} from "../../../core/downloads/DownloadQueueContext";
import { useI18n, type TranslationKey } from "../../../core/i18n/context";
import { openExternalUrl } from "../../../core/utils/openExternal";
import { getPlatform } from "../../../core/utils/platform";
import { confirmBottomMenu } from "../../components/ConfirmBottomMenu";
import { BottomMenu } from "../../components/BottomMenu";
import { toast } from "../../components/toast";
import { cn } from "../../design-tokens";
import { InlineDownloadCards } from "./components/DownloadQueueBar";

type RuntimeAsset = {
  name: string;
  backend: string;
  bytes: number;
  dependencies: { name: string; bytes: number }[];
};

type RuntimeRelease = {
  tag: string;
  name: string;
  publishedAt: string | null;
  prerelease: boolean;
  assets: RuntimeAsset[];
};

type CatalogVariant = {
  id: string;
  label: string;
  description: string;
  downloadBytes: number;
  installed: boolean;
  recommended: boolean;
  smaller: boolean;
};

type CatalogProfile = {
  id: string;
  displayName: string;
  description: string;
  license: string;
  sourceUrl: string;
  supportsTextToImage: boolean;
  supportsImageEdit: boolean;
  supportsLora: boolean;
  maxReferenceImages: number | null;
  requiresReferenceImage: boolean;
  recommendedForScenes: boolean;
  defaultWidth: number;
  defaultHeight: number;
  defaultSteps: number;
  defaultCfg: number;
  variants: CatalogVariant[];
};

type RuntimeCatalog = {
  runtimeSupported: boolean;
  unsupportedReason: string | null;
  runtimeReleases: RuntimeRelease[];
  profiles: CatalogProfile[];
};

type InstalledModel = {
  profileId: string;
  variantId: string;
  displayName: string;
  runtimeRelease: string | null;
  runtimeAsset: string | null;
  runtimeBackend: string | null;
  componentBytesOnDisk: number;
  modelId: string | null;
  supportsTextToImage: boolean;
  supportsImageEdit: boolean;
  recommendedForScenes: boolean;
  requiresReferenceImage: boolean;
};

type RunnabilityPlacement = {
  component: string;
  paramsBytes: number;
  computeReserveBytes: number;
  targets: string[];
  cpu: boolean;
  split: boolean;
};

type RunnabilityEstimate = {
  modelBytes: number;
  availableRamBytes: number | null;
  planMode: string;
  deviceSource: string;
  devices: { name: string; description: string; freeBytes: number; budgetBytes: number }[];
  placements: RunnabilityPlacement[];
};

type Runnability = {
  status: string;
  method: string;
  exact: boolean;
  scope: string;
  placementPolicy: string;
  elapsedMs: number | null;
  reason: string;
  estimate?: RunnabilityEstimate | null;
};

type FitTone = "good" | "ok" | "warn" | "muted";

const FIT_TEXT: Record<FitTone, string> = {
  good: "text-success",
  ok: "text-fg/70",
  warn: "text-warning",
  muted: "text-fg/40",
};

const FIT_DOT: Record<FitTone, string> = {
  good: "bg-success",
  ok: "bg-fg/40",
  warn: "bg-warning",
  muted: "bg-fg/25",
};

function classifyFit(
  fit: Runnability | "loading" | undefined,
): { tone: FitTone; key: string } | null {
  if (fit === undefined) return null;
  if (fit === "loading") return { tone: "muted", key: "fitChecking" };
  switch (fit.status) {
    case "passed":
      return { tone: "good", key: "fitVerified" };
    case "failed":
      return { tone: "warn", key: "fitFailed" };
    case "inconclusive":
      return { tone: "muted", key: "fitInconclusive" };
    case "estimatedRunnable": {
      const estimate = fit.estimate;
      const offloads = Boolean(
        estimate &&
        (estimate.planMode === "defaultBackend" ||
          estimate.placements.some((placement) => placement.cpu)),
      );
      if (offloads) return { tone: "warn", key: "fitCpuOffload" };
      if (estimate?.planMode === "timeShare") return { tone: "ok", key: "fitPhased" };
      return { tone: "good", key: "fitFitsGpu" };
    }
    default:
      return null;
  }
}

function fitOffloadsToCpu(fit: Runnability | "loading" | undefined): boolean {
  if (!fit || fit === "loading" || fit.status !== "estimatedRunnable") return false;
  const estimate = fit.estimate;
  return Boolean(
    estimate &&
    (estimate.planMode === "defaultBackend" ||
      estimate.placements.some((placement) => placement.cpu)),
  );
}

type InstalledRuntime = {
  release: string;
  asset: string;
  backend: string;
  sizeBytes: number;
  active: boolean;
};

type RuntimeInventory = {
  installed: InstalledRuntime[];
  active: { release: string; asset: string } | null;
};

type GpuDevice = {
  index: number;
  name: string;
  description: string;
  backend: string;
  memoryTotal: number;
  memoryFree: number;
  deviceType: string;
};

type RuntimeInstall = {
  items: QueuedDownload[];
  active: boolean;
};

const focusRing =
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/55 focus-visible:ring-offset-2 focus-visible:ring-offset-bg";

function formatBytes(bytes: number): string {
  const gib = bytes / 1024 ** 3;
  const mib = bytes / 1024 ** 2;
  const value = gib >= 1 ? gib : mib;
  return `${new Intl.NumberFormat(undefined, {
    maximumFractionDigits: gib >= 1 ? 1 : 0,
  }).format(value)} ${gib >= 1 ? "GiB" : "MiB"}`;
}

function totalAssetBytes(asset: RuntimeAsset): number {
  return asset.bytes + asset.dependencies.reduce((total, item) => total + item.bytes, 0);
}

function usableGpuDevices(devices: GpuDevice[]): GpuDevice[] {
  return devices.filter(
    (device) =>
      device.deviceType.toLowerCase() !== "integratedgpu" &&
      device.deviceType.toLowerCase() !== "cpu",
  );
}

function recommendAsset(
  os: ReturnType<typeof getPlatform>["os"],
  devices: GpuDevice[],
  assets: RuntimeAsset[],
): RuntimeAsset | null {
  if (assets.length === 0) return null;
  const discrete = usableGpuDevices(devices);
  const hardware = discrete
    .map((device) => `${device.name} ${device.description} ${device.backend}`.toLowerCase())
    .join(" ");
  const isAmd = /\bamd\b|radeon|rocm/.test(hardware);
  const isNvidia = /nvidia|cuda/.test(hardware);

  let priorities: string[];
  if (os === "linux") {
    priorities = isAmd
      ? ["rocm", "vulkan", "cpu"]
      : discrete.length > 0
        ? ["vulkan", isNvidia ? "cuda" : "rocm", "cpu"]
        : ["cpu", "vulkan"];
  } else if (os === "macos") {
    priorities = ["metal", "cpu"];
  } else if (isAmd) {
    priorities = ["rocm", "vulkan", "cpu"];
  } else if (isNvidia) {
    priorities = ["cuda", "vulkan", "cpu"];
  } else if (discrete.length > 0) {
    priorities = ["vulkan", "cpu"];
  } else {
    priorities = ["cpu", "vulkan", "cuda", "rocm", "metal"];
  }

  for (const backend of priorities) {
    const match = assets.find((asset) => asset.backend.toLowerCase() === backend);
    if (match) return match;
  }
  return assets[0];
}

function runtimeInstallGroups(queue: QueuedDownload[]): RuntimeInstall[] {
  const groups = new Map<string, QueuedDownload[]>();
  for (const item of queue) {
    if (item.queueKind !== "sdcpp" || item.installKind !== "runtime") continue;
    const id = item.installId ?? item.id;
    groups.set(id, [...(groups.get(id) ?? []), item]);
  }
  return Array.from(groups.values(), (items) => ({
    items,
    active: items.some((item) => item.status === "queued" || item.status === "downloading"),
  }));
}

function Skeleton({ className }: { className?: string }) {
  return (
    <div
      aria-hidden="true"
      className={cn("animate-pulse rounded-md bg-fg/8 motion-reduce:animate-none", className)}
    />
  );
}

type ModelInstall = {
  installId: string;
  profileId: string;
  variantId: string;
  items: QueuedDownload[];
  active: boolean;
};

function modelInstallGroups(queue: QueuedDownload[]): ModelInstall[] {
  const groups = new Map<string, QueuedDownload[]>();
  for (const item of queue) {
    if (item.queueKind !== "sdcpp") continue;
    const id = item.installId ?? item.id;
    if (!id.startsWith("sdcpp:")) continue;
    groups.set(id, [...(groups.get(id) ?? []), item]);
  }
  return Array.from(groups.entries(), ([installId, items]) => {
    const [, profileId = "", variantId = ""] = installId.split(":");
    return {
      installId,
      profileId,
      variantId,
      items,
      active: items.some((item) => item.status === "queued" || item.status === "downloading"),
    };
  });
}

function selectedVariant(
  profile: CatalogProfile,
  selection: Record<string, string>,
): CatalogVariant | null {
  const chosen = profile.variants.find((variant) => variant.id === selection[profile.id]);
  if (chosen) return chosen;
  return profile.variants.find((variant) => variant.recommended) ?? profile.variants[0] ?? null;
}

type FitDetailsSelection = {
  modelName: string;
  variantName: string;
  fit: Runnability;
};

function RunnabilityDetailsMenu({
  selection,
  onClose,
}: {
  selection: FitDetailsSelection | null;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const estimate = selection?.fit.estimate ?? null;
  const cpuOffloadBytes =
    estimate?.placements
      .filter((placement) => placement.cpu)
      .reduce((total, placement) => total + placement.paramsBytes, 0) ?? 0;
  const gpuPlacedBytes =
    estimate?.placements
      .filter((placement) => !placement.cpu)
      .reduce((total, placement) => total + placement.paramsBytes, 0) ?? 0;
  const planLabel =
    estimate?.planMode === "concurrent"
      ? t("imageGeneration.local.modelLibrary.planConcurrent")
      : estimate?.planMode === "timeShare"
        ? t("imageGeneration.local.modelLibrary.planTimeShare")
        : t("imageGeneration.local.modelLibrary.planDefaultBackend");

  return (
    <BottomMenu
      isOpen={selection !== null}
      onClose={onClose}
      title={selection ? `${selection.modelName} · ${selection.variantName}` : undefined}
    >
      {selection && estimate ? (
        <div className="space-y-5 pb-2">
          <section>
            <h4 className="text-xs font-semibold text-fg/80">
              {t("imageGeneration.local.modelLibrary.memorySummary")}
            </h4>
            <dl className="mt-2 divide-y divide-fg/8 border-y border-fg/8 text-xs">
              <div className="flex items-baseline justify-between gap-4 py-2">
                <dt className="text-fg/45">
                  {t("imageGeneration.local.modelLibrary.modelWeights")}
                </dt>
                <dd className="font-mono tabular-nums text-fg/75">
                  {formatBytes(estimate.modelBytes)}
                </dd>
              </div>
              <div className="flex items-baseline justify-between gap-4 py-2">
                <dt className="text-fg/45">
                  {t("imageGeneration.local.modelLibrary.gpuPlacedWeights")}
                </dt>
                <dd className="font-mono tabular-nums text-fg/75">{formatBytes(gpuPlacedBytes)}</dd>
              </div>
              <div className="flex items-baseline justify-between gap-4 py-2">
                <dt className="text-fg/45">
                  {t("imageGeneration.local.modelLibrary.cpuOffloadedWeights")}
                </dt>
                <dd
                  className={cn(
                    "font-mono tabular-nums",
                    cpuOffloadBytes > 0 ? "text-warning" : "text-fg/75",
                  )}
                >
                  {cpuOffloadBytes > 0 ? formatBytes(cpuOffloadBytes) : t("common.labels.none")}
                </dd>
              </div>
              <div className="flex items-baseline justify-between gap-4 py-2">
                <dt className="text-fg/45">
                  {t("imageGeneration.local.modelLibrary.availableRam")}
                </dt>
                <dd className="font-mono tabular-nums text-fg/75">
                  {estimate.availableRamBytes ? formatBytes(estimate.availableRamBytes) : "—"}
                </dd>
              </div>
              <div className="flex items-baseline justify-between gap-4 py-2">
                <dt className="text-fg/45">{t("imageGeneration.local.modelLibrary.strategy")}</dt>
                <dd className="text-right text-fg/75">{planLabel}</dd>
              </div>
            </dl>
          </section>

          <section>
            <h4 className="text-xs font-semibold text-fg/80">
              {t("imageGeneration.local.modelLibrary.deviceMemory")}
            </h4>
            {estimate.devices.length > 0 ? (
              <div className="mt-2 divide-y divide-fg/8 border-y border-fg/8">
                {estimate.devices.map((device) => (
                  <div key={device.name} className="flex items-center justify-between gap-4 py-2.5">
                    <div className="min-w-0">
                      <div className="truncate text-xs font-medium text-fg/75">
                        {device.description || device.name}
                      </div>
                      <div className="mt-0.5 font-mono text-[10px] text-fg/35">{device.name}</div>
                    </div>
                    <dl className="grid shrink-0 grid-cols-2 gap-x-5 text-right text-[11px]">
                      <div>
                        <dt className="text-fg/35">
                          {t("imageGeneration.local.modelLibrary.freeMemory")}
                        </dt>
                        <dd className="mt-0.5 font-mono tabular-nums text-fg/65">
                          {formatBytes(device.freeBytes)}
                        </dd>
                      </div>
                      <div>
                        <dt className="text-fg/35">
                          {t("imageGeneration.local.modelLibrary.usableBudget")}
                        </dt>
                        <dd className="mt-0.5 font-mono tabular-nums text-fg/75">
                          {formatBytes(device.budgetBytes)}
                        </dd>
                      </div>
                    </dl>
                  </div>
                ))}
              </div>
            ) : (
              <p className="mt-2 text-xs text-fg/45">
                {t("imageGeneration.local.modelLibrary.noGpuPlacement")}
              </p>
            )}
          </section>

          <section>
            <h4 className="text-xs font-semibold text-fg/80">
              {t("imageGeneration.local.modelLibrary.componentPlacement")}
            </h4>
            <div className="mt-2 divide-y divide-fg/8 border-y border-fg/8">
              {estimate.placements.map((placement) => (
                <div key={placement.component} className="py-2.5">
                  <div className="flex items-baseline justify-between gap-4">
                    <span className="text-xs font-medium text-fg/80">{placement.component}</span>
                    <span className={cn("text-xs", placement.cpu ? "text-warning" : "text-fg/70")}>
                      {placement.targets.join(" + ")}
                      {placement.split ? ` · ${t("imageGeneration.local.modelLibrary.split")}` : ""}
                    </span>
                  </div>
                  <dl className="mt-2 grid grid-cols-3 gap-3 text-[11px]">
                    <div>
                      <dt className="text-fg/35">
                        {t("imageGeneration.local.modelLibrary.weights")}
                      </dt>
                      <dd className="mt-0.5 font-mono tabular-nums text-fg/65">
                        {formatBytes(placement.paramsBytes)}
                      </dd>
                    </div>
                    <div>
                      <dt className="text-fg/35">
                        {t("imageGeneration.local.modelLibrary.computeReserve")}
                      </dt>
                      <dd className="mt-0.5 font-mono tabular-nums text-fg/65">
                        {formatBytes(placement.computeReserveBytes)}
                      </dd>
                    </div>
                    <div>
                      <dt className="text-fg/35">
                        {t("imageGeneration.local.modelLibrary.singleGpuNeed")}
                      </dt>
                      <dd className="mt-0.5 font-mono tabular-nums text-fg/75">
                        {formatBytes(placement.paramsBytes + placement.computeReserveBytes)}
                      </dd>
                    </div>
                  </dl>
                </div>
              ))}
            </div>
          </section>

          <p className="text-[11px] leading-5 text-fg/38">{selection.fit.reason}</p>
        </div>
      ) : null}
    </BottomMenu>
  );
}

export function StableDiffusionSettingsPage() {
  const { t } = useI18n();
  const platform = useMemo(() => getPlatform(), []);
  const downloadQueue = useDownloadQueueOptional();
  const [inventory, setInventory] = useState<RuntimeInventory | null>(null);
  const [catalog, setCatalog] = useState<RuntimeCatalog | null>(null);
  const [gpuDevices, setGpuDevices] = useState<GpuDevice[]>([]);
  const [inventoryLoading, setInventoryLoading] = useState(platform.type === "desktop");
  const [catalogLoading, setCatalogLoading] = useState(platform.type === "desktop");
  const [inventoryError, setInventoryError] = useState<string | null>(null);
  const [catalogError, setCatalogError] = useState<string | null>(null);
  const [selectedRelease, setSelectedRelease] = useState<string | null>(null);
  const [selectedAsset, setSelectedAsset] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [switchingKey, setSwitchingKey] = useState<string | null>(null);
  const [deletingKey, setDeletingKey] = useState<string | null>(null);
  const [installedModels, setInstalledModels] = useState<InstalledModel[]>([]);
  const [selectedVariants, setSelectedVariants] = useState<Record<string, string>>({});
  const [installingModelKey, setInstallingModelKey] = useState<string | null>(null);
  const [uninstallingModelKey, setUninstallingModelKey] = useState<string | null>(null);
  const [runnability, setRunnability] = useState<Record<string, Runnability | "loading">>({});
  const [fitDetails, setFitDetails] = useState<FitDetailsSelection | null>(null);

  const installs = useMemo(
    () => runtimeInstallGroups(downloadQueue?.queue ?? []),
    [downloadQueue?.queue],
  );
  const hasActiveInstall = installs.some((install) => install.active);
  const visibleInstall =
    installs.find((install) => install.active) ?? installs[installs.length - 1] ?? null;
  const visibleInstallItemIds = useMemo(
    () => new Set(visibleInstall?.items.map((item) => item.id) ?? []),
    [visibleInstall],
  );

  const modelInstalls = useMemo(
    () => modelInstallGroups(downloadQueue?.queue ?? []),
    [downloadQueue?.queue],
  );
  const hasActiveModelInstall = modelInstalls.some((install) => install.active);
  const activeModelKeys = useMemo(
    () =>
      new Set(
        modelInstalls
          .filter((install) => install.active)
          .map((install) => `${install.profileId}:${install.variantId}`),
      ),
    [modelInstalls],
  );
  const modelInstallItemIds = useMemo(
    () =>
      new Set(
        modelInstalls.flatMap((install) =>
          install.active ? install.items.map((item) => item.id) : [],
        ),
      ),
    [modelInstalls],
  );
  const installedModelKeys = useMemo(
    () => new Set(installedModels.map((model) => `${model.profileId}:${model.variantId}`)),
    [installedModels],
  );

  const loadInventory = useCallback(async () => {
    if (platform.type === "mobile") return;
    setInventoryLoading(true);
    setInventoryError(null);
    try {
      setInventory(await invoke<RuntimeInventory>("sdcpp_runtime_inventory"));
    } catch (error) {
      setInventoryError(error instanceof Error ? error.message : String(error));
    } finally {
      setInventoryLoading(false);
    }
  }, [platform.type]);

  const loadInstalledModels = useCallback(async () => {
    if (platform.type === "mobile") return;
    const result = await invoke<InstalledModel[]>("sdcpp_installed").catch(() => null);
    if (result) setInstalledModels(result);
  }, [platform.type]);

  const loadCatalog = useCallback(async () => {
    if (platform.type === "mobile") return;
    setCatalogLoading(true);
    setCatalogError(null);
    const [catalogResult, devicesResult] = await Promise.allSettled([
      invoke<RuntimeCatalog>("sdcpp_catalog"),
      invoke<GpuDevice[]>("llamacpp_backend_devices"),
    ]);
    if (catalogResult.status === "fulfilled") {
      setCatalog(catalogResult.value);
      setSelectedRelease((current) =>
        catalogResult.value.runtimeReleases.some((release) => release.tag === current)
          ? current
          : (catalogResult.value.runtimeReleases[0]?.tag ?? null),
      );
    } else {
      setCatalogError(
        catalogResult.reason instanceof Error
          ? catalogResult.reason.message
          : String(catalogResult.reason),
      );
    }
    if (devicesResult.status === "fulfilled") setGpuDevices(devicesResult.value);
    setCatalogLoading(false);
  }, [platform.type]);

  useEffect(() => {
    void loadInventory();
    void loadCatalog();
    void loadInstalledModels();
  }, [loadCatalog, loadInventory, loadInstalledModels]);

  useEffect(() => {
    if (!hasActiveInstall) void loadInventory();
  }, [hasActiveInstall, loadInventory]);

  useEffect(() => {
    if (!hasActiveModelInstall) void loadInstalledModels();
  }, [hasActiveModelInstall, loadInstalledModels]);

  const release =
    catalog?.runtimeReleases.find((candidate) => candidate.tag === selectedRelease) ?? null;
  const recommendedAsset = release ? recommendAsset(platform.os, gpuDevices, release.assets) : null;
  const asset =
    release?.assets.find((candidate) => candidate.name === selectedAsset) ?? recommendedAsset;
  const assetInstalled = Boolean(
    asset &&
    inventory?.installed.some((item) => item.release === release?.tag && item.asset === asset.name),
  );
  const discreteGpu = usableGpuDevices(gpuDevices)[0] ?? null;

  const runtimePairing = useMemo<{ release: string; asset: string } | null>(() => {
    if (inventory?.active) return inventory.active;
    const installedRuntime = inventory?.installed[0];
    if (installedRuntime) {
      return { release: installedRuntime.release, asset: installedRuntime.asset };
    }
    const latest = catalog?.runtimeReleases[0];
    const recommended = latest ? recommendAsset(platform.os, gpuDevices, latest.assets) : null;
    if (latest && recommended) return { release: latest.tag, asset: recommended.name };
    return null;
  }, [inventory, catalog, gpuDevices, platform.os]);
  const hasInstalledEngine = (inventory?.installed.length ?? 0) > 0;

  const installedEnginePairing = useMemo<{ release: string; asset: string } | null>(() => {
    if (inventory?.active) return inventory.active;
    const installedRuntime = inventory?.installed[0];
    return installedRuntime
      ? { release: installedRuntime.release, asset: installedRuntime.asset }
      : null;
  }, [inventory]);

  const fitKey = useCallback(
    (profileId: string, variantId: string) =>
      installedEnginePairing
        ? `${profileId}:${variantId}@${installedEnginePairing.release}:${installedEnginePairing.asset}`
        : null,
    [installedEnginePairing],
  );

  const checkRunnability = useCallback(
    async (profileId: string, variantId: string, pairing: { release: string; asset: string }) => {
      const key = `${profileId}:${variantId}@${pairing.release}:${pairing.asset}`;
      setRunnability((current) => ({ ...current, [key]: "loading" }));
      const result = await invoke<Runnability>("sdcpp_runnability", {
        request: {
          profileId,
          variantId,
          runtimeRelease: pairing.release,
          runtimeAsset: pairing.asset,
        },
      }).catch(() => null);
      setRunnability((current) => {
        const next = { ...current };
        if (result) next[key] = result;
        else delete next[key];
        return next;
      });
    },
    [],
  );

  useEffect(() => {
    if (!installedEnginePairing || !catalog) return;
    for (const profile of catalog.profiles) {
      const variant = selectedVariant(profile, selectedVariants);
      if (!variant) continue;
      if (installedModelKeys.has(`${profile.id}:${variant.id}`)) continue;
      const key = `${profile.id}:${variant.id}@${installedEnginePairing.release}:${installedEnginePairing.asset}`;
      if (runnability[key] !== undefined) continue;
      void checkRunnability(profile.id, variant.id, installedEnginePairing);
    }
  }, [
    catalog,
    selectedVariants,
    installedEnginePairing,
    installedModelKeys,
    runnability,
    checkRunnability,
  ]);

  useEffect(() => {
    setSelectedAsset(recommendedAsset?.name ?? null);
  }, [recommendedAsset?.name, release?.tag]);

  const install = async () => {
    if (!release || !asset) return;
    setInstalling(true);
    try {
      await invoke<string[]>("sdcpp_runtime_install", {
        request: {
          runtimeRelease: release.tag,
          runtimeAsset: asset.name,
        },
      });
      toast.success(
        t("imageGeneration.local.engineManager.queued"),
        `${release.tag} · ${asset.backend.toUpperCase()}`,
      );
    } catch (error) {
      toast.error(
        t("imageGeneration.local.engineManager.installFailed"),
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setInstalling(false);
    }
  };

  const switchRuntime = async (runtime: InstalledRuntime) => {
    const key = `${runtime.release}:${runtime.asset}`;
    setSwitchingKey(key);
    try {
      await invoke("sdcpp_runtime_switch", {
        request: { runtimeRelease: runtime.release, runtimeAsset: runtime.asset },
      });
      await loadInventory();
    } catch (error) {
      toast.error(
        t("imageGeneration.local.engineManager.switchFailed"),
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setSwitchingKey(null);
    }
  };

  const deleteRuntime = async (runtime: InstalledRuntime) => {
    const confirmed = await confirmBottomMenu({
      title: t("imageGeneration.local.engineManager.deleteConfirmTitle", {
        version: runtime.release,
      }),
      message: runtime.active
        ? t("imageGeneration.local.engineManager.deleteActiveConfirm")
        : t("imageGeneration.local.engineManager.deleteConfirm"),
      confirmLabel: t("common.buttons.delete"),
      destructive: true,
    });
    if (!confirmed) return;
    const key = `${runtime.release}:${runtime.asset}`;
    setDeletingKey(key);
    try {
      await invoke("sdcpp_runtime_delete", {
        request: { runtimeRelease: runtime.release, runtimeAsset: runtime.asset },
      });
      await loadInventory();
    } catch (error) {
      toast.error(
        t("imageGeneration.local.engineManager.deleteFailed"),
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setDeletingKey(null);
    }
  };

  const installModel = async (
    profile: CatalogProfile,
    variant: CatalogVariant,
    fit: Runnability | "loading" | undefined,
  ) => {
    if (!runtimePairing) {
      toast.error(
        t("imageGeneration.local.modelLibrary.installFailed"),
        t("imageGeneration.local.modelLibrary.noRuntime"),
      );
      return;
    }
    if (fitOffloadsToCpu(fit)) {
      const confirmed = await confirmBottomMenu({
        title: t("imageGeneration.local.modelLibrary.offloadConfirmTitle", {
          model: profile.displayName,
        }),
        message: t("imageGeneration.local.modelLibrary.offloadConfirm"),
        confirmLabel: t("imageGeneration.local.modelLibrary.download"),
      });
      if (!confirmed) return;
    }
    const key = `${profile.id}:${variant.id}`;
    setInstallingModelKey(key);
    try {
      await invoke<string[]>("sdcpp_install", {
        request: {
          profileId: profile.id,
          variantId: variant.id,
          runtimeRelease: runtimePairing.release,
          runtimeAsset: runtimePairing.asset,
        },
      });
      toast.success(
        t("imageGeneration.local.modelLibrary.queued"),
        `${profile.displayName} · ${variant.label}`,
      );
    } catch (error) {
      toast.error(
        t("imageGeneration.local.modelLibrary.installFailed"),
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setInstallingModelKey(null);
    }
  };

  const uninstallModel = async (profile: CatalogProfile, variant: CatalogVariant) => {
    const confirmed = await confirmBottomMenu({
      title: t("imageGeneration.local.modelLibrary.uninstallConfirmTitle", {
        model: profile.displayName,
      }),
      message: t("imageGeneration.local.modelLibrary.uninstallConfirm"),
      confirmLabel: t("common.buttons.delete"),
      destructive: true,
    });
    if (!confirmed) return;
    const key = `${profile.id}:${variant.id}`;
    setUninstallingModelKey(key);
    try {
      await invoke("sdcpp_uninstall", { profileId: profile.id, variantId: variant.id });
      await loadInstalledModels();
    } catch (error) {
      toast.error(
        t("imageGeneration.local.modelLibrary.uninstallFailed"),
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setUninstallingModelKey(null);
    }
  };

  if (platform.type === "mobile") {
    return (
      <main className="flex min-h-screen items-center justify-center px-6 text-center">
        <div>
          <h1 className="text-lg font-semibold text-fg">
            {t("imageGeneration.local.hub.desktopOnlyTitle")}
          </h1>
          <p className="mt-2 text-sm text-fg/50">
            {t("imageGeneration.local.hub.desktopOnlyBody")}
          </p>
        </div>
      </main>
    );
  }

  return (
    <main className="min-h-screen px-4 pb-24 pt-5">
      <div className="mx-auto w-full max-w-4xl space-y-6">
        <header className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <h1 className="text-lg font-semibold text-fg">
              {t("imageGeneration.local.engineManager.title")}
            </h1>
            <p className="mt-1 max-w-2xl text-sm leading-6 text-fg/50">
              {t("imageGeneration.local.engineManager.description")}
            </p>
          </div>
          <button
            type="button"
            onClick={() => {
              void loadInventory();
              void loadCatalog();
            }}
            disabled={inventoryLoading || catalogLoading}
            className={cn(
              "inline-flex h-9 shrink-0 items-center gap-2 rounded-lg border border-fg/12 px-3 text-sm font-medium text-fg/70 transition-colors hover:bg-fg/5 disabled:opacity-50",
              focusRing,
            )}
          >
            <RefreshCw
              aria-hidden="true"
              className={cn(
                "h-4 w-4",
                (inventoryLoading || catalogLoading) && "animate-spin motion-reduce:animate-none",
              )}
            />
            {t("common.buttons.refresh")}
          </button>
        </header>

        {inventoryError ? (
          <div
            aria-live="polite"
            className="rounded-lg border border-danger/25 bg-danger/5 px-4 py-3 text-sm text-danger/80"
          >
            <div className="font-medium">
              {t("imageGeneration.local.engineManager.inventoryFailed")}
            </div>
            <div className="mt-1 break-words text-xs">{inventoryError}</div>
          </div>
        ) : null}

        {downloadQueue && visibleInstall ? (
          <section aria-live="polite" className="space-y-3">
            <h2 className="text-sm font-semibold text-fg">
              {t("imageGeneration.local.engineManager.downloads")}
            </h2>
            <InlineDownloadCards filter={(item) => visibleInstallItemIds.has(item.id)} />
          </section>
        ) : null}

        <section>
          <h2 className="mb-3 text-sm font-semibold text-fg">
            {t("imageGeneration.local.engineManager.installedTitle")}
          </h2>
          {inventoryLoading && !inventory ? (
            <div
              role="status"
              aria-busy="true"
              className="divide-y divide-fg/8 overflow-hidden rounded-lg border border-fg/10"
            >
              <span className="sr-only">{t("common.labels.loading")}</span>
              {[0, 1].map((row) => (
                <div
                  key={row}
                  className="flex items-center justify-between gap-4 bg-fg/[0.025] px-4 py-4"
                >
                  <div className="min-w-0 space-y-2">
                    <Skeleton className="h-4 w-32" />
                    <Skeleton className="h-3 w-52" />
                  </div>
                  <Skeleton className="h-8 w-28" />
                </div>
              ))}
            </div>
          ) : inventory && inventory.installed.length > 0 ? (
            <div className="divide-y divide-fg/8 overflow-hidden rounded-lg border border-fg/10">
              {inventory.installed.map((runtime) => {
                const key = `${runtime.release}:${runtime.asset}`;
                const switching = switchingKey === key;
                const deleting = deletingKey === key;
                return (
                  <div
                    key={key}
                    className="flex flex-wrap items-center justify-between gap-4 bg-fg/[0.025] px-4 py-4"
                  >
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="font-mono text-sm font-semibold text-fg">
                          {runtime.release}
                        </span>
                        {runtime.active ? (
                          <span className="inline-flex items-center gap-1 text-xs font-medium text-success">
                            <Check aria-hidden="true" className="h-3.5 w-3.5" />
                            {t("imageGeneration.local.engineManager.active")}
                          </span>
                        ) : null}
                      </div>
                      <p className="mt-1 break-all text-xs text-fg/45">
                        {runtime.backend.toUpperCase()} · {formatBytes(runtime.sizeBytes)} ·{" "}
                        {runtime.asset}
                      </p>
                    </div>
                    <div className="flex items-center gap-2">
                      {!runtime.active ? (
                        <button
                          type="button"
                          onClick={() => void switchRuntime(runtime)}
                          disabled={switching || deleting}
                          className={cn(
                            "inline-flex h-8 items-center gap-2 rounded-lg border border-fg/12 px-3 text-xs font-medium text-fg/70 transition-colors hover:bg-fg/5 disabled:opacity-50",
                            focusRing,
                          )}
                        >
                          {switching ? (
                            <Loader2
                              aria-hidden="true"
                              className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none"
                            />
                          ) : null}
                          {t("imageGeneration.local.engineManager.useVersion")}
                        </button>
                      ) : null}
                      <button
                        type="button"
                        onClick={() => void deleteRuntime(runtime)}
                        disabled={switching || deleting}
                        aria-label={t("imageGeneration.local.engineManager.deleteVersion", {
                          version: runtime.release,
                        })}
                        className={cn(
                          "rounded-lg p-2 text-fg/42 transition-colors hover:bg-danger/8 hover:text-danger disabled:opacity-50",
                          focusRing,
                        )}
                      >
                        {deleting ? (
                          <Loader2
                            aria-hidden="true"
                            className="h-4 w-4 animate-spin motion-reduce:animate-none"
                          />
                        ) : (
                          <Trash2 aria-hidden="true" className="h-4 w-4" />
                        )}
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="rounded-lg border border-fg/10 bg-fg/[0.025] px-4 py-5">
              <div className="flex items-start gap-3">
                <HardDrive aria-hidden="true" className="mt-0.5 h-5 w-5 shrink-0 text-fg/40" />
                <div>
                  <h3 className="text-sm font-semibold text-fg">
                    {t("imageGeneration.local.engineManager.notInstalled")}
                  </h3>
                  <p className="mt-1 text-sm leading-6 text-fg/50">
                    {t("imageGeneration.local.engineManager.notInstalledBody")}
                  </p>
                </div>
              </div>
            </div>
          )}
        </section>

        <section className="border-t border-fg/8 pt-5">
          <h2 className="text-sm font-semibold text-fg">
            {inventory?.installed.length
              ? t("imageGeneration.local.engineManager.addVersion")
              : t("imageGeneration.local.engineManager.installTitle")}
          </h2>
          {catalogError ? (
            <div
              aria-live="polite"
              className="mt-3 rounded-lg border border-danger/25 bg-danger/5 px-4 py-3 text-sm text-danger/80"
            >
              <div className="font-medium">
                {t("imageGeneration.local.engineManager.catalogFailed")}
              </div>
              <div className="mt-1 break-words text-xs">{catalogError}</div>
            </div>
          ) : catalogLoading ? (
            <div
              role="status"
              aria-busy="true"
              className="mt-3 overflow-hidden rounded-lg border border-fg/10 bg-fg/[0.02]"
            >
              <span className="sr-only">{t("imageGeneration.local.engineLoadingVariants")}</span>
              <div className="grid items-end gap-3 px-4 py-4 md:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)_auto]">
                <label className="block">
                  <span className="mb-1.5 block text-xs font-medium text-fg/60">
                    {t("imageGeneration.local.engineManager.version")}
                  </span>
                  <div className="flex h-10 w-full items-center rounded-lg border border-fg/12 bg-surface px-3 text-sm text-fg/35">
                    {t("common.labels.loading")}
                  </div>
                </label>
                <label className="block">
                  <span className="mb-1.5 block text-xs font-medium text-fg/60">
                    {t("imageGeneration.local.engineManager.variant")}
                  </span>
                  <div className="flex h-10 w-full items-center rounded-lg border border-fg/12 bg-surface px-3 text-sm text-fg/35">
                    {t("common.labels.loading")}
                  </div>
                </label>
                <Skeleton className="h-10 md:w-36" />
              </div>
              <div className="flex items-center gap-2.5 border-t border-fg/8 px-4 py-3">
                <Skeleton className="h-4 w-4 shrink-0 rounded" />
                <Skeleton className="h-3 w-full max-w-md" />
              </div>
            </div>
          ) : catalog && !catalog.runtimeSupported ? (
            <p className="mt-3 text-sm text-warning">
              {catalog.unsupportedReason || t("imageGeneration.local.hub.unsupportedTitle")}
            </p>
          ) : release && asset && recommendedAsset ? (
            <div className="mt-3 overflow-hidden rounded-lg border border-fg/10 bg-fg/[0.02]">
              <div className="grid items-end gap-3 px-4 py-4 md:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)_auto]">
                <label className="block">
                  <span className="mb-1.5 block text-xs font-medium text-fg/60">
                    {t("imageGeneration.local.engineManager.version")}
                  </span>
                  <select
                    name="sdcpp-engine-version"
                    autoComplete="off"
                    value={release.tag}
                    onChange={(event) => setSelectedRelease(event.target.value)}
                    className={cn(
                      "h-10 w-full rounded-lg border border-fg/12 bg-surface px-3 text-sm text-fg",
                      focusRing,
                    )}
                  >
                    {catalog?.runtimeReleases.map((candidate) => (
                      <option key={candidate.tag} value={candidate.tag}>
                        {candidate.tag}
                        {candidate.prerelease ? " (pre-release)" : ""}
                      </option>
                    ))}
                  </select>
                </label>

                <label className="block">
                  <span className="mb-1.5 block text-xs font-medium text-fg/60">
                    {t("imageGeneration.local.engineManager.variant")}
                  </span>
                  <select
                    name="sdcpp-engine-variant"
                    autoComplete="off"
                    value={asset.name}
                    onChange={(event) => setSelectedAsset(event.target.value)}
                    className={cn(
                      "h-10 w-full rounded-lg border border-fg/12 bg-surface px-3 text-sm text-fg",
                      focusRing,
                    )}
                  >
                    {release.assets.map((candidate) => (
                      <option key={candidate.name} value={candidate.name}>
                        {candidate.backend.toUpperCase()} ·{" "}
                        {formatBytes(totalAssetBytes(candidate))}
                        {candidate.name === recommendedAsset.name
                          ? ` · ${t("imageGeneration.local.engineManager.recommended")}`
                          : ""}
                      </option>
                    ))}
                  </select>
                </label>
                <button
                  type="button"
                  onClick={() => void install()}
                  disabled={installing || hasActiveInstall || assetInstalled}
                  className={cn(
                    "inline-flex h-10 items-center justify-center gap-2 rounded-lg px-4 text-sm font-semibold transition-[filter] md:min-w-36",
                    assetInstalled
                      ? "cursor-default border border-fg/12 bg-fg/5 text-fg/55"
                      : "bg-accent text-bg hover:brightness-110 disabled:opacity-50",
                    focusRing,
                  )}
                >
                  {assetInstalled ? (
                    <Check aria-hidden="true" className="h-4 w-4 text-success" />
                  ) : installing || hasActiveInstall ? (
                    <Loader2
                      aria-hidden="true"
                      className="h-4 w-4 animate-spin motion-reduce:animate-none"
                    />
                  ) : (
                    <Download aria-hidden="true" className="h-4 w-4" />
                  )}
                  {assetInstalled
                    ? t("imageGeneration.local.engineManager.alreadyInstalled")
                    : t("imageGeneration.local.engineManager.install")}
                </button>
              </div>

              <div className="flex flex-col gap-2 border-t border-fg/8 px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
                <div className="flex min-w-0 items-start gap-2.5">
                  <Cpu aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0 text-fg/40" />
                  <p className="min-w-0 text-xs leading-5 text-fg/50">
                    <span className="font-medium text-fg/75">{asset.backend.toUpperCase()}</span>
                    <span className="mx-1.5 text-fg/25">·</span>
                    {asset.name === recommendedAsset.name
                      ? discreteGpu
                        ? t("imageGeneration.local.engineManager.detectedGpu", {
                            device: discreteGpu.description || discreteGpu.name,
                          })
                        : t("imageGeneration.local.engineManager.noDiscreteGpu")
                      : t("imageGeneration.local.engineManager.manualVariant", {
                          backend: recommendedAsset.backend.toUpperCase(),
                        })}
                  </p>
                </div>
                <div className="flex shrink-0 items-center gap-2 pl-6 text-xs text-fg/38 sm:pl-0">
                  <span className="max-w-72 truncate" title={asset.name}>
                    {asset.name}
                  </span>
                  <span aria-hidden="true">·</span>
                  <span className="tabular-nums">{formatBytes(totalAssetBytes(asset))}</span>
                </div>
              </div>
            </div>
          ) : null}
        </section>

        <section className="border-t border-fg/8 pt-5">
          <h2 className="text-sm font-semibold text-fg">
            {t("imageGeneration.local.modelLibrary.title")}
          </h2>
          <p className="mt-1 max-w-2xl text-sm leading-6 text-fg/50">
            {t("imageGeneration.local.modelLibrary.description")}
          </p>

          {downloadQueue && modelInstallItemIds.size > 0 ? (
            <div className="mt-3">
              <InlineDownloadCards filter={(item) => modelInstallItemIds.has(item.id)} />
            </div>
          ) : null}

          {catalogError ? (
            <div
              aria-live="polite"
              className="mt-3 rounded-lg border border-danger/25 bg-danger/5 px-4 py-3 text-sm text-danger/80"
            >
              <div className="font-medium">
                {t("imageGeneration.local.engineManager.catalogFailed")}
              </div>
              <div className="mt-1 break-words text-xs">{catalogError}</div>
            </div>
          ) : catalogLoading ? (
            <div role="status" aria-busy="true" className="mt-3 space-y-3">
              <span className="sr-only">{t("common.labels.loading")}</span>
              {[0, 1].map((row) => (
                <div
                  key={row}
                  className="flex items-center justify-between gap-4 rounded-lg border border-fg/10 bg-fg/[0.02] px-4 py-4"
                >
                  <div className="min-w-0 space-y-2">
                    <Skeleton className="h-4 w-40" />
                    <Skeleton className="h-3 w-64" />
                  </div>
                  <Skeleton className="h-10 w-32" />
                </div>
              ))}
            </div>
          ) : catalog && !catalog.runtimeSupported ? (
            <p className="mt-3 text-sm text-warning">
              {catalog.unsupportedReason || t("imageGeneration.local.hub.unsupportedTitle")}
            </p>
          ) : catalog && catalog.profiles.length > 0 ? (
            <div className="mt-3 space-y-3">
              {!hasInstalledEngine ? (
                <p className="text-xs leading-5 text-fg/45">
                  {t("imageGeneration.local.modelLibrary.engineNeededForFit")}
                </p>
              ) : null}
              {catalog.profiles.map((profile) => {
                const variant = selectedVariant(profile, selectedVariants);
                if (!variant) return null;
                const key = `${profile.id}:${variant.id}`;
                const installed = installedModelKeys.has(key);
                const installing = installingModelKey === key || activeModelKeys.has(key);
                const uninstalling = uninstallingModelKey === key;
                const runKey = fitKey(profile.id, variant.id);
                const fit = runKey ? runnability[runKey] : undefined;
                const fitInfo = installed ? null : classifyFit(fit);
                const resolvedFit = fit && fit !== "loading" ? fit : null;
                const estimate = resolvedFit?.estimate ?? null;
                return (
                  <div
                    key={profile.id}
                    className="overflow-hidden rounded-lg border border-fg/10 bg-fg/[0.02]"
                  >
                    <div className="px-4 py-4">
                      <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
                        <h3 className="text-sm font-semibold text-fg">{profile.displayName}</h3>
                        {fitInfo ? (
                          <span
                            title={fit && fit !== "loading" ? fit.reason : undefined}
                            className={cn(
                              "inline-flex items-center gap-1.5 text-xs font-medium",
                              FIT_TEXT[fitInfo.tone],
                            )}
                          >
                            <span
                              aria-hidden="true"
                              className={cn("h-1.5 w-1.5 rounded-full", FIT_DOT[fitInfo.tone])}
                            />
                            {t(
                              `imageGeneration.local.modelLibrary.${fitInfo.key}` as TranslationKey,
                            )}
                          </span>
                        ) : null}
                      </div>
                      <p className="mt-1 text-xs leading-5 text-fg/50">{profile.description}</p>
                      {profile.requiresReferenceImage ? (
                        <p className="mt-1 text-xs leading-5 text-fg/40">
                          {t("imageGeneration.local.modelLibrary.needsReference")}
                        </p>
                      ) : null}
                      <div className="mt-3 flex flex-col gap-3 sm:flex-row sm:items-end">
                        <label className="block sm:flex-1">
                          <span className="mb-1.5 block text-xs font-medium text-fg/60">
                            {t("imageGeneration.local.modelLibrary.variant")}
                          </span>
                          <select
                            name={`sdcpp-model-${profile.id}`}
                            autoComplete="off"
                            value={variant.id}
                            onChange={(event) =>
                              setSelectedVariants((current) => ({
                                ...current,
                                [profile.id]: event.target.value,
                              }))
                            }
                            className={cn(
                              "h-10 w-full rounded-lg border border-fg/12 bg-surface px-3 text-sm text-fg",
                              focusRing,
                            )}
                          >
                            {profile.variants.map((candidate) => (
                              <option key={candidate.id} value={candidate.id}>
                                {candidate.label} · {formatBytes(candidate.downloadBytes)}
                                {installedModelKeys.has(`${profile.id}:${candidate.id}`)
                                  ? ` · ${t("imageGeneration.local.modelLibrary.installedShort")}`
                                  : ""}
                              </option>
                            ))}
                          </select>
                        </label>
                        <div className="flex items-center gap-2">
                          {installed ? (
                            <>
                              <span className="inline-flex h-10 items-center gap-2 rounded-lg border border-fg/12 bg-fg/5 px-4 text-sm font-semibold text-fg/55">
                                <Check aria-hidden="true" className="h-4 w-4 text-success" />
                                {t("imageGeneration.local.modelLibrary.installedShort")}
                              </span>
                              <button
                                type="button"
                                onClick={() => void uninstallModel(profile, variant)}
                                disabled={uninstalling}
                                aria-label={t("imageGeneration.local.modelLibrary.uninstallModel", {
                                  model: profile.displayName,
                                })}
                                className={cn(
                                  "rounded-lg p-2 text-fg/42 transition-colors hover:bg-danger/8 hover:text-danger disabled:opacity-50",
                                  focusRing,
                                )}
                              >
                                {uninstalling ? (
                                  <Loader2
                                    aria-hidden="true"
                                    className="h-4 w-4 animate-spin motion-reduce:animate-none"
                                  />
                                ) : (
                                  <Trash2 aria-hidden="true" className="h-4 w-4" />
                                )}
                              </button>
                            </>
                          ) : (
                            <button
                              type="button"
                              onClick={() => void installModel(profile, variant, fit)}
                              disabled={installing}
                              className={cn(
                                "inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-accent px-4 text-sm font-semibold text-bg transition-[filter] hover:brightness-110 disabled:opacity-50 sm:min-w-36",
                                focusRing,
                              )}
                            >
                              {installing ? (
                                <Loader2
                                  aria-hidden="true"
                                  className="h-4 w-4 animate-spin motion-reduce:animate-none"
                                />
                              ) : (
                                <Download aria-hidden="true" className="h-4 w-4" />
                              )}
                              {installing
                                ? t("imageGeneration.local.modelLibrary.installing")
                                : t("imageGeneration.local.modelLibrary.download")}
                            </button>
                          )}
                        </div>
                      </div>
                    </div>

                    <div className="flex min-h-11 flex-wrap items-center justify-between gap-x-4 gap-y-2 border-t border-fg/8 px-4 py-2 text-xs text-fg/45">
                      <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                        <span>{profile.license}</span>
                        {profile.sourceUrl ? (
                          <>
                            <span aria-hidden="true">·</span>
                            <button
                              type="button"
                              onClick={() => void openExternalUrl(profile.sourceUrl)}
                              className={cn(
                                "underline-offset-2 transition-colors hover:text-fg/70 hover:underline",
                                focusRing,
                              )}
                            >
                              {t("imageGeneration.local.modelLibrary.modelCard")}
                            </button>
                          </>
                        ) : null}
                      </div>
                      {estimate && resolvedFit ? (
                        <button
                          type="button"
                          onClick={() =>
                            setFitDetails({
                              modelName: profile.displayName,
                              variantName: variant.label,
                              fit: resolvedFit,
                            })
                          }
                          className={cn(
                            "inline-flex h-7 items-center gap-1.5 rounded-md px-2 font-medium text-fg/55 transition-colors hover:bg-fg/5 hover:text-fg/80",
                            focusRing,
                          )}
                        >
                          {t("imageGeneration.local.modelLibrary.more")}
                          <ChevronRight aria-hidden="true" className="h-3.5 w-3.5" />
                        </button>
                      ) : null}
                    </div>
                  </div>
                );
              })}
            </div>
          ) : null}
        </section>
      </div>
      <RunnabilityDetailsMenu selection={fitDetails} onClose={() => setFitDetails(null)} />
    </main>
  );
}
