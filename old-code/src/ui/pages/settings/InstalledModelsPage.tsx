import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import {
  AlertCircle,
  ArrowUpDown,
  ChevronDown,
  Copy,
  Cpu,
  HardDrive,
  Loader,
  RefreshCw,
  Search,
  Trash2,
} from "lucide-react";

import { cn } from "../../design-tokens";
import { confirmBottomMenu } from "../../components/ConfirmBottomMenu";
import { toast } from "../../components/toast";
import { useI18n } from "../../../core/i18n/context";
import { BottomMenu } from "../../components/BottomMenu";
import {
  readSettings,
  readSettingsCached,
  SETTINGS_UPDATED_EVENT,
} from "../../../core/storage/repo";
import type { ProviderCredential } from "../../../core/storage/schemas";
import { getPlatform } from "../../../core/utils/platform";
import OllamaIcon from "../../../assets/ollama_light.png";

type InstalledGgufModel = {
  modelId: string;
  filename: string;
  path: string;
  size: number;
  quantization: string;
  isMmproj: boolean;
  architecture?: string | null;
  contextLength?: number | null;
  imageRole?: string | null;
};

type OllamaInstalledModel = {
  name: string;
  size: number | null;
  modified_at: string | null;
  digest: string | null;
  parameter_size: string | null;
  quantization_level: string | null;
  family: string | null;
};

type Tab = "local" | "ollama";
type ContentMode = "all" | "llm" | "image";

const IMAGE_ROLE_LABELS: Record<string, string> = {
  checkpoint: "CHECKPOINT",
  diffusionModel: "DIFFUSION",
  clipL: "CLIP-L",
  clipG: "CLIP-G",
  t5xxl: "T5-XXL",
  llm: "TEXT-ENC",
  llmVision: "VISION-ENC",
  vae: "VAE",
  lora: "LORA",
};

type SortField = "params" | "arch" | "context" | "size";
type SortDirection = "desc" | "asc";

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / Math.pow(1024, index);
  return `${value.toFixed(index > 1 ? 1 : 0)} ${units[index]}`;
}

function extractParamSize(modelId: string, filename: string): string | null {
  const tagSource = `${modelId} ${filename}`;
  const direct = tagSource.match(/(?:^|[-_/ ])(\d+(?:\.\d+)?)(b|m|k|t)(?:[-_/ .]|$)/i);
  if (!direct) return null;
  return `${direct[1]}${direct[2].toUpperCase()}`;
}

function paramSizeToNumber(value: string | null): number | null {
  if (!value) return null;
  const match = value.match(/^(\d+(?:\.\d+)?)([KMBT])$/i);
  if (!match) return null;
  const numeric = Number(match[1]);
  const unit = match[2].toUpperCase();
  const multiplier =
    unit === "T"
      ? 1_000_000_000_000
      : unit === "B"
        ? 1_000_000_000
        : unit === "M"
          ? 1_000_000
          : 1_000;
  return numeric * multiplier;
}

function deriveDisplayName(filename: string): string {
  return (filename.split("/").pop() || filename).replace(/\.gguf$/i, "");
}

function formatContextLength(contextLength?: number | null): string {
  if (!contextLength || contextLength <= 0) return "—";
  return `${contextLength.toLocaleString()} ctx`;
}

