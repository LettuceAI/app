import { useMemo, useState, useEffect } from "react";
import { Check, ChevronRight, EthernetPort, Edit3, Trash2, Star, StarOff } from "lucide-react";
import { BottomMenu, MenuButton } from "../../components/BottomMenu";
import { confirmBottomMenu } from "../../components/ConfirmBottomMenu";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import { useModelsController } from "./hooks/useModelsController";
import { useNavigationManager } from "../../navigation";
import { cn } from "../../design-tokens";

type SortMode = "alphabetical" | "provider";
const SORT_STORAGE_KEY = "lettuce.models.sortMode";

export function ModelsPage() {
  const [selectedModel, setSelectedModel] = useState<any | null>(null);
  const [sortMode, setSortMode] = useState<SortMode>(() => {
    if (typeof window === "undefined") return "alphabetical";
    const stored = window.localStorage.getItem(SORT_STORAGE_KEY);
    return stored === "provider" ? "provider" : "alphabetical";
  });
  const [showSortMenu, setShowSortMenu] = useState(false);
  const { toNewModel, toEditModel } = useNavigationManager();
  const {
    state: { providers, models, defaultModelId },
    handleSetDefault,
    handleDelete,
  } = useModelsController();

  const EmptyState = ({ onCreate }: { onCreate: () => void }) => (
    <div className="flex h-64 flex-col items-center justify-center">
      <EthernetPort className="mb-3 h-12 w-12 text-fg/20" />
      <h3 className="mb-1 text-lg font-medium text-fg">No Models yet</h3>
      <p className="mb-4 text-center text-sm text-fg/50">
        Add and manage AI models from different providers
      </p>
      <button
        onClick={onCreate}
        className="rounded-full border border-accent/40 bg-accent/20 px-6 py-2 text-sm font-medium text-accent/90 transition hover:bg-accent/30 active:scale-[0.99]"
      >
        Add Model
      </button>
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

  const getProviderLabel = useMemo(
    () => (model: any) => {
      const providerInfo = providers.find((p) => p.providerId === model.providerId);
      return model.providerLabel || providerInfo?.label || model.providerId;
    },
    [providers],
  );

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

  const listItems = useMemo(() => {
    if (sortMode !== "provider") {
      return sortedModels.map((model) => ({ type: "model" as const, model }));
    }
    const items: Array<
      { type: "divider"; label: string; key: string } | { type: "model"; model: any }
    > = [];
    let lastProvider = "";
    for (const model of sortedModels) {
      const providerLabel = getProviderLabel(model);
      if (providerLabel !== lastProvider) {
        lastProvider = providerLabel;
        items.push({
          type: "divider",
          label: providerLabel,
          key: `provider-${providerLabel}`,
        });
      }
      items.push({ type: "model", model });
    }
    return items;
  }, [sortedModels, sortMode, getProviderLabel]);

  return (
    <div className="flex h-full flex-col">
      {/* List (TopNav handles title/back) */}
      <div className="flex-1 overflow-y-auto mx-3 py-3 space-y-3">
        {models.length === 0 && <EmptyState onCreate={() => toNewModel()} />}

        {/* Model Cards */}
        {listItems.map((item, idx) => {
          if (item.type === "divider") {
            return (
              <div key={item.key} className={cn("flex items-center gap-3 px-1", idx > 0 && "pt-2")}>
                <span className="text-[10px] font-semibold uppercase tracking-wider text-fg/40">
                  {item.label}
                </span>
                <div className="h-px flex-1 bg-fg/5" />
              </div>
            );
          }
          const model = item.model;
          const isDefault = model.id === defaultModelId;
          const providerLabel = getProviderLabel(model);
          return (
            <button
              key={model.id}
              onClick={() => setSelectedModel(model)}
              className="group w-full rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-left transition hover:border-fg/20 hover:bg-fg/10 focus:outline-none focus:ring-2 focus:ring-fg/20 active:scale-[0.99]"
            >
              <div className="flex items-center gap-3">
                {getProviderIcon(model.providerId)}
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate text-sm font-medium text-fg">
                      {model.displayName || model.name}
                    </span>
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

                    {(model.inputScopes?.includes("image") ||
                      model.outputScopes?.includes("image")) && (
                      <>
                        <span className="opacity-40">•</span>
                        <span className="text-info/80">Vision</span>
                      </>
                    )}

                    {(model.inputScopes?.includes("audio") ||
                      model.outputScopes?.includes("audio")) && (
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
        })}
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

      <BottomMenu isOpen={showSortMenu} onClose={() => setShowSortMenu(false)} title="Sort Models">
        <div className="space-y-3">
          <MenuButton
            icon={sortMode === "alphabetical" ? Check : StarOff}
            title="Alphabetical"
            description="Sort by model name"
            onClick={() => {
              setSortMode("alphabetical");
              setShowSortMenu(false);
            }}
            color="from-accent to-accent/80"
          />
          <MenuButton
            icon={sortMode === "provider" ? Check : StarOff}
            title="By Provider"
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
