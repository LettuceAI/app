import { useCallback, useEffect, useMemo, useReducer, useRef } from "react";
import { useParams, useSearchParams } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import {
  LLAMA_RUNTIME_REPORT_UPDATED_EVENT,
  readSettings,
  addOrUpdateModel,
  removeModel,
  setDefaultModel,
} from "../../../../core/storage/repo";
import type {
  AdvancedModelSettings,
  Model,
  ProviderCredential,
} from "../../../../core/storage/schemas";
import {
  getProviderCapabilities,
  toCamel,
  type ProviderCapabilitiesCamel,
} from "../../../../core/providers/capabilities";
import { createDefaultAdvancedModelSettings } from "../../../../core/storage/schemas";
import { sanitizeAdvancedModelSettings } from "../../../components/AdvancedModelSettingsForm";
import {
  initialModelEditorState,
  modelEditorReducer,
  type ModelEditorState,
} from "./modelEditorReducer";
import { Routes, useNavigationManager } from "../../../navigation";
import { getPlatform } from "../../../../core/utils/platform";
import { useI18n } from "../../../../core/i18n/context";

type ControllerReturn = {
  state: ModelEditorState;
  isNew: boolean;
  canSave: boolean;
  hasUnsavedChanges: boolean;
  providerDisplay: (prov: ProviderCredential) => string;
  updateEditorModel: (patch: Partial<Model>) => void;
  handleDisplayNameChange: (value: string) => void;
  handleModelNameChange: (value: string) => Promise<void>;
  handleProviderSelection: (providerId: string, providerLabel: string) => Promise<void>;
  setModelAdvancedDraft: (settings: AdvancedModelSettings) => void;
  toggleOverride: () => void;
  handleTemperatureChange: (value: number | null) => void;
  handleTopPChange: (value: number | null) => void;
  handleMaxTokensChange: (value: number | null) => void;
  handleContextLengthChange: (value: number | null) => void;
  handleFrequencyPenaltyChange: (value: number | null) => void;
  handlePresencePenaltyChange: (value: number | null) => void;
  handleTopKChange: (value: number | null) => void;
  handleLlamaGpuLayersChange: (value: number | null) => void;
  handleLlamaThreadsChange: (value: number | null) => void;
  handleLlamaThreadsBatchChange: (value: number | null) => void;
  handleLlamaSeedChange: (value: number | null) => void;
  handleLlamaRopeFreqBaseChange: (value: number | null) => void;
  handleLlamaRopeFreqScaleChange: (value: number | null) => void;
  handleLlamaOffloadKqvChange: (value: boolean | null) => void;
  handleLlamaBatchSizeChange: (value: number | null) => void;
  handleLlamaKvTypeChange: (value: AdvancedModelSettings["llamaKvType"]) => void;
  handleLlamaFlashAttentionChange: (value: AdvancedModelSettings["llamaFlashAttention"]) => void;
  handleLlamaSamplerProfileChange: (value: AdvancedModelSettings["llamaSamplerProfile"]) => void;
  handleLlamaSamplerOrderChange: (value: AdvancedModelSettings["llamaSamplerOrder"]) => void;
  handleLlamaMinPChange: (value: number | null) => void;
  handleLlamaTypicalPChange: (value: number | null) => void;
  handleLlamaDryMultiplierChange: (value: number | null) => void;
  handleLlamaDryBaseChange: (value: number | null) => void;
  handleLlamaDryAllowedLengthChange: (value: number | null) => void;
  handleLlamaDryPenaltyLastNChange: (value: number | null) => void;
  handleLlamaDrySequenceBreakersChange: (value: string[] | null) => void;
  handleLlamaChatTemplateOverrideChange: (value: string | null) => void;
  handleLlamaMmprojPathChange: (value: string | null) => void;
  handleLlamaChatTemplatePresetChange: (value: string | null) => void;
  handleLlamaRawCompletionFallbackChange: (value: boolean | null) => void;
  handleLlamaStrictModeChange: (value: boolean | null) => void;
  handleLlamaStreamingEnabledChange: (value: boolean | null) => void;
  handleOllamaNumCtxChange: (value: number | null) => void;
  handleOllamaNumPredictChange: (value: number | null) => void;
  handleOllamaNumKeepChange: (value: number | null) => void;
  handleOllamaNumBatchChange: (value: number | null) => void;
  handleOllamaNumGpuChange: (value: number | null) => void;
  handleOllamaNumThreadChange: (value: number | null) => void;
  handleOllamaTfsZChange: (value: number | null) => void;
  handleOllamaTypicalPChange: (value: number | null) => void;
  handleOllamaMinPChange: (value: number | null) => void;
  handleOllamaMirostatChange: (value: number | null) => void;
  handleOllamaMirostatTauChange: (value: number | null) => void;
  handleOllamaMirostatEtaChange: (value: number | null) => void;
  handleOllamaRepeatPenaltyChange: (value: number | null) => void;
  handleOllamaSeedChange: (value: number | null) => void;
  handleOllamaStopChange: (value: string[] | null) => void;
  handleReasoningEnabledChange: (value: boolean) => void;
  handleReasoningEffortChange: (value: "low" | "medium" | "high" | null) => void;
  handleReasoningBudgetChange: (value: number | null) => void;
  handlePromptCachingEnabledChange: (value: boolean) => void;
  handlePromptCachingTtlChange: (value: string) => void;
  applyLlamaRuntimeSuggestion: () => Promise<boolean>;
  handleSave: () => Promise<void>;
  saveModel: () => Promise<boolean>;
  handleDelete: () => Promise<void>;
  handleSetDefault: () => Promise<void>;
  resetToInitial: () => void;
  clearError: () => void;
  fetchModels: () => Promise<void>;
};

