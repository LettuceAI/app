import { useCallback, useEffect, useReducer, useState } from "react";

import {
  engineGetConfig,
  engineConfigLlm,
  engineConfigLlmDefault,
  engineConfigLlmDelete,
  engineConfigEngine,
  engineConfigBackground,
  engineConfigMemory,
  engineConfigSafety,
  engineConfigResearch,
} from "../../../../core/engine/api";
import {
  ENGINE_LLM_PROVIDERS,
  type EngineLlmProviderId,
} from "../../../../core/engine/types";
import { readSettings } from "../../../../core/storage/repo";
import type { ProviderCredential } from "../../../../core/storage/schemas";

// ── LLM Provider Config state ──────────────────────────────────────────────

export type LlmProviderConfig = {
  enabled: boolean;
  model: string;
  apiKey: string;
  apiKeyRedacted: string;
  apiKeyChanged: boolean;
  baseUrl: string;
  maxTokens: number;
  temperature: number;
};

type ProvidersState = {
  loading: boolean;
  saving: boolean;
  error: string | null;
  defaultBackend: EngineLlmProviderId | null;
  providers: Record<EngineLlmProviderId, LlmProviderConfig>;
};

const emptyProvider = (): LlmProviderConfig => ({
  enabled: false,
  model: "",
  apiKey: "",
  apiKeyRedacted: "",
  apiKeyChanged: false,
  baseUrl: "",
  maxTokens: 1024,
  temperature: 0.9,
});

const initialProvidersState: ProvidersState = {
  loading: true,
  saving: false,
  error: null,
  defaultBackend: null,
  providers: {
    anthropic: emptyProvider(),
    openai: emptyProvider(),
    openrouter: emptyProvider(),
    ollama: { ...emptyProvider(), baseUrl: "http://localhost:11434" },
  },
};

type ProvidersAction =
  | { type: "set_loading"; payload: boolean }
  | { type: "set_saving"; payload: boolean }
  | { type: "set_error"; payload: string | null }
  | { type: "set_default_backend"; payload: EngineLlmProviderId | null }
  | { type: "load_config"; payload: { providers: Record<string, any>; defaultBackend: string | null } }
  | { type: "update_provider"; payload: { id: EngineLlmProviderId; updates: Partial<LlmProviderConfig> } };

function providersReducer(state: ProvidersState, action: ProvidersAction): ProvidersState {
  switch (action.type) {
    case "set_loading":
      return { ...state, loading: action.payload };
    case "set_saving":
      return { ...state, saving: action.payload };
    case "set_error":
      return { ...state, error: action.payload };
    case "set_default_backend":
      return { ...state, defaultBackend: action.payload };
    case "load_config": {
      const providers = { ...state.providers };
      for (const [id, config] of Object.entries(action.payload.providers)) {
        const pid = id as EngineLlmProviderId;
        if (providers[pid]) {
          providers[pid] = {
            enabled: true,
            model: (config as any).model ?? "",
            apiKey: "",
            apiKeyRedacted: (config as any).api_key ?? "",
            apiKeyChanged: false,
            baseUrl: (config as any).base_url ?? "",
            maxTokens: (config as any).max_tokens ?? 1024,
            temperature: (config as any).temperature ?? 0.9,
          };
        }
      }
      return {
        ...state,
        loading: false,
        providers,
        defaultBackend: (action.payload.defaultBackend as EngineLlmProviderId) ?? null,
      };
    }
    case "update_provider":
      return {
        ...state,
        providers: {
          ...state.providers,
          [action.payload.id]: {
            ...state.providers[action.payload.id],
            ...action.payload.updates,
          },
        },
      };
    default:
      return state;
  }
}

