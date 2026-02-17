import type { ProviderCredential } from "../../../../core/storage/schemas";
import type { ProviderCapabilitiesCamel } from "../../../../core/providers/capabilities";

export type EngineSetupResult = {
  credentialId: string;
  needsSetup: boolean;
} | null;

export type ProvidersPageState = {
  providers: ProviderCredential[];
  selectedProvider: ProviderCredential | null;
  isEditorOpen: boolean;
  editorProvider: ProviderCredential | null;
  apiKey: string;
  isSaving: boolean;
  isDeleting: boolean;
  validationError: string | null;
  capabilities: ProviderCapabilitiesCamel[];
  engineSetupResult: EngineSetupResult;
};

export type ProvidersPageAction =
  | { type: "set_providers"; payload: ProviderCredential[] }
  | { type: "set_selected_provider"; payload: ProviderCredential | null }
  | {
      type: "open_editor";
      payload: { provider: ProviderCredential; apiKey: string };
    }
  | { type: "close_editor" }
  | { type: "update_editor_provider"; payload: Partial<ProviderCredential> }
  | { type: "set_api_key"; payload: string }
  | { type: "set_is_saving"; payload: boolean }
  | { type: "set_is_deleting"; payload: boolean }
  | { type: "set_validation_error"; payload: string | null }
  | { type: "set_capabilities"; payload: ProviderCapabilitiesCamel[] }
  | { type: "set_engine_setup_result"; payload: EngineSetupResult };

export const initialProvidersPageState: ProvidersPageState = {
  providers: [],
  selectedProvider: null,
  isEditorOpen: false,
  editorProvider: null,
  apiKey: "",
  isSaving: false,
  isDeleting: false,
  validationError: null,
  capabilities: [],
  engineSetupResult: null,
};

export function providersPageReducer(
  state: ProvidersPageState,
  action: ProvidersPageAction,
): ProvidersPageState {
  switch (action.type) {
    case "set_providers":
      return {
        ...state,
        providers: action.payload,
      };
    case "set_selected_provider":
      return {
        ...state,
        selectedProvider: action.payload,
      };
    case "open_editor":
      return {
        ...state,
        isEditorOpen: true,
        editorProvider: action.payload.provider,
        apiKey: action.payload.apiKey,
        validationError: null,
      };
    case "close_editor":
      return {
        ...state,
        isEditorOpen: false,
        editorProvider: null,
        apiKey: "",
        validationError: null,
      };
    case "update_editor_provider":
      return state.editorProvider
        ? {
            ...state,
            editorProvider: {
              ...state.editorProvider,
              ...action.payload,
            },
          }
        : state;
    case "set_api_key":
      return {
        ...state,
        apiKey: action.payload,
      };
    case "set_is_saving":
      return {
        ...state,
        isSaving: action.payload,
      };
    case "set_is_deleting":
      return {
        ...state,
        isDeleting: action.payload,
      };
    case "set_validation_error":
      return {
        ...state,
        validationError: action.payload,
      };
    case "set_capabilities":
      return {
        ...state,
        capabilities: action.payload,
      };
    case "set_engine_setup_result":
      return {
        ...state,
        engineSetupResult: action.payload,
      };
    default:
      return state;
  }
}
