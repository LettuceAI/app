import { useCallback, useEffect, useMemo, useReducer, useRef } from "react";
import { useParams } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";

import {
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

type ControllerReturn = {
  state: ModelEditorState;
  isNew: boolean;
  canSave: boolean;
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
  handleSave: () => Promise<void>;
  handleDelete: () => Promise<void>;
  handleSetDefault: () => Promise<void>;
  resetToInitial: () => void;
  clearError: () => void;
  fetchModels: () => Promise<void>;
};

function useModelEditorState() {
  return useReducer(modelEditorReducer, initialModelEditorState);
}

export function useModelEditorController(): ControllerReturn {
  const { toModelsList, backOrReplace } = useNavigationManager();
  const { modelId } = useParams<{ modelId: string }>();
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
      id: "local-llamacpp",
      providerId: "llamacpp",
      label: "llama.cpp (Local)",
      apiKey: "",
    }),
    [],
  );

  const ensureLocalProvider = useCallback(
    (providers: ProviderCredential[]) => {
      const hasLocal = providers.some((p) => p.providerId === localProvider.providerId);
      if (hasLocal) return providers;
      return providers.length === 0 ? [localProvider] : [...providers, localProvider];
    },
    [localProvider],
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

        // Use a standard default if no settings exist
        const defaultAdvanced = createDefaultAdvancedModelSettings();

        let nextEditorModel: Model | null = null;
        let nextDraft = sanitizeAdvancedModelSettings(defaultAdvanced);

        if (isNew) {
          const firstProvider = providers[0];
          const firstCap = capabilities[0];
          nextEditorModel = {
            id: crypto.randomUUID(),
            name: "",
            displayName: "",
            providerId: firstProvider?.providerId || firstCap?.id || "",
            providerLabel: firstProvider?.label || firstCap?.name || "",
            createdAt: Date.now(),
            inputScopes: ["text"],
            outputScopes: ["text"],
          } as Model;
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
          initialStateRef.current = {
            editorModel: nextEditorModel ? JSON.parse(JSON.stringify(nextEditorModel)) : null,
            modelAdvancedDraft: JSON.parse(JSON.stringify(nextDraft)),
          };
        }
      } catch (error) {
        console.error("Failed to load model settings", error);
        if (!cancelled) {
          dispatch({
            type: "set_error",
            payload: "Failed to load model settings",
          });
          dispatch({ type: "set_loading", payload: false });
        }
      }
    };

    load();
    return () => {
      cancelled = true;
    };
  }, [ensureLocalProvider, isNew, modelId, toModelsList]);

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

  const clearError = useCallback(() => {
    dispatch({ type: "set_error", payload: null });
  }, [dispatch]);

  const handleSave = useCallback(async () => {
    const { editorModel, providers, modelAdvancedDraft } = state;
    if (!editorModel) return;

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
      return;
    }

    const shouldVerify = ["openai", "anthropic"].includes(providerCred.providerId);
    if (shouldVerify) {
      try {
        dispatch({ type: "set_verifying", payload: true });
        const name = editorModel.name.trim();
        if (!name) {
          dispatch({ type: "set_error", payload: "Model name required" });
          return;
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
          return;
        }
        if (!resp.exists) {
          dispatch({
            type: "set_error",
            payload: resp.error || "Model not found on provider",
          });
          return;
        }
      } catch (error: any) {
        dispatch({
          type: "set_error",
          payload: error?.message || "Verification failed",
        });
        return;
      } finally {
        dispatch({ type: "set_verifying", payload: false });
      }
    }

    dispatch({ type: "set_saving", payload: true });
    try {
      await addOrUpdateModel({
        ...editorModel,
        providerId: providerCred.providerId,
        providerCredentialId: providerCred.id,
        providerLabel: providerCred.label,
        advancedModelSettings: sanitizeAdvancedModelSettings(modelAdvancedDraft),
      });
      backOrReplace(Routes.settingsModels);
    } catch (error: any) {
      console.error("Failed to save model", error);
      dispatch({
        type: "set_error",
        payload: error?.message || "Failed to save model",
      });
    } finally {
      dispatch({ type: "set_saving", payload: false });
    }
  }, [backOrReplace, state]);

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
    if (editorModel.providerId === "llamacpp") {
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

  return {
    state,
    isNew,
    canSave,
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
    handleSave,
    handleDelete,
    handleSetDefault,
    resetToInitial,
    clearError,
    fetchModels,
  };
}