function relativeTime(iso: string | null): string {
  if (!iso) return "—";
  const date = new Date(iso);
  if (isNaN(date.getTime())) return "—";
  const seconds = Math.floor((Date.now() - date.getTime()) / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  const months = Math.floor(days / 30);
  if (months < 12) return `${months}mo ago`;
  return `${Math.floor(months / 12)}y ago`;
}

export function InstalledModelsPage() {
  const { t } = useI18n();
  const isMobilePlatform = useMemo(() => getPlatform().type === "mobile", []);
  const [tab, setTab] = useState<Tab>(isMobilePlatform ? "ollama" : "local");
  const [contentMode, setContentMode] = useState<ContentMode>("all");

  // ---- Local state ----
  const [query, setQuery] = useState("");
  const [modelsDir, setModelsDir] = useState("");
  const [models, setModels] = useState<InstalledGgufModel[]>([]);
  const [imageModels, setImageModels] = useState<InstalledGgufModel[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [deletingPath, setDeletingPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [sortField, setSortField] = useState<SortField | null>(null);
  const [sortDirection, setSortDirection] = useState<SortDirection | null>(null);

  // ---- Ollama state ----
  const [ollamaProviders, setOllamaProviders] = useState<ProviderCredential[]>([]);
  const [selectedOllamaProviderId, setSelectedOllamaProviderId] = useState<string | null>(null);
  const [showProviderPicker, setShowProviderPicker] = useState(false);
  const [ollamaQuery, setOllamaQuery] = useState("");
  const [ollamaModels, setOllamaModels] = useState<OllamaInstalledModel[]>([]);
  const [ollamaLoading, setOllamaLoading] = useState(false);
  const [ollamaRefreshing, setOllamaRefreshing] = useState(false);
  const [ollamaError, setOllamaError] = useState<string | null>(null);
  const [deletingOllama, setDeletingOllama] = useState<string | null>(null);

  const selectedOllamaProvider = useMemo(
    () => ollamaProviders.find((p) => p.id === selectedOllamaProviderId) ?? null,
    [ollamaProviders, selectedOllamaProviderId],
  );

  // ---- Local: load ----
  const loadModels = useCallback(async (mode: "initial" | "refresh" = "initial") => {
    if (mode === "initial") setLoading(true);
    else setRefreshing(true);
    try {
      const [dir, downloaded, imageFiles] = await Promise.all([
        invoke<string>("hf_get_gguf_models_dir"),
        invoke<InstalledGgufModel[]>("hf_list_downloaded_models"),
        invoke<InstalledGgufModel[]>("hf_list_downloaded_image_models"),
      ]);
      setModelsDir(dir);
      setModels(
        [...downloaded].sort((l, r) =>
          l.modelId !== r.modelId
            ? l.modelId.localeCompare(r.modelId)
            : l.filename.localeCompare(r.filename),
        ),
      );
      setImageModels(imageFiles);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    if (isMobilePlatform) return;
    void loadModels("initial");
  }, [loadModels, isMobilePlatform]);

  // ---- Ollama: provider sync ----
  useEffect(() => {
    const sync = async () => {
      const settings = await readSettings();
      const list = settings.providerCredentials.filter((p) => p.providerId === "ollama");
      setOllamaProviders(list);
      setSelectedOllamaProviderId((prev) => {
        if (prev && list.some((p) => p.id === prev)) return prev;
        return list[0]?.id ?? null;
      });
    };

    const cached = readSettingsCached();
    if (cached) {
      const list = cached.providerCredentials.filter((p) => p.providerId === "ollama");
      setOllamaProviders(list);
      setSelectedOllamaProviderId((prev) => {
        if (prev && list.some((p) => p.id === prev)) return prev;
        return list[0]?.id ?? null;
      });
    }

    void sync();
    const handler = () => void sync();
    window.addEventListener(SETTINGS_UPDATED_EVENT, handler);
    return () => window.removeEventListener(SETTINGS_UPDATED_EVENT, handler);
  }, []);

  // ---- Ollama: load ----
  const loadOllama = useCallback(
    async (mode: "initial" | "refresh" = "initial") => {
      if (!selectedOllamaProviderId) {
        setOllamaModels([]);
        setOllamaError(null);
        return;
      }
      if (mode === "initial") setOllamaLoading(true);
      else setOllamaRefreshing(true);
      try {
        const list = await invoke<OllamaInstalledModel[]>("ollama_inventory_list", {
          credentialId: selectedOllamaProviderId,
        });
        setOllamaModels(list);
        setOllamaError(null);
      } catch (err) {
        setOllamaError(
          typeof err === "string" ? err : err instanceof Error ? err.message : String(err),
        );
        setOllamaModels([]);
      } finally {
        setOllamaLoading(false);
        setOllamaRefreshing(false);
      }
    },
    [selectedOllamaProviderId],
  );

  useEffect(() => {
    void loadOllama("initial");
  }, [loadOllama]);

  // ---- Local: derived ----
  const visibleModels = useMemo(() => {
    if (contentMode === "llm") return models;
    if (contentMode === "image") return imageModels;
    return [...models, ...imageModels];
  }, [contentMode, models, imageModels]);

  const filteredModels = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return visibleModels;
    return visibleModels.filter((m) => {
      return (
        m.modelId.toLowerCase().includes(needle) ||
        m.filename.toLowerCase().includes(needle) ||
        m.path.toLowerCase().includes(needle) ||
        (m.architecture ?? "").toLowerCase().includes(needle) ||
        (m.imageRole ?? "").toLowerCase().includes(needle) ||
        m.quantization.toLowerCase().includes(needle)
      );
    });
  }, [visibleModels, query]);

  const sortedModels = useMemo(() => {
    if (!sortField || !sortDirection) return filteredModels;
    const compareN = (a: number | null, b: number | null) => {
      if (a === null && b === null) return 0;
      if (a === null) return 1;
      if (b === null) return -1;
      return a - b;
    };
    const compareS = (a: string | null, b: string | null) => {
      if (!a && !b) return 0;
      if (!a) return 1;
      if (!b) return -1;
      return a.localeCompare(b);
    };
    const dir = sortDirection === "desc" ? -1 : 1;
    return [...filteredModels].sort((l, r) => {
      let c = 0;
      if (sortField === "params") {
        c = compareN(
          paramSizeToNumber(extractParamSize(l.modelId, l.filename)),
          paramSizeToNumber(extractParamSize(r.modelId, r.filename)),
        );
      } else if (sortField === "arch") {
        c = compareS(l.architecture ?? null, r.architecture ?? null);
      } else if (sortField === "context") {
        c = compareN(l.contextLength ?? null, r.contextLength ?? null);
      } else if (sortField === "size") {
        c = l.size - r.size;
      }
      if (c === 0) return l.filename.localeCompare(r.filename);
      return c * dir;
    });
  }, [filteredModels, sortDirection, sortField]);

  const totalSize = useMemo(() => sortedModels.reduce((s, m) => s + m.size, 0), [sortedModels]);

  const filteredOllamaModels = useMemo(() => {
    const needle = ollamaQuery.trim().toLowerCase();
    const sorted = [...ollamaModels].sort((l, r) => l.name.localeCompare(r.name));
    if (!needle) return sorted;
    return sorted.filter(
      (m) =>
        m.name.toLowerCase().includes(needle) ||
        (m.family ?? "").toLowerCase().includes(needle) ||
        (m.quantization_level ?? "").toLowerCase().includes(needle),
    );
  }, [ollamaModels, ollamaQuery]);

  const ollamaTotalSize = useMemo(
    () => filteredOllamaModels.reduce((s, m) => s + (m.size ?? 0), 0),
    [filteredOllamaModels],
  );

  // ---- Handlers ----
  const handleCopyPath = useCallback(
    async (path: string) => {
      try {
        await navigator.clipboard.writeText(path);
        toast.success(t("installedModels.toasts.pathCopied"), path);
      } catch (err) {
        toast.error(
          t("installedModels.toasts.copyFailed"),
          err instanceof Error ? err.message : String(err),
        );
      }
    },
    [t],
  );

  const findFileUsage = useCallback(
    async (path: string): Promise<string[]> => {
      const normalize = (value: string) => value.replace(/\\/g, "/");
      const target = normalize(path);
      const matches = (candidate: string | null | undefined) =>
        typeof candidate === "string" && candidate.length > 0 && normalize(candidate) === target;

      try {
        const settings = await readSettings();
        const usage: string[] = [];
        for (const configured of settings.models ?? []) {
          const advanced = configured.advancedModelSettings;
          if (matches(configured.name)) {
            usage.push(configured.displayName);
          }
          if (matches(advanced?.llamaMmprojPath)) {
            usage.push(`${configured.displayName} (mmproj)`);
          }
          if (matches(advanced?.llamaMtpModelPath)) {
            usage.push(`${configured.displayName} (MTP)`);
          }
        }
        const globalAdvanced = settings.advancedModelSettings;
        if (matches(globalAdvanced?.llamaMmprojPath) || matches(globalAdvanced?.llamaMtpModelPath)) {
          usage.push(t("installedModels.confirm.inUseGlobalDefaults"));
        }
        return usage;
      } catch {
        return [];
      }
    },
    [t],
  );

  const handleDeleteModel = useCallback(
    async (model: InstalledGgufModel) => {
      const usedBy = await findFileUsage(model.path);
      const confirmed = await confirmBottomMenu({
        title: t("installedModels.confirm.deleteTitle"),
        message: t("installedModels.confirm.deleteMessage", { filename: model.filename }),
        confirmLabel: t("common.buttons.delete"),
        destructive: true,
        warning:
          usedBy.length > 0
            ? {
                title: t("installedModels.confirm.inUseTitle"),
                body: t("installedModels.confirm.inUseBody", { models: usedBy.join(", ") }),
              }
            : undefined,
      });
      if (!confirmed) return;
      try {
        setDeletingPath(model.path);
        await invoke("hf_delete_downloaded_model", { filePath: model.path });
        toast.success(t("installedModels.toasts.modelDeleted"), model.filename);
        await loadModels("refresh");
      } catch (err) {
        toast.error(
          t("installedModels.toasts.deleteFailed"),
          err instanceof Error ? err.message : String(err),
        );
      } finally {
        setDeletingPath(null);
      }
    },
    [findFileUsage, loadModels, t],
  );

  const handleDeleteOllama = useCallback(
    async (model: OllamaInstalledModel) => {
      if (!selectedOllamaProvider) return;
      const confirmed = await confirmBottomMenu({
        title: t("installedModels.ollama.confirm.deleteTitle"),
        message: t("installedModels.ollama.confirm.deleteMessage", {
          name: model.name,
          provider: selectedOllamaProvider.label,
        }),
        confirmLabel: t("common.buttons.delete"),
        destructive: true,
      });
      if (!confirmed) return;
      try {
        setDeletingOllama(model.name);
        await invoke("ollama_inventory_delete", {
          credentialId: selectedOllamaProvider.id,
          modelName: model.name,
        });
        toast.success(t("installedModels.ollama.toasts.modelDeleted"), model.name);
        await loadOllama("refresh");
      } catch (err) {
        toast.error(
          t("installedModels.ollama.toasts.deleteFailed"),
          err instanceof Error ? err.message : String(err),
        );
      } finally {
        setDeletingOllama(null);
      }
    },
    [loadOllama, selectedOllamaProvider, t],
  );

  const cycleSort = useCallback(
    (field: SortField) => {
      if (sortField !== field) {
        setSortField(field);
        setSortDirection("desc");
        return;
      }
      if (sortDirection === "desc") {
        setSortDirection("asc");
        return;
      }
      setSortField(null);
      setSortDirection(null);
    },
    [sortDirection, sortField],
  );

  const sortIndicator = useCallback(
    (field: SortField) => {
      if (sortField !== field || !sortDirection) return null;
      return sortDirection === "desc" ? "↓" : "↑";
    },
    [sortDirection, sortField],
  );

  const renderSortHeader = useCallback(
    (field: SortField, label: string) => {
      const isActive = sortField === field && !!sortDirection;
      return (
        <button
          onClick={() => cycleSort(field)}
          className={cn(
            "group flex w-full items-center justify-between gap-1.5 rounded-md px-1.5 py-1 text-left transition",
            isActive ? "text-fg/85" : "text-fg/40 hover:text-fg/70",
          )}
        >
          <span>{label}</span>
          <span
            className={cn(
              "inline-flex items-center text-[11px] transition",
              isActive ? "text-accent" : "text-fg/30 group-hover:text-fg/50",
            )}
          >
            {isActive ? sortIndicator(field) : <ArrowUpDown size={11} strokeWidth={2} />}
          </span>
        </button>
      );
    },
    [cycleSort, sortDirection, sortField, sortIndicator],
  );

  // ---- Render helpers ----
  const tabs: Array<{ key: Tab; label: string; count: number; size: number }> = [
    {
      key: "local",
      label: t("installedModels.tabs.local"),
      count: models.length + imageModels.length,
      size: [...models, ...imageModels].reduce((s, m) => s + m.size, 0),
    },
    {
      key: "ollama",
      label: t("installedModels.tabs.ollama"),
      count: ollamaModels.length,
      size: ollamaModels.reduce((s, m) => s + (m.size ?? 0), 0),
    },
  ];

  return (
    <div
      className="mx-auto flex w-full max-w-[1280px] flex-col gap-6 overflow-hidden px-5 pt-5 sm:px-8"
      style={{ height: "calc(100dvh - var(--topnav-h, 72px))" }}
    >
      {/* Header */}
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-[16px] font-semibold tracking-tight text-fg">
            {t("installedModels.title")}
          </h1>
          <p className="mt-1 text-[12.5px] text-fg/50">{t("installedModels.subtitle")}</p>
        </div>

        {/* Tab pills (left-aligned, content-sized) */}
        {!isMobilePlatform && (
          <div className="flex items-center gap-1 rounded-full border border-fg/8 bg-fg/[0.025] p-0.5">
            {tabs.map(({ key, label, count }) => {
              const active = tab === key;
              return (
                <button
                  key={key}
                  onClick={() => setTab(key)}
                  className={cn(
                    "relative flex items-center gap-2 rounded-full px-3.5 py-1.5 text-[12.5px] font-medium transition-colors",
                    active ? "text-fg" : "text-fg/55 hover:text-fg/85",
                  )}
                >
                  {active && (
                    <motion.span
                      layoutId="installedTabIndicator"
                      className="absolute inset-0 rounded-full bg-fg/[0.09] ring-1 ring-inset ring-fg/15"
                      transition={{ type: "spring", stiffness: 420, damping: 34, mass: 0.6 }}
                    />
                  )}
                  <span className="relative z-10 flex items-center gap-2">
                    {key === "local" ? (
                      <HardDrive size={12} />
                    ) : (
                      <img src={OllamaIcon} alt="Ollama" className="h-3 w-3" />
                    )}
                    {label}
                    <span
                      className={cn(
                        "rounded px-1.5 text-[10px] tabular-nums leading-[1.4]",
                        active ? "bg-fg/12 text-fg/75" : "bg-fg/5 text-fg/40",
                      )}
                    >
                      {count}
                    </span>
                  </span>
                </button>
              );
            })}
          </div>
        )}

      </header>

      {tab === "local" ? (
        <LocalView
          modelsDir={modelsDir}
          query={query}
          setQuery={setQuery}
          contentMode={contentMode}
          setContentMode={setContentMode}
          loading={loading}
          refreshing={refreshing}
          error={error}
          totalSize={totalSize}
          sortedModels={sortedModels}
          deletingPath={deletingPath}
          onCopyPath={handleCopyPath}
          onDelete={handleDeleteModel}
          onRefresh={() => void loadModels("refresh")}
          renderSortHeader={renderSortHeader}
          t={t}
        />
      ) : (
        <OllamaView
          providers={ollamaProviders}
          selectedProvider={selectedOllamaProvider}
          onPickProvider={() => setShowProviderPicker(true)}
          hideProviderSelector={isMobilePlatform}
          query={ollamaQuery}
          setQuery={setOllamaQuery}
          loading={ollamaLoading}
          refreshing={ollamaRefreshing}
          error={ollamaError}
          models={filteredOllamaModels}
          totalCount={ollamaModels.length}
          totalSize={ollamaTotalSize}
          deletingName={deletingOllama}
          onDelete={handleDeleteOllama}
          onRefresh={() => void loadOllama("refresh")}
          t={t}
        />
      )}

      <BottomMenu
        isOpen={showProviderPicker}
        onClose={() => setShowProviderPicker(false)}
        title={t("installedModels.ollama.pickProvider")}
      >
        <div>
          <p className="mb-4 text-[12.5px] leading-relaxed text-fg/55">
            {t("installedModels.ollama.pickProviderHint")}
          </p>
          {ollamaProviders.length === 0 ? (
            <div className="rounded-xl border border-dashed border-fg/10 bg-fg/2 px-4 py-6 text-center">
              <img src={OllamaIcon} alt="Ollama" className="mx-auto mb-2 h-5 w-5 opacity-40" />
              <p className="text-[12px] leading-snug text-fg/55">
                {t("installedModels.ollama.noProviders")}
              </p>
            </div>
          ) : (
            <div className="space-y-1.5">
              {ollamaProviders.map((p) => {
                const active = p.id === selectedOllamaProviderId;
                return (
                  <button
                    key={p.id}
                    onClick={() => {
                      setSelectedOllamaProviderId(p.id);
                      setShowProviderPicker(false);
                    }}
                    className={cn(
                      "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                      active
                        ? "border-emerald-400/35 bg-emerald-500/8"
                        : "border-fg/10 bg-fg/3 hover:border-fg/20 hover:bg-fg/5",
                    )}
                  >
                    <span
                      className={cn(
                        "flex h-9 w-9 items-center justify-center rounded-lg",
                        active ? "bg-emerald-500/15" : "bg-fg/8",
                      )}
                    >
                      <img
                        src={OllamaIcon}
                        alt="Ollama"
                        className={cn("h-5 w-5", !active && "opacity-60")}
                      />
                    </span>
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-[13px] font-medium text-fg">{p.label}</div>
                      {p.baseUrl && (
                        <div className="truncate font-mono text-[11px] text-fg/45">{p.baseUrl}</div>
                      )}
                    </div>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </BottomMenu>
    </div>
  );
}

// =============================================================================
// Local view
// =============================================================================

interface LocalViewProps {
  modelsDir: string;
  query: string;
  setQuery: (v: string) => void;
  contentMode: ContentMode;
  setContentMode: (mode: ContentMode) => void;
  loading: boolean;
  refreshing: boolean;
  error: string | null;
  totalSize: number;
  sortedModels: InstalledGgufModel[];
  deletingPath: string | null;
  onCopyPath: (p: string) => void;
  onDelete: (m: InstalledGgufModel) => void;
  onRefresh: () => void;
  renderSortHeader: (field: SortField, label: string) => React.ReactElement;
  t: (k: any, v?: any) => string;
}

function LocalView({
  modelsDir,
  query,
  setQuery,
  contentMode,
  setContentMode,
  loading,
  refreshing,
  error,
  totalSize,
  sortedModels,
  deletingPath,
  onCopyPath,
  onDelete,
  onRefresh,
  renderSortHeader,
  t,
}: LocalViewProps) {
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-6">
      {/* Toolbar */}
      <Toolbar
        leading={
          <div className="flex min-w-0 items-center gap-2">
            <div className="min-w-0 flex-1">
              <SearchInput
                value={query}
                onChange={setQuery}
                placeholder={t("installedModels.searchPlaceholder")}
              />
            </div>
            <div className="flex shrink-0 items-center gap-1 rounded-full border border-fg/8 bg-fg/[0.025] p-0.5">
              {(
                [
                  ["all", t("installedModels.modes.all")],
                  ["llm", t("installedModels.modes.llm")],
                  ["image", t("installedModels.modes.image")],
                ] as Array<[ContentMode, string]>
              ).map(([mode, label]) => {
                const active = contentMode === mode;
                return (
                  <button
                    key={mode}
                    onClick={() => setContentMode(mode)}
                    className={cn(
                      "rounded-full px-3 py-1 text-[12px] font-medium transition-colors",
                      active
                        ? "bg-fg/[0.09] text-fg ring-1 ring-inset ring-fg/15"
                        : "text-fg/55 hover:text-fg/85",
                    )}
                  >
                    {label}
                  </button>
                );
              })}
            </div>
          </div>
        }
        meta={[
          { value: String(sortedModels.length), unit: t("installedModels.units.files") },
          { value: formatBytes(totalSize), tone: "accent" },
        ]}
        onRefresh={onRefresh}
        refreshing={refreshing}
        refreshLabel={t("common.buttons.refresh")}
      />

      {/* Path hint */}
      <PathHint
        icon={<HardDrive size={11} />}
        path={modelsDir}
        onCopy={() => void onCopyPath(modelsDir)}
      />

      {loading ? (
        <SkeletonList />
      ) : error ? (
        <ErrorBox message={t("installedModels.loadFailed", { error })} />
      ) : sortedModels.length === 0 ? (
        <EmptyState
          icon={<Cpu size={18} className="text-fg/45" />}
          title={t("installedModels.empty.title")}
          description={t("installedModels.empty.description")}
        />
      ) : (
        <div className="flex min-h-0 flex-1 flex-col">
          <div className="hidden grid-cols-[minmax(0,1.6fr)_70px_80px_90px_120px_100px_160px] items-center gap-3 border-b border-fg/10 px-2 py-2 text-[10px] font-semibold uppercase tracking-[0.12em] text-fg/35 lg:grid">
            <div className="pl-0.5">{t("common.labels.name")}</div>
            <div>{t("installedModels.columns.type")}</div>
            {renderSortHeader("params", t("installedModels.columns.params"))}
            {renderSortHeader("arch", t("installedModels.columns.arch"))}
            {renderSortHeader("context", t("installedModels.columns.context"))}
            {renderSortHeader("size", t("installedModels.columns.size"))}
            <div className="pr-0.5 text-right">{t("installedModels.columns.action")}</div>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto pb-10">
            {sortedModels.map((model) => {
              const paramSize = extractParamSize(model.modelId, model.filename);
              const isMmproj = model.isMmproj;
              return (
                <div
                  key={`${model.path}-${model.filename}`}
                  className="group border-b border-fg/[0.05] px-2 py-2.5 transition hover:bg-fg/[0.02]"
                >
                  <div className="grid gap-3 lg:grid-cols-[minmax(0,1.6fr)_70px_80px_90px_120px_100px_160px] lg:items-center">
                    <div className="min-w-0">
                      <div className="truncate text-[13px] font-medium text-fg">
                        {deriveDisplayName(model.filename)}
                      </div>
                      <div className="mt-0.5 truncate text-[11px] text-fg/40">{model.modelId}</div>
                      <div className="mt-2 flex flex-wrap items-center gap-1.5 lg:hidden">
                        <TypeBadge isMmproj={isMmproj} imageRole={model.imageRole} />
                        <Pill>{model.quantization}</Pill>
                        {paramSize && <Pill>{paramSize}</Pill>}
                        <Pill>{model.architecture?.toUpperCase() || "—"}</Pill>
                        <Pill>{formatContextLength(model.contextLength)}</Pill>
                        <Pill>{formatBytes(model.size)}</Pill>
                      </div>
                    </div>

                    <div className="hidden lg:block">
                      <TypeBadge isMmproj={isMmproj} imageRole={model.imageRole} />
                    </div>
                    <div className="hidden text-[12.5px] text-fg/70 lg:block">
                      {paramSize || "—"}
                    </div>
                    <div className="hidden text-[12.5px] text-fg/70 lg:block">
                      {model.architecture?.toUpperCase() || "—"}
                    </div>
                    <div className="hidden text-[12.5px] text-fg/70 lg:block">
                      {formatContextLength(model.contextLength)}
                    </div>
                    <div className="hidden text-[12.5px] font-medium tabular-nums text-fg lg:block">
                      {formatBytes(model.size)}
                    </div>

                    <div className="flex items-center justify-end gap-1">
                      <span className="hidden rounded bg-fg/6 px-1.5 py-0.5 font-mono text-[10px] text-fg/55 lg:inline-flex">
                        {model.quantization}
                      </span>
                      <RowAction
                        icon={<Copy size={12} />}
                        label={t("common.buttons.copy")}
                        onClick={() => void onCopyPath(model.path)}
                      />
                      <RowAction
                        icon={<Trash2 size={12} />}
                        label={t("common.buttons.delete")}
                        onClick={() => void onDelete(model)}
                        disabled={deletingPath === model.path}
                        destructive
                      />
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}

// =============================================================================
// Ollama view
// =============================================================================

interface OllamaViewProps {
  providers: ProviderCredential[];
  selectedProvider: ProviderCredential | null;
  onPickProvider: () => void;
  hideProviderSelector?: boolean;
  query: string;
  setQuery: (v: string) => void;
  loading: boolean;
  refreshing: boolean;
  error: string | null;
  models: OllamaInstalledModel[];
  totalCount: number;
  totalSize: number;
  deletingName: string | null;
  onDelete: (m: OllamaInstalledModel) => void;
  onRefresh: () => void;
  t: (k: any, v?: any) => string;
}

function OllamaView({
  providers,
  selectedProvider,
  onPickProvider,
  hideProviderSelector,
  query,
  setQuery,
  loading,
  refreshing,
  error,
  models,
  totalCount,
  totalSize,
  deletingName,
  onDelete,
  onRefresh,
  t,
}: OllamaViewProps) {
  if (providers.length === 0) {
    return (
      <EmptyState
        icon={<img src={OllamaIcon} alt="Ollama" className="h-5 w-5 opacity-50" />}
        title={t("installedModels.ollama.noProviders")}
        description={t("installedModels.ollama.pickProviderHint")}
      />
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-6">
      <Toolbar
        leading={
          <SearchInput
            value={query}
            onChange={setQuery}
            placeholder={t("installedModels.ollama.searchPlaceholder")}
          />
        }
        extra={
          hideProviderSelector ? (
            <button
              onClick={onPickProvider}
              className="group flex h-9 min-w-0 max-w-full items-center gap-2 rounded-lg border border-fg/10 bg-fg/3 px-2.5 text-left transition hover:border-fg/20 hover:bg-fg/5"
              aria-haspopup="menu"
            >
              <img src={OllamaIcon} alt="Ollama" className="h-4 w-4 shrink-0" />
              <span className="min-w-0 truncate text-[12px] font-medium text-fg">
                {selectedProvider?.label ?? "—"}
              </span>
              <ChevronDown
                size={12}
                className="shrink-0 text-fg/35 transition group-hover:text-fg/60"
              />
            </button>
          ) : undefined
        }
        meta={[
          { value: String(totalCount), unit: t("installedModels.units.models") },
          { value: formatBytes(totalSize), tone: "accent" },
        ]}
        onRefresh={onRefresh}
        refreshing={refreshing}
        refreshLabel={t("common.buttons.refresh")}
      />

      {/* Provider selector strip */}
      {!hideProviderSelector && (
        <button
          onClick={onPickProvider}
          className="group flex items-center gap-2.5 rounded-lg border border-fg/8 bg-fg/[0.02] px-3 py-2 text-left transition hover:border-fg/20 hover:bg-fg/[0.05]"
        >
          <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md bg-emerald-500/15">
            <img src={OllamaIcon} alt="Ollama" className="h-4 w-4" />
          </span>
          <div className="min-w-0 flex-1">
            <div className="truncate text-[12px] font-medium text-fg">
              {selectedProvider?.label ?? "—"}
            </div>
            {selectedProvider?.baseUrl && (
              <div className="truncate font-mono text-[10.5px] text-fg/45">
                {selectedProvider.baseUrl}
              </div>
            )}
          </div>
          <ChevronDown
            size={13}
            className="shrink-0 text-fg/30 transition group-hover:text-fg/60"
          />
        </button>
      )}

      {loading ? (
        <SkeletonList />
      ) : error ? (
        <ErrorBox message={error} />
      ) : models.length === 0 ? (
        <EmptyState
          icon={<img src={OllamaIcon} alt="Ollama" className="h-5 w-5 opacity-50" />}
          title={t("installedModels.ollama.empty.title")}
          description={t("installedModels.ollama.empty.description")}
        />
      ) : (
        <div className="flex min-h-0 flex-1 flex-col">
          <div className="hidden grid-cols-[minmax(0,1.6fr)_92px_80px_90px_92px_104px_120px] items-center gap-3 border-b border-fg/10 px-2 py-2 text-[10px] font-semibold uppercase tracking-[0.12em] text-fg/35 lg:grid">
            <div className="pl-0.5">{t("common.labels.name")}</div>
            <div>{t("installedModels.ollama.columns.family")}</div>
            <div>{t("installedModels.ollama.columns.params")}</div>
            <div>{t("installedModels.ollama.columns.quant")}</div>
            <div>{t("installedModels.ollama.columns.size")}</div>
            <div>{t("installedModels.ollama.columns.modified")}</div>
            <div className="pr-0.5 text-right">{t("installedModels.ollama.columns.action")}</div>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto pb-10">
            {models.map((model) => (
              <div
                key={model.name}
                className="group border-b border-fg/[0.05] px-2 py-2.5 transition hover:bg-fg/[0.02]"
              >
                <div className="grid gap-3 lg:grid-cols-[minmax(0,1.6fr)_92px_80px_90px_92px_104px_120px] lg:items-center">
                  <div className="min-w-0">
                    <div className="flex items-start justify-between gap-2">
                      <div className="min-w-0 flex-1">
                        <div className="truncate text-[13px] font-medium text-fg">{model.name}</div>
                        {model.digest && (
                          <div className="mt-0.5 truncate font-mono text-[10.5px] text-fg/35">
                            {model.digest.slice(0, 12)}
                          </div>
                        )}
                      </div>
                      <div className="shrink-0 lg:hidden">
                        <RowAction
                          icon={<Trash2 size={12} />}
                          label={t("common.buttons.delete")}
                          onClick={() => void onDelete(model)}
                          disabled={deletingName === model.name}
                          destructive
                        />
                      </div>
                    </div>
                    <div className="mt-2 flex flex-wrap items-center gap-1.5 lg:hidden">
                      {model.family && <Pill>{model.family.toUpperCase()}</Pill>}
                      {model.parameter_size && <Pill>{model.parameter_size}</Pill>}
                      {model.quantization_level && <Pill>{model.quantization_level}</Pill>}
                      <Pill>{formatBytes(model.size ?? 0)}</Pill>
                      <Pill>{relativeTime(model.modified_at)}</Pill>
                    </div>
                  </div>
                  <div className="hidden text-[12.5px] text-fg/70 lg:block">
                    {model.family?.toUpperCase() ?? "—"}
                  </div>
                  <div className="hidden text-[12.5px] text-fg/70 lg:block">
                    {model.parameter_size ?? "—"}
                  </div>
                  <div className="hidden lg:block">
                    {model.quantization_level ? (
                      <span className="rounded bg-fg/6 px-1.5 py-0.5 font-mono text-[10px] text-fg/70">
                        {model.quantization_level}
                      </span>
                    ) : (
                      <span className="text-[12.5px] text-fg/40">—</span>
                    )}
                  </div>
                  <div className="hidden text-[12.5px] font-medium tabular-nums text-fg lg:block">
                    {model.size ? formatBytes(model.size) : "—"}
                  </div>
                  <div className="hidden text-[11.5px] text-fg/55 lg:block">
                    {relativeTime(model.modified_at)}
                  </div>
                  <div className="hidden items-center justify-end gap-1 lg:flex">
                    <RowAction
                      icon={<Trash2 size={12} />}
                      label={t("common.buttons.delete")}
                      onClick={() => void onDelete(model)}
                      disabled={deletingName === model.name}
                      destructive
                    />
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// =============================================================================
// Shared atoms
// =============================================================================

function Toolbar({
  leading,
  extra,
  meta,
  onRefresh,
  refreshing,
  refreshLabel,
}: {
  leading: React.ReactNode;
  extra?: React.ReactNode;
  meta: Array<{ value: string; unit?: string; tone?: "accent" }>;
  onRefresh: () => void;
  refreshing: boolean;
  refreshLabel: string;
}) {
  return (
    <div className="flex flex-col gap-2 sm:flex-row sm:flex-wrap sm:items-center sm:gap-3">
      <div className="min-w-0 sm:flex-1">{leading}</div>
      <div className="flex min-w-0 items-center justify-between gap-3 sm:justify-start">
        {extra && <div className="min-w-0 flex-1 sm:flex-none">{extra}</div>}
        <div className="flex shrink-0 items-center gap-1 text-[12px] text-fg/55">
          {meta.map((m, idx) => (
            <span key={idx} className="flex items-center gap-1">
              <span
                className={cn(
                  "tabular-nums",
                  m.tone === "accent" ? "font-medium text-accent" : "text-fg/75",
                )}
              >
                {m.value}
              </span>
              {m.unit && <span className="text-fg/40">{m.unit}</span>}
              {idx < meta.length - 1 && <span className="px-0.5 text-fg/25">·</span>}
            </span>
          ))}
        </div>
        <button
          onClick={onRefresh}
          className="inline-flex shrink-0 items-center gap-1.5 rounded-md px-2 py-1.5 text-[12px] font-medium text-fg/65 transition hover:bg-fg/6 hover:text-fg"
        >
          <RefreshCw size={12} className={refreshing ? "animate-spin" : ""} />
          {refreshLabel}
        </button>
      </div>
    </div>
  );
}

function PathHint({
  icon,
  path,
  onCopy,
}: {
  icon: React.ReactNode;
  path: string;
  onCopy: () => void;
}) {
  if (!path) return null;
  return (
    <button
      onClick={onCopy}
      className="group flex items-center gap-2 text-left text-fg/45 transition hover:text-fg/75"
    >
      <span className="shrink-0">{icon}</span>
      <span className="truncate font-mono text-[11px]">{path}</span>
      <Copy size={11} className="ml-1 shrink-0 text-fg/25 transition group-hover:text-fg/55" />
    </button>
  );
}

function RowAction({
  icon,
  label,
  onClick,
  disabled,
  destructive,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  disabled?: boolean;
  destructive?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={label}
      aria-label={label}
      className={cn(
        "flex h-7 w-7 items-center justify-center rounded-md transition",
        destructive
          ? disabled
            ? "cursor-not-allowed text-rose-200/30"
            : "text-fg/40 hover:bg-rose-500/10 hover:text-rose-300"
          : disabled
            ? "cursor-not-allowed text-fg/20"
            : "text-fg/40 hover:bg-fg/8 hover:text-fg/85",
      )}
    >
      {icon}
    </button>
  );
}

function Pill({ children }: { children: React.ReactNode }) {
  return (
    <span className="rounded-md bg-fg/6 px-2 py-0.5 text-[10.5px] text-fg/65">{children}</span>
  );
}

function TypeBadge({ isMmproj, imageRole }: { isMmproj: boolean; imageRole?: string | null }) {
  return (
    <span
      className={cn(
        "inline-flex items-center whitespace-nowrap rounded-md px-2 py-0.5 text-[10.5px] font-semibold uppercase tracking-wider",
        imageRole
          ? "bg-violet-400/12 text-violet-300 ring-1 ring-inset ring-violet-400/20"
          : isMmproj
            ? "bg-blue-400/12 text-blue-300 ring-1 ring-inset ring-blue-400/20"
            : "bg-emerald-400/12 text-emerald-300 ring-1 ring-inset ring-emerald-400/20",
      )}
    >
      {imageRole
        ? (IMAGE_ROLE_LABELS[imageRole] ?? imageRole.toUpperCase())
        : isMmproj
          ? "MMPROJ"
          : "LLM"}
    </span>
  );
}

function SearchInput({
  value,
  onChange,
  placeholder,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder: string;
}) {
  return (
    <div className="relative">
      <Search
        size={14}
        className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-fg/40"
      />
      <input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="h-9 w-full rounded-lg border border-fg/10 bg-fg/3 pl-9 pr-3 text-[13px] text-fg outline-none transition placeholder:text-fg/35 focus:border-accent/40 focus:bg-fg/5 focus:ring-1 focus:ring-accent/20"
      />
    </div>
  );
}

function SkeletonList() {
  const { t } = useI18n();
  return (
    <div className="space-y-2">
      {Array.from({ length: 4 }).map((_, i) => (
        <div
          key={i}
          className="flex items-center gap-3 rounded-xl border border-fg/8 bg-fg/[0.025] px-4 py-3.5"
        >
          <div className="h-9 w-9 shrink-0 animate-pulse rounded-lg bg-fg/8" />
          <div className="flex-1 space-y-1.5">
            <div className="h-3 w-1/2 animate-pulse rounded bg-fg/8" />
            <div className="h-2.5 w-1/3 animate-pulse rounded bg-fg/6" />
          </div>
          <div className="hidden h-3 w-16 animate-pulse rounded bg-fg/6 lg:block" />
        </div>
      ))}
      <div className="flex items-center justify-center gap-2 py-2 text-[12px] text-fg/45">
        <Loader size={12} className="animate-spin" />
        {t("common.labels.loading")}
      </div>
    </div>
  );
}

function ErrorBox({ message }: { message: string }) {
  return (
    <div className="flex items-start gap-3 rounded-xl border border-rose-500/20 bg-rose-500/5 px-4 py-3 text-[13px] text-rose-200">
      <AlertCircle size={15} className="mt-0.5 shrink-0" />
      <span>{message}</span>
    </div>
  );
}

function EmptyState({
  icon,
  title,
  description,
}: {
  icon: React.ReactNode;
  title: string;
  description: string;
}) {
  return (
    <div className="rounded-xl border border-dashed border-fg/10 bg-fg/[0.02] px-4 py-12 text-center">
      <div className="mx-auto mb-3 flex h-11 w-11 items-center justify-center rounded-lg border border-fg/10 bg-fg/[0.04]">
        {icon}
      </div>
      <div className="text-[13px] font-medium text-fg">{title}</div>
      <p className="mx-auto mt-1 max-w-md text-[12.5px] text-fg/55">{description}</p>
    </div>
  );
}