export function useEngineProvidersConfigController(baseUrl: string, apiKey: string) {
  const [state, dispatch] = useReducer(providersReducer, initialProvidersState);
  const [appProviders, setAppProviders] = useState<ProviderCredential[]>([]);

  const load = useCallback(async () => {
    dispatch({ type: "set_loading", payload: true });
    dispatch({ type: "set_error", payload: null });
    try {
      const raw = await engineGetConfig(baseUrl, apiKey);
      const config = (typeof raw === "string" ? JSON.parse(raw) : raw) as Record<string, unknown>;
      const llm = (config?.llm ?? {}) as Record<string, unknown>;
      const engine = (config?.engine ?? {}) as Record<string, unknown>;

      // llm contains providers directly (no "providers" wrapper)
      const knownProviderIds: Set<string> = new Set(ENGINE_LLM_PROVIDERS.map((p) => p.id));
      const providers: Record<string, unknown> = {};
      for (const [key, value] of Object.entries(llm)) {
        if (knownProviderIds.has(key)) providers[key] = value;
      }

      // default_backend lives under engine, not llm
      const defaultBackend = (engine?.default_backend as string) ?? null;

      dispatch({
        type: "load_config",
        payload: { providers, defaultBackend },
      });
    } catch (e) {
      console.error("[EngineConfig] Failed to load config:", e);
      dispatch({ type: "set_error", payload: typeof e === "string" ? e : String(e) });
      dispatch({ type: "set_loading", payload: false });
    }
  }, [baseUrl, apiKey]);

  useEffect(() => {
    void load();
  }, [load]);

  // Load importable app providers
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const settings = await readSettings();
      if (cancelled) return;
      const engineIds: Set<string> = new Set(ENGINE_LLM_PROVIDERS.map((p) => p.id));
      setAppProviders(
        settings.providerCredentials.filter(
          (p) => engineIds.has(p.providerId) && p.providerId !== "lettuce-engine",
        ),
      );
    })();
    return () => { cancelled = true; };
  }, []);

  const updateProvider = useCallback((id: EngineLlmProviderId, updates: Partial<LlmProviderConfig>) => {
    // Track if user changed the API key
    if ("apiKey" in updates && updates.apiKey !== undefined) {
      updates.apiKeyChanged = true;
    }
    dispatch({ type: "update_provider", payload: { id, updates } });
  }, []);

  const setDefaultBackend = useCallback((id: EngineLlmProviderId) => {
    dispatch({ type: "set_default_backend", payload: id });
  }, []);

  const importAppProvider = useCallback((provider: ProviderCredential) => {
    const engineId = provider.providerId as EngineLlmProviderId;
    dispatch({
      type: "update_provider",
      payload: {
        id: engineId,
        updates: {
          enabled: true,
          apiKey: provider.apiKey || "",
          apiKeyChanged: true,
          baseUrl: provider.baseUrl || "",
        },
      },
    });
  }, []);

  const save = useCallback(async (): Promise<boolean> => {
    dispatch({ type: "set_saving", payload: true });
    dispatch({ type: "set_error", payload: null });
    try {
      const enabledProviders = ENGINE_LLM_PROVIDERS.filter((p) => state.providers[p.id].enabled);

      if (enabledProviders.length === 0) {
        dispatch({ type: "set_error", payload: "At least one provider must be enabled." });
        return false;
      }

      // Save each enabled provider
      for (const provider of enabledProviders) {
        const config = state.providers[provider.id];
        if (!config.model.trim()) {
          dispatch({ type: "set_error", payload: `Model is required for ${provider.name}.` });
          return false;
        }
        if (provider.requiresKey && !config.apiKey.trim() && !config.apiKeyRedacted) {
          dispatch({ type: "set_error", payload: `API key is required for ${provider.name}.` });
          return false;
        }

        await engineConfigLlm(baseUrl, apiKey, provider.id, {
          model: config.model.trim(),
          api_key: config.apiKeyChanged && config.apiKey.trim() ? config.apiKey.trim() : undefined,
          base_url: config.baseUrl.trim() || undefined,
          max_tokens: config.maxTokens,
          temperature: config.temperature,
        });
      }

      // Delete disabled providers that were previously configured
      const disabledConfigured = ENGINE_LLM_PROVIDERS.filter(
        (p) => !state.providers[p.id].enabled && state.providers[p.id].apiKeyRedacted,
      );
      for (const provider of disabledConfigured) {
        try {
          await engineConfigLlmDelete(baseUrl, apiKey, provider.id);
        } catch {
          // Ignore — may not exist on server
        }
      }

      // Set default backend
      const defaultProvider = state.defaultBackend && state.providers[state.defaultBackend]?.enabled
        ? state.defaultBackend
        : enabledProviders[0].id;
      await engineConfigLlmDefault(baseUrl, apiKey, { provider: defaultProvider });

      return true;
    } catch (e) {
      dispatch({ type: "set_error", payload: typeof e === "string" ? e : String(e) });
      return false;
    } finally {
      dispatch({ type: "set_saving", payload: false });
    }
  }, [state.providers, state.defaultBackend, baseUrl, apiKey]);

  return {
    state,
    appProviders,
    updateProvider,
    setDefaultBackend,
    importAppProvider,
    save,
    reload: load,
  };
}

