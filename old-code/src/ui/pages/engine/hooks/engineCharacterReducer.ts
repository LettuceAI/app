import type {
  SpeechPatterns,
  TimeBehaviors,
  BaselineEmotions,
  CharacterCreateRequest,
} from "../../../../core/engine/types";

export type EngineCharacterStep = "mode" | "identity" | "personality" | "world" | "review";

export const STEP_ORDER: EngineCharacterStep[] = [
  "mode",
  "identity",
  "personality",
  "world",
  "review",
];

export type EngineCharacterState = {
  step: EngineCharacterStep;
  // Boost
  boostSeed: string;
  boostName: string;
  boostEra: string;
  boosting: boolean;
  boostError: string | null;
  boosted: boolean;
  // Identity
  name: string;
  era: string;
  setting: string;
  role: string;
  coreIdentity: string;
  backstory: string;
  // Personality
  personalityTraits: string[];
  speechPatterns: SpeechPatterns;
  // Knowledge
  knowledgeDomains: string[];
  knowledgeBoundaries: string[];
  researchSeeds: string[];
  researchEnabled: boolean;
  // Physical
  physicalDescription: string;
  physicalHabits: string[];
  idleBehaviors: string[];
  timeBehaviors: TimeBehaviors;
  // Emotions
  baselineEmotions: BaselineEmotions;
  // Engine overrides
  backend: string;
  model: string;
  temperature: number;
  // UI
  saving: boolean;
  error: string | null;
};

export const initialCharacterState: EngineCharacterState = {
  step: "mode",
  boostSeed: "",
  boostName: "",
  boostEra: "",
  boosting: false,
  boostError: null,
  boosted: false,
  name: "",
  era: "",
  setting: "",
  role: "",
  coreIdentity: "",
  backstory: "",
  personalityTraits: [],
  speechPatterns: {},
  knowledgeDomains: [],
  knowledgeBoundaries: [],
  researchSeeds: [],
  researchEnabled: false,
  physicalDescription: "",
  physicalHabits: [],
  idleBehaviors: [],
  timeBehaviors: {},
  baselineEmotions: {},
  backend: "",
  model: "",
  temperature: 0.9,
  saving: false,
  error: null,
};

export type EngineCharacterAction =
  | { type: "set_step"; payload: EngineCharacterStep }
  | { type: "set_boost_field"; payload: { field: "boostSeed" | "boostName" | "boostEra"; value: string } }
  | { type: "set_boosting"; payload: boolean }
  | { type: "set_boost_error"; payload: string | null }
  | { type: "populate_from_boost"; payload: CharacterCreateRequest }
  | { type: "set_field"; payload: { field: string; value: unknown } }
  | { type: "set_speech_pattern"; payload: { field: string; value: unknown } }
  | { type: "set_time_behavior"; payload: { field: string; value: string } }
  | { type: "set_emotion"; payload: { field: string; value: number } }
  | { type: "set_saving"; payload: boolean }
  | { type: "set_error"; payload: string | null };

export function engineCharacterReducer(
  state: EngineCharacterState,
  action: EngineCharacterAction,
): EngineCharacterState {
  switch (action.type) {
    case "set_step":
      return { ...state, step: action.payload, error: null };
    case "set_boost_field":
      return { ...state, [action.payload.field]: action.payload.value };
    case "set_boosting":
      return { ...state, boosting: action.payload };
    case "set_boost_error":
      return { ...state, boostError: action.payload };
    case "populate_from_boost": {
      const c = action.payload;
      return {
        ...state,
        boosted: true,
        name: c.name || "",
        era: c.era || "",
        setting: c.setting || "",
        role: c.role || "",
        coreIdentity: c.core_identity || "",
        backstory: c.backstory || "",
        personalityTraits: c.personality_traits || [],
        speechPatterns: c.speech_patterns || {},
        knowledgeDomains: c.knowledge_domains || [],
        knowledgeBoundaries: c.knowledge_boundaries || [],
        researchSeeds: c.research_seeds || [],
        researchEnabled: c.research_enabled ?? false,
        physicalDescription: c.physical_description || "",
        physicalHabits: c.physical_habits || [],
        idleBehaviors: c.idle_behaviors || [],
        timeBehaviors: c.time_behaviors || {},
        baselineEmotions: c.baseline_emotions || {},
        backend: c.backend || "",
        model: c.model || "",
        temperature: c.temperature ?? 0.9,
        step: "identity",
      };
    }
    case "set_field":
      return { ...state, [action.payload.field]: action.payload.value };
    case "set_speech_pattern":
      return {
        ...state,
        speechPatterns: {
          ...state.speechPatterns,
          [action.payload.field]: action.payload.value,
        },
      };
    case "set_time_behavior":
      return {
        ...state,
        timeBehaviors: {
          ...state.timeBehaviors,
          [action.payload.field]: action.payload.value,
        },
      };
    case "set_emotion":
      return {
        ...state,
        baselineEmotions: {
          ...state.baselineEmotions,
          [action.payload.field]: action.payload.value,
        },
      };
    case "set_saving":
      return { ...state, saving: action.payload };
    case "set_error":
      return { ...state, error: action.payload };
    default:
      return state;
  }
}
