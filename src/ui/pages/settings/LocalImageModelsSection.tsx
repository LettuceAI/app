import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, Download, HardDrive, Loader2, RefreshCw } from "lucide-react";

import { useI18n } from "../../../core/i18n/context";
import { SETTINGS_UPDATED_EVENT } from "../../../core/storage/repo";
import { getPlatform } from "../../../core/utils/platform";
import { cn } from "../../design-tokens";
import { toast } from "../../components/toast";

type RuntimeAsset = {
  name: string;
  backend: string;
  bytes: number;
  sha256: string | null;
  dependencies: { name: string; bytes: number; sha256: string | null }[];
};

type RuntimeRelease = {
  tag: string;
  name: string;
  publishedAt: string | null;
  prerelease: boolean;
  assets: RuntimeAsset[];
};

type ModelVariant = {
  id: string;
  label: string;
  description: string;
  downloadBytes: number;
  installed: boolean;
};

type ModelProfile = {
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
  variants: ModelVariant[];
};

type Catalog = {
  runtimeSupported: boolean;
  unsupportedReason: string | null;
  runtimeReleases: RuntimeRelease[];
  profiles: ModelProfile[];
};

type RunnabilityResult = {
  status: "notInstalled" | "passed" | "failed";
  method: string;
  exact: boolean;
  elapsedMs: number | null;
  reason: string;
};

function formatBytes(bytes: number): string {
  const gib = bytes / 1024 ** 3;
  return gib >= 1 ? `${gib.toFixed(1)} GiB` : `${(bytes / 1024 ** 2).toFixed(0)} MiB`;
}