// ── Engine Settings state ──────────────────────────────────────────────────

export type EngineSettingsValues = {
  // engine
  dataDir: string;
  logLevel: string;
  maxHistory: number;
  // background
  synthesisInterval: number;
  consolidationInterval: number;
  bm25RebuildInterval: number;
  dripResearchInterval: number;
  // memory
  embeddingModel: string;
  maxRetrievalResults: number;
  denseWeight: number;
  bm25Weight: number;
  graphWeight: number;
  recencyBoostHours: number;
  randomSurfaceProbability: number;
  // safety
  honestySection: boolean;
  userDataDeletion: boolean;
  // research
  initialScrapeOnBoot: boolean;
  periodicIntervalHours: number;
};

type SettingsState = {
  loading: boolean;
  saving: boolean;
  error: string | null;
  values: EngineSettingsValues;
};

const defaultSettingsValues: EngineSettingsValues = {
  dataDir: "./data",
  logLevel: "INFO",
  maxHistory: 40,
  synthesisInterval: 10,
  consolidationInterval: 60,
  bm25RebuildInterval: 15,
  dripResearchInterval: 60,
  embeddingModel: "all-MiniLM-L6-v2",
  maxRetrievalResults: 15,
  denseWeight: 0.5,
  bm25Weight: 0.3,
  graphWeight: 0.2,
  recencyBoostHours: 2.0,
  randomSurfaceProbability: 0.05,
  honestySection: true,
  userDataDeletion: true,
  initialScrapeOnBoot: true,
  periodicIntervalHours: 6,
};

type SettingsAction =
  | { type: "set_loading"; payload: boolean }
  | { type: "set_saving"; payload: boolean }
  | { type: "set_error"; payload: string | null }
  | { type: "load_config"; payload: any }
  | { type: "update"; payload: Partial<EngineSettingsValues> };

function settingsReducer(state: SettingsState, action: SettingsAction): SettingsState {
  switch (action.type) {
    case "set_loading":
      return { ...state, loading: action.payload };
    case "set_saving":
      return { ...state, saving: action.payload };
    case "set_error":
      return { ...state, error: action.payload };
    case "load_config": {
      const c = action.payload;
      const engine = c?.engine ?? {};
      const bg = c?.background ?? {};
      const mem = c?.memory ?? {};
      const safety = c?.safety ?? {};
      const research = c?.research ?? {};
      return {
        ...state,
        loading: false,
        values: {
          dataDir: engine.data_dir ?? defaultSettingsValues.dataDir,
          logLevel: engine.log_level ?? defaultSettingsValues.logLevel,
          maxHistory: engine.max_history ?? defaultSettingsValues.maxHistory,
          synthesisInterval: bg.synthesis_interval_minutes ?? defaultSettingsValues.synthesisInterval,
          consolidationInterval: bg.consolidation_interval_minutes ?? defaultSettingsValues.consolidationInterval,
          bm25RebuildInterval: bg.bm25_rebuild_interval_minutes ?? defaultSettingsValues.bm25RebuildInterval,
          dripResearchInterval: bg.drip_research_interval_minutes ?? defaultSettingsValues.dripResearchInterval,
          embeddingModel: mem.embedding_model ?? defaultSettingsValues.embeddingModel,
          maxRetrievalResults: mem.max_retrieval_results ?? defaultSettingsValues.maxRetrievalResults,
          denseWeight: mem.dense_weight ?? defaultSettingsValues.denseWeight,
          bm25Weight: mem.bm25_weight ?? defaultSettingsValues.bm25Weight,
          graphWeight: mem.graph_weight ?? defaultSettingsValues.graphWeight,
          recencyBoostHours: mem.recency_boost_hours ?? defaultSettingsValues.recencyBoostHours,
          randomSurfaceProbability: mem.random_surface_probability ?? defaultSettingsValues.randomSurfaceProbability,
          honestySection: safety.honesty_section ?? defaultSettingsValues.honestySection,
          userDataDeletion: safety.user_data_deletion ?? defaultSettingsValues.userDataDeletion,
          initialScrapeOnBoot: research.initial_scrape_on_boot ?? defaultSettingsValues.initialScrapeOnBoot,
          periodicIntervalHours: research.periodic_interval_hours ?? defaultSettingsValues.periodicIntervalHours,
        },
      };
    }
    case "update":
      return { ...state, values: { ...state.values, ...action.payload } };
    default:
      return state;
  }
}

