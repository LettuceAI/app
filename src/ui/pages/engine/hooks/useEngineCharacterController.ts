import { useCallback, useReducer } from "react";
import { useNavigate } from "react-router-dom";

import {
  engineCharacterReducer,
  initialCharacterState,
  type EngineCharacterStep,
  type EngineCharacterState,
} from "./engineCharacterReducer";
import {
  engineCharacterBoost,
  engineCharacterCreate,
} from "../../../../core/engine/api";
import type { CharacterCreateRequest } from "../../../../core/engine/types";
import { Routes } from "../../../navigation";

function buildCreateRequest(s: EngineCharacterState): Record<string, unknown> {
  const req: Record<string, unknown> = { name: s.name.trim() };

  if (s.era.trim()) req.era = s.era.trim();
  if (s.setting.trim()) req.setting = s.setting.trim();
  if (s.role.trim()) req.role = s.role.trim();
  if (s.coreIdentity.trim()) req.core_identity = s.coreIdentity.trim();
  if (s.backstory.trim()) req.backstory = s.backstory.trim();
  if (s.personalityTraits.length > 0) req.personality_traits = s.personalityTraits;
  if (Object.keys(s.speechPatterns).length > 0) req.speech_patterns = s.speechPatterns;
  if (s.knowledgeDomains.length > 0) req.knowledge_domains = s.knowledgeDomains;
  if (s.knowledgeBoundaries.length > 0) req.knowledge_boundaries = s.knowledgeBoundaries;
  if (s.researchSeeds.length > 0) req.research_seeds = s.researchSeeds;
  if (s.researchEnabled) req.research_enabled = true;
  if (s.physicalDescription.trim()) req.physical_description = s.physicalDescription.trim();
  if (s.physicalHabits.length > 0) req.physical_habits = s.physicalHabits;
  if (s.idleBehaviors.length > 0) req.idle_behaviors = s.idleBehaviors;
  if (Object.keys(s.timeBehaviors).length > 0) req.time_behaviors = s.timeBehaviors;
  if (Object.keys(s.baselineEmotions).length > 0) req.baseline_emotions = s.baselineEmotions;
  if (s.backend.trim()) req.backend = s.backend.trim();
  if (s.model.trim()) req.model = s.model.trim();
  if (s.temperature !== 0.9) req.temperature = s.temperature;

  return req;
}

export function useEngineCharacterController(
  baseUrl: string,
  apiKey: string,
  credentialId: string,
) {
  const [state, dispatch] = useReducer(engineCharacterReducer, initialCharacterState);
  const navigate = useNavigate();

  const setStep = useCallback((step: EngineCharacterStep) => {
    dispatch({ type: "set_step", payload: step });
  }, []);

  const setBoostField = useCallback(
    (field: "boostSeed" | "boostName" | "boostEra", value: string) => {
      dispatch({ type: "set_boost_field", payload: { field, value } });
    },
    [],
  );

  const setField = useCallback((field: string, value: unknown) => {
    dispatch({ type: "set_field", payload: { field, value } });
  }, []);

  const setSpeechPattern = useCallback((field: string, value: unknown) => {
    dispatch({ type: "set_speech_pattern", payload: { field, value } });
  }, []);

  const setTimeBehavior = useCallback((field: string, value: string) => {
    dispatch({ type: "set_time_behavior", payload: { field, value } });
  }, []);

  const setEmotion = useCallback((field: string, value: number) => {
    dispatch({ type: "set_emotion", payload: { field, value } });
  }, []);

  const boost = useCallback(async () => {
    if (!state.boostSeed.trim()) {
      dispatch({ type: "set_boost_error", payload: "Seed description is required." });
      return;
    }
    dispatch({ type: "set_boosting", payload: true });
    dispatch({ type: "set_boost_error", payload: null });
    try {
      const result = (await engineCharacterBoost(baseUrl, apiKey, {
        name: state.boostName.trim() || undefined,
        seed: state.boostSeed.trim(),
        era: state.boostEra.trim() || undefined,
      })) as { character?: CharacterCreateRequest } & CharacterCreateRequest;
      // The API may return { character: {...} } or the character directly
      const character = result.character ?? result;
      dispatch({ type: "populate_from_boost", payload: character });
    } catch (e) {
      dispatch({
        type: "set_boost_error",
        payload: e instanceof Error ? e.message : String(e),
      });
    } finally {
      dispatch({ type: "set_boosting", payload: false });
    }
  }, [baseUrl, apiKey, state.boostSeed, state.boostName, state.boostEra]);

  const save = useCallback(async () => {
    if (!state.name.trim()) {
      dispatch({ type: "set_error", payload: "Character name is required." });
      return false;
    }
    dispatch({ type: "set_saving", payload: true });
    dispatch({ type: "set_error", payload: null });
    try {
      const body = buildCreateRequest(state);
      await engineCharacterCreate(baseUrl, apiKey, body);
      navigate(Routes.engineHome(credentialId), { replace: true });
      return true;
    } catch (e) {
      dispatch({
        type: "set_error",
        payload: e instanceof Error ? e.message : String(e),
      });
      return false;
    } finally {
      dispatch({ type: "set_saving", payload: false });
    }
  }, [baseUrl, apiKey, credentialId, state, navigate]);

  return {
    state,
    setStep,
    setBoostField,
    setField,
    setSpeechPattern,
    setTimeBehavior,
    setEmotion,
    boost,
    save,
  };
}