export function LocalImageModelsSection() {
  const { t } = useI18n();
  const isMobile = useMemo(() => getPlatform().type === "mobile", []);
  const [catalog, setCatalog] = useState<Catalog | null>(null);
  const [loading, setLoading] = useState(!isMobile);
  const [error, setError] = useState<string | null>(null);
  const [profileId, setProfileId] = useState<string | null>(null);
  const [variantId, setVariantId] = useState<string | null>(null);
  const [releaseTag, setReleaseTag] = useState<string | null>(null);
  const [assetName, setAssetName] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [probing, setProbing] = useState(false);
  const [probeSize, setProbeSize] = useState("1024x1024");
  const [probeReferences, setProbeReferences] = useState(0);
  const [probeResult, setProbeResult] = useState<RunnabilityResult | null>(null);

  const loadCatalog = useCallback(async () => {
    if (isMobile) return;
    setLoading(true);
    setError(null);
    try {
      const next = await invoke<Catalog>("sdcpp_catalog");
      setCatalog(next);
      setProfileId((current) => current ?? next.profiles[0]?.id ?? null);
      setReleaseTag((current) => current ?? next.runtimeReleases[0]?.tag ?? null);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setLoading(false);
    }
  }, [isMobile]);

  useEffect(() => {
    void loadCatalog();
  }, [loadCatalog]);

  useEffect(() => {
    if (isMobile) return;
    let refreshTimer: ReturnType<typeof setTimeout> | null = null;
    const handleSettingsUpdated = () => {
      if (refreshTimer) clearTimeout(refreshTimer);
      refreshTimer = setTimeout(() => void loadCatalog(), 300);
    };
    window.addEventListener(SETTINGS_UPDATED_EVENT, handleSettingsUpdated);
    return () => {
      if (refreshTimer) clearTimeout(refreshTimer);
      window.removeEventListener(SETTINGS_UPDATED_EVENT, handleSettingsUpdated);
    };
  }, [isMobile, loadCatalog]);

  const profile = catalog?.profiles.find((candidate) => candidate.id === profileId) ?? null;
  const release =
    catalog?.runtimeReleases.find((candidate) => candidate.tag === releaseTag) ?? null;
  const asset = release?.assets.find((candidate) => candidate.name === assetName) ?? null;
  const variant = profile?.variants.find((candidate) => candidate.id === variantId) ?? null;

  useEffect(() => {
    if (!profile) return;
    const recommended = profile.variants.find((candidate) =>
      candidate.label.toLowerCase().includes("recommended"),
    );
    setVariantId((current) =>
      profile.variants.some((candidate) => candidate.id === current)
        ? current
        : (recommended?.id ?? profile.variants[0]?.id ?? null),
    );
    setProbeSize(`${profile.defaultWidth}x${profile.defaultHeight}`);
    setProbeReferences(profile.requiresReferenceImage ? 1 : 0);
    setProbeResult(null);
  }, [profile]);

  useEffect(() => {
    if (!release) return;
    setAssetName((current) =>
      release.assets.some((candidate) => candidate.name === current) ? current : null,
    );
  }, [release]);

  if (isMobile) return null;

  const install = async () => {
    if (!profile || !variant || !release || !asset) return;
    setInstalling(true);
    try {
      await invoke<string[]>("sdcpp_install", {
        request: {
          profileId: profile.id,
          variantId: variant.id,
          runtimeRelease: release.tag,
          runtimeAsset: asset.name,
        },
      });
      toast.success(
        t("imageGeneration.local.installQueued"),
        `${profile.displayName} · ${variant.label} · ${asset.backend.toUpperCase()}`,
      );
    } catch (caught) {
      toast.error(
        t("imageGeneration.local.engineInstallFailed"),
        caught instanceof Error ? caught.message : String(caught),
      );
    } finally {
      setInstalling(false);
    }
  };

  const runFitTest = async () => {
    if (!profile || !variant || !release || !asset) return;
    const [width, height] = probeSize.split("x").map(Number);
    setProbing(true);
    setProbeResult(null);
    try {
      const result = await invoke<RunnabilityResult>("sdcpp_runnability", {
        request: {
          profileId: profile.id,
          variantId: variant.id,
          runtimeRelease: release.tag,
          runtimeAsset: asset.name,
          width,
          height,
          referenceImageCount: probeReferences,
          loras: [],
        },
      });
      setProbeResult(result);
    } catch (caught) {
      setProbeResult({
        status: "failed",
        method: "stableDiffusionCppRuntimeProbe",
        exact: false,
        elapsedMs: null,
        reason: caught instanceof Error ? caught.message : String(caught),
      });
    } finally {
      setProbing(false);
    }
  };

  return (
    <section className="overflow-hidden rounded-[12px] border border-fg/10 bg-fg/5">
      <div className="flex items-start justify-between gap-4 border-b border-fg/8 px-4 py-4">
        <div>
          <h3 className="text-sm font-semibold text-fg">
            {t("imageGeneration.local.managedTitle")}
          </h3>
          <p className="mt-1 max-w-3xl text-sm leading-6 text-fg/52">
            {t("imageGeneration.local.managedDescription")}
          </p>
        </div>
        <button
          type="button"
          onClick={() => void loadCatalog()}
          disabled={loading}
          className="rounded-[8px] border border-fg/10 p-2 text-fg/45 transition-colors hover:bg-fg/5 hover:text-fg disabled:opacity-50"
          aria-label={t("imageGeneration.local.refreshBuilds")}
        >
          <RefreshCw className={cn("h-4 w-4", loading && "animate-spin")} />
        </button>
      </div>

      {loading ? (
        <div className="flex items-center gap-2 px-4 py-5 text-sm text-fg/50">
          <Loader2 className="h-4 w-4 animate-spin" />
          {t("imageGeneration.local.engineLoadingVariants")}
        </div>
      ) : error ? (
        <div className="px-4 py-4 text-sm text-danger/80">{error}</div>
      ) : !catalog?.runtimeSupported ? (
        <div className="px-4 py-4 text-sm text-fg/55">{catalog?.unsupportedReason}</div>
      ) : (
        <div className="space-y-5 px-4 py-4">
          <div className="grid gap-px overflow-hidden rounded-[9px] border border-fg/10 bg-fg/10 lg:grid-cols-2">
            {catalog.profiles.map((candidate) => {
              const selected = candidate.id === profile?.id;
              return (
                <button
                  key={candidate.id}
                  type="button"
                  onClick={() => setProfileId(candidate.id)}
                  className={cn(
                    "bg-surface px-3.5 py-3 text-left transition-colors",
                    selected ? "bg-accent/8" : "hover:bg-surface-el/55",
                  )}
                >
                  <div className="flex items-start justify-between gap-3">
                    <div>
                      <div className="text-sm font-medium text-fg">{candidate.displayName}</div>
                      <p className="mt-1 text-xs leading-5 text-fg/48">{candidate.description}</p>
                    </div>
                    {selected ? <Check className="mt-0.5 h-4 w-4 shrink-0 text-accent" /> : null}
                  </div>
                  <div className="mt-2 text-[11px] text-fg/40">
                    {candidate.recommendedForScenes
                      ? t("imageGeneration.local.sceneReady")
                      : t("imageGeneration.local.textToImage")}
                    {candidate.maxReferenceImages !== null && candidate.maxReferenceImages > 0
                      ? ` · ${t("imageGeneration.local.referenceCount", { count: candidate.maxReferenceImages })}`
                      : ""}
                    {candidate.supportsLora ? ` · ${t("imageGeneration.local.loraReady")}` : ""}
                  </div>
                </button>
              );
            })}
          </div>

          <div className="grid gap-4 lg:grid-cols-3">
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-fg/55">
                {t("imageGeneration.local.modelVariant")}
              </span>
              <select
                value={variantId ?? ""}
                onChange={(event) => setVariantId(event.target.value)}
                className="h-10 w-full rounded-[8px] border border-fg/12 bg-surface px-3 text-sm text-fg outline-none focus:border-accent/50"
              >
                {profile?.variants.map((candidate) => (
                  <option key={candidate.id} value={candidate.id}>
                    {candidate.label} · {formatBytes(candidate.downloadBytes)}
                  </option>
                ))}
              </select>
            </label>

            <label className="space-y-1.5">
              <span className="text-xs font-medium text-fg/55">
                {t("imageGeneration.local.engineVersion")}
              </span>
              <select
                value={releaseTag ?? ""}
                onChange={(event) => setReleaseTag(event.target.value)}
                className="h-10 w-full rounded-[8px] border border-fg/12 bg-surface px-3 text-sm text-fg outline-none focus:border-accent/50"
              >
                {catalog.runtimeReleases.map((candidate, index) => (
                  <option key={candidate.tag} value={candidate.tag}>
                    {candidate.tag}
                    {index === 0 ? ` · ${t("imageGeneration.local.latest")}` : ""}
                  </option>
                ))}
              </select>
            </label>

            <label className="space-y-1.5">
              <span className="text-xs font-medium text-fg/55">
                {t("imageGeneration.local.computeBackend")}
              </span>
              <select
                value={assetName ?? ""}
                onChange={(event) => setAssetName(event.target.value)}
                className="h-10 w-full rounded-[8px] border border-fg/12 bg-surface px-3 text-sm text-fg outline-none focus:border-accent/50"
              >
                <option value="" disabled>
                  {t("imageGeneration.local.computeBackend")}
                </option>
                {release?.assets.map((candidate) => (
                  <option key={candidate.name} value={candidate.name}>
                    {candidate.backend.toUpperCase()} · {candidate.name} ·{" "}
                    {formatBytes(
                      candidate.bytes +
                        candidate.dependencies.reduce(
                          (total, dependency) => total + dependency.bytes,
                          0,
                        ),
                    )}
                  </option>
                ))}
              </select>
            </label>
          </div>

          {variant && profile ? (
            <div className="space-y-4 border-t border-fg/8 pt-4">
              <div className="grid gap-3 lg:grid-cols-[1fr_180px_180px_auto] lg:items-end">
                <div className="flex min-w-0 items-center gap-2 pb-2 text-xs text-fg/48">
                  <HardDrive className="h-4 w-4 shrink-0" />
                  <span>{variant.description}</span>
                </div>
                <label className="space-y-1.5">
                  <span className="text-xs font-medium text-fg/55">
                    {t("imageGeneration.local.fitResolution")}
                  </span>
                  <select
                    value={probeSize}
                    onChange={(event) => setProbeSize(event.target.value)}
                    className="h-9 w-full rounded-[8px] border border-fg/12 bg-surface px-3 text-sm text-fg outline-none focus:border-accent/50"
                  >
                    {["768x768", "1024x1024", "1152x896", "896x1152"].map((size) => (
                      <option key={size} value={size}>
                        {size}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="space-y-1.5">
                  <span className="text-xs font-medium text-fg/55">
                    {t("imageGeneration.local.fitReferences")}
                  </span>
                  <input
                    type="number"
                    min={profile.requiresReferenceImage ? 1 : 0}
                    value={probeReferences}
                    onChange={(event) =>
                      setProbeReferences(Math.max(0, Number(event.target.value)))
                    }
                    disabled={profile.maxReferenceImages === 0}
                    className="h-9 w-full rounded-[8px] border border-fg/12 bg-surface px-3 text-sm text-fg outline-none focus:border-accent/50"
                  />
                </label>
                <div className="flex gap-2">
                  {variant.installed ? (
                    <button
                      type="button"
                      onClick={() => void runFitTest()}
                      disabled={probing || !asset}
                      className="flex h-9 shrink-0 items-center gap-2 rounded-[8px] border border-fg/14 px-3.5 text-sm font-medium text-fg/75 transition-colors hover:bg-fg/5 disabled:opacity-50"
                    >
                      {probing ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                      {t("imageGeneration.local.testExactFit")}
                    </button>
                  ) : null}
                  <button
                    type="button"
                    onClick={() => void install()}
                    disabled={installing || !asset}
                    className="flex h-9 shrink-0 items-center gap-2 rounded-[8px] bg-accent px-3.5 text-sm font-medium text-bg transition-colors hover:brightness-110 disabled:opacity-50"
                  >
                    {installing ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <Download className="h-4 w-4" />
                    )}
                    {t("imageGeneration.local.installSelected")}
                  </button>
                </div>
              </div>
              {probeResult ? (
                <div
                  className={cn(
                    "rounded-[8px] border px-3 py-2 text-xs leading-5",
                    probeResult.status === "passed"
                      ? "border-success/20 bg-success/5 text-success/85"
                      : probeResult.status === "notInstalled"
                        ? "border-warning/20 bg-warning/5 text-warning/85"
                        : "border-danger/20 bg-danger/5 text-danger/85",
                  )}
                >
                  {probeResult.reason}
                </div>
              ) : (
                <p className="text-xs leading-5 text-fg/42">
                  {t("imageGeneration.local.fitTestDescription")}
                </p>
              )}
            </div>
          ) : null}
        </div>
      )}
    </section>
  );
}
