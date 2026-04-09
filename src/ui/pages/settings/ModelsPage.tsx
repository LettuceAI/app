import { useMemo, useState, useEffect, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { AnimatePresence, motion } from "framer-motion";
import {
  Check,
  ChevronRight,
  EthernetPort,
  Edit3,
  Trash2,
  Star,
  StarOff,
  Download,
  AlertTriangle,
} from "lucide-react";
import { BottomMenu, MenuButton } from "../../components/BottomMenu";
import { ModelExportMenu } from "../../components";
import { confirmBottomMenu } from "../../components/ConfirmBottomMenu";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import { useModelsController } from "./hooks/useModelsController";
import { useNavigationManager, Routes } from "../../navigation";
import { cn } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import { ModelsDownloadIndicator } from "./components/ModelsDownloadIndicator";
import { toast } from "../../components/toast";
import { downloadJson, readFileAsText } from "../../../core/storage/lorebookTransfer";
import {
  exportModelAsUsc,
  generateModelExportFilename,
  importModel,
  serializeModelExport,
} from "../../../core/storage/modelTransfer";
import { addOrUpdateModel } from "../../../core/storage/repo";
import type { Model } from "../../../core/storage/schemas";
import type { ModelExportFormat } from "../../components/ModelExportMenu";

type SortMode = "alphabetical" | "provider";
type ModelsViewMode = "list" | "grid";
const SORT_STORAGE_KEY = "lettuce.models.sortMode";
const VIEW_STORAGE_KEY = "lettuce.models.viewMode";

type ModelGroup = {
  label: string;
  key: string;
  models: Model[];
};

const viewModeTransition = {
  initial: { opacity: 0 },
  animate: { opacity: 1 },
  exit: { opacity: 0 },
  transition: { duration: 0.2, ease: "easeInOut" as const },
};

function buildBalancedColumns(groups: ModelGroup[], columnCount: number): ModelGroup[][] {
  const columns = Array.from({ length: columnCount }, () => [] as ModelGroup[]);
  const columnWeights = Array.from({ length: columnCount }, () => 0);

  for (const group of groups) {
    let targetColumn = 0;
    for (let index = 1; index < columnWeights.length; index += 1) {
      if (columnWeights[index] < columnWeights[targetColumn]) {
        targetColumn = index;
      }
    }

    columns[targetColumn].push(group);
    columnWeights[targetColumn] += group.models.length + 1;
  }

  return columns;
}

function getLlamaRuntimeAlert(model: Model): { tone: string; title: string } | null {
  if (model.providerId !== "llamacpp") {
    return null;
  }
  const status = model.advancedModelSettings?.llamaLastRuntimeReport?.status;
  if (status === "cpuFallbackSucceeded") {
    return {
      tone: "text-warning/75",
      title: "This model fell back to CPU on its last run.",
    };
  }
  if (status === "cpuFallbackFailed" || status === "failed") {
    return {
      tone: "text-danger/75",
      title: "This model failed on its last run.",
    };
  }
  return null;
}

export function ModelsPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const importInputRef = useRef<HTMLInputElement | null>(null);
  const [selectedModel, setSelectedModel] = useState<Model | null>(null);
  const [exportTarget, setExportTarget] = useState<Model | null>(null);
  const [showExportMenu, setShowExportMenu] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [sortMode, setSortMode] = useState<SortMode>(() => {
    if (typeof window === "undefined") return "alphabetical";
    const stored = window.localStorage.getItem(SORT_STORAGE_KEY);
    return stored === "provider" ? "provider" : "alphabetical";
  });
  const [viewMode, setViewMode] = useState<ModelsViewMode>(() => {
    if (typeof window === "undefined") return "list";
    return window.localStorage.getItem(VIEW_STORAGE_KEY) === "grid" ? "grid" : "list";
  });
  const [showSortMenu, setShowSortMenu] = useState(false);
  const { toNewModel, toEditModel } = useNavigationManager();
  const {
    state: { providers, models, defaultModelId, loading },
    handleSetDefault,
    handleDelete,
  } = useModelsController();

  const EmptyState = ({ onCreate }: { onCreate: () => void }) => (
    <div className="flex h-64 flex-col items-center justify-center">
      <EthernetPort className="mb-3 h-12 w-12 text-fg/20" />
      <h3 className="mb-1 text-lg font-medium text-fg">{t("models.empty.title")}</h3>
      <p className="mb-4 text-center text-sm text-fg/50">{t("models.empty.description")}</p>
      <div className="flex flex-col gap-2">
        <button
          onClick={onCreate}
          className="rounded-full border border-accent/40 bg-accent/20 px-6 py-2 text-sm font-medium text-accent/90 transition hover:bg-accent/30 active:scale-[0.99]"
        >
          {t("models.empty.addButton")}
        </button>
        <button
          onClick={() => navigate(Routes.settingsModelsBrowse)}
          className="hidden items-center justify-center gap-2 rounded-full border border-fg/15 bg-fg/5 px-6 py-2 text-sm font-medium text-fg/70 transition hover:bg-fg/10 active:scale-[0.99] md:flex"
        >
          <Download size={14} />
          {t("hfBrowser.title")}
        </button>
      </div>
    </div>
  );

  useEffect(() => {
    (window as any).__openAddModel = () => toNewModel();
    const listener = () => toNewModel();
    window.addEventListener("models:add", listener);
    return () => {
      if ((window as any).__openAddModel) {
        delete (window as any).__openAddModel;
      }
      window.removeEventListener("models:add", listener);
    };
  }, [toNewModel]);

  useEffect(() => {
    const listener = () => importInputRef.current?.click();
    window.addEventListener("models:import", listener);
    return () => window.removeEventListener("models:import", listener);
  }, []);

  useEffect(() => {
    const globalWindow = window as any;
    globalWindow.__openModelsSort = () => setShowSortMenu(true);
    const listener = () => setShowSortMenu(true);
    window.addEventListener("models:sort", listener);
    return () => {
      if (globalWindow.__openModelsSort) {
        delete globalWindow.__openModelsSort;
      }
      window.removeEventListener("models:sort", listener);
    };
  }, []);

  useEffect(() => {
    if (typeof window === "undefined") return;
    window.localStorage.setItem(SORT_STORAGE_KEY, sortMode);
  }, [sortMode]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    window.localStorage.setItem(VIEW_STORAGE_KEY, viewMode);
    (window as any).__modelsViewMode = viewMode;
    window.dispatchEvent(new CustomEvent("models:viewModeChanged"));
  }, [viewMode]);

  useEffect(() => {
    const handler = () => {
      setViewMode((prev) => (prev === "list" ? "grid" : "list"));
    };
    window.addEventListener("models:cycleViewMode", handler);
    return () => window.removeEventListener("models:cycleViewMode", handler);
  }, []);

  const getProviderLabel = useMemo(
    () => (model: Model) => {
      const providerInfo = providers.find((p) => p.providerId === model.providerId);
      return model.providerLabel || providerInfo?.label || model.providerId;
    },
    [providers],
  );

  const handleExportModel = async (format: ModelExportFormat) => {
    if (!exportTarget || exporting) return;
    try {
      setExporting(true);
      const exportJson =
        format === "usc"
          ? await exportModelAsUsc(exportTarget)
          : serializeModelExport(exportTarget);
      await downloadJson(
        exportJson,
        generateModelExportFilename(exportTarget.displayName || exportTarget.name, format),
      );
      setShowExportMenu(false);
      setExportTarget(null);
    } catch (error) {
      console.error("Failed to export model:", error);
      toast.error("Export failed", String(error));
    } finally {
      setExporting(false);
    }
  };

  const handleImportModel = async (file: File) => {
    try {
      const raw = await readFileAsText(file);
      const importedModel = importModel(raw);
      await addOrUpdateModel(importedModel);
      toast.success("Imported successfully", `Model "${importedModel.displayName}" was imported.`);
    } catch (error) {
      console.error("Failed to import model:", error);
      toast.error("Import failed", String(error));
    } finally {
      if (importInputRef.current) {
        importInputRef.current.value = "";
      }
    }
  };

  const sortedModels = useMemo(() => {
    const list = [...models];
    if (sortMode === "alphabetical") {
      return list.sort((a, b) => {
        const aName = (a.displayName || a.name).toLowerCase();
        const bName = (b.displayName || b.name).toLowerCase();
        if (aName !== bName) return aName.localeCompare(bName);
        return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
      });
    }

    return list.sort((a, b) => {
      const aProvider = getProviderLabel(a).toLowerCase();
      const bProvider = getProviderLabel(b).toLowerCase();
      if (aProvider !== bProvider) return aProvider.localeCompare(bProvider);
      const aName = (a.displayName || a.name).toLowerCase();
      const bName = (b.displayName || b.name).toLowerCase();
      if (aName !== bName) return aName.localeCompare(bName);
      return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
    });
  }, [models, sortMode, getProviderLabel]);

  const providerGroups = useMemo<ModelGroup[]>(() => {
    if (sortMode !== "provider") return [];
    const groups = new Map<string, ModelGroup>();
    for (const model of sortedModels) {
      const label = getProviderLabel(model);
      const existing = groups.get(label);
      if (existing) {
        existing.models.push(model);
        continue;
      }
      groups.set(label, {
        label,
        key: `provider-${label}`,
        models: [model],
      });
    }
    return Array.from(groups.values());
  }, [sortedModels, sortMode, getProviderLabel]);

  const providerGroupColumns = useMemo(
    () => buildBalancedColumns(providerGroups, 2),
    [providerGroups],
  );

  const renderModelCard = (model: Model, compact = false) => {
    const isDefault = model.id === defaultModelId;
    const providerLabel = getProviderLabel(model);
    const runtimeAlert = getLlamaRuntimeAlert(model);
    return (
      <button
        key={model.id}
        onClick={() => setSelectedModel(model)}
        className={cn(
          "group w-full rounded-xl border border-fg/10 bg-fg/5 text-left transition hover:border-fg/20 hover:bg-fg/10 focus:outline-none focus:ring-2 focus:ring-fg/20 active:scale-[0.99]",
          compact ? "px-3 py-2.5" : "px-4 py-3",
        )}
      >
        <div className="flex items-center gap-3">
          {getProviderIcon(model.providerId)}
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <span
                className={cn("truncate font-medium text-fg", compact ? "text-[13px]" : "text-sm")}
              >
                {model.displayName || model.name}
              </span>
              {runtimeAlert && (
                <span
                  title={runtimeAlert.title}
                  className={cn("inline-flex items-center", runtimeAlert.tone)}
                >
                  <AlertTriangle className="h-3.5 w-3.5" />
                </span>
              )}
              {isDefault && (
                <span className="inline-flex items-center gap-1 rounded-md bg-accent/20 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-accent/80">
                  <Check className="h-2.5 w-2.5" />
                  Default
                </span>
              )}
            </div>
            <div className="mt-0.5 flex flex-wrap items-center gap-1 text-[11px] text-fg/50">
              <span className="truncate">{providerLabel}</span>
              <span className="opacity-40">•</span>
              <span className="truncate max-w-37.5 font-mono text-[10px]">{model.name}</span>

              {(model.inputScopes?.includes("image") || model.outputScopes?.includes("image")) && (
                <>
                  <span className="opacity-40">•</span>
                  <span className="text-info/80">Vision</span>
                </>
              )}

              {(model.inputScopes?.includes("audio") || model.outputScopes?.includes("audio")) && (
                <>
                  <span className="opacity-40">•</span>
                  <span className="text-secondary/80">Audio</span>
                </>
              )}
            </div>
          </div>
          <ChevronRight className="h-4 w-4 text-fg/30 group-hover:text-fg/60 transition" />
        </div>
      </button>
    );
  };

  return (
    <div className="flex h-full flex-col">
      <input
        ref={importInputRef}
        type="file"
        className="hidden"
        onChange={(event) => {
          const file = event.target.files?.[0];
          if (file) {
            void handleImportModel(file);
          }
          event.currentTarget.value = "";
        }}
      />
      {/* List (TopNav handles title/back) */}
      <div className="flex-1 overflow-y-auto mx-3 py-3 space-y-3">
        {!loading && models.length === 0 && <EmptyState onCreate={() => toNewModel()} />}

        {/* Active/completed downloads from HuggingFace browser */}
        <ModelsDownloadIndicator />

        {/* Browse GGUF Models button */}
        {models.length > 0 && (
          <button
            onClick={() => navigate(Routes.settingsModelsBrowse)}
            className={cn(
              "group hidden w-full rounded-xl border border-dashed border-fg/15 bg-fg/2 px-4 py-3 text-left transition md:block",
              "hover:border-fg/25 hover:bg-fg/5 active:scale-[0.995]",
            )}
          >
            <div className="flex items-center gap-3">
              <div className="flex h-8 w-8 items-center justify-center rounded-lg border border-fg/10 bg-fg/5">
                <Download size={14} className="text-fg/50" />
              </div>
              <div className="min-w-0 flex-1">
                <span className="text-sm font-medium text-fg/70">{t("hfBrowser.title")}</span>
                <p className="text-[11px] text-fg/40">{t("hfBrowser.browseOnHuggingFace")}</p>
              </div>
              <ChevronRight size={14} className="text-fg/25 group-hover:text-fg/50 transition" />
            </div>
          </button>
        )}

        {/* Model Cards */}
        <AnimatePresence mode="wait" initial={false}>
          {viewMode === "grid" && sortMode !== "provider" && (
            <motion.div
              key="grid-models"
              {...viewModeTransition}
              className="grid grid-cols-1 gap-3 lg:grid-cols-2"
            >
              {sortedModels.map((model) => renderModelCard(model))}
            </motion.div>
          )}

          {viewMode === "grid" && sortMode === "provider" && (
            <motion.div key="grid-provider" {...viewModeTransition} className="space-y-3">
              <div className="space-y-3 lg:hidden">
                {providerGroups.map((group) => (
                  <section key={group.key} className="min-w-0">
                    <div className="mb-3 flex items-center gap-3 px-1">
                      <span className="text-[10px] font-semibold uppercase tracking-wider text-fg/40">
                        {group.label}
                      </span>
                      <div className="h-px flex-1 bg-fg/5" />
                    </div>
                    <div className="space-y-2">
                      {group.models.map((model) => renderModelCard(model, true))}
                    </div>
                  </section>
                ))}
              </div>

              <div className="hidden lg:grid lg:grid-cols-2 lg:items-start lg:gap-3">
                {providerGroupColumns.map((column, columnIndex) => (
                  <div key={`provider-column-${columnIndex}`} className="space-y-3">
                    {column.map((group) => (
                      <section key={group.key} className="min-w-0">
                        <div className="mb-3 flex items-center gap-3 px-1">
                          <span className="text-[10px] font-semibold uppercase tracking-wider text-fg/40">
                            {group.label}
                          </span>
                          <div className="h-px flex-1 bg-fg/5" />
                        </div>
                        <div className="space-y-2">
                          {group.models.map((model) => renderModelCard(model, true))}
                        </div>
                      </section>
                    ))}
                  </div>
                ))}
              </div>
            </motion.div>
          )}

          {viewMode === "list" && (
            <motion.div key={`list-${sortMode}`} {...viewModeTransition} className="space-y-3">
              {sortMode === "provider"
                ? providerGroups.map((group, idx) => (
                    <div key={group.key} className="space-y-3">
                      <div className={cn("flex items-center gap-3 px-1", idx > 0 && "pt-2")}>
                        <span className="text-[10px] font-semibold uppercase tracking-wider text-fg/40">
                          {group.label}
                        </span>
                        <div className="h-px flex-1 bg-fg/5" />
                      </div>
                      <div className="space-y-3">
                        {group.models.map((model) => renderModelCard(model))}
                      </div>
                    </div>
                  ))
                : sortedModels.map((model) => <div key={model.id}>{renderModelCard(model)}</div>)}
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      <BottomMenu
        isOpen={!!selectedModel}
        onClose={() => setSelectedModel(null)}
        title={selectedModel?.displayName || selectedModel?.name || "Model"}
      >
        {selectedModel && (
          <div className="space-y-4">
            <div className="rounded-lg border border-fg/10 bg-fg/5 px-3 py-2">
              <div className="flex items-center gap-2">
                <span className="truncate text-sm font-medium text-fg">
                  {selectedModel.displayName || selectedModel.name}
                </span>
                {selectedModel.id === defaultModelId && (
                  <span className="inline-flex items-center gap-1 rounded-md bg-accent/20 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-accent/80">
                    Default
                  </span>
                )}
              </div>
              <p className="mt-0.5 truncate text-[11px] text-fg/50">{selectedModel.name}</p>
            </div>

            <MenuButton
              icon={Edit3}
              title="Edit"
              description="Configure model parameters"
              onClick={() => {
                toEditModel(selectedModel.id);
                setSelectedModel(null);
              }}
              color="from-info to-info/80"
            />

            <MenuButton
              icon={selectedModel.id === defaultModelId ? StarOff : Star}
              title={selectedModel.id === defaultModelId ? "Already Default" : "Set as Default"}
              description="Make this your primary model"
              disabled={selectedModel.id === defaultModelId}
              onClick={() => {
                void handleSetDefault(selectedModel.id);
                setSelectedModel(null);
              }}
              color="from-accent to-accent/80"
            />

            <MenuButton
              icon={Download}
              title="Export"
              description="Save this model profile"
              onClick={() => {
                setExportTarget(selectedModel);
                setSelectedModel(null);
                setShowExportMenu(true);
              }}
              color="from-emerald-500 to-emerald-600"
            />

            <MenuButton
              icon={Trash2}
              title="Delete"
              description="Remove this model permanently"
              onClick={async () => {
                const confirmed = await confirmBottomMenu({
                  title: "Delete model?",
                  message: `Are you sure you want to delete ${selectedModel.displayName || selectedModel.name}?`,
                  confirmLabel: "Delete",
                  destructive: true,
                });
                if (!confirmed) return;
                void handleDelete(selectedModel.id);
                setSelectedModel(null);
              }}
              color="from-danger to-danger/80"
            />
          </div>
        )}
      </BottomMenu>

      <ModelExportMenu
        isOpen={showExportMenu}
        onClose={() => {
          if (exporting) return;
          setShowExportMenu(false);
          setExportTarget(null);
        }}
        onSelect={(format) => {
          void handleExportModel(format);
        }}
        exporting={exporting}
      />

      <BottomMenu isOpen={showSortMenu} onClose={() => setShowSortMenu(false)} title="Sort Models">
        <div className="space-y-3">
          <MenuButton
            icon={sortMode === "alphabetical" ? Check : StarOff}
            title={t("models.sort.alphabetical")}
            description="Sort by model name"
            onClick={() => {
              setSortMode("alphabetical");
              setShowSortMenu(false);
            }}
            color="from-accent to-accent/80"
          />
          <MenuButton
            icon={sortMode === "provider" ? Check : StarOff}
            title={t("models.sort.byProvider")}
            description="Group models by provider"
            onClick={() => {
              setSortMode("provider");
              setShowSortMenu(false);
            }}
            color="from-info to-info/80"
          />
        </div>
      </BottomMenu>
    </div>
  );
}
