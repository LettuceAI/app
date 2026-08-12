import type { Model, ProviderCredential, AdvancedModelSettings } from "../../../../core/storage/schemas";


export type ModelInfo = {
  id: string;
  displayName?: string;
  description?: string;
  contextLength?: number;
  inputModalities?: string[];
  outputModalities?: string[];
  supportedEndpoints?: string[];
  inputPrice?: number;
  outputPrice?: number;
};

export type ModelEditorState = {
  loading: boolean;
  saving: boolean;
  deleting: boolean;
  verifying: boolean;
  fetchingModels: boolean;
  fetchedModels: ModelInfo[];
  error: string | null;
  providers: ProviderCredential[];
  defaultModelId: string | null;
  editorModel: Model | null;
  modelAdvancedDraft: AdvancedModelSettings;
};

export type ModelEditorAction =
  | { type: "set_loading"; payload: boolean }
  | { type: "set_saving"; payload: boolean }
  | { type: "set_deleting"; payload: boolean }
  | { type: "set_verifying"; payload: boolean }
  | { type: "set_fetching_models"; payload: boolean }
  | { type: "set_fetched_models"; payload: ModelInfo[] }
  | { type: "set_error"; payload: string | null }
  | {
    type: "load_success";
    payload: {
      providers: ProviderCredential[];
      defaultModelId: string | null;
      editorModel: Model | null;
      modelAdvancedDraft: AdvancedModelSettings;
    };
  }
  | { type: "update_editor_model"; payload: Partial<Model> }
  | { type: "set_providers"; payload: ProviderCredential[] }
  | { type: "set_default_model_id"; payload: string | null }
  | { type: "set_model_advanced_draft"; payload: AdvancedModelSettings };

export const initialModelEditorState: ModelEditorState = {
  loading: true,
  saving: false,
  deleting: false,
  verifying: false,
  fetchingModels: false,
  fetchedModels: [],
  error: null,
  providers: [],
  defaultModelId: null,
  editorModel: null,
  modelAdvancedDraft: {} as AdvancedModelSettings,
};

export function modelEditorReducer(
  state: ModelEditorState,
  action: ModelEditorAction,
): ModelEditorState {
  switch (action.type) {
    case "set_loading":
      return { ...state, loading: action.payload };
    case "set_saving":
      return { ...state, saving: action.payload };
    case "set_deleting":
      return { ...state, deleting: action.payload };
    case "set_verifying":
      return { ...state, verifying: action.payload };
    case "set_fetching_models":
      return { ...state, fetchingModels: action.payload };
    case "set_fetched_models":
      return { ...state, fetchedModels: action.payload };
    case "set_error":
      return { ...state, error: action.payload };
    case "load_success":
      return {
        ...state,
        providers: action.payload.providers,
        defaultModelId: action.payload.defaultModelId,
        editorModel: action.payload.editorModel,
        modelAdvancedDraft: action.payload.modelAdvancedDraft,
        loading: false,
      };
    case "update_editor_model":
      return state.editorModel
        ? {
          ...state,
          editorModel: {
            ...state.editorModel,
            ...action.payload,
          },
        }
        : state;
    case "set_providers":
      return {
        ...state,
        providers: action.payload,
      };
    case "set_default_model_id":
      return {
        ...state,
        defaultModelId: action.payload,
      };
    case "set_model_advanced_draft":
      return {
        ...state,
        modelAdvancedDraft: action.payload,
      };
    default:
      return state;
  }
}
