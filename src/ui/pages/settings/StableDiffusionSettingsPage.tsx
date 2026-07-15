import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, Cpu, Download, HardDrive, Loader2, RefreshCw, Trash2 } from "lucide-react";

import {
  useDownloadQueueOptional,
  type QueuedDownload,
} from "../../../core/downloads/DownloadQueueContext";
import { useI18n } from "../../../core/i18n/context";
import { getPlatform } from "../../../core/utils/platform";
import { confirmBottomMenu } from "../../components/ConfirmBottomMenu";
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

type RuntimeCatalog = {
  runtimeSupported: boolean;
  unsupportedReason: string | null;
  runtimeReleases: RuntimeRelease[];
};

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

  const loadCatalog = useCallback(async () => {
    if (platform.type === "mobile") return;
    setCatalogLoading(true);
    setCatalogError(null);
    const [catalogResult, devicesResult] = await Promise.allSettled([
      invoke<RuntimeCatalog>("sdcpp_runtime_catalog"),
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
  }, [loadCatalog, loadInventory]);

  useEffect(() => {
    if (!hasActiveInstall) void loadInventory();
  }, [hasActiveInstall, loadInventory]);

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
      </div>
    </main>
  );
}