export function useEngineSettingsConfigController(baseUrl: string, apiKey: string) {
  const [state, dispatch] = useReducer(settingsReducer, {
    loading: true,
    saving: false,
    error: null,
    values: defaultSettingsValues,
  });

  const load = useCallback(async () => {
    dispatch({ type: "set_loading", payload: true });
    dispatch({ type: "set_error", payload: null });
    try {
      const raw = await engineGetConfig(baseUrl, apiKey);
      const config = typeof raw === "string" ? JSON.parse(raw) : raw;
      dispatch({ type: "load_config", payload: config });
    } catch (e) {
      dispatch({ type: "set_error", payload: typeof e === "string" ? e : String(e) });
      dispatch({ type: "set_loading", payload: false });
    }
  }, [baseUrl, apiKey]);

  useEffect(() => {
    void load();
  }, [load]);

  const update = useCallback((updates: Partial<EngineSettingsValues>) => {
    dispatch({ type: "update", payload: updates });
  }, []);

  const save = useCallback(async (): Promise<boolean> => {
    dispatch({ type: "set_saving", payload: true });
    dispatch({ type: "set_error", payload: null });
    try {
      const v = state.values;
      await engineConfigEngine(baseUrl, apiKey, {
        data_dir: v.dataDir,
        log_level: v.logLevel,
        max_history: v.maxHistory,
      });
      await engineConfigBackground(baseUrl, apiKey, {
        synthesis_interval_minutes: v.synthesisInterval,
        consolidation_interval_minutes: v.consolidationInterval,
        bm25_rebuild_interval_minutes: v.bm25RebuildInterval,
        drip_research_interval_minutes: v.dripResearchInterval,
      });
      await engineConfigMemory(baseUrl, apiKey, {
        embedding_model: v.embeddingModel,
        max_retrieval_results: v.maxRetrievalResults,
        dense_weight: v.denseWeight,
        bm25_weight: v.bm25Weight,
        graph_weight: v.graphWeight,
        recency_boost_hours: v.recencyBoostHours,
        random_surface_probability: v.randomSurfaceProbability,
      });
      await engineConfigSafety(baseUrl, apiKey, {
        honesty_section: v.honestySection,
        user_data_deletion: v.userDataDeletion,
      });
      await engineConfigResearch(baseUrl, apiKey, {
        initial_scrape_on_boot: v.initialScrapeOnBoot,
        periodic_interval_hours: v.periodicIntervalHours,
      });
      return true;
    } catch (e) {
      dispatch({ type: "set_error", payload: typeof e === "string" ? e : String(e) });
      return false;
    } finally {
      dispatch({ type: "set_saving", payload: false });
    }
  }, [state.values, baseUrl, apiKey]);

  return { state, update, save, reload: load };
}
