import type { ProviderCredential } from "../../../../core/storage/schemas";

export type ModelState = {
  providers: ProviderCredential[];
  selectedProvider: ProviderCredential | null;
  modelName: string;
  displayName: string;
  isLoading: boolean;
  isSaving: boolean;
  verificationError: string | null;
};

export type ModelAction =
  | {
      type: "LOAD_SUCCESS";
      payload: {
        providers: ProviderCredential[];
        selectedProvider: ProviderCredential | null;
        modelName: string;
        displayName: string;
      };
    }
  | { type: "LOAD_ERROR" }
  | {
      type: "SELECT_PROVIDER";
      payload: { provider: ProviderCredential; defaultModel: string };
    }
  | { type: "SET_MODEL_NAME"; payload: string }
  | { type: "SET_DISPLAY_NAME"; payload: string }
  | { type: "SET_SAVING"; payload: boolean }
  | { type: "SET_VERIFICATION_ERROR"; payload: string | null };

export const initialModelState: ModelState = {
  providers: [],
  selectedProvider: null,
  modelName: "",
  displayName: "",
  isLoading: true,
  isSaving: false,
  verificationError: null,
};

export function modelReducer(state: ModelState, action: ModelAction): ModelState {
  switch (action.type) {
    case "LOAD_SUCCESS": {
      const { providers, selectedProvider, modelName, displayName } = action.payload;
      return {
        ...state,
        providers,
        selectedProvider,
        modelName,
        displayName,
        isLoading: false,
        verificationError: null,
      };
    }
    case "LOAD_ERROR":
      return {
        ...state,
        providers: [],
        selectedProvider: null,
        modelName: "",
        displayName: "",
        isLoading: false,
      };
    case "SELECT_PROVIDER": {
      const { provider, defaultModel } = action.payload;
      return {
        ...state,
        selectedProvider: provider,
        modelName: defaultModel,
        displayName: defaultModel,
        verificationError: null,
      };
    }
    case "SET_MODEL_NAME":
      return {
        ...state,
        modelName: action.payload,
        verificationError: null,
      };
    case "SET_DISPLAY_NAME":
      return {
        ...state,
        displayName: action.payload,
      };
    case "SET_SAVING":
      return {
        ...state,
        isSaving: action.payload,
      };
    case "SET_VERIFICATION_ERROR":
      return {
        ...state,
        verificationError: action.payload,
      };
    default:
      return state;
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
