import { useEffect, useMemo, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  AlertTriangle,
  CheckCircle2,
  Cpu,
  Download,
  FolderOpen,
  Loader2,
  Mic2,
  Play,
  RefreshCw,
  Search,
  Wrench,
} from "lucide-react";

import {
  kokoroDefaultAssetRoot,
  kokoroInstallModel,
  kokoroInstallVoice,
  kokoroListAvailableVoices,
  kokoroListInstalledVoices,
  kokoroPreview,
  kokoroSupportedVariants,
  kokoroTokenizePreview,
  kokoroValidateAssets,
  type KokoroAssetStatus,
  type KokoroAvailableVoice,
  type KokoroInstalledVoice,
  type KokoroModelVariant,
  type KokoroSupportedVariant,
  type KokoroTokenizePreview,
  type KokoroVoiceBlendEntry,
} from "../../../core/storage/audioProviders";
import {
  useDownloadQueue,
  type QueuedDownload,
} from "../../../core/downloads/DownloadQueueContext";
import { getPlatform } from "../../../core/utils/platform";
import { cn, radius, typography } from "../../design-tokens";

const STORAGE_KEY = "kokoro-test-page:v2";

type PersistedState = {
  assetRoot: string;
  variant: KokoroModelVariant | "";
  voiceId: string;
  previewText: string;
  speed: string;
  espeakBinPath: string;
  espeakDataPath: string;
  voiceSearch: string;
  voiceWeights: Record<string, string>;
};

type VariantStatusMap = Partial<Record<KokoroModelVariant, KokoroAssetStatus>>;

const DEFAULT_TEXT =
  "The rain keeps falling over Auric, and the ledger should not have survived the fire.";

