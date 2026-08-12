import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useNavigate, useSearchParams } from "react-router-dom";
import { AnimatePresence, motion } from "framer-motion";
import {
  ArrowLeft,
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Copy,
  Download,
  Eye,
  FileUp,
  Heart,
  ImageOff,
  KeyRound,
  Layers,
  Loader,
  Pencil,
  Search,
  Sparkles,
  Trash2,
  X,
} from "lucide-react";

import { cn } from "../../design-tokens";
import { BottomMenu } from "../../components/BottomMenu";
import { confirmBottomMenu } from "../../components/ConfirmBottomMenu";
import { toast } from "../../components/toast";
import { useI18n, type TranslationKey } from "../../../core/i18n/context";
import { useDownloadQueue } from "../../../core/downloads/DownloadQueueContext";
import { useShowNsfwImages } from "../discovery/hooks/useDiscoveryNsfw";
import {
  loraArchitectureLabel,
  parseLoraKeywordDraft,
  type SdcppLoraFile,
  type SdcppLoraKeywordDiscovery,
} from "../../../core/image-generation/loras";
import { InlineDownloadCards } from "./components/DownloadQueueBar";
import {
  CivitaiTokenMenu,
  isCivitaiAuthError,
  type CivitaiAuthStatus,
} from "./components/CivitaiTokenMenu";
import { GuidedTour, useGuidedTour } from "../../components/GuidedTour";
import { getPlatform } from "../../../core/utils/platform";

type CivitaiImage = {
  url: string;
  nsfwLevel: number;
  width: number;
  height: number;
};

type CivitaiLoraSummary = {
  id: number;
  name: string;
  nsfw: boolean;
  nsfwLevel: number;
  creatorUsername: string | null;
  downloadCount: number;
  thumbsUpCount: number;
  previewImage: CivitaiImage | null;
  baseModels: string[];
  latestVersionId: number | null;
};

type CivitaiSearchPage = {
  items: CivitaiLoraSummary[];
  nextCursor: string | null;
};

type CivitaiFile = {
  id: number;
  name: string;
  sizeKb: number;
  primary: boolean;
  format: string | null;
  fp: string | null;
  sha256: string | null;
  downloadUrl: string | null;
};

type CivitaiVersion = {
  id: number;
  name: string;
  baseModel: string | null;
  architecture: string | null;
  publishedAt: string | null;
  trainedWords: string[];
  images: CivitaiImage[];
  files: CivitaiFile[];
};

type CivitaiModelDetail = {
  id: number;
  name: string;
  description: string | null;
  nsfw: boolean;
  nsfwLevel: number;
  creatorUsername: string | null;
  downloadCount: number;
  thumbsUpCount: number;
  tags: string[];
  versions: CivitaiVersion[];
};

type Tab = "library" | "browse";
type SortOption = "Highest Rated" | "Most Downloaded" | "Newest";
type PeriodOption = "AllTime" | "Year" | "Month" | "Week" | "Day";

const SORT_OPTIONS: { value: SortOption; labelKey: TranslationKey }[] = [
  { value: "Highest Rated", labelKey: "loraLibrary.sortHighestRated" },
  { value: "Most Downloaded", labelKey: "loraLibrary.sortMostDownloaded" },
  { value: "Newest", labelKey: "loraLibrary.sortNewest" },
];

const PERIOD_OPTIONS: { value: PeriodOption; labelKey: TranslationKey }[] = [
  { value: "AllTime", labelKey: "loraLibrary.periodAllTime" },
  { value: "Year", labelKey: "loraLibrary.periodYear" },
  { value: "Month", labelKey: "loraLibrary.periodMonth" },
  { value: "Week", labelKey: "loraLibrary.periodWeek" },
  { value: "Day", labelKey: "loraLibrary.periodDay" },
];

function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / Math.pow(1024, index);
  return `${value.toFixed(index > 1 ? 1 : 0)} ${units[index]}`;
}

function formatCount(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
  return `${value}`;
}

