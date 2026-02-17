import type { CharacterActivity, UsageResponse } from "../../../../core/engine/types";

export type CharacterCard = {
  slug: string;
  name: string;
  role?: string;
  era?: string;
  loaded: boolean;
};

export type EngineHomeState = {
  loading: boolean;
  error: string | null;
  version: string | null;
  connected: boolean;
  needsSetup: boolean;
  characters: CharacterCard[];
  usage: UsageResponse | null;
  activities: Record<string, CharacterActivity>;
  togglingSlug: string | null;
  selectedCharacter: string | null;
  deletingSlug: string | null;
};

export const initialEngineHomeState: EngineHomeState = {
  loading: true,
  error: null,
  version: null,
  connected: false,
  needsSetup: false,
  characters: [],
  usage: null,
  activities: {},
  togglingSlug: null,
  selectedCharacter: null,
  deletingSlug: null,
};

export type EngineHomeAction =
  | { type: "set_loading"; payload: boolean }
  | { type: "set_error"; payload: string | null }
  | { type: "set_health"; payload: { version: string | null; connected: boolean } }
  | { type: "set_needs_setup"; payload: boolean }
  | { type: "set_characters"; payload: CharacterCard[] }
  | { type: "set_usage"; payload: UsageResponse | null }
  | { type: "set_activity"; payload: { slug: string; activity: CharacterActivity } }
  | { type: "set_toggling_slug"; payload: string | null }
  | {
      type: "toggle_character_loaded";
      payload: { slug: string; loaded: boolean };
    }
  | { type: "set_selected_character"; payload: string | null }
  | { type: "set_deleting_slug"; payload: string | null }
  | { type: "remove_character"; payload: string };

export function engineHomeReducer(
  state: EngineHomeState,
  action: EngineHomeAction,
): EngineHomeState {
  switch (action.type) {
    case "set_loading":
      return { ...state, loading: action.payload };
    case "set_error":
      return { ...state, error: action.payload };
    case "set_health":
      return {
        ...state,
        version: action.payload.version,
        connected: action.payload.connected,
      };
    case "set_needs_setup":
      return { ...state, needsSetup: action.payload };
    case "set_characters":
      return { ...state, characters: action.payload };
    case "set_usage":
      return { ...state, usage: action.payload };
    case "set_activity":
      return {
        ...state,
        activities: {
          ...state.activities,
          [action.payload.slug]: action.payload.activity,
        },
      };
    case "set_toggling_slug":
      return { ...state, togglingSlug: action.payload };
    case "toggle_character_loaded":
      return {
        ...state,
        characters: state.characters.map((c) =>
          c.slug === action.payload.slug ? { ...c, loaded: action.payload.loaded } : c,
        ),
      };
    case "set_selected_character":
      return { ...state, selectedCharacter: action.payload };
    case "set_deleting_slug":
      return { ...state, deletingSlug: action.payload };
    case "remove_character":
      return {
        ...state,
        characters: state.characters.filter((c) => c.slug !== action.payload),
        selectedCharacter: null,
      };
    default:
      return state;
  }
}
