import type { EngineLlmProviderId } from "../../../../core/engine/types";

export type WizardStep = "welcome" | "providers" | "settings" | "done";

export type LlmProviderConfig = {
  enabled: boolean;
  model: string;
  apiKey: string;
  baseUrl: string;
  maxTokens: number;
  temperature: number;
};

export type EngineSettings = {
  dataDir: string;
  logLevel: string;
  maxHistory: number;
  synthesisInterval: number;
  consolidationInterval: number;
  bm25RebuildInterval: number;
  dripResearchInterval: number;
  embeddingModel: string;
  maxRetrievalResults: number;
  denseWeight: number;
  bm25Weight: number;
  graphWeight: number;
};

export type EngineSetupState = {
  step: WizardStep;
  isSaving: boolean;
  error: string | null;
  defaultBackend: EngineLlmProviderId;
  llmProviders: Record<EngineLlmProviderId, LlmProviderConfig>;
  engineSettings: EngineSettings;
};

const defaultLlmConfig = (requiresKey: boolean): LlmProviderConfig => ({
  enabled: false,
  model: "",
  apiKey: "",
  baseUrl: requiresKey ? "" : "http://localhost:11434",
  maxTokens: 1024,
  temperature: 0.9,
});

export const initialEngineSetupState: EngineSetupState = {
  step: "welcome",
  isSaving: false,
  error: null,
  defaultBackend: "anthropic",
  llmProviders: {
    anthropic: { ...defaultLlmConfig(true), model: "claude-sonnet-4-5-20250929" },
    openai: { ...defaultLlmConfig(true), model: "gpt-4o" },
    openrouter: defaultLlmConfig(true),
    ollama: { ...defaultLlmConfig(false), model: "llama3" },
  },
  engineSettings: {
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
  },
};

export type EngineSetupAction =
  | { type: "set_step"; payload: WizardStep }
  | { type: "set_saving"; payload: boolean }
  | { type: "set_error"; payload: string | null }
  | { type: "set_default_backend"; payload: EngineLlmProviderId }
  | {
      type: "update_llm_provider";
      payload: { provider: EngineLlmProviderId; updates: Partial<LlmProviderConfig> };
    }
  | { type: "update_engine_settings"; payload: Partial<EngineSettings> };

export function engineSetupReducer(
  state: EngineSetupState,
  action: EngineSetupAction,
): EngineSetupState {
  switch (action.type) {
    case "set_step":
      return { ...state, step: action.payload, error: null };
    case "set_saving":
      return { ...state, isSaving: action.payload };
    case "set_error":
      return { ...state, error: action.payload };
    case "set_default_backend":
      return { ...state, defaultBackend: action.payload };
    case "update_llm_provider":
      return {
        ...state,
        llmProviders: {
          ...state.llmProviders,
          [action.payload.provider]: {
            ...state.llmProviders[action.payload.provider],
            ...action.payload.updates,
          },
        },
      };
    case "update_engine_settings":
      return {
        ...state,
        engineSettings: { ...state.engineSettings, ...action.payload },
      };
    default:
      return state;
  }
}