function resizedImageUrl(url: string, width: number): string {
  if (!url.includes("image.civitai.com")) return url;
  return url.replace(/\/(?:original=true|width=\d+)[^/]*\//, `/anim=false,width=${width}/`);
}

function stripHtml(value: string): string {
  const withBreaks = value.replace(/<(?:br|\/p|\/div|\/li|\/h[1-6])[^>]*>/gi, "\n");
  const doc = new DOMParser().parseFromString(withBreaks, "text/html");
  const text = doc.body.textContent ?? "";
  return text
    .split("\n")
    .map((line) => line.trim())
    .join("\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function isSafetensorFile(file: CivitaiFile): boolean {
  return file.name.toLowerCase().endsWith(".safetensors");
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function LoraLibraryPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const showNsfw = useShowNsfwImages();
  const { queue } = useDownloadQueue();

  const returnTo = searchParams.get("returnTo");
  const detailId = searchParams.get("model");

  const [tab, setTab] = useState<Tab>("library");
  const [installed, setInstalled] = useState<SdcppLoraFile[]>([]);
  const [installedLoading, setInstalledLoading] = useState(true);
  const [importing, setImporting] = useState(false);
  const [deletingPath, setDeletingPath] = useState<string | null>(null);
  const [discoveringPath, setDiscoveringPath] = useState<string | null>(null);
  const [keywordEditPath, setKeywordEditPath] = useState<string | null>(null);
  const [keywordDraft, setKeywordDraft] = useState("");
  const [savingKeywords, setSavingKeywords] = useState(false);

  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<SortOption>("Highest Rated");
  const [period, setPeriod] = useState<PeriodOption>("AllTime");
  const [baseModelFilter, setBaseModelFilter] = useState<string | null>(null);
  const [results, setResults] = useState<CivitaiLoraSummary[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [searching, setSearching] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [searchedOnce, setSearchedOnce] = useState(false);
  const [sortMenuOpen, setSortMenuOpen] = useState(false);
  const [revealed, setRevealed] = useState<Set<number>>(new Set());

  const [detail, setDetail] = useState<CivitaiModelDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [selectedVersionId, setSelectedVersionId] = useState<number | null>(null);
  const [lightboxIndex, setLightboxIndex] = useState<number | null>(null);
  const [queuingFileId, setQueuingFileId] = useState<number | null>(null);
  const [tokenMenuOpen, setTokenMenuOpen] = useState(false);
  const [civitaiAuth, setCivitaiAuth] = useState<CivitaiAuthStatus | null>(null);

  const { shouldShow: showLibraryTour, dismiss: dismissLibraryTour } =
    useGuidedTour("loraLibrary");
  const { shouldShow: showBrowseTour, dismiss: dismissBrowseTour } =
    useGuidedTour("civitaiBrowse");

  const searchSeq = useRef(0);

  useEffect(() => {
    if (tab !== "browse") return;
    let cancelled = false;
    invoke<CivitaiAuthStatus>("civitai_auth_status")
      .then((status) => {
        if (!cancelled) setCivitaiAuth(status);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [tab]);

  const loadInstalled = useCallback(async () => {
    if (getPlatform().type === "mobile") {
      setInstalledLoading(false);
      return;
    }
    try {
      const files = await invoke<SdcppLoraFile[]>("sdcpp_loras", { profileId: null });
      setInstalled(files);
    } catch {
      setInstalled([]);
    } finally {
      setInstalledLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadInstalled();
  }, [loadInstalled]);

  const loraQueueItems = queue.filter((item) => item.queueKind === "civitai_lora");
  const completedLoraCount = loraQueueItems.filter((item) => item.status === "complete").length;
  useEffect(() => {
    if (completedLoraCount > 0) void loadInstalled();
  }, [completedLoraCount, loadInstalled]);

  const runSearch = useCallback(
    async (options: { cursor?: string | null; append?: boolean }) => {
      const seq = ++searchSeq.current;
      if (options.append) setLoadingMore(true);
      else setSearching(true);
      setSearchError(null);
      try {
        const page = await invoke<CivitaiSearchPage>("civitai_search_loras", {
          query: query.trim() || null,
          sort,
          period,
          baseModels: baseModelFilter ? [baseModelFilter] : null,
          cursor: options.cursor ?? null,
          limit: 30,
        });
        if (seq !== searchSeq.current) return;
        setResults((current) => {
          if (!options.append) return page.items;
          const seen = new Set(current.map((item) => item.id));
          return [...current, ...page.items.filter((item) => !seen.has(item.id))];
        });
        setNextCursor(page.nextCursor);
        setSearchedOnce(true);
      } catch (error) {
        if (seq !== searchSeq.current) return;
        setSearchError(errorText(error));
      } finally {
        if (seq === searchSeq.current) {
          setSearching(false);
          setLoadingMore(false);
        }
      }
    },
    [query, sort, period, baseModelFilter],
  );

  useEffect(() => {
    if (tab !== "browse") return;
    const handle = window.setTimeout(() => {
      void runSearch({});
    }, 350);
    return () => window.clearTimeout(handle);
  }, [tab, runSearch]);

  useEffect(() => {
    if (!detailId) {
      setDetail(null);
      setDetailError(null);
      return;
    }
    let cancelled = false;
    setDetailLoading(true);
    setDetailError(null);
    invoke<CivitaiModelDetail>("civitai_get_model", { modelId: Number(detailId) })
      .then((model) => {
        if (cancelled) return;
        setDetail(model);
        const firstWithFiles =
          model.versions.find((version) => version.files.some(isSafetensorFile)) ??
          model.versions[0];
        setSelectedVersionId(firstWithFiles?.id ?? null);
      })
      .catch((error) => {
        if (!cancelled) setDetailError(errorText(error));
      })
      .finally(() => {
        if (!cancelled) setDetailLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [detailId]);

  const setParams = useCallback(
    (updates: Record<string, string | null>) => {
      const params = new URLSearchParams(searchParams);
      for (const [key, value] of Object.entries(updates)) {
        if (value === null) params.delete(key);
        else params.set(key, value);
      }
      if (returnTo) params.set("returnTo", returnTo);
      setSearchParams(params);
    },
    [searchParams, setSearchParams, returnTo],
  );

  const openDetail = (id: number) => setParams({ model: String(id) });
  const closeDetail = () => setParams({ model: null });

  const availableBaseModels = useMemo(() => {
    const seen = new Set<string>();
    for (const item of results) {
      for (const baseModel of item.baseModels) seen.add(baseModel);
    }
    if (baseModelFilter) seen.add(baseModelFilter);
    return [...seen].sort();
  }, [results, baseModelFilter]);

  const installedFilenames = useMemo(
    () => new Set(installed.map((file) => file.filename.toLowerCase())),
    [installed],
  );

  const handleImport = async () => {
    if (importing) return;
    const selection = await open({
      multiple: false,
      filters: [{ name: "LoRA", extensions: ["safetensors", "ckpt", "pt"] }],
    });
    if (!selection || typeof selection !== "string") return;
    setImporting(true);
    try {
      await invoke<SdcppLoraFile>("sdcpp_import_lora", { sourcePath: selection });
      toast.success(t("loraLibrary.importSuccess"));
      await loadInstalled();
    } catch (error) {
      toast.error(t("loraLibrary.importFailed"), errorText(error));
    } finally {
      setImporting(false);
    }
  };

  const handleDelete = async (file: SdcppLoraFile) => {
    const confirmed = await confirmBottomMenu({
      title: t("loraLibrary.deleteTitle"),
      message: t("loraLibrary.deleteMessage", { name: file.filename }),
      confirmLabel: t("loraLibrary.deleteConfirm"),
      destructive: true,
    });
    if (!confirmed) return;
    setDeletingPath(file.path);
    try {
      await invoke("sdcpp_delete_lora", { path: file.path });
      toast.success(t("loraLibrary.deleteSuccess"));
      await loadInstalled();
    } catch (error) {
      toast.error(t("loraLibrary.deleteFailed"), errorText(error));
    } finally {
      setDeletingPath(null);
    }
  };

  const handleDiscover = async (file: SdcppLoraFile) => {
    setDiscoveringPath(file.path);
    try {
      const discovery = await invoke<SdcppLoraKeywordDiscovery>("sdcpp_discover_lora_keywords", {
        path: file.path,
        profileId: null,
      });
      setInstalled((current) =>
        current.map((entry) =>
          entry.path === file.path
            ? {
              ...entry,
              keywords: discovery.keywords,
              keywordSource: discovery.source,
              architecture: discovery.architecture,
              architectureSource: discovery.architectureSource,
              compatibility: discovery.compatibility,
            }
            : entry,
        ),
      );
    } catch (error) {
      toast.error(t("loraLibrary.discoverFailed"), errorText(error));
    } finally {
      setDiscoveringPath(null);
    }
  };

  const openKeywordEditor = (file: SdcppLoraFile) => {
    setKeywordDraft(file.keywords.join("\n"));
    setKeywordEditPath(file.path);
  };

  const saveKeywords = async () => {
    if (!keywordEditPath || savingKeywords) return;
    const keywords = parseLoraKeywordDraft(keywordDraft);
    setSavingKeywords(true);
    try {
      const saved = await invoke<SdcppLoraKeywordDiscovery>("sdcpp_update_lora_keywords", {
        path: keywordEditPath,
        keywords,
        profileId: null,
      });
      setInstalled((current) =>
        current.map((entry) =>
          entry.path === keywordEditPath
            ? { ...entry, keywords: saved.keywords, keywordSource: saved.source }
            : entry,
        ),
      );
      setKeywordEditPath(null);
    } catch (error) {
      toast.error(t("loraLibrary.keywordsSaveFailed"), errorText(error));
    } finally {
      setSavingKeywords(false);
    }
  };

  const queueDownload = async (version: CivitaiVersion, file: CivitaiFile) => {
    if (!detail || queuingFileId !== null) return;
    setQueuingFileId(file.id);
    try {
      await invoke<string>("civitai_queue_lora_download", {
        request: {
          modelName: detail.name,
          versionId: version.id,
          fileName: file.name,
          sha256: file.sha256,
          downloadUrl: file.downloadUrl,
          trainedWords: version.trainedWords,
          baseModel: version.baseModel,
        },
      });
      toast.success(t("loraLibrary.downloadQueued"), file.name);
    } catch (error) {
      const message = errorText(error);
      if (isCivitaiAuthError(message)) {
        setTokenMenuOpen(true);
      }
      toast.error(t("loraLibrary.downloadFailed"), message);
    } finally {
      setQueuingFileId(null);
    }
  };

  const shouldBlur = (imageNsfwLevel: number, id: number) =>
    imageNsfwLevel > 1 && !showNsfw && !revealed.has(id);

  const copyText = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      toast.success(t("loraLibrary.copied"));
    } catch (error) {
      toast.error(errorText(error));
    }
  };

  const reveal = (id: number) =>
    setRevealed((current) => {
      const next = new Set(current);
      next.add(id);
      return next;
    });

  const architectureBadge = (architecture: string | null) => (
    <span className="rounded bg-violet-500/10 px-1.5 py-0.5 text-[10px] font-medium text-violet-400">
      {loraArchitectureLabel(architecture, t("editModel.sdcpp.architectureUnknown"))}
    </span>
  );

  const renderLibrary = () => (
    <div className="min-h-0 flex-1 overflow-y-auto pb-10">
      <InlineDownloadCards filter={(item) => item.queueKind === "civitai_lora"} />
      {installedLoading ? (
        <div className="flex items-center justify-center py-16">
          <Loader size={18} className="animate-spin text-fg/30" />
        </div>
      ) : installed.length === 0 ? (
        <div className="rounded-xl border border-dashed border-fg/10 bg-fg/2 px-4 py-10 text-center">
          <Layers size={18} className="mx-auto mb-2 text-fg/25" />
          <p className="text-[13px] font-medium text-fg/70">{t("loraLibrary.installedEmpty")}</p>
          <p className="mt-1 text-[12px] text-fg/45">{t("loraLibrary.installedEmptyHint")}</p>
        </div>
      ) : (
        <div data-tour-id="lora-installed" className="space-y-2">
          {installed.map((file) => (
            <div
              key={file.path}
              className="rounded-xl border border-fg/8 bg-fg/[0.025] px-4 py-3"
            >
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0 flex-1">
                  <p className="truncate text-[13px] font-medium text-fg/85">{file.filename}</p>
                  <div className="mt-1 flex flex-wrap items-center gap-1.5 text-[11px] text-fg/45">
                    <span>{formatBytes(file.bytesOnDisk)}</span>
                    {architectureBadge(file.architecture)}
                  </div>
                  {file.keywords.length > 0 ? (
                    <div className="mt-2 flex flex-wrap gap-1">
                      {file.keywords.slice(0, 8).map((keyword) => (
                        <span
                          key={keyword}
                          className="rounded bg-fg/6 px-1.5 py-0.5 text-[10.5px] text-fg/60"
                        >
                          {keyword}
                        </span>
                      ))}
                      {file.keywords.length > 8 && (
                        <span className="px-1 py-0.5 text-[10.5px] text-fg/35">
                          +{file.keywords.length - 8}
                        </span>
                      )}
                    </div>
                  ) : file.keywordSource !== "none" ? (
                    <p className="mt-2 text-[11px] text-fg/35">{t("loraLibrary.alwaysActive")}</p>
                  ) : (
                    <p className="mt-2 text-[11px] text-fg/35">{t("loraLibrary.noKeywords")}</p>
                  )}
                </div>
                <div className="flex shrink-0 items-center gap-1">
                  <button
                    type="button"
                    onClick={() => void handleDiscover(file)}
                    disabled={discoveringPath === file.path}
                    title={t("loraLibrary.discover")}
                    className="rounded-lg p-2 text-fg/40 transition hover:bg-fg/8 hover:text-fg/80"
                  >
                    {discoveringPath === file.path ? (
                      <Loader size={14} className="animate-spin" />
                    ) : (
                      <Sparkles size={14} />
                    )}
                  </button>
                  <button
                    type="button"
                    onClick={() => openKeywordEditor(file)}
                    title={t("loraLibrary.editKeywords")}
                    className="rounded-lg p-2 text-fg/40 transition hover:bg-fg/8 hover:text-fg/80"
                  >
                    <Pencil size={14} />
                  </button>
                  <button
                    type="button"
                    onClick={() => void handleDelete(file)}
                    disabled={deletingPath === file.path}
                    title={t("loraLibrary.deleteConfirm")}
                    className="rounded-lg p-2 text-fg/40 transition hover:bg-fg/8 hover:text-danger"
                  >
                    {deletingPath === file.path ? (
                      <Loader size={14} className="animate-spin" />
                    ) : (
                      <Trash2 size={14} />
                    )}
                  </button>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );

  const renderCard = (item: CivitaiLoraSummary) => {
    const blurred = item.previewImage ? shouldBlur(item.previewImage.nsfwLevel, item.id) : false;
    return (
      <button
        key={item.id}
        type="button"
        onClick={() => openDetail(item.id)}
        className="group overflow-hidden rounded-xl border border-fg/8 bg-fg/[0.025] text-left transition hover:border-fg/20"
      >
        <div className="relative aspect-[3/4] w-full overflow-hidden bg-fg/5">
          {item.previewImage ? (
            <>
              <img
                src={resizedImageUrl(item.previewImage.url, 450)}
                alt={item.name}
                loading="lazy"
                decoding="async"
                className={cn(
                  "h-full w-full object-cover transition group-hover:scale-[1.02]",
                  blurred && "blur-2xl",
                )}
              />
              {blurred && (
                <span
                  role="button"
                  tabIndex={0}
                  onClick={(event) => {
                    event.stopPropagation();
                    reveal(item.id);
                  }}
                  className="absolute inset-0 flex flex-col items-center justify-center gap-1.5 text-fg/70"
                >
                  <Eye size={16} />
                  <span className="text-[11px] font-medium">{t("loraLibrary.showImage")}</span>
                </span>
              )}
            </>
          ) : (
            <div className="flex h-full w-full items-center justify-center text-fg/20">
              <ImageOff size={20} />
            </div>
          )}
        </div>
        <div className="px-3 py-2.5">
          <p className="truncate text-[12.5px] font-medium text-fg/85">{item.name}</p>
          {item.creatorUsername && (
            <p className="truncate text-[10.5px] text-fg/40">
              {t("loraLibrary.by", { name: item.creatorUsername })}
            </p>
          )}
          <div className="mt-1.5 flex items-center gap-3 text-[10.5px] text-fg/45">
            <span className="flex items-center gap-1">
              <Download size={10} />
              {formatCount(item.downloadCount)}
            </span>
            <span className="flex items-center gap-1">
              <Heart size={10} />
              {formatCount(item.thumbsUpCount)}
            </span>
            {item.baseModels[0] && (
              <span className="ml-auto truncate rounded bg-fg/6 px-1.5 py-0.5 text-[9.5px] text-fg/55">
                {item.baseModels[0]}
              </span>
            )}
          </div>
        </div>
      </button>
    );
  };

  const renderBrowse = () => (
    <div className="min-h-0 flex-1 overflow-y-auto pb-10">
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <div data-tour-id="civitai-search" className="relative min-w-0 flex-1">
          <Search
            size={14}
            className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-fg/30"
          />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t("loraLibrary.searchPlaceholder")}
            className="h-10 w-full rounded-xl border border-fg/10 bg-surface pl-9 pr-3 text-[13px] text-fg outline-none transition focus:border-accent/40"
          />
        </div>
        <button
          type="button"
          data-tour-id="civitai-sort"
          onClick={() => setSortMenuOpen(true)}
          className="flex h-10 items-center gap-1.5 rounded-xl border border-fg/10 bg-fg/4 px-3 text-[12px] font-medium text-fg/70 transition hover:border-fg/20 hover:text-fg"
        >
          {t(SORT_OPTIONS.find((option) => option.value === sort)!.labelKey)}
          <ChevronDown size={13} className="text-fg/40" />
        </button>
        <button
          type="button"
          data-tour-id="civitai-token"
          onClick={() => setTokenMenuOpen(true)}
          title={t("loraLibrary.tokenTitle")}
          className="flex h-10 w-10 items-center justify-center rounded-xl border border-fg/10 bg-fg/4 text-fg/50 transition hover:border-fg/20 hover:text-fg"
        >
          <KeyRound size={14} />
        </button>
      </div>
      {availableBaseModels.length > 0 && (
        <div data-tour-id="civitai-filters" className="mb-3 flex flex-wrap gap-1.5">
          <button
            type="button"
            onClick={() => setBaseModelFilter(null)}
            className={cn(
              "rounded-full px-2.5 py-1 text-[11px] font-medium transition",
              baseModelFilter === null
                ? "bg-accent/15 text-accent ring-1 ring-inset ring-accent/30"
                : "bg-fg/5 text-fg/55 hover:text-fg/85",
            )}
          >
            {t("loraLibrary.baseModelAll")}
          </button>
          {availableBaseModels.map((baseModel) => (
            <button
              key={baseModel}
              type="button"
              onClick={() =>
                setBaseModelFilter((current) => (current === baseModel ? null : baseModel))
              }
              className={cn(
                "rounded-full px-2.5 py-1 text-[11px] font-medium transition",
                baseModelFilter === baseModel
                  ? "bg-accent/15 text-accent ring-1 ring-inset ring-accent/30"
                  : "bg-fg/5 text-fg/55 hover:text-fg/85",
              )}
            >
              {baseModel}
            </button>
          ))}
        </div>
      )}
      {!showNsfw && (
        <p className="mb-3 text-[11px] text-fg/40">{t("loraLibrary.pureModeNote")}</p>
      )}
      {civitaiAuth && (!civitaiAuth.saved || civitaiAuth.errorKind === "invalidOrExpired") && (
        <div className="mb-3 flex flex-wrap items-center gap-3 rounded-xl border border-warning/25 bg-warning/10 px-4 py-3">
          <KeyRound size={15} className="shrink-0 text-warning/80" />
          <div className="min-w-0 flex-1">
            <p className="text-[12.5px] font-medium text-fg/85">
              {civitaiAuth.saved
                ? t("loraLibrary.tokenStatusInvalid")
                : t("loraLibrary.tokenWarningTitle")}
            </p>
            <p className="mt-0.5 text-[12px] leading-5 text-fg/55">
              {t("loraLibrary.tokenWarningBody")}
            </p>
          </div>
          <button
            type="button"
            onClick={() => setTokenMenuOpen(true)}
            className="rounded-xl border border-fg/10 bg-fg/4 px-3 py-2 text-[12px] font-medium text-fg/80 transition hover:border-fg/20 hover:text-fg"
          >
            {civitaiAuth.saved ? t("loraLibrary.tokenReplace") : t("loraLibrary.tokenAdd")}
          </button>
        </div>
      )}
      <InlineDownloadCards filter={(item) => item.queueKind === "civitai_lora"} />
      {searchError ? (
        <div className="rounded-xl border border-danger/20 bg-danger/5 px-4 py-6 text-center">
          <p className="text-[12.5px] text-danger/90">{searchError}</p>
        </div>
      ) : searching && results.length === 0 ? (
        <div className="flex items-center justify-center py-16">
          <Loader size={18} className="animate-spin text-fg/30" />
        </div>
      ) : results.length === 0 && searchedOnce ? (
        <div className="rounded-xl border border-dashed border-fg/10 bg-fg/2 px-4 py-10 text-center">
          <p className="text-[13px] font-medium text-fg/70">{t("loraLibrary.noResults")}</p>
          <p className="mt-1 text-[12px] text-fg/45">{t("loraLibrary.noResultsHint")}</p>
        </div>
      ) : (
        <>
          <div
            data-tour-id="civitai-results"
            className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5"
          >
            {results.map(renderCard)}
          </div>
          {nextCursor && (
            <div className="mt-4 flex justify-center">
              <button
                type="button"
                onClick={() => void runSearch({ cursor: nextCursor, append: true })}
                disabled={loadingMore}
                className={cn(
                  "flex items-center gap-2 rounded-xl border border-fg/10 bg-fg/4 px-4 py-2.5 text-[12.5px] font-medium text-fg/75 transition",
                  loadingMore ? "cursor-not-allowed opacity-60" : "hover:border-fg/20 hover:text-fg",
                )}
              >
                {loadingMore && <Loader size={13} className="animate-spin" />}
                {t("loraLibrary.loadMore")}
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );

  const selectedVersion =
    detail?.versions.find((version) => version.id === selectedVersionId) ?? detail?.versions[0];
  const lightboxImages = selectedVersion?.images ?? [];

  useEffect(() => {
    setLightboxIndex(null);
  }, [selectedVersionId, detailId]);

  useEffect(() => {
    if (lightboxIndex === null) return;
    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setLightboxIndex(null);
      } else if (event.key === "ArrowRight") {
        setLightboxIndex((current) =>
          current === null ? null : Math.min(current + 1, lightboxImages.length - 1),
        );
      } else if (event.key === "ArrowLeft") {
        setLightboxIndex((current) => (current === null ? null : Math.max(current - 1, 0)));
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [lightboxIndex, lightboxImages.length]);

  const renderDetail = () => {
    if (detailLoading) {
      return (
        <div className="flex flex-1 items-center justify-center">
          <Loader size={18} className="animate-spin text-fg/30" />
        </div>
      );
    }
    if (detailError || !detail) {
      return (
        <div className="flex-1">
          <div className="rounded-xl border border-danger/20 bg-danger/5 px-4 py-6 text-center">
            <p className="text-[12.5px] text-danger/90">
              {detailError ?? t("loraLibrary.detailLoadFailed")}
            </p>
          </div>
        </div>
      );
    }
    const description = detail.description ? stripHtml(detail.description) : null;
    const images = selectedVersion?.images ?? [];
    const modelUrl = `https://civitai.com/models/${detail.id}`;
    const safeFiles = selectedVersion ? selectedVersion.files.filter(isSafetensorFile) : [];
    return (
      <div className="flex min-h-0 flex-1 flex-col">
        <button
          type="button"
          onClick={closeDetail}
          className="mb-4 flex items-center gap-1.5 text-[12.5px] font-medium text-fg/55 transition hover:text-fg"
        >
          <ArrowLeft size={14} />
          {t("loraLibrary.backToResults")}
        </button>
        <div className="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto pb-8 lg:flex-row lg:overflow-hidden">
          <div className="w-full shrink-0 lg:min-h-0 lg:w-[380px] lg:overflow-y-auto lg:pb-8 lg:pr-1">
            <div className="flex items-start justify-between gap-2">
              <h2 className="text-[16px] font-semibold tracking-tight text-fg">{detail.name}</h2>
              <button
                type="button"
                onClick={() => void copyText(modelUrl)}
                title={t("loraLibrary.copyLink")}
                className="shrink-0 rounded-lg p-2 text-fg/40 transition hover:bg-fg/8 hover:text-fg/80"
              >
                <Copy size={14} />
              </button>
            </div>
            <div className="mt-1 flex flex-wrap items-center gap-x-4 gap-y-1 text-[12px] text-fg/45">
              {detail.creatorUsername && (
                <span>{t("loraLibrary.by", { name: detail.creatorUsername })}</span>
              )}
              <span className="flex items-center gap-1.5">
                <Download size={11} />
                {formatCount(detail.downloadCount)}
              </span>
              <span className="flex items-center gap-1.5">
                <Heart size={11} />
                {formatCount(detail.thumbsUpCount)}
              </span>
            </div>
            {detail.versions.length > 1 && (
              <div className="mt-4">
                <p className="mb-1.5 text-[11px] font-medium uppercase tracking-wide text-fg/40">
                  {t("loraLibrary.versions")}
                </p>
                <div className="flex flex-wrap gap-1.5">
                  {detail.versions.map((version) => (
                    <button
                      key={version.id}
                      type="button"
                      onClick={() => setSelectedVersionId(version.id)}
                      className={cn(
                        "rounded-full px-2.5 py-1 text-[11px] font-medium transition",
                        version.id === selectedVersion?.id
                          ? "bg-accent/15 text-accent ring-1 ring-inset ring-accent/30"
                          : "bg-fg/5 text-fg/55 hover:text-fg/85",
                      )}
                    >
                      {version.name}
                    </button>
                  ))}
                </div>
              </div>
            )}
            {selectedVersion && (
              <>
                <div className="mt-3 flex flex-wrap items-center gap-1.5 text-[11px] text-fg/50">
                  {selectedVersion.baseModel && (
                    <span className="rounded bg-fg/6 px-1.5 py-0.5">{selectedVersion.baseModel}</span>
                  )}
                  {architectureBadge(selectedVersion.architecture)}
                </div>
                <div className="mt-4">
                  <p className="mb-1.5 text-[11px] font-medium uppercase tracking-wide text-fg/40">
                    {t("loraLibrary.filesTitle")}
                  </p>
                  <InlineDownloadCards
                    filter={(item) => item.queueKind === "civitai_lora"}
                    compact
                  />
                  {safeFiles.length === 0 ? (
                    <p className="rounded-xl border border-dashed border-fg/10 bg-fg/2 px-4 py-5 text-center text-[12px] text-fg/45">
                      {t("loraLibrary.noSafeFiles")}
                    </p>
                  ) : (
                    <div className="space-y-1.5">
                      {safeFiles.map((file) => {
                        const alreadyInstalled = installedFilenames.has(file.name.toLowerCase());
                        return (
                          <div
                            key={file.id || file.name}
                            className="flex items-center justify-between gap-3 rounded-xl border border-fg/8 bg-fg/[0.025] px-3.5 py-2.5"
                          >
                            <div className="min-w-0 flex-1">
                              <p className="truncate text-[12.5px] font-medium text-fg/85">
                                {file.name}
                              </p>
                              <div className="mt-0.5 flex items-center gap-2 text-[10.5px] text-fg/45">
                                <span>{formatBytes(file.sizeKb * 1024)}</span>
                                {file.fp && <span className="rounded bg-fg/6 px-1 py-px">{file.fp}</span>}
                                {file.primary && (
                                  <span className="rounded bg-accent/10 px-1 py-px text-accent/80">
                                    {t("loraLibrary.primaryFile")}
                                  </span>
                                )}
                              </div>
                            </div>
                            {alreadyInstalled ? (
                              <span className="flex shrink-0 items-center gap-1.5 text-[11.5px] font-medium text-emerald-500">
                                <Check size={13} />
                                {t("loraLibrary.downloaded")}
                              </span>
                            ) : (
                              <button
                                type="button"
                                onClick={() => void queueDownload(selectedVersion, file)}
                                disabled={queuingFileId !== null}
                                className={cn(
                                  "flex shrink-0 items-center gap-1.5 rounded-lg border border-accent/40 bg-accent/12 px-3 py-1.5 text-[11.5px] font-medium text-accent transition",
                                  queuingFileId !== null
                                    ? "cursor-not-allowed opacity-60"
                                    : "hover:bg-accent/20 active:scale-[0.98]",
                                )}
                              >
                                {queuingFileId === file.id ? (
                                  <Loader size={12} className="animate-spin" />
                                ) : (
                                  <Download size={12} />
                                )}
                                {t("loraLibrary.download")}
                              </button>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>
                <div className="mt-4">
                  <p className="mb-1.5 text-[11px] font-medium uppercase tracking-wide text-fg/40">
                    {t("loraLibrary.triggerWords")}
                  </p>
                  {selectedVersion.trainedWords.length > 0 ? (
                    <div className="flex flex-wrap gap-1">
                      {selectedVersion.trainedWords.map((word) => (
                        <button
                          key={word}
                          type="button"
                          onClick={() => void copyText(word)}
                          title={word}
                          className="rounded bg-fg/6 px-1.5 py-0.5 text-[10.5px] text-fg/60 transition hover:bg-fg/12 hover:text-fg/85"
                        >
                          {word}
                        </button>
                      ))}
                    </div>
                  ) : (
                    <p className="text-[11.5px] text-fg/40">{t("loraLibrary.noKeywords")}</p>
                  )}
                </div>
              </>
            )}
            {description && (
              <p className="mt-5 whitespace-pre-line border-t border-fg/8 pt-4 text-[12.5px] leading-relaxed text-fg/60">
                {description}
              </p>
            )}
          </div>
          <div className="min-w-0 flex-1 lg:min-h-0 lg:overflow-y-auto lg:pb-8">
            {images.length === 0 ? (
              <div className="flex h-64 items-center justify-center rounded-xl border border-dashed border-fg/10 bg-fg/2 text-fg/20">
                <ImageOff size={22} />
              </div>
            ) : (
              <AnimatePresence mode="wait">
                <motion.div
                  key={selectedVersion?.id ?? "none"}
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.35, ease: "easeOut" }}
                  className="columns-2 gap-3 xl:columns-3"
                >
                  {images.map((image, index) => {
                    const blurred = shouldBlur(image.nsfwLevel, detail.id);
                    return (
                      <motion.div
                        key={image.url}
                        initial={{ opacity: 0, y: 12 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{
                          duration: 0.45,
                          delay: Math.min(index * 0.07, 0.6),
                          ease: "easeOut",
                        }}
                        onClick={() => {
                          if (!blurred) setLightboxIndex(index);
                        }}
                        className={cn(
                          "relative mb-3 overflow-hidden rounded-xl border border-fg/8 bg-fg/5 break-inside-avoid",
                          !blurred && "cursor-zoom-in",
                        )}
                      >
                        <img
                          src={resizedImageUrl(image.url, 700)}
                          alt=""
                          loading="lazy"
                          decoding="async"
                          style={{
                            aspectRatio:
                              image.width > 0 && image.height > 0
                                ? `${image.width} / ${image.height}`
                                : undefined,
                          }}
                          className={cn("w-full object-cover", blurred && "blur-2xl")}
                        />
                        {blurred && (
                          <button
                            type="button"
                            onClick={() => reveal(detail.id)}
                            className="absolute inset-0 flex flex-col items-center justify-center gap-1.5 text-fg/70"
                          >
                            <Eye size={16} />
                            <span className="text-[11px] font-medium">
                              {t("loraLibrary.showImage")}
                            </span>
                          </button>
                        )}
                      </motion.div>
                    );
                  })}
                </motion.div>
              </AnimatePresence>
            )}
          </div>
        </div>
      </div>
    );
  };

  if (getPlatform().type === "mobile") {
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
    <div
      className="mx-auto flex w-full max-w-[1280px] flex-col gap-5 overflow-hidden px-5 pt-5 sm:px-8"
      style={{ height: "calc(100dvh - var(--topnav-h, 72px))" }}
    >
      {!detailId && (
        <header className="flex flex-wrap items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <div
              data-tour-id="lora-tabs"
              className="flex items-center gap-1 rounded-full border border-fg/8 bg-fg/[0.025] p-0.5"
            >
              {(
                [
                  { key: "library", label: t("loraLibrary.tabLibrary") },
                  { key: "browse", label: t("loraLibrary.tabBrowse") },
                ] as { key: Tab; label: string }[]
              ).map(({ key, label }) => {
                const active = tab === key;
                return (
                  <button
                    key={key}
                    type="button"
                    data-tour-id={key === "browse" ? "lora-tab-browse" : undefined}
                    onClick={() => setTab(key)}
                    className={cn(
                      "relative flex items-center gap-2 rounded-full px-3.5 py-1.5 text-[12.5px] font-medium transition-colors",
                      active ? "text-fg" : "text-fg/55 hover:text-fg/85",
                    )}
                  >
                    {active && (
                      <motion.span
                        layoutId="loraLibraryTabIndicator"
                        className="absolute inset-0 rounded-full bg-fg/[0.09] ring-1 ring-inset ring-fg/15"
                        transition={{ type: "spring", stiffness: 420, damping: 34, mass: 0.6 }}
                      />
                    )}
                    <span className="relative z-10">{label}</span>
                  </button>
                );
              })}
            </div>
          </div>
          <div className="flex items-center gap-3">
            {tab === "library" && (
              <>
                <p className="text-[12px] text-fg/45">
                  {t("loraLibrary.installedCount", { count: installed.length })}
                </p>
                <button
                  type="button"
                  data-tour-id="lora-import"
                  onClick={() => void handleImport()}
                  disabled={importing}
                  className={cn(
                    "flex items-center gap-2 rounded-xl border border-fg/10 bg-fg/4 px-3 py-2 text-[12.5px] font-medium text-fg/80 transition",
                    importing ? "cursor-not-allowed opacity-60" : "hover:border-fg/20 hover:text-fg",
                  )}
                >
                  {importing ? (
                    <Loader size={13} className="animate-spin" />
                  ) : (
                    <FileUp size={13} className="text-fg/45" />
                  )}
                  {t("loraLibrary.importButton")}
                </button>
              </>
            )}
            {returnTo && (
              <button
                type="button"
                onClick={() => navigate(returnTo)}
                className="flex items-center gap-1.5 rounded-xl border border-fg/10 bg-fg/4 px-3 py-2 text-[12.5px] font-medium text-fg/75 transition hover:border-fg/20 hover:text-fg"
              >
                <ArrowLeft size={13} />
                {t("loraLibrary.backToCaller")}
              </button>
            )}
          </div>
        </header>
      )}

      {detailId ? renderDetail() : tab === "library" ? renderLibrary() : renderBrowse()}

      <BottomMenu
        isOpen={sortMenuOpen}
        onClose={() => setSortMenuOpen(false)}
        title={t("loraLibrary.sortTitle")}
      >
        <div className="space-y-4">
          <div className="space-y-1.5">
            {SORT_OPTIONS.map((option) => (
              <button
                key={option.value}
                type="button"
                onClick={() => {
                  setSort(option.value);
                  setSortMenuOpen(false);
                }}
                className={cn(
                  "flex w-full items-center justify-between rounded-xl border px-4 py-3 text-left text-[13px] font-medium transition",
                  sort === option.value
                    ? "border-accent/40 bg-accent/10 text-accent"
                    : "border-fg/10 bg-fg/3 text-fg/75 hover:border-fg/20",
                )}
              >
                {t(option.labelKey)}
                {sort === option.value && <Check size={14} />}
              </button>
            ))}
          </div>
          <div>
            <p className="mb-1.5 text-[11px] font-medium uppercase tracking-wide text-fg/40">
              {t("loraLibrary.periodTitle")}
            </p>
            <div className="flex flex-wrap gap-1.5">
              {PERIOD_OPTIONS.map((option) => (
                <button
                  key={option.value}
                  type="button"
                  onClick={() => setPeriod(option.value)}
                  className={cn(
                    "rounded-full px-2.5 py-1 text-[11px] font-medium transition",
                    period === option.value
                      ? "bg-accent/15 text-accent ring-1 ring-inset ring-accent/30"
                      : "bg-fg/5 text-fg/55 hover:text-fg/85",
                  )}
                >
                  {t(option.labelKey)}
                </button>
              ))}
            </div>
          </div>
        </div>
      </BottomMenu>

      <BottomMenu
        isOpen={keywordEditPath !== null}
        onClose={() => {
          if (!savingKeywords) setKeywordEditPath(null);
        }}
        title={t("loraLibrary.keywordsTitle")}
      >
        <p className="mb-3 text-[12.5px] leading-relaxed text-fg/55">
          {t("loraLibrary.keywordsHint")}
        </p>
        <textarea
          value={keywordDraft}
          onChange={(event) => setKeywordDraft(event.target.value)}
          rows={6}
          className="w-full rounded-xl border border-fg/10 bg-surface px-3 py-2.5 text-sm text-fg outline-none transition focus:border-accent/40"
        />
        <button
          type="button"
          onClick={() => void saveKeywords()}
          disabled={savingKeywords}
          className={cn(
            "mt-4 flex w-full items-center justify-center gap-2 rounded-xl border border-accent/40 bg-accent/15 px-4 py-3 text-sm font-medium text-accent transition",
            savingKeywords ? "cursor-not-allowed opacity-60" : "hover:bg-accent/25 active:scale-[0.99]",
          )}
        >
          {savingKeywords && <Loader size={14} className="animate-spin" />}
          {t("loraLibrary.keywordsSave")}
        </button>
      </BottomMenu>

      <AnimatePresence>
        {detail && lightboxIndex !== null && lightboxImages[lightboxIndex] && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
            className="fixed inset-0 z-100 flex items-center justify-center bg-black/95 p-4"
            onClick={() => setLightboxIndex(null)}
          >
            <button
              type="button"
              onClick={() => setLightboxIndex(null)}
              className="absolute right-5 top-5 z-101 flex h-10 w-10 items-center justify-center rounded-full bg-white/10 text-white/80 transition hover:bg-white/20 hover:text-white"
            >
              <X size={18} />
            </button>
            {lightboxIndex > 0 && (
              <button
                type="button"
                onClick={(event) => {
                  event.stopPropagation();
                  setLightboxIndex(lightboxIndex - 1);
                }}
                className="absolute left-5 top-1/2 z-101 flex h-10 w-10 -translate-y-1/2 items-center justify-center rounded-full bg-white/10 text-white/80 transition hover:bg-white/20 hover:text-white"
              >
                <ChevronLeft size={18} />
              </button>
            )}
            {lightboxIndex < lightboxImages.length - 1 && (
              <button
                type="button"
                onClick={(event) => {
                  event.stopPropagation();
                  setLightboxIndex(lightboxIndex + 1);
                }}
                className="absolute right-5 top-1/2 z-101 flex h-10 w-10 -translate-y-1/2 items-center justify-center rounded-full bg-white/10 text-white/80 transition hover:bg-white/20 hover:text-white"
              >
                <ChevronRight size={18} />
              </button>
            )}
            <motion.img
              key={lightboxImages[lightboxIndex].url}
              initial={{ opacity: 0, scale: 0.96 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.96 }}
              transition={{ duration: 0.2 }}
              src={resizedImageUrl(lightboxImages[lightboxIndex].url, 1200)}
              alt=""
              className="max-h-[92vh] max-w-[92vw] rounded-2xl object-contain shadow-[0_30px_80px_rgba(0,0,0,0.45)]"
              onClick={(event) => event.stopPropagation()}
            />
            <span className="absolute bottom-5 left-1/2 -translate-x-1/2 rounded-full bg-white/10 px-2.5 py-1 text-[11px] tabular-nums text-white/70">
              {lightboxIndex + 1} / {lightboxImages.length}
            </span>
          </motion.div>
        )}
      </AnimatePresence>

      <CivitaiTokenMenu
        isOpen={tokenMenuOpen}
        onClose={() => setTokenMenuOpen(false)}
        onSaved={(status) => {
          setCivitaiAuth(status);
          if (status.valid) toast.success(t("loraLibrary.tokenSaved"));
          else toast.warning(t("loraLibrary.tokenUnverified"));
        }}
      />

      {!detailId && tab === "library" && showLibraryTour && (
        <GuidedTour tour="loraLibrary" onDismiss={dismissLibraryTour} />
      )}
      {!detailId && tab === "browse" && showBrowseTour && (
        <GuidedTour tour="civitaiBrowse" onDismiss={dismissBrowseTour} />
      )}
    </div>
  );
}

export default LoraLibraryPage;
