export type TestResult = { success: boolean; message: string } | null;

export type ProviderState = {
  selectedProviderId: string;
  label: string;
  apiKey: string;
  baseUrl: string;
  isTesting: boolean;
  testResult: TestResult;
  isSubmitting: boolean;
  showForm: boolean;
};

export type ProviderAction =
  | { type: "select_provider"; payload: { providerId: string; label: string; baseUrl: string } }
  | { type: "set_label"; payload: string }
  | { type: "set_api_key"; payload: string }
  | { type: "set_base_url"; payload: string }
  | { type: "set_is_testing"; payload: boolean }
  | { type: "set_test_result"; payload: TestResult }
  | { type: "set_is_submitting"; payload: boolean }
  | { type: "set_show_form"; payload: boolean };

export const initialProviderState: ProviderState = {
  selectedProviderId: "",
  label: "",
  apiKey: "",
  baseUrl: "",
  isTesting: false,
  testResult: null,
  isSubmitting: false,
  showForm: false,
};

export function providerReducer(state: ProviderState, action: ProviderAction): ProviderState {
  switch (action.type) {
    case "select_provider":
      return {
        ...state,
        selectedProviderId: action.payload.providerId,
        label: action.payload.label,
        baseUrl: action.payload.baseUrl,
        showForm: true,
        testResult: null,
      };
    case "set_label":
      return { ...state, label: action.payload };
    case "set_api_key":
      return { ...state, apiKey: action.payload };
    case "set_base_url":
      return { ...state, baseUrl: action.payload };
    case "set_is_testing":
      return { ...state, isTesting: action.payload };
    case "set_test_result":
      return { ...state, testResult: action.payload };
    case "set_is_submitting":
      return { ...state, isSubmitting: action.payload };
    case "set_show_form":
      return { ...state, showForm: action.payload };
    default:
      return state;
  }
}

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
    case "cerebras":
      return "https://api.cerebras.ai/v1";
    case "anthropic":
      return "https://api.anthropic.com";
    default:
      return "";
  }
}
