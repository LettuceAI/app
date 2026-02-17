import { useCallback, useEffect, useReducer, useState } from "react";
import { useNavigate } from "react-router-dom";

import {
  engineSetupReducer,
  initialEngineSetupState,
  type WizardStep,
  type LlmProviderConfig,
  type EngineSettings,
} from "./engineSetupReducer";
import {
  engineConfigLlm,
  engineConfigLlmDefault,
  engineConfigEngine,
  engineConfigBackground,
  engineConfigMemory,
  engineSetupComplete,
} from "../../../../core/engine/api";
import { ENGINE_LLM_PROVIDERS, type EngineLlmProviderId } from "../../../../core/engine/types";
import { Routes } from "../../../navigation";
import { readSettings } from "../../../../core/storage/repo";
import type { ProviderCredential } from "../../../../core/storage/schemas";

/** App provider IDs that map to Engine LLM provider IDs */
const IMPORTABLE_PROVIDER_IDS = new Set<string>(ENGINE_LLM_PROVIDERS.map((p) => p.id));

export function useEngineSetupController(baseUrl: string, apiKey: string, credentialId: string) {
  const [state, dispatch] = useReducer(engineSetupReducer, initialEngineSetupState);
  const [appProviders, setAppProviders] = useState<ProviderCredential[]>([]);
  const navigate = useNavigate();

  // Load existing app providers that can be imported into the Engine
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const settings = await readSettings();
      if (cancelled) return;
      const importable = settings.providerCredentials.filter(
        (p) => IMPORTABLE_PROVIDER_IDS.has(p.providerId) && p.providerId !== "lettuce-engine",
      );
      setAppProviders(importable);
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const importAppProvider = useCallback((provider: ProviderCredential) => {
    const engineId = provider.providerId as EngineLlmProviderId;
    dispatch({
      type: "update_llm_provider",
      payload: {
        provider: engineId,
        updates: {
          enabled: true,
          apiKey: provider.apiKey || "",
          baseUrl: provider.baseUrl || "",
        },
      },
    });
  }, []);

  const setStep = useCallback((step: WizardStep) => {
    dispatch({ type: "set_step", payload: step });
  }, []);

  const setDefaultBackend = useCallback((provider: EngineLlmProviderId) => {
    dispatch({ type: "set_default_backend", payload: provider });
  }, []);

  const updateLlmProvider = useCallback(
    (provider: EngineLlmProviderId, updates: Partial<LlmProviderConfig>) => {
      dispatch({ type: "update_llm_provider", payload: { provider, updates } });
    },
    [],
  );

  const updateEngineSettings = useCallback((updates: Partial<EngineSettings>) => {
    dispatch({ type: "update_engine_settings", payload: updates });
  }, []);

  const saveProviders = useCallback(async () => {
    dispatch({ type: "set_saving", payload: true });
    dispatch({ type: "set_error", payload: null });

    try {
      const enabledProviders = ENGINE_LLM_PROVIDERS.filter(
        (p) => state.llmProviders[p.id].enabled,
      );

      if (enabledProviders.length === 0) {
        dispatch({ type: "set_error", payload: "Please enable at least one LLM provider." });
        return false;
      }

      for (const provider of enabledProviders) {
        const config = state.llmProviders[provider.id];
        if (!config.model.trim()) {
          dispatch({
            type: "set_error",
            payload: `Model is required for ${provider.name}.`,
          });
          return false;
        }
        if (provider.requiresKey && !config.apiKey.trim()) {
          dispatch({
            type: "set_error",
            payload: `API key is required for ${provider.name}.`,
          });
          return false;
        }

        await engineConfigLlm(baseUrl, apiKey, provider.id, {
          model: config.model.trim(),
          api_key: config.apiKey.trim() || undefined,
          base_url: config.baseUrl.trim() || undefined,
          max_tokens: config.maxTokens,
          temperature: config.temperature,
        });
      }

      // Ensure default backend is actually enabled
      const defaultProvider = state.llmProviders[state.defaultBackend];
      const actualDefault = defaultProvider.enabled
        ? state.defaultBackend
        : enabledProviders[0].id;

      await engineConfigLlmDefault(baseUrl, apiKey, { provider: actualDefault });
      return true;
    } catch (e) {
      dispatch({
        type: "set_error",
        payload: e instanceof Error ? e.message : String(e),
      });
      return false;
    } finally {
      dispatch({ type: "set_saving", payload: false });
    }
  }, [state.llmProviders, state.defaultBackend, baseUrl, apiKey]);

  const saveSettings = useCallback(async () => {
    dispatch({ type: "set_saving", payload: true });
    dispatch({ type: "set_error", payload: null });

    try {
      const s = state.engineSettings;
      await engineConfigEngine(baseUrl, apiKey, {
        data_dir: s.dataDir,
        log_level: s.logLevel,
        max_history: s.maxHistory,
      });
      await engineConfigBackground(baseUrl, apiKey, {
        synthesis_interval_minutes: s.synthesisInterval,
        consolidation_interval_minutes: s.consolidationInterval,
        bm25_rebuild_interval_minutes: s.bm25RebuildInterval,
        drip_research_interval_minutes: s.dripResearchInterval,
      });
      await engineConfigMemory(baseUrl, apiKey, {
        embedding_model: s.embeddingModel,
        max_retrieval_results: s.maxRetrievalResults,
        dense_weight: s.denseWeight,
        bm25_weight: s.bm25Weight,
        graph_weight: s.graphWeight,
      });

      await engineSetupComplete(baseUrl, apiKey);

      dispatch({ type: "set_step", payload: "done" });
      return true;
    } catch (e) {
      dispatch({
        type: "set_error",
        payload: e instanceof Error ? e.message : String(e),
      });
      return false;
    } finally {
      dispatch({ type: "set_saving", payload: false });
    }
  }, [state.engineSettings, baseUrl, apiKey]);

  const finishWizard = useCallback(() => {
    navigate(Routes.engineHome(credentialId), { replace: true });
  }, [navigate, credentialId]);

  return {
    state,
    appProviders,
    setStep,
    setDefaultBackend,
    updateLlmProvider,
    updateEngineSettings,
    importAppProvider,
    saveProviders,
    saveSettings,
    finishWizard,
  };
}