function readPersistedState(): PersistedState {
  const fallback: PersistedState = {
    assetRoot: "",
    variant: "",
    voiceId: "",
    previewText: DEFAULT_TEXT,
    speed: "1.0",
    espeakBinPath: "",
    espeakDataPath: "",
    voiceSearch: "",
    voiceWeights: {},
  };

  if (typeof window === "undefined") return fallback;

  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return fallback;
    const parsed = JSON.parse(raw) as Partial<PersistedState>;
    return {
      assetRoot: parsed.assetRoot ?? fallback.assetRoot,
      variant: parsed.variant ?? fallback.variant,
      voiceId: parsed.voiceId ?? fallback.voiceId,
      previewText: parsed.previewText ?? fallback.previewText,
      speed: parsed.speed ?? fallback.speed,
      espeakBinPath: parsed.espeakBinPath ?? fallback.espeakBinPath,
      espeakDataPath: parsed.espeakDataPath ?? fallback.espeakDataPath,
      voiceSearch: parsed.voiceSearch ?? fallback.voiceSearch,
      voiceWeights:
        parsed.voiceWeights && typeof parsed.voiceWeights === "object" ? parsed.voiceWeights : {},
    };
  } catch {
    return fallback;
  }
}

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${value >= 100 ? value.toFixed(0) : value.toFixed(1)} ${units[unitIndex]}`;
}

function formatProgressRatio(downloaded: number, total: number) {
  if (total <= 0) return "Waiting for size…";
  return `${formatBytes(downloaded)} / ${formatBytes(total)}`;
}

function StatusPill({
  tone,
  children,
}: {
  tone: "neutral" | "success" | "warning" | "info";
  children: React.ReactNode;
}) {
  const tones = {
    neutral: "border-fg/10 bg-fg/5 text-fg/70",
    success: "border-accent/30 bg-accent/10 text-accent/80",
    warning: "border-warning/30 bg-warning/10 text-warning/80",
    info: "border-info/30 bg-info/10 text-info/80",
  };

  return (
    <div
      className={cn(
        "inline-flex items-center gap-2 rounded-full border px-3 py-1",
        typography.caption.size,
        typography.caption.weight,
        tones[tone],
      )}
    >
      {children}
    </div>
  );
}

function SectionCard({
  title,
  subtitle,
  icon,
  children,
}: {
  title: string;
  subtitle: string;
  icon: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <section className={cn("rounded-2xl border border-fg/10 bg-fg/[0.03] p-4", radius.lg)}>
      <div className="mb-4 flex items-start gap-3">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-fg/10 bg-fg/5 text-fg/70">
          <span className="[&_svg]:h-4 [&_svg]:w-4">{icon}</span>
        </div>
        <div className="min-w-0">
          <h2 className={cn(typography.h3.size, typography.h3.weight, "text-fg")}>{title}</h2>
          <p className={cn("mt-1 text-fg/50", typography.bodySmall.size)}>{subtitle}</p>
        </div>
      </div>
      {children}
    </section>
  );
}

function FieldLabel({ children }: { children: string }) {
  return (
    <label
      className={cn("mb-2 block text-fg/45", typography.overline.size, typography.overline.weight)}
    >
      {children}
    </label>
  );
}

export function KokoroTestPage() {
  const initialState = useMemo(() => readPersistedState(), []);
  const platform = useMemo(() => getPlatform(), []);
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const shouldAutoplayRef = useRef(false);
  const prevKokoroQueueRef = useRef<QueuedDownload[]>([]);
  const { queue, cancelItem, dismissItem } = useDownloadQueue();

  const [managedRoot, setManagedRoot] = useState("");
  const [assetRoot, setAssetRoot] = useState(initialState.assetRoot);
  const [variant, setVariant] = useState<KokoroModelVariant | "">(initialState.variant);
  const [voiceId, setVoiceId] = useState(initialState.voiceId);
  const [previewText, setPreviewText] = useState(initialState.previewText);
  const [speed, setSpeed] = useState(initialState.speed);
  const [espeakBinPath, setEspeakBinPath] = useState(initialState.espeakBinPath);
  const [espeakDataPath, setEspeakDataPath] = useState(initialState.espeakDataPath);
  const [voiceSearch, setVoiceSearch] = useState(initialState.voiceSearch);
  const [voiceWeights, setVoiceWeights] = useState<Record<string, string>>(initialState.voiceWeights);

  const [variants, setVariants] = useState<KokoroSupportedVariant[]>([]);
  const [variantStatuses, setVariantStatuses] = useState<VariantStatusMap>({});
  const [installedVoices, setInstalledVoices] = useState<KokoroInstalledVoice[]>([]);
  const [availableVoices, setAvailableVoices] = useState<KokoroAvailableVoice[]>([]);
  const [previewSrc, setPreviewSrc] = useState<string | null>(null);
  const [tokenizePreview, setTokenizePreview] = useState<KokoroTokenizePreview | null>(null);
  const [isBooting, setIsBooting] = useState(true);
  const [isRefreshingAssets, setIsRefreshingAssets] = useState(false);
  const [isRefreshingCatalog, setIsRefreshingCatalog] = useState(false);
  const [isTokenizing, setIsTokenizing] = useState(false);
  const [isPreviewing, setIsPreviewing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void (async () => {
      try {
        const [supportedVariants, defaultRoot] = await Promise.all([
          kokoroSupportedVariants(),
          kokoroDefaultAssetRoot(),
        ]);

        setVariants(supportedVariants);
        setManagedRoot(defaultRoot);
        setAssetRoot((current) => current || defaultRoot);
        setVariant((current) => current || supportedVariants[0]?.id || "");
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setIsBooting(false);
      }
    })();
  }, []);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const payload: PersistedState = {
      assetRoot,
      variant,
      voiceId,
      previewText,
      speed,
      espeakBinPath,
      espeakDataPath,
      voiceSearch,
      voiceWeights,
    };
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(payload));
  }, [assetRoot, variant, voiceId, previewText, speed, espeakBinPath, espeakDataPath, voiceSearch, voiceWeights]);

  useEffect(() => {
    if (!previewSrc || !shouldAutoplayRef.current || !audioRef.current) return;
    shouldAutoplayRef.current = false;
    void audioRef.current.play().catch(() => {});
  }, [previewSrc]);

  useEffect(() => {
    if (!assetRoot.trim() || variants.length === 0) return;
    void refreshInstalledState(assetRoot);
    void refreshVoiceCatalog(assetRoot);
  }, [assetRoot, variants.length]);

  const trimmedAssetRoot = assetRoot.trim();
  const kokoroQueue = useMemo(
    () =>
      queue.filter(
        (item) =>
          item.queueKind === "kokoro" &&
          (!trimmedAssetRoot || item.assetRoot === trimmedAssetRoot),
      ),
    [queue, trimmedAssetRoot],
  );

  useEffect(() => {
    const prev = prevKokoroQueueRef.current;
    for (const item of kokoroQueue) {
      const prevItem = prev.find((candidate) => candidate.id === item.id);
      const becameComplete = (!prevItem || prevItem.status !== "complete") && item.status === "complete";
      if (becameComplete) {
        setError(null);
        void refreshInstalledState(item.assetRoot ?? assetRoot);
        void refreshVoiceCatalog(item.assetRoot ?? assetRoot);
      }
    }
    prevKokoroQueueRef.current = kokoroQueue;
  }, [assetRoot, kokoroQueue]);

  const selectedVariantStatus = variant ? variantStatuses[variant] ?? null : null;
  const selectedVoice = installedVoices.find((candidate) => candidate.id === voiceId) ?? null;
  const activeKokoroQueue = kokoroQueue.filter(
    (item) => item.status === "downloading" || item.status === "queued",
  );
  const latestActiveKokoroItem =
    activeKokoroQueue.find((item) => item.status === "downloading") ?? activeKokoroQueue[0] ?? null;

  const filteredAvailableVoices = useMemo(() => {
    const query = voiceSearch.trim().toLowerCase();
    if (!query) return availableVoices;
    return availableVoices.filter((voice) => voice.id.toLowerCase().includes(query));
  }, [availableVoices, voiceSearch]);

  const activeVoiceBlend = useMemo<KokoroVoiceBlendEntry[]>(() => {
    const weighted = installedVoices
      .map((voice) => {
        const raw = voiceWeights[voice.id];
        const parsed =
          raw == null || raw === "" ? (voice.id === voiceId ? 1 : 0) : Number.parseFloat(raw);
        return {
          voiceId: voice.id,
          weight: Number.isFinite(parsed) && parsed > 0 ? parsed : 0,
        };
      })
      .filter((voice) => voice.weight > 0);

    if (weighted.length === 0 && voiceId.trim()) {
      return [{ voiceId: voiceId.trim(), weight: 1 }];
    }

    weighted.sort((left, right) => {
      if (left.voiceId === voiceId) return -1;
      if (right.voiceId === voiceId) return 1;
      return right.weight - left.weight;
    });
    return weighted;
  }, [installedVoices, voiceWeights, voiceId]);

  const primaryBlendVoiceId = activeVoiceBlend[0]?.voiceId ?? "";

  const canPreview =
    Boolean(assetRoot.trim()) &&
    Boolean(variant) &&
    activeVoiceBlend.length > 0 &&
    Boolean(previewText.trim()) &&
    !isPreviewing;

  async function refreshInstalledState(root: string) {
    const trimmedRoot = root.trim();
    if (!trimmedRoot || variants.length === 0) return;

    setIsRefreshingAssets(true);
    try {
      const statusList = await Promise.all(
        variants.map((supportedVariant) =>
          kokoroValidateAssets(trimmedRoot, supportedVariant.id, voiceId || undefined),
        ),
      );
      const nextStatuses = statusList.reduce<VariantStatusMap>((acc, status) => {
        acc[status.variant] = status;
        return acc;
      }, {});
      setVariantStatuses(nextStatuses);

      const currentStatus =
        (variant ? nextStatuses[variant] : null) ?? statusList[0] ?? null;
      const nextInstalledVoices = currentStatus?.installedVoices ?? [];
      setInstalledVoices(nextInstalledVoices);
      setVoiceWeights((current) => {
        const next: Record<string, string> = {};
        for (const voice of nextInstalledVoices) {
          if (current[voice.id] != null) next[voice.id] = current[voice.id];
        }
        return next;
      });

      if (nextInstalledVoices.length > 0) {
        const hasCurrentVoice = nextInstalledVoices.some((candidate) => candidate.id === voiceId);
        if (!hasCurrentVoice) {
          setVoiceId(nextInstalledVoices[0].id);
        }
      } else if (voiceId) {
        setVoiceId("");
      }
    } catch (err) {
      setVariantStatuses({});
      setInstalledVoices([]);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsRefreshingAssets(false);
    }
  }

  async function refreshVoiceCatalog(root: string) {
    const trimmedRoot = root.trim();
    if (!trimmedRoot) return;

    setIsRefreshingCatalog(true);
    try {
      const [voices, installed] = await Promise.all([
        kokoroListAvailableVoices(trimmedRoot),
        kokoroListInstalledVoices(trimmedRoot),
      ]);
      setAvailableVoices(voices);
      setInstalledVoices(installed);
      setVoiceWeights((current) => {
        const next: Record<string, string> = {};
        for (const voice of installed) {
          if (current[voice.id] != null) next[voice.id] = current[voice.id];
        }
        return next;
      });
      if (installed.length > 0 && !installed.some((candidate) => candidate.id === voiceId)) {
        setVoiceId(installed[0].id);
      }
      if (installed.length === 0 && voiceId) {
        setVoiceId("");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsRefreshingCatalog(false);
    }
  }

  const browseForDirectory = async (setter: (value: string) => void) => {
    try {
      const selected = await open({
        multiple: false,
        directory: true,
      });
      if (typeof selected === "string") {
        setter(selected);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const browseForFile = async (setter: (value: string) => void) => {
    try {
      const selected = await open({ multiple: false });
      if (typeof selected === "string") {
        setter(selected);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleRefreshEverything = async () => {
    if (!assetRoot.trim()) {
      setError("Set an asset root first.");
      return;
    }
    setError(null);
    await Promise.all([refreshInstalledState(assetRoot), refreshVoiceCatalog(assetRoot)]);
  };

  const handleInstallModel = async (nextVariant: KokoroModelVariant) => {
    if (!assetRoot.trim()) {
      setError("Set an asset root first.");
      return;
    }
    setError(null);
    setVariant(nextVariant);
    try {
      await kokoroInstallModel(assetRoot.trim(), nextVariant);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleInstallVoice = async (nextVoiceId: string) => {
    if (!assetRoot.trim()) {
      setError("Set an asset root first.");
      return;
    }
    setError(null);
    try {
      await kokoroInstallVoice(assetRoot.trim(), nextVoiceId);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleCancelDownloads = async () => {
    setError(null);
    try {
      await Promise.all(activeKokoroQueue.map((item) => cancelItem(item.id)));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handlePreview = async () => {
    if (!variant) {
      setError("Choose a model variant first.");
      return;
    }
    if (activeVoiceBlend.length === 0) {
      setError("Choose at least one installed voice with a positive blend weight.");
      return;
    }
    if (!previewText.trim()) {
      setError("Enter some preview text first.");
      return;
    }

    setError(null);
    setIsPreviewing(true);
    shouldAutoplayRef.current = true;
    audioRef.current?.pause();

    const parsedSpeed = Number.parseFloat(speed);
    const normalizedSpeed =
      Number.isFinite(parsedSpeed) && parsedSpeed > 0 ? parsedSpeed : undefined;

    try {
      const preview = await kokoroPreview(
        assetRoot.trim(),
        variant,
        activeVoiceBlend,
        previewText.trim(),
        normalizedSpeed,
        espeakBinPath.trim() || undefined,
        espeakDataPath.trim() || undefined,
      );
      setPreviewSrc(`data:${preview.format};base64,${preview.audioBase64}`);
    } catch (err) {
      shouldAutoplayRef.current = false;
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsPreviewing(false);
    }
  };

  const handleTokenizePreview = async () => {
    if (!assetRoot.trim()) {
      setError("Set an asset root first.");
      return;
    }
    if (activeVoiceBlend.length === 0) {
      setError("Choose at least one installed voice with a positive blend weight.");
      return;
    }
    if (!previewText.trim()) {
      setError("Enter some preview text first.");
      return;
    }

    setError(null);
    setIsTokenizing(true);
    try {
      const preview = await kokoroTokenizePreview(
        assetRoot.trim(),
        activeVoiceBlend,
        previewText.trim(),
        espeakBinPath.trim() || undefined,
        espeakDataPath.trim() || undefined,
      );
      setTokenizePreview(preview);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsTokenizing(false);
    }
  };

  return (
    <div className="flex h-full flex-col text-fg/90">
      <main className="flex-1 overflow-y-auto px-4 py-5">
        <div className="mx-auto flex w-full max-w-7xl flex-col gap-4 pb-10">
          <div className="rounded-3xl border border-fg/10 bg-[linear-gradient(180deg,rgba(255,255,255,0.04),rgba(255,255,255,0.02))] p-5">
            <div className="flex flex-wrap items-start justify-between gap-4">
              <div className="max-w-3xl">
                <div className="mb-3 flex items-center gap-2 text-fg/60">
                  <Wrench className="h-4 w-4" />
                  <span className={cn(typography.overline.size, typography.overline.weight)}>
                    Developer
                  </span>
                </div>
                <h1 className={cn(typography.h1.size, typography.h1.weight, "text-fg")}>
                  Kokoro Test Bench
                </h1>
                <p className={cn("mt-2 max-w-2xl text-fg/55", typography.body.size)}>
                  Temporary page for installing Kokoro model assets, downloading individual voice
                  bins, validating the local directory layout, and generating preview audio against
                  the native Rust backend.
                </p>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <StatusPill tone="neutral">
                  {platform.type === "mobile" ? "Mobile runtime" : "Desktop runtime"}
                </StatusPill>
                <StatusPill
                  tone={selectedVariantStatus?.resolvedModelPath ? "success" : "warning"}
                >
                  {selectedVariantStatus?.resolvedModelPath ? "Selected model installed" : "No selected model"}
                </StatusPill>
                <StatusPill tone={installedVoices.length > 0 ? "success" : "info"}>
                  {installedVoices.length} installed voice{installedVoices.length === 1 ? "" : "s"}
                </StatusPill>
              </div>
            </div>
            <div className="mt-4 flex items-start gap-2 rounded-2xl border border-warning/20 bg-warning/10 p-3 text-warning/85">
              <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
              <p className={typography.bodySmall.size}>
                {platform.type === "mobile"
                  ? "Mobile builds only surface the int8 Kokoro model. Voices stay individually installable."
                  : "Model files and voices are managed separately. Install one model variant first, then install only the voice bins you want."}
              </p>
            </div>
          </div>

          {error && (
            <div className="flex items-start gap-2 rounded-2xl border border-danger/30 bg-danger/10 px-4 py-3 text-danger/85">
              <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
              <p className={typography.body.size}>{error}</p>
            </div>
          )}

          <div className="grid gap-4 xl:grid-cols-[minmax(0,1.1fr)_minmax(360px,0.9fr)]">
            <div className="space-y-4">
              <SectionCard
                icon={<FolderOpen />}
                title="Asset Root"
                subtitle="Use an app-managed directory or point at any writable folder. The installer will create `onnx/` and `voices/` as needed."
              >
                <FieldLabel>Asset directory</FieldLabel>
                <div className="flex flex-col gap-2 sm:flex-row">
                  <input
                    value={assetRoot}
                    onChange={(event) => setAssetRoot(event.target.value)}
                    placeholder={managedRoot || "/path/to/kokoro"}
                    className="min-w-0 flex-1 rounded-xl border border-fg/10 bg-fg/5 px-3 py-3 text-sm text-fg outline-none transition focus:border-fg/20 focus:bg-fg/[0.07]"
                  />
                  <button
                    onClick={() => void browseForDirectory(setAssetRoot)}
                    className="inline-flex items-center justify-center gap-2 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-sm text-fg/80 transition hover:border-fg/20 hover:bg-fg/10"
                  >
                    <FolderOpen className="h-4 w-4" />
                    Browse
                  </button>
                  <button
                    onClick={() => setAssetRoot(managedRoot)}
                    disabled={!managedRoot}
                    className="inline-flex items-center justify-center gap-2 rounded-xl border border-info/20 bg-info/10 px-4 py-3 text-sm text-info/85 transition hover:border-info/35 hover:bg-info/15 disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    Use managed
                  </button>
                </div>
                {managedRoot && (
                  <p className="mt-2 break-all text-xs text-fg/45">Managed root: {managedRoot}</p>
                )}
                <div className="mt-4 flex flex-wrap gap-2">
                  <button
                    onClick={() => void handleRefreshEverything()}
                    disabled={isBooting || isRefreshingAssets || isRefreshingCatalog || !assetRoot.trim()}
                    className="inline-flex items-center gap-2 rounded-xl border border-accent/30 bg-accent/10 px-4 py-2.5 text-sm text-accent transition hover:border-accent/50 hover:bg-accent/15 disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {isRefreshingAssets || isRefreshingCatalog ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <RefreshCw className="h-4 w-4" />
                    )}
                    Refresh state
                  </button>
                </div>
              </SectionCard>

              <SectionCard
                icon={<Cpu />}
                title="Model Installer"
                subtitle="Install exactly one supported model variant at a time. The installer also fetches the small shared metadata files used around the model."
              >
                <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
                  {variants.map((supportedVariant) => {
                    const installed = Boolean(variantStatuses[supportedVariant.id]?.resolvedModelPath);
                    const isCurrentDownload = activeKokoroQueue.some(
                      (item) =>
                        item.installKind === "model" && item.variant === supportedVariant.id,
                    );
                    const isSelected = variant === supportedVariant.id;

                    return (
                      <div
                        key={supportedVariant.id}
                        className={cn(
                          "rounded-2xl border p-4 transition",
                          isSelected
                            ? "border-accent/30 bg-accent/10"
                            : "border-fg/10 bg-fg/5 hover:border-fg/20 hover:bg-fg/10",
                        )}
                      >
                        <div className="flex items-start justify-between gap-3">
                          <div className="min-w-0">
                            <div className="text-sm font-medium text-fg">{supportedVariant.label}</div>
                            <div className="mt-1 text-xs text-fg/45">
                              {supportedVariant.filename} • {supportedVariant.sizeMb} MB
                            </div>
                          </div>
                          {installed && <CheckCircle2 className="h-4 w-4 shrink-0 text-accent" />}
                        </div>
                        <div className="mt-3 flex items-center gap-2">
                          <StatusPill tone={installed ? "success" : "neutral"}>
                            {installed ? "Installed" : "Not installed"}
                          </StatusPill>
                        </div>
                        <div className="mt-4 flex gap-2">
                          <button
                            onClick={() => setVariant(supportedVariant.id)}
                            className="flex-1 rounded-xl border border-fg/10 bg-fg/5 px-3 py-2 text-sm text-fg/75 transition hover:border-fg/20 hover:bg-fg/10"
                          >
                            {isSelected ? "Selected" : "Select"}
                          </button>
                          <button
                            onClick={() => void handleInstallModel(supportedVariant.id)}
                            disabled={!assetRoot.trim()}
                            className="flex-1 rounded-xl border border-info/25 bg-info/10 px-3 py-2 text-sm text-info/85 transition hover:border-info/40 hover:bg-info/15 disabled:cursor-not-allowed disabled:opacity-50"
                          >
                            {isCurrentDownload ? "Queued…" : installed ? "Reinstall" : "Install"}
                          </button>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </SectionCard>

              <SectionCard
                icon={<Mic2 />}
                title="Voice Installers"
                subtitle="Browse the available voice catalog from the current Kokoro repository, then install individual `.bin` files into your selected asset root."
              >
                <div className="mb-3 flex flex-col gap-2 sm:flex-row">
                  <div className="relative min-w-0 flex-1">
                    <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg/35" />
                    <input
                      value={voiceSearch}
                      onChange={(event) => setVoiceSearch(event.target.value)}
                      placeholder="Search voice ids"
                      className="w-full rounded-xl border border-fg/10 bg-fg/5 py-3 pl-9 pr-3 text-sm text-fg outline-none transition focus:border-fg/20 focus:bg-fg/[0.07]"
                    />
                  </div>
                  <button
                    onClick={() => void refreshVoiceCatalog(assetRoot)}
                    disabled={isRefreshingCatalog || !assetRoot.trim()}
                    className="inline-flex items-center justify-center gap-2 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-sm text-fg/75 transition hover:border-fg/20 hover:bg-fg/10 disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {isRefreshingCatalog ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <RefreshCw className="h-4 w-4" />
                    )}
                    Refresh catalog
                  </button>
                </div>

                <div className="mb-3 grid gap-3 sm:grid-cols-2">
                  <div className="rounded-2xl border border-fg/10 bg-fg/5 p-3">
                    <div className={cn(typography.overline.size, typography.overline.weight, "text-fg/40")}>
                      Available voices
                    </div>
                    <div className="mt-2 text-2xl font-semibold text-fg">{availableVoices.length}</div>
                  </div>
                  <div className="rounded-2xl border border-fg/10 bg-fg/5 p-3">
                    <div className={cn(typography.overline.size, typography.overline.weight, "text-fg/40")}>
                      Installed voices
                    </div>
                    <div className="mt-2 text-2xl font-semibold text-fg">{installedVoices.length}</div>
                  </div>
                </div>

                {filteredAvailableVoices.length === 0 ? (
                  <div className="rounded-2xl border border-dashed border-fg/10 bg-fg/[0.025] p-4 text-sm text-fg/50">
                    {availableVoices.length === 0
                      ? "No voice catalog loaded yet. Refresh the catalog after setting an asset root."
                      : "No voices matched your search."}
                  </div>
                ) : (
                  <div className="max-h-[28rem] space-y-2 overflow-y-auto pr-1">
                    {filteredAvailableVoices.map((voice) => {
                      const isInstalled = installedVoices.some((candidate) => candidate.id === voice.id);
                      const isSelected = voice.id === voiceId;
                      const isCurrentDownload = activeKokoroQueue.some(
                        (item) => item.installKind === "voice" && item.voiceId === voice.id,
                      );

                      return (
                        <div
                          key={voice.id}
                          className={cn(
                            "rounded-2xl border px-3 py-3 transition",
                            isSelected
                              ? "border-accent/30 bg-accent/10"
                              : "border-fg/10 bg-fg/5 hover:border-fg/20 hover:bg-fg/10",
                          )}
                        >
                          <div className="flex items-center justify-between gap-3">
                            <button
                              onClick={() => setVoiceId(voice.id)}
                              className="min-w-0 flex-1 text-left"
                            >
                              <div className="truncate text-sm font-medium text-fg">{voice.id}</div>
                              <div className="mt-1 text-xs text-fg/45">
                                {isInstalled ? "Installed locally" : "Not installed yet"}
                              </div>
                              {isInstalled && (
                                <div className="mt-2 flex items-center gap-2">
                                  <input
                                    value={
                                      voiceWeights[voice.id] ?? (voice.id === voiceId ? "1" : "")
                                    }
                                    onChange={(event) =>
                                      setVoiceWeights((current) => ({
                                        ...current,
                                        [voice.id]: event.target.value,
                                      }))
                                    }
                                    placeholder="weight"
                                    className="w-24 rounded-lg border border-fg/10 bg-black/20 px-2 py-1 text-xs text-fg outline-none transition focus:border-fg/20"
                                  />
                                  <span className="text-[11px] text-fg/45">
                                    {voice.id === primaryBlendVoiceId ? "Primary" : "Blend"}
                                  </span>
                                </div>
                              )}
                            </button>
                            <div className="flex items-center gap-2">
                              {isInstalled && <CheckCircle2 className="h-4 w-4 text-accent" />}
                              <button
                                onClick={() => void handleInstallVoice(voice.id)}
                                disabled={!assetRoot.trim()}
                                className="rounded-xl border border-info/25 bg-info/10 px-3 py-2 text-sm text-info/85 transition hover:border-info/40 hover:bg-info/15 disabled:cursor-not-allowed disabled:opacity-50"
                              >
                                {isCurrentDownload ? "Queued…" : isInstalled ? "Reinstall" : "Install"}
                              </button>
                            </div>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                )}
              </SectionCard>
            </div>

            <div className="space-y-4">
              <SectionCard
                icon={<Download />}
                title="Download Progress"
                subtitle="Kokoro installs now use the shared download queue, so they behave like Hugging Face model downloads and appear in the global queue UI."
              >
                <div className="space-y-3">
                  <div className="rounded-2xl border border-fg/10 bg-fg/5 p-3">
                    <div className="flex items-center justify-between gap-3">
                      <div>
                        <div className={cn(typography.overline.size, typography.overline.weight, "text-fg/40")}>
                          Status
                        </div>
                        <div className="mt-2 text-sm font-medium text-fg">
                          {activeKokoroQueue.length > 0
                            ? `${activeKokoroQueue.length} active queue item${activeKokoroQueue.length === 1 ? "" : "s"}`
                            : "Idle"}
                        </div>
                      </div>
                      {activeKokoroQueue.length > 0 && (
                        <button
                          onClick={() => void handleCancelDownloads()}
                          className="inline-flex items-center gap-2 rounded-xl border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-danger/85 transition hover:border-danger/40 hover:bg-danger/15"
                        >
                          Cancel active
                        </button>
                      )}
                    </div>
                  </div>

                  <div className="grid gap-3 sm:grid-cols-2">
                    <div className="rounded-2xl border border-fg/10 bg-fg/5 p-3">
                      <div className={cn(typography.overline.size, typography.overline.weight, "text-fg/40")}>
                        Active install
                      </div>
                      <div className="mt-2 text-sm font-medium text-fg">
                        {latestActiveKokoroItem
                          ? latestActiveKokoroItem.installKind === "model"
                            ? `Model ${latestActiveKokoroItem.variant ?? ""}`.trim()
                            : `Voice ${latestActiveKokoroItem.voiceId ?? ""}`.trim()
                          : "None"}
                      </div>
                    </div>
                    <div className="rounded-2xl border border-fg/10 bg-fg/5 p-3">
                      <div className={cn(typography.overline.size, typography.overline.weight, "text-fg/40")}>
                        Target directory
                      </div>
                      <div className="mt-2 break-all text-xs text-fg/65">
                        {(latestActiveKokoroItem?.assetRoot ?? assetRoot) || "Unset"}
                      </div>
                    </div>
                  </div>

                  {kokoroQueue.length === 0 ? (
                    <div className="rounded-2xl border border-dashed border-fg/10 bg-fg/[0.025] p-4 text-sm text-fg/50">
                      No Kokoro downloads have been queued for this asset root yet.
                    </div>
                  ) : (
                    <div className="space-y-2">
                      {kokoroQueue.map((item) => (
                        <div
                          key={item.id}
                          className="rounded-2xl border border-fg/10 bg-fg/5 px-3 py-3"
                        >
                          <div className="flex items-start justify-between gap-3">
                            <div className="min-w-0 flex-1">
                              <div className="truncate text-sm font-medium text-fg">
                                {item.filename}
                              </div>
                              <div className="mt-1 text-xs text-fg/45">
                                {item.displayName || "Kokoro asset"} • {item.status}
                              </div>
                              <div className="mt-2 text-xs text-fg/45">
                                {formatProgressRatio(item.downloaded, item.total)}
                              </div>
                              {item.status === "downloading" && (
                                <div className="mt-3 h-2 overflow-hidden rounded-full bg-fg/10">
                                  <div
                                    className="h-full rounded-full bg-info transition-all"
                                    style={{
                                      width:
                                        item.total > 0
                                          ? `${Math.max(
                                              3,
                                              Math.min(100, (item.downloaded / item.total) * 100),
                                            )}%`
                                          : "12%",
                                    }}
                                  />
                                </div>
                              )}
                              {item.error && (
                                <div className="mt-2 text-xs text-danger/70">{item.error}</div>
                              )}
                            </div>
                            <div className="flex items-center gap-2">
                              {(item.status === "downloading" || item.status === "queued") && (
                                <button
                                  onClick={() => void cancelItem(item.id)}
                                  className="rounded-xl border border-danger/25 bg-danger/10 px-3 py-2 text-xs text-danger/85 transition hover:border-danger/40 hover:bg-danger/15"
                                >
                                  Cancel
                                </button>
                              )}
                              {(item.status === "complete" ||
                                item.status === "error" ||
                                item.status === "cancelled") && (
                                <button
                                  onClick={() => void dismissItem(item.id)}
                                  className="rounded-xl border border-fg/10 bg-fg/5 px-3 py-2 text-xs text-fg/75 transition hover:border-fg/20 hover:bg-fg/10"
                                >
                                  Dismiss
                                </button>
                              )}
                            </div>
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </SectionCard>

              <SectionCard
                icon={<Play />}
                title="Preview Synthesis"
                subtitle="Once a model variant and at least one voice are installed, inspect tokenization first, then generate local WAV previews with optional `espeak-ng` overrides."
              >
                <div className="space-y-4">
                  <div className="grid gap-3 sm:grid-cols-2">
                    <div className="rounded-2xl border border-fg/10 bg-fg/5 p-3">
                      <div className={cn(typography.overline.size, typography.overline.weight, "text-fg/40")}>
                        Selected variant
                      </div>
                      <div className="mt-2 text-sm font-medium text-fg">
                        {variant || "None"}
                      </div>
                    </div>
                    <div className="rounded-2xl border border-fg/10 bg-fg/5 p-3">
                      <div className={cn(typography.overline.size, typography.overline.weight, "text-fg/40")}>
                        Selected voice
                      </div>
                      <div className="mt-2 text-sm font-medium text-fg">
                        {primaryBlendVoiceId || selectedVoice?.id || "None"}
                      </div>
                    </div>
                  </div>

                  <div className="rounded-2xl border border-fg/10 bg-fg/5 p-3">
                    <div className={cn(typography.overline.size, typography.overline.weight, "text-fg/40")}>
                      Active blend
                    </div>
                    <div className="mt-2 text-xs text-fg/65">
                      {activeVoiceBlend.length > 0
                        ? activeVoiceBlend
                            .map((voice) => `${voice.voiceId} ${voice.weight}`)
                            .join(" • ")
                        : "No voices selected"}
                    </div>
                  </div>

                  <div>
                    <FieldLabel>espeak-ng binary path (optional)</FieldLabel>
                    <div className="flex gap-2">
                      <input
                        value={espeakBinPath}
                        onChange={(event) => setEspeakBinPath(event.target.value)}
                        placeholder="/usr/bin/espeak-ng"
                        className="min-w-0 flex-1 rounded-xl border border-fg/10 bg-fg/5 px-3 py-3 text-sm text-fg outline-none transition focus:border-fg/20 focus:bg-fg/[0.07]"
                      />
                      <button
                        onClick={() => void browseForFile(setEspeakBinPath)}
                        className="rounded-xl border border-fg/10 bg-fg/5 px-3 py-3 text-sm text-fg/75 transition hover:border-fg/20 hover:bg-fg/10"
                      >
                        Browse
                      </button>
                    </div>
                  </div>

                  <div>
                    <FieldLabel>espeak-ng data directory (optional)</FieldLabel>
                    <div className="flex gap-2">
                      <input
                        value={espeakDataPath}
                        onChange={(event) => setEspeakDataPath(event.target.value)}
                        placeholder="/usr/share/espeak-ng-data"
                        className="min-w-0 flex-1 rounded-xl border border-fg/10 bg-fg/5 px-3 py-3 text-sm text-fg outline-none transition focus:border-fg/20 focus:bg-fg/[0.07]"
                      />
                      <button
                        onClick={() => void browseForDirectory(setEspeakDataPath)}
                        className="rounded-xl border border-fg/10 bg-fg/5 px-3 py-3 text-sm text-fg/75 transition hover:border-fg/20 hover:bg-fg/10"
                      >
                        Browse
                      </button>
                    </div>
                  </div>

                  <div className="grid gap-4 sm:grid-cols-[minmax(0,1fr)_120px]">
                    <div>
                      <FieldLabel>Preview text</FieldLabel>
                      <textarea
                        value={previewText}
                        onChange={(event) => setPreviewText(event.target.value)}
                        rows={6}
                        className="w-full rounded-2xl border border-fg/10 bg-fg/5 px-3 py-3 text-sm text-fg outline-none transition focus:border-fg/20 focus:bg-fg/[0.07]"
                      />
                    </div>
                    <div>
                      <FieldLabel>Speed</FieldLabel>
                      <input
                        value={speed}
                        onChange={(event) => setSpeed(event.target.value)}
                        inputMode="decimal"
                        placeholder="1.0"
                        className="w-full rounded-xl border border-fg/10 bg-fg/5 px-3 py-3 text-sm text-fg outline-none transition focus:border-fg/20 focus:bg-fg/[0.07]"
                      />
                    </div>
                  </div>

                  <button
                    onClick={() => void handleTokenizePreview()}
                    disabled={!assetRoot.trim() || activeVoiceBlend.length === 0 || !previewText.trim() || isTokenizing}
                    className="inline-flex w-full items-center justify-center gap-2 rounded-2xl border border-fg/15 bg-fg/5 px-4 py-3 text-sm font-medium text-fg/85 transition hover:border-fg/25 hover:bg-fg/10 disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {isTokenizing ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <Search className="h-4 w-4" />
                    )}
                    {isTokenizing ? "Tokenizing" : "Inspect tokenization"}
                  </button>

                  {tokenizePreview && (
                    <div className="space-y-3 rounded-2xl border border-fg/10 bg-fg/5 p-3">
                      <div className="grid gap-3 sm:grid-cols-2">
                        <div className="rounded-xl border border-fg/10 bg-black/20 p-3">
                          <div className={cn(typography.overline.size, typography.overline.weight, "text-fg/40")}>
                            Language
                          </div>
                          <div className="mt-2 text-sm font-medium text-fg">
                            {tokenizePreview.language} via {tokenizePreview.primaryVoiceId}
                          </div>
                        </div>
                        <div className="rounded-xl border border-fg/10 bg-black/20 p-3">
                          <div className={cn(typography.overline.size, typography.overline.weight, "text-fg/40")}>
                            Tokens / chunks
                          </div>
                          <div className="mt-2 text-sm font-medium text-fg">
                            {tokenizePreview.tokenCount} tokens • {tokenizePreview.chunkLengths.length} chunk{tokenizePreview.chunkLengths.length === 1 ? "" : "s"}
                          </div>
                        </div>
                      </div>

                      <div className="rounded-xl border border-fg/10 bg-black/20 p-3">
                        <div className={cn(typography.overline.size, typography.overline.weight, "text-fg/40")}>
                          Lexicon
                        </div>
                        <div className="mt-2 break-all text-xs text-fg/65">
                          {tokenizePreview.lexiconPath}
                        </div>
                        <div className="mt-2 text-xs text-fg/55">
                          {tokenizePreview.lexiconEntryCount} entries
                          {tokenizePreview.usedLexiconEntries.length > 0
                            ? ` • used: ${tokenizePreview.usedLexiconEntries.join(", ")}`
                            : " • no overrides hit"}
                        </div>
                      </div>

                      <div className="rounded-xl border border-fg/10 bg-black/20 p-3">
                        <div className={cn(typography.overline.size, typography.overline.weight, "text-fg/40")}>
                          Voice blend
                        </div>
                        <div className="mt-2 text-xs text-fg/70">
                          {tokenizePreview.voiceBlend
                            .map((voice) => `${voice.voiceId} ${voice.weight.toFixed(2)}`)
                            .join(" • ")}
                        </div>
                      </div>

                      <div className="rounded-xl border border-fg/10 bg-black/20 p-3">
                        <div className={cn(typography.overline.size, typography.overline.weight, "text-fg/40")}>
                          Normalized text
                        </div>
                        <div className="mt-2 whitespace-pre-wrap text-sm text-fg/80">
                          {tokenizePreview.normalizedText}
                        </div>
                      </div>

                      <div className="rounded-xl border border-fg/10 bg-black/20 p-3">
                        <div className={cn(typography.overline.size, typography.overline.weight, "text-fg/40")}>
                          Effective text
                        </div>
                        <div className="mt-2 whitespace-pre-wrap break-words font-mono text-xs text-fg/75">
                          {tokenizePreview.effectiveText}
                        </div>
                      </div>

                      {tokenizePreview.warnings.length > 0 && (
                        <div className="space-y-2 rounded-xl border border-warning/20 bg-warning/10 p-3">
                          {tokenizePreview.warnings.map((warning) => (
                            <div key={warning} className="text-sm text-warning/85">
                              {warning}
                            </div>
                          ))}
                        </div>
                      )}

                      <div className="rounded-xl border border-fg/10 bg-black/20 p-3">
                        <div className={cn(typography.overline.size, typography.overline.weight, "text-fg/40")}>
                          Segment IPA trace
                        </div>
                        <div className="mt-3 max-h-72 space-y-2 overflow-y-auto">
                          {tokenizePreview.segments.map((segment, index) => (
                            <div key={`${segment.kind}-${index}`} className="rounded-lg border border-fg/10 bg-fg/5 p-2">
                              <div className="flex items-center justify-between gap-2 text-xs text-fg/45">
                                <span>{segment.kind}</span>
                                <span>{segment.tokenIds.length} tokens</span>
                              </div>
                              <div className="mt-1 text-xs text-fg/70">{segment.sourceText}</div>
                              <div className="mt-1 break-words font-mono text-xs text-fg">
                                {segment.ipa}
                              </div>
                            </div>
                          ))}
                        </div>
                      </div>

                      <div className="rounded-xl border border-fg/10 bg-black/20 p-3">
                        <div className={cn(typography.overline.size, typography.overline.weight, "text-fg/40")}>
                          Token ids
                        </div>
                        <div className="mt-2 max-h-32 overflow-y-auto break-words font-mono text-[11px] text-fg/70">
                          {tokenizePreview.tokenIds.join(", ")}
                        </div>
                      </div>
                    </div>
                  )}

                  <button
                    onClick={() => void handlePreview()}
                    disabled={!canPreview}
                    className="inline-flex w-full items-center justify-center gap-2 rounded-2xl border border-info/30 bg-info/10 px-4 py-3 text-sm font-medium text-info transition hover:border-info/50 hover:bg-info/15 disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {isPreviewing ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <Play className="h-4 w-4" />
                    )}
                    {isPreviewing ? "Generating preview" : "Generate preview"}
                  </button>

                  <div className="rounded-2xl border border-fg/10 bg-black/20 p-3">
                    <div className="mb-2 flex items-center gap-2 text-fg/55">
                      <Mic2 className="h-4 w-4" />
                      <span className={typography.bodySmall.size}>Latest output</span>
                    </div>
                    {previewSrc ? (
                      <audio ref={audioRef} controls className="w-full" src={previewSrc} />
                    ) : (
                      <div className="rounded-xl border border-dashed border-fg/10 px-3 py-6 text-center text-sm text-fg/40">
                        No preview generated yet.
                      </div>
                    )}
                  </div>
                </div>
              </SectionCard>
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}
