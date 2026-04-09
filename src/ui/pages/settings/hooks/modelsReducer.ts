import type {
  Model,
  ProviderCredential,
} from "../../../../core/storage/schemas";

export type ModelsState = {
  providers: ProviderCredential[];
  models: Model[];
  defaultModelId: string | null;
  loading: boolean;
};

export type ModelsAction =
  | {
    type: "load_success";
    payload: {
      providers: ProviderCredential[];
      models: Model[];
      defaultModelId: string | null;
    };
  }
  | { type: "set_models"; payload: Model[] }
  | { type: "set_providers"; payload: ProviderCredential[] }
  | { type: "set_default_model_id"; payload: string | null };

export const initialModelsState: ModelsState = {
  providers: [],
  models: [],
  defaultModelId: null,
  loading: true,
};

export function modelsReducer(
  state: ModelsState,
  action: ModelsAction,
): ModelsState {
  switch (action.type) {
    case "load_success":
      return {
        ...state,
        providers: action.payload.providers,
        models: action.payload.models,
        defaultModelId: action.payload.defaultModelId,
        loading: false,
      };
    case "set_models":
      return {
        ...state,
        models: action.payload,
      };
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
    default:
      return state;
  }
}
