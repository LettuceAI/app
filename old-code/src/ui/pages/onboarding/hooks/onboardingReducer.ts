import type { ProviderCredential } from "../../../../core/storage/schemas";
import type { ProviderCapabilitiesCamel } from "../../../../core/providers/capabilities";

export enum OnboardingStep {
  Welcome = 0,
  Provider = 1,
  Model = 2,
  Memory = 3,
  Sync = 4,
  Intro = 5,
  Learn = 6,
  Path = 7,
  GeminiSetup = 8,
  OpenRouterSetup = 9,
  MemorySetup = 10,
}

export type TestResult = { success: boolean; message: string } | null;
export type MemoryType = "manual" | "dynamic";

export interface OnboardingState {
  // Navigation
  step: OnboardingStep;

  // Provider capabilities
  capabilities: ProviderCapabilitiesCamel[];
  capabilitiesLoading: boolean;

  // Provider setup
  selectedProviderId: string;
  providerLabel: string;
  apiKey: string;
  baseUrl: string;
  config: Record<string, any> | undefined;
  isTesting: boolean;
  testResult: TestResult;
  isSubmittingProvider: boolean;

  // Model setup
  providerCredentials: ProviderCredential[];
  selectedCredential: ProviderCredential | null;
  modelName: string;
  displayName: string;
  modelLoading: boolean;
  isSavingModel: boolean;
  modelError: string | null;
  savedModelId: string | null;

  // Memory setup
  memoryType: MemoryType | null;
  isProcessingMemory: boolean;
}

export type OnboardingAction =
  // Navigation
  | { type: "SET_STEP"; payload: OnboardingStep }

  // Capabilities
  | { type: "SET_CAPABILITIES"; payload: ProviderCapabilitiesCamel[] }
  | { type: "SET_CAPABILITIES_LOADING"; payload: boolean }

  // Provider
  | {
      type: "SELECT_PROVIDER";
      payload: { providerId: string; label: string; baseUrl: string; config?: Record<string, any> };
    }
  | { type: "SET_PROVIDER_LABEL"; payload: string }
  | { type: "SET_API_KEY"; payload: string }
  | { type: "SET_BASE_URL"; payload: string }
  | { type: "SET_CONFIG"; payload: Record<string, any> | undefined }
  | { type: "SET_TESTING"; payload: boolean }
  | { type: "SET_TEST_RESULT"; payload: TestResult }
  | { type: "SET_SUBMITTING_PROVIDER"; payload: boolean }

  // Model
  | {
      type: "LOAD_CREDENTIALS";
      payload: {
        credentials: ProviderCredential[];
        selected: ProviderCredential | null;
        modelName: string;
        displayName: string;
      };
    }
  | { type: "SET_MODEL_LOADING"; payload: boolean }
  | { type: "SELECT_CREDENTIAL"; payload: { credential: ProviderCredential; defaultModel: string } }
  | { type: "SET_MODEL_NAME"; payload: string }
  | { type: "SET_DISPLAY_NAME"; payload: string }
  | { type: "SET_SAVING_MODEL"; payload: boolean }
  | { type: "SET_MODEL_ERROR"; payload: string | null }
  | { type: "SET_SAVED_MODEL_ID"; payload: string | null }

  // Memory
  | { type: "SET_MEMORY_TYPE"; payload: MemoryType | null }
  | { type: "SET_PROCESSING_MEMORY"; payload: boolean }

  // Reset
  | { type: "RESET" };

export const initialOnboardingState: OnboardingState = {
  step: OnboardingStep.Welcome,

  capabilities: [],
  capabilitiesLoading: true,

  selectedProviderId: "",
  providerLabel: "",
  apiKey: "",
  baseUrl: "",
  config: undefined,
  isTesting: false,
  testResult: null,
  isSubmittingProvider: false,

  providerCredentials: [],
  selectedCredential: null,
  modelName: "",
  displayName: "",
  modelLoading: true,
  isSavingModel: false,
  modelError: null,
  savedModelId: null,

  memoryType: null,
  isProcessingMemory: false,
};

export function onboardingReducer(
  state: OnboardingState,
  action: OnboardingAction,
): OnboardingState {
  switch (action.type) {
    // Navigation
    case "SET_STEP":
      return { ...state, step: action.payload };

    // Capabilities
    case "SET_CAPABILITIES":
      return { ...state, capabilities: action.payload, capabilitiesLoading: false };
    case "SET_CAPABILITIES_LOADING":
      return { ...state, capabilitiesLoading: action.payload };

    // Provider
    case "SELECT_PROVIDER":
      return {
        ...state,
        selectedProviderId: action.payload.providerId,
        providerLabel: action.payload.label,
        baseUrl: action.payload.baseUrl,
        config: action.payload.config,
        testResult: null,
      };
    case "SET_PROVIDER_LABEL":
      return { ...state, providerLabel: action.payload };
    case "SET_API_KEY":
      return { ...state, apiKey: action.payload };
    case "SET_BASE_URL":
      return { ...state, baseUrl: action.payload };
    case "SET_CONFIG":
      return { ...state, config: action.payload };
    case "SET_TESTING":
      return { ...state, isTesting: action.payload };
    case "SET_TEST_RESULT":
      return { ...state, testResult: action.payload };
    case "SET_SUBMITTING_PROVIDER":
      return { ...state, isSubmittingProvider: action.payload };

    // Model
    case "LOAD_CREDENTIALS":
      return {
        ...state,
        providerCredentials: action.payload.credentials,
        selectedCredential: action.payload.selected,
        modelName: action.payload.modelName,
        displayName: action.payload.displayName,
        modelLoading: false,
        modelError: null,
      };
    case "SET_MODEL_LOADING":
      return { ...state, modelLoading: action.payload };
    case "SELECT_CREDENTIAL":
      return {
        ...state,
        selectedCredential: action.payload.credential,
        modelName: action.payload.defaultModel,
        displayName: action.payload.defaultModel,
        modelError: null,
      };
    case "SET_MODEL_NAME":
      return { ...state, modelName: action.payload, modelError: null };
    case "SET_DISPLAY_NAME":
      return { ...state, displayName: action.payload };
    case "SET_SAVING_MODEL":
      return { ...state, isSavingModel: action.payload };
    case "SET_MODEL_ERROR":
      return { ...state, modelError: action.payload };
    case "SET_SAVED_MODEL_ID":
      return { ...state, savedModelId: action.payload };

    // Memory
    case "SET_MEMORY_TYPE":
      return { ...state, memoryType: action.payload };
    case "SET_PROCESSING_MEMORY":
      return { ...state, isProcessingMemory: action.payload };

    // Reset
    case "RESET":
      return initialOnboardingState;

    default:
      return state;
  }
}

// Helper functions
export function getDefaultBaseUrl(providerId: string): string {
  switch (providerId) {
    case "intenserp":
      return "http://127.0.0.1:7777/v1";
    case "chutes":
      return "https://llm.chutes.ai";
    case "featherless":
      return "https://api.featherless.ai/v1";
    case "openrouter":
      return "https://openrouter.ai/api";
    case "openai":
      return "https://api.openai.com";
    case "anthropic":
      return "https://api.anthropic.com";
    default:
      return "";
  }
}

export function getDefaultModelName(providerId: string): string {
  switch (providerId) {
    case "intenserp":
      return "deepseek-auto";
    case "openrouter":
      return "deepseek/deepseek-v3.2";
    case "mistral":
      return "mistral-large-latest";
    case "gemini":
      return "gemini-3-flash-preview";
    default:
      return "";
  }
}