function useModelEditorState() {
  return useReducer(modelEditorReducer, initialModelEditorState);
}

function getHardCappedScopes(
  providerId?: string | null,
): Pick<Model, "inputScopes" | "outputScopes"> | null {
  if (providerId === "automatic1111") {
    return {
      inputScopes: ["text", "image"],
      outputScopes: ["image"],
    };
  }

  return null;
}

function isImageOnlyProvider(providerId?: string | null): boolean {
  return providerId === "automatic1111" || providerId === "stability";
}

function cloneSnapshot<T>(value: T): T {
  return JSON.parse(JSON.stringify(value));
}

export function useModelEditorController(): ControllerReturn {
  const { t } = useI18n();
  const { toModelsList, backOrReplace } = useNavigationManager();
  const { modelId } = useParams<{ modelId: string }>();
  const [searchParams] = useSearchParams();
  const isMobile = getPlatform().type === "mobile";
  const isNew = !modelId || modelId === "new";
  const [state, dispatch] = useModelEditorState();
  const initialStateRef = useRef<{
    editorModel: Model | null;
    modelAdvancedDraft: AdvancedModelSettings;
  } | null>(null);
  const [capabilities, setCapabilities] = useReducer(
    (_: ProviderCapabilitiesCamel[], a: ProviderCapabilitiesCamel[]) => a,
    [],
  );
  const localProvider = useMemo<ProviderCredential>(
    () => ({
      id: crypto.randomUUID(),
      providerId: "llamacpp",
      label: "llama.cpp (Local)",
      apiKey: "",
    }),
    [],
  );

  const visibleCapabilities = useMemo(
    () =>
      isMobile
        ? capabilities.filter((capability) => capability.id !== "llamacpp")
        : capabilities,
    [capabilities, isMobile],
  );

  const ensureLocalProvider = useCallback(
    (providers: ProviderCredential[]) => {
      const capabilityIds = new Set(visibleCapabilities.map((capability) => capability.id));
      const filteredProviders =
        capabilityIds.size > 0
          ? providers.filter(
              (provider) =>
                provider.providerId === localProvider.providerId ||
                capabilityIds.has(provider.providerId),
            )
          : providers;

      if (isMobile) {
        return filteredProviders.filter(
          (provider) => provider.providerId !== localProvider.providerId,
        );
      }
      const hasLocal = filteredProviders.some((p) => p.providerId === localProvider.providerId);
      if (hasLocal) return filteredProviders;
      return filteredProviders.length === 0
        ? [localProvider]
        : [...filteredProviders, localProvider];
    },
    [isMobile, localProvider, visibleCapabilities],
  );

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const caps = (await getProviderCapabilities()).map(toCamel);
        if (!cancelled) setCapabilities(caps);
      } catch (e) {
        console.warn("[ModelEditor] Failed to load provider capabilities", e);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      dispatch({ type: "set_loading", payload: true });
      dispatch({ type: "set_error", payload: null });
      try {
        const settings = await readSettings();
        const providers = ensureLocalProvider(settings.providerCredentials);
        const defaultModelId = settings.defaultModelId ?? null;

        const defaultAdvanced = createDefaultAdvancedModelSettings();

        let nextEditorModel: Model | null = null;
        let nextDraft = sanitizeAdvancedModelSettings(defaultAdvanced);

        if (isNew) {
          const hfModelPath = searchParams.get("hfModelPath");
          const hfModelName = searchParams.get("hfModelName");
          const hfDisplayName = searchParams.get("hfDisplayName");
          const hfMmprojPath = searchParams.get("hfMmprojPath");

          const isFromHfBrowser = !!hfModelPath;

          const providerParam = searchParams.get("provider");
          let selectedProvider: ProviderCredential | undefined;
          if (providerParam) {
            selectedProvider = providers.find((p) => p.providerId === providerParam);
          } else if (isFromHfBrowser) {
            selectedProvider = providers.find((p) => p.providerId === "llamacpp");
          }
          if (!selectedProvider) {
            selectedProvider = providers[0];
          }

          const firstCap = visibleCapabilities[0];
          nextEditorModel = {
            id: crypto.randomUUID(),
            name: isFromHfBrowser ? hfModelPath! : "",
            displayName: isFromHfBrowser ? hfDisplayName || hfModelName || "" : "",
            providerId: selectedProvider?.providerId || firstCap?.id || "",
            providerLabel: selectedProvider?.label || firstCap?.name || "",
            createdAt: Date.now(),
            inputScopes:
              hfMmprojPath || isImageOnlyProvider(selectedProvider?.providerId)
                ? ["text", "image"]
                : ["text"],
            outputScopes: isImageOnlyProvider(selectedProvider?.providerId) ? ["image"] : ["text"],
          } as Model;
          const hardCappedScopes = getHardCappedScopes(selectedProvider?.providerId);
          if (hardCappedScopes) {
            nextEditorModel = {
              ...nextEditorModel,
              ...hardCappedScopes,
            };
          }
          if (hfMmprojPath) {
            nextDraft = sanitizeAdvancedModelSettings({
              ...defaultAdvanced,
              llamaMmprojPath: hfMmprojPath,
            });
          }
        } else {
          const existing = settings.models.find((m) => m.id === modelId) || null;
          if (!existing) {
            toModelsList({ replace: true });
            return;
          }
          nextEditorModel = existing;
          if (existing.advancedModelSettings) {
            nextDraft = sanitizeAdvancedModelSettings(existing.advancedModelSettings);
          }
        }

        if (nextEditorModel && providers.length > 0) {
          const hasMatch = providers.some(
            (p) =>
              p.providerId === nextEditorModel!.providerId &&
              p.label === nextEditorModel!.providerLabel,
          );
          if (!hasMatch) {
            const fallback = providers[0];
            nextEditorModel = {
              ...nextEditorModel,
              providerId: fallback.providerId,
              providerLabel: fallback.label,
            };
          }
        }

        if (nextEditorModel) {
          const hardCappedScopes = getHardCappedScopes(nextEditorModel.providerId);
          if (hardCappedScopes) {
            nextEditorModel = {
              ...nextEditorModel,
              ...hardCappedScopes,
            };
          }
        }

        if (!cancelled) {
          dispatch({
            type: "load_success",
            payload: {
              providers,
              defaultModelId,
              editorModel: nextEditorModel,
              modelAdvancedDraft: nextDraft,
            },
          });
          const isFromHfBrowser = isNew && !!searchParams.get("hfModelPath");
          if (isFromHfBrowser && nextEditorModel) {
            initialStateRef.current = {
              editorModel: {
                ...cloneSnapshot(nextEditorModel),
                name: "",
                displayName: "",
              },
              modelAdvancedDraft: cloneSnapshot(nextDraft),
            };
          } else {
            initialStateRef.current = {
              editorModel: nextEditorModel ? cloneSnapshot(nextEditorModel) : null,
              modelAdvancedDraft: cloneSnapshot(nextDraft),
            };
          }
        }
      } catch (error) {
        console.error("Failed to load model settings", error);
        if (!cancelled) {
          dispatch({
            type: "set_error",
            payload: t("editModel.errors.loadFailed"),
          });
          dispatch({ type: "set_loading", payload: false });
        }
      }
    };

    load();
    return () => {
      cancelled = true;
    };
  }, [ensureLocalProvider, isNew, modelId, searchParams, toModelsList, visibleCapabilities]);

  const syncRuntimeReportFromStore = useCallback(async () => {
    const currentModelId = state.editorModel?.id;
    if (!currentModelId) {
      return;
    }
    try {
      const settings = await readSettings();
      const nextModel = settings.models.find((model) => model.id === currentModelId);
      if (!nextModel) {
        return;
      }
      const nextReport = nextModel.advancedModelSettings?.llamaLastRuntimeReport ?? null;
      dispatch({
        type: "update_editor_model",
        payload: {
          advancedModelSettings: nextModel.advancedModelSettings,
        },
      });
      dispatch({
        type: "set_model_advanced_draft",
        payload: sanitizeAdvancedModelSettings({
          ...state.modelAdvancedDraft,
          llamaLastRuntimeReport: nextReport,
        }),
      });

      if (initialStateRef.current) {
        initialStateRef.current = {
          ...initialStateRef.current,
          editorModel: initialStateRef.current.editorModel
            ? {
                ...initialStateRef.current.editorModel,
                advancedModelSettings: nextModel.advancedModelSettings,
              }
            : null,
          modelAdvancedDraft: sanitizeAdvancedModelSettings({
            ...initialStateRef.current.modelAdvancedDraft,
            llamaLastRuntimeReport: nextReport,
          }),
        };
      }
    } catch (error) {
      console.error("Failed to sync llama runtime report", error);
    }
  }, [state.editorModel?.id, state.modelAdvancedDraft]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    void listen(LLAMA_RUNTIME_REPORT_UPDATED_EVENT, () => {
      void syncRuntimeReportFromStore();
    }).then((dispose) => {
      unlisten = dispose;
    });
    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [syncRuntimeReportFromStore]);

  const providerDisplay = useMemo(() => {
    return (prov: ProviderCredential) => {
      if (prov.providerId === "llamacpp") {
        return prov.label;
      }
      const cap = capabilities.find((p) => p.id === prov.providerId);
      return `${prov.label} (${cap?.name || prov.providerId})`;
    };
  }, [capabilities]);

  const updateEditorModel = useCallback(
    (patch: Partial<Model>) => {
      dispatch({ type: "update_editor_model", payload: patch });
    },
    [dispatch],
  );

  const canSave = useMemo(() => {
    const { editorModel, providers, saving, verifying } = state;
    if (!editorModel) return false;
    const hasProvider =
      providers.find(
        (p) => p.providerId === editorModel.providerId && p.label === editorModel.providerLabel,
      ) || providers.find((p) => p.providerId === editorModel.providerId);
    const valid =
      !!editorModel.displayName?.trim() &&
      !!editorModel.name?.trim() &&
      !!hasProvider &&
      !saving &&
      !verifying;
    const initial = initialStateRef.current;
    if (!initial) return false;
    const editorChanged =
      JSON.stringify(editorModel) !== JSON.stringify(initial.editorModel ?? null);
    const draftChanged =
      JSON.stringify(state.modelAdvancedDraft) !==
      JSON.stringify(initial.modelAdvancedDraft ?? null);
    return valid && (editorChanged || draftChanged);
  }, [state]);

  const hasUnsavedChanges = useMemo(() => {
    const { editorModel } = state;
    const initial = initialStateRef.current;
    if (!editorModel || !initial) return false;
    const editorChanged =
      JSON.stringify(editorModel) !== JSON.stringify(initial.editorModel ?? null);
    const draftChanged =
      JSON.stringify(state.modelAdvancedDraft) !==
      JSON.stringify(initial.modelAdvancedDraft ?? null);
    return editorChanged || draftChanged;
  }, [state]);

  const handleDisplayNameChange = useCallback(
    (value: string) => {
      updateEditorModel({ displayName: value });
    },
    [updateEditorModel],
  );

  const handleModelNameChange = useCallback(
    async (name: string) => {
      if (!state.editorModel) return;

      updateEditorModel({ name });
    },
    [updateEditorModel, state.editorModel],
  );

  const handleProviderSelection = useCallback(
    async (providerId: string, providerLabel: string) => {
      if (!state.editorModel) return;

      dispatch({
        type: "update_editor_model",
        payload: {
          providerId,
          providerLabel,
          ...getHardCappedScopes(providerId),
          ...(isImageOnlyProvider(providerId)
            ? {
                inputScopes: ["text", "image"],
                outputScopes: ["image"],
              }
            : {}),
        },
      });
    },
    [dispatch, state.editorModel],
  );

  const setModelAdvancedDraft = useCallback(
    (settings: AdvancedModelSettings) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: sanitizeAdvancedModelSettings(settings),
      });
    },
    [dispatch],
  );

  const toggleOverride = useCallback(() => {
    // No-op for now, removing usage
  }, []);

  const handleTemperatureChange = useCallback(
    (value: number | null) => {
      const rounded = value === null || !Number.isFinite(value) ? null : Number(value.toFixed(2));
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          temperature: rounded,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleTopPChange = useCallback(
    (value: number | null) => {
      const rounded = value === null || !Number.isFinite(value) ? null : Number(value.toFixed(2));
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          topP: rounded,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleMaxTokensChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          maxOutputTokens: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleContextLengthChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          contextLength: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleFrequencyPenaltyChange = useCallback(
    (value: number | null) => {
      const rounded = value === null || !Number.isFinite(value) ? null : Number(value.toFixed(2));
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          frequencyPenalty: rounded,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handlePresencePenaltyChange = useCallback(
    (value: number | null) => {
      const rounded = value === null || !Number.isFinite(value) ? null : Number(value.toFixed(2));
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          presencePenalty: rounded,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleTopKChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          topK: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaGpuLayersChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaGpuLayers: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaThreadsChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaThreads: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaThreadsBatchChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaThreadsBatch: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaSeedChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaSeed: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaRopeFreqBaseChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaRopeFreqBase: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaRopeFreqScaleChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaRopeFreqScale: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaOffloadKqvChange = useCallback(
    (value: boolean | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaOffloadKqv: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaBatchSizeChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaBatchSize: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaKvTypeChange = useCallback(
    (value: AdvancedModelSettings["llamaKvType"]) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaKvType: value ?? null,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaFlashAttentionChange = useCallback(
    (value: AdvancedModelSettings["llamaFlashAttention"]) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaFlashAttention: value ?? null,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaSamplerProfileChange = useCallback(
    (value: AdvancedModelSettings["llamaSamplerProfile"]) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaSamplerProfile: value ?? null,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaSamplerOrderChange = useCallback(
    (value: AdvancedModelSettings["llamaSamplerOrder"]) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaSamplerOrder: value ?? null,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaMinPChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaMinP: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaTypicalPChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaTypicalP: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaDryMultiplierChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaDryMultiplier: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaDryBaseChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaDryBase: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaDryAllowedLengthChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaDryAllowedLength: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaDryPenaltyLastNChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaDryPenaltyLastN: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaDrySequenceBreakersChange = useCallback(
    (value: string[] | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaDrySequenceBreakers: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaChatTemplateOverrideChange = useCallback(
    (value: string | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaChatTemplateOverride: value?.trim() ? value.trim() : null,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaMmprojPathChange = useCallback(
    (value: string | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaMmprojPath: value?.trim() ? value.trim() : null,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaChatTemplatePresetChange = useCallback(
    (value: string | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaChatTemplatePreset: value?.trim() ? value.trim() : null,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaRawCompletionFallbackChange = useCallback(
    (value: boolean | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaRawCompletionFallback: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaStrictModeChange = useCallback(
    (value: boolean | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaStrictMode: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleLlamaStreamingEnabledChange = useCallback(
    (value: boolean | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          llamaStreamingEnabled: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleOllamaNumCtxChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          ollamaNumCtx: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleOllamaNumPredictChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          ollamaNumPredict: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleOllamaNumKeepChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          ollamaNumKeep: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleOllamaNumBatchChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          ollamaNumBatch: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleOllamaNumGpuChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          ollamaNumGpu: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleOllamaNumThreadChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          ollamaNumThread: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleOllamaTfsZChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          ollamaTfsZ: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleOllamaTypicalPChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          ollamaTypicalP: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleOllamaMinPChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          ollamaMinP: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleOllamaMirostatChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          ollamaMirostat: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleOllamaMirostatTauChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          ollamaMirostatTau: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleOllamaMirostatEtaChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          ollamaMirostatEta: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleOllamaRepeatPenaltyChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          ollamaRepeatPenalty: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleOllamaSeedChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          ollamaSeed: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleOllamaStopChange = useCallback(
    (value: string[] | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          ollamaStop: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleReasoningEnabledChange = useCallback(
    (value: boolean) => {
      const effortBudgets: Record<string, number> = {
        low: 2048,
        medium: 8192,
        high: 16384,
      };

      let newEffort = state.modelAdvancedDraft.reasoningEffort;
      let newBudget = state.modelAdvancedDraft.reasoningBudgetTokens;

      if (value) {
        if (!newEffort) {
          newEffort = "medium";
        }
        if (!newBudget && newEffort) {
          newBudget = effortBudgets[newEffort] ?? 8192;
        }
      }

      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          reasoningEnabled: value,
          reasoningEffort: newEffort,
          reasoningBudgetTokens: newBudget,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleReasoningEffortChange = useCallback(
    (value: "low" | "medium" | "high" | null) => {
      const effortBudgets: Record<string, number> = {
        low: 2048,
        medium: 8192,
        high: 16384,
      };

      let newBudget = state.modelAdvancedDraft.reasoningBudgetTokens;
      if (value && !newBudget) {
        newBudget = effortBudgets[value];
      }

      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          reasoningEffort: value,
          reasoningBudgetTokens: newBudget,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handleReasoningBudgetChange = useCallback(
    (value: number | null) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          reasoningBudgetTokens: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handlePromptCachingEnabledChange = useCallback(
    (value: boolean) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          promptCachingEnabled: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const handlePromptCachingTtlChange = useCallback(
    (value: string) => {
      dispatch({
        type: "set_model_advanced_draft",
        payload: {
          ...state.modelAdvancedDraft,
          promptCachingTtl: value,
        },
      });
    },
    [dispatch, state.modelAdvancedDraft],
  );

  const clearError = useCallback(() => {
    dispatch({ type: "set_error", payload: null });
  }, [dispatch]);

  const getProviderCredentialIdForSave = useCallback((providerCred: ProviderCredential) => {
    if (providerCred.providerId === "llamacpp") {
      return null;
    }
    return providerCred.id;
  }, []);

  const doSave = useCallback(
    async (): Promise<boolean> => {
      const { editorModel, providers, modelAdvancedDraft } = state;
      if (!editorModel) return false;

      dispatch({ type: "set_error", payload: null });

      const providerCred =
        providers.find(
          (p) => p.providerId === editorModel.providerId && p.label === editorModel.providerLabel,
        ) || providers.find((p) => p.providerId === editorModel.providerId);

      if (!providerCred) {
        dispatch({
          type: "set_error",
          payload: "Select a provider with valid credentials",
        });
        return false;
      }

      const shouldVerify = ["openai", "anthropic"].includes(providerCred.providerId);
      if (shouldVerify) {
        try {
          dispatch({ type: "set_verifying", payload: true });
          const name = editorModel.name.trim();
          if (!name) {
            dispatch({ type: "set_error", payload: "Model name required" });
            return false;
          }

          let resp: { exists: boolean; error?: string } | undefined;
          try {
            resp = await invoke<{ exists: boolean; error?: string }>("verify_model_exists", {
              providerId: providerCred.providerId,
              credentialId: providerCred.id,
              model: name,
            });
          } catch (err) {
            console.warn("Invoke verify_model_exists failed, treating as undefined:", err);
          }
          if (!resp) {
            dispatch({
              type: "set_error",
              payload: "Model verification unavailable (backend)",
            });
            return false;
          }
          if (!resp.exists) {
            dispatch({
              type: "set_error",
              payload: resp.error || "Model not found on provider",
            });
            return false;
          }
        } catch (error: any) {
          dispatch({
            type: "set_error",
            payload: error?.message || "Verification failed",
          });
          return false;
        } finally {
          dispatch({ type: "set_verifying", payload: false });
        }
      }

      dispatch({ type: "set_saving", payload: true });
      try {
        const hardCappedScopes = getHardCappedScopes(providerCred.providerId);
        const providerCredentialId = getProviderCredentialIdForSave(providerCred);
        const savedModel = {
          ...editorModel,
          providerId: providerCred.providerId,
          providerCredentialId,
          providerLabel: providerCred.label,
          ...hardCappedScopes,
          advancedModelSettings: sanitizeAdvancedModelSettings(modelAdvancedDraft),
        };
        await addOrUpdateModel({
          ...savedModel,
        });
        dispatch({ type: "update_editor_model", payload: savedModel });
        initialStateRef.current = {
          editorModel: cloneSnapshot(savedModel),
          modelAdvancedDraft: cloneSnapshot(savedModel.advancedModelSettings ?? modelAdvancedDraft),
        };
        return true;
      } catch (error: any) {
        console.error("Failed to save model", error);
        dispatch({
          type: "set_error",
          payload: error?.message || "Failed to save model",
        });
        return false;
      } finally {
        dispatch({ type: "set_saving", payload: false });
      }
    },
    [dispatch, getProviderCredentialIdForSave, state],
  );

  const handleSave = useCallback(async () => {
    await doSave();
  }, [doSave]);

  const saveModel = useCallback(async (): Promise<boolean> => {
    return doSave();
  }, [doSave]);

  const handleDelete = useCallback(async () => {
    const { editorModel } = state;
    if (!editorModel || isNew) return;
    dispatch({ type: "set_deleting", payload: true });
    dispatch({ type: "set_error", payload: null });
    try {
      await removeModel(editorModel.id);
      backOrReplace(Routes.settingsModels);
    } catch (error: any) {
      console.error("Failed to delete model", error);
      dispatch({
        type: "set_error",
        payload: error?.message || "Failed to delete model",
      });
    } finally {
      dispatch({ type: "set_deleting", payload: false });
    }
  }, [backOrReplace, state, isNew]);

  const handleSetDefault = useCallback(async () => {
    const { editorModel } = state;
    if (!editorModel) return;
    try {
      await setDefaultModel(editorModel.id);
      dispatch({ type: "set_default_model_id", payload: editorModel.id });
    } catch (error) {
      console.error("Failed to set default model", error);
    }
  }, [state]);

  const fetchModels = useCallback(async () => {
    const { editorModel, providers } = state;
    if (!editorModel) return;
    if (
      editorModel.providerId === "llamacpp" ||
      editorModel.providerId === "intenserp" ||
      editorModel.providerId === "stability"
    ) {
      dispatch({ type: "set_fetched_models", payload: [] });
      return;
    }

    const providerCred =
      providers.find(
        (p) => p.providerId === editorModel.providerId && p.label === editorModel.providerLabel,
      ) || providers.find((p) => p.providerId === editorModel.providerId);

    if (!providerCred) {
      dispatch({
        type: "set_error",
        payload: "Select a provider with valid credentials to fetch models",
      });
      return;
    }

    if (
      (providerCred.providerId === "custom" || providerCred.providerId === "custom-anthropic") &&
      providerCred.config?.fetchModelsEnabled !== true
    ) {
      dispatch({
        type: "set_error",
        payload: "Model fetching is disabled for this custom provider.",
      });
      dispatch({ type: "set_fetched_models", payload: [] });
      return;
    }

    dispatch({ type: "set_fetching_models", payload: true });
    dispatch({ type: "set_error", payload: null });

    try {
      const models = await invoke<any[]>("get_remote_models", {
        credentialId: providerCred.id,
      });
      dispatch({ type: "set_fetched_models", payload: models });
    } catch (error: any) {
      console.error("Failed to fetch models", error);
      dispatch({
        type: "set_error",
        payload: error?.message || "Failed to fetch models",
      });
    } finally {
      dispatch({ type: "set_fetching_models", payload: false });
    }
  }, [state]);

  const resetToInitial = useCallback(() => {
    const initial = initialStateRef.current;
    if (!initial) return;
    dispatch({
      type: "load_success",
      payload: {
        providers: state.providers,
        defaultModelId: state.defaultModelId,
        editorModel: initial.editorModel ? JSON.parse(JSON.stringify(initial.editorModel)) : null,
        modelAdvancedDraft: JSON.parse(JSON.stringify(initial.modelAdvancedDraft)),
      },
    });
    dispatch({ type: "set_error", payload: null });
  }, [dispatch, state.defaultModelId, state.providers]);

  const applyLlamaRuntimeSuggestion = useCallback(async (): Promise<boolean> => {
    const { editorModel, modelAdvancedDraft, providers } = state;
    const report = modelAdvancedDraft.llamaLastRuntimeReport;
    const suggestedSettings = report?.suggestedSettings;
    if (
      !editorModel ||
      editorModel.providerId !== "llamacpp" ||
      report?.status !== "cpuFallbackSucceeded" ||
      !suggestedSettings
    ) {
      return false;
    }

    const providerCred =
      providers.find(
        (provider) =>
          provider.providerId === editorModel.providerId &&
          provider.label === editorModel.providerLabel,
      ) || providers.find((provider) => provider.providerId === editorModel.providerId);
    if (!providerCred) {
      dispatch({
        type: "set_error",
        payload: "Select a provider with valid credentials before applying the runtime config",
      });
      return false;
    }

    const nextDraft = sanitizeAdvancedModelSettings({
      ...modelAdvancedDraft,
      contextLength: suggestedSettings.contextLength ?? modelAdvancedDraft.contextLength ?? null,
      llamaBatchSize: suggestedSettings.llamaBatchSize ?? modelAdvancedDraft.llamaBatchSize ?? null,
      llamaLastRuntimeReport: report,
    });

    dispatch({ type: "set_saving", payload: true });
    dispatch({ type: "set_error", payload: null });
    try {
      const hardCappedScopes = getHardCappedScopes(providerCred.providerId);
      const providerCredentialId = getProviderCredentialIdForSave(providerCred);
      await addOrUpdateModel({
        ...editorModel,
        providerId: providerCred.providerId,
        providerCredentialId,
        providerLabel: providerCred.label,
        ...hardCappedScopes,
        advancedModelSettings: nextDraft,
      });
      dispatch({
        type: "update_editor_model",
        payload: {
          providerId: providerCred.providerId,
          providerCredentialId,
          providerLabel: providerCred.label,
          ...hardCappedScopes,
          advancedModelSettings: nextDraft,
        },
      });
      dispatch({ type: "set_model_advanced_draft", payload: nextDraft });
      initialStateRef.current = {
        editorModel: cloneSnapshot({
          ...editorModel,
          providerId: providerCred.providerId,
          providerCredentialId,
          providerLabel: providerCred.label,
          ...hardCappedScopes,
          advancedModelSettings: nextDraft,
        }),
        modelAdvancedDraft: cloneSnapshot(nextDraft),
      };
      return true;
    } catch (error: any) {
      console.error("Failed to apply llama runtime suggestion", error);
      dispatch({
        type: "set_error",
        payload: error?.message || "Failed to apply runtime recommendation",
      });
      return false;
    } finally {
      dispatch({ type: "set_saving", payload: false });
    }
  }, [getProviderCredentialIdForSave, state]);

  return {
    state,
    isNew,
    canSave,
    hasUnsavedChanges,
    providerDisplay,
    updateEditorModel,
    handleDisplayNameChange,
    handleModelNameChange,
    handleProviderSelection,
    setModelAdvancedDraft,
    toggleOverride,
    handleTemperatureChange,
    handleTopPChange,
    handleMaxTokensChange,
    handleContextLengthChange,
    handleFrequencyPenaltyChange,
    handlePresencePenaltyChange,
    handleTopKChange,
    handleLlamaGpuLayersChange,
    handleLlamaThreadsChange,
    handleLlamaThreadsBatchChange,
    handleLlamaSeedChange,
    handleLlamaRopeFreqBaseChange,
    handleLlamaRopeFreqScaleChange,
    handleLlamaOffloadKqvChange,
    handleLlamaBatchSizeChange,
    handleLlamaKvTypeChange,
    handleLlamaFlashAttentionChange,
    handleLlamaSamplerProfileChange,
    handleLlamaSamplerOrderChange,
    handleLlamaMinPChange,
    handleLlamaTypicalPChange,
    handleLlamaDryMultiplierChange,
    handleLlamaDryBaseChange,
    handleLlamaDryAllowedLengthChange,
    handleLlamaDryPenaltyLastNChange,
    handleLlamaDrySequenceBreakersChange,
    handleLlamaChatTemplateOverrideChange,
    handleLlamaMmprojPathChange,
    handleLlamaChatTemplatePresetChange,
    handleLlamaRawCompletionFallbackChange,
    handleLlamaStrictModeChange,
    handleLlamaStreamingEnabledChange,
    handleOllamaNumCtxChange,
    handleOllamaNumPredictChange,
    handleOllamaNumKeepChange,
    handleOllamaNumBatchChange,
    handleOllamaNumGpuChange,
    handleOllamaNumThreadChange,
    handleOllamaTfsZChange,
    handleOllamaTypicalPChange,
    handleOllamaMinPChange,
    handleOllamaMirostatChange,
    handleOllamaMirostatTauChange,
    handleOllamaMirostatEtaChange,
    handleOllamaRepeatPenaltyChange,
    handleOllamaSeedChange,
    handleOllamaStopChange,
    handleReasoningEnabledChange,
    handleReasoningEffortChange,
    handleReasoningBudgetChange,
    handlePromptCachingEnabledChange,
    handlePromptCachingTtlChange,
    applyLlamaRuntimeSuggestion,
    handleSave,
    saveModel,
    handleDelete,
    handleSetDefault,
    resetToInitial,
    clearError,
    fetchModels,
  };
}
