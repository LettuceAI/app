import { useCallback, useEffect, useReducer } from "react";

import {
  engineHomeReducer,
  initialEngineHomeState,
  type CharacterCard,
} from "./engineHomeReducer";
import {
  engineHealth,
  engineSetupStatus,
  engineStatus,
  engineUsage,
  engineCharacterLoad,
  engineCharacterUnload,
  engineCharacterActivity,
  engineCharacterDeleteCmd,
} from "../../../../core/engine/api";
import type { CharacterActivity, UsageResponse } from "../../../../core/engine/types";

export function useEngineHomeController(baseUrl: string, apiKey: string) {
  const [state, dispatch] = useReducer(engineHomeReducer, initialEngineHomeState);

  const load = useCallback(async () => {
    dispatch({ type: "set_loading", payload: true });
    dispatch({ type: "set_error", payload: null });

    try {
      // Health check
      let version: string | null = null;
      try {
        const health = await engineHealth(baseUrl, apiKey);
        version = health.version ?? null;
        dispatch({ type: "set_health", payload: { version, connected: true } });
      } catch {
        dispatch({ type: "set_health", payload: { version: null, connected: false } });
        dispatch({ type: "set_error", payload: "Engine is offline or unreachable." });
        return;
      }

      // Setup status
      try {
        const setup = await engineSetupStatus(baseUrl, apiKey);
        dispatch({ type: "set_needs_setup", payload: setup.needs_setup });
      } catch {
        // Non-critical
      }

      // Characters from status endpoint
      try {
        const statusData = (await engineStatus(baseUrl, apiKey)) as Record<string, unknown>;
        const chars = statusData.characters;
        if (Array.isArray(chars)) {
          const cards: CharacterCard[] = chars.map((c: Record<string, unknown>) => ({
            slug: String(c.slug ?? c.name ?? ""),
            name: String(c.name ?? c.slug ?? "Unknown"),
            role: c.role ? String(c.role) : undefined,
            era: c.era ? String(c.era) : undefined,
            loaded: c.loaded === true,
          }));
          dispatch({ type: "set_characters", payload: cards });

          // Fetch activity for loaded characters
          for (const card of cards) {
            if (card.loaded) {
              try {
                const activity = (await engineCharacterActivity(
                  baseUrl,
                  apiKey,
                  card.slug,
                )) as CharacterActivity;
                dispatch({
                  type: "set_activity",
                  payload: { slug: card.slug, activity },
                });
              } catch {
                // Non-critical
              }
            }
          }
        }
      } catch {
        // Non-critical — characters may not exist yet
      }

      // Usage
      try {
        const usage = (await engineUsage(baseUrl, apiKey)) as UsageResponse;
        dispatch({ type: "set_usage", payload: usage });
      } catch {
        // Non-critical
      }
    } finally {
      dispatch({ type: "set_loading", payload: false });
    }
  }, [baseUrl, apiKey]);

  useEffect(() => {
    void load();
  }, [load]);

  const toggleCharacter = useCallback(
    async (slug: string, currentlyLoaded: boolean) => {
      dispatch({ type: "set_toggling_slug", payload: slug });
      try {
        if (currentlyLoaded) {
          await engineCharacterUnload(baseUrl, apiKey, slug);
        } else {
          await engineCharacterLoad(baseUrl, apiKey, slug);
        }
        dispatch({
          type: "toggle_character_loaded",
          payload: { slug, loaded: !currentlyLoaded },
        });
      } catch {
        // Ignore errors — state remains unchanged
      } finally {
        dispatch({ type: "set_toggling_slug", payload: null });
      }
    },
    [baseUrl, apiKey],
  );

  const selectCharacter = useCallback((slug: string | null) => {
    dispatch({ type: "set_selected_character", payload: slug });
  }, []);

  const deleteCharacter = useCallback(
    async (slug: string) => {
      dispatch({ type: "set_deleting_slug", payload: slug });
      try {
        await engineCharacterDeleteCmd(baseUrl, apiKey, slug);
        dispatch({ type: "remove_character", payload: slug });
      } catch {
        dispatch({ type: "set_error", payload: "Failed to delete character." });
        dispatch({ type: "set_selected_character", payload: null });
      } finally {
        dispatch({ type: "set_deleting_slug", payload: null });
      }
    },
    [baseUrl, apiKey],
  );

  return { state, reload: load, toggleCharacter, selectCharacter, deleteCharacter };
}
