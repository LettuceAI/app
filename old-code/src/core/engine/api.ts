import { invoke } from "@tauri-apps/api/core";
import type {
  HealthResponse,
  SetupStatusResponse,
  ConfigLlmRequest,
  ConfigLlmDefaultRequest,
  ConfigEngineRequest,
  ConfigBackgroundRequest,
  ConfigMemoryRequest,
  ConfigSafetyRequest,
  ConfigResearchRequest,
} from "./types";

// ── Health & Setup ──────────────────────────────────────────────────────────

export function engineHealth(baseUrl: string, apiKey?: string): Promise<HealthResponse> {
  return invoke("engine_health", { baseUrl, apiKey: apiKey ?? null });
}

export function engineSetupStatus(
  baseUrl: string,
  apiKey?: string,
): Promise<SetupStatusResponse> {
  return invoke("engine_setup_status", { baseUrl, apiKey: apiKey ?? null });
}

export function engineSetupComplete(
  baseUrl: string,
  apiKey?: string,
): Promise<{ status: string }> {
  return invoke("engine_setup_complete", { baseUrl, apiKey: apiKey ?? null });
}

// ── Config ──────────────────────────────────────────────────────────────────

export function engineConfigLlm(
  baseUrl: string,
  apiKey: string,
  provider: string,
  config: ConfigLlmRequest,
): Promise<unknown> {
  return invoke("engine_config_llm", { baseUrl, apiKey, provider, config });
}

export function engineConfigLlmDefault(
  baseUrl: string,
  apiKey: string,
  config: ConfigLlmDefaultRequest,
): Promise<unknown> {
  return invoke("engine_config_llm_default", { baseUrl, apiKey, config });
}

export function engineConfigEngine(
  baseUrl: string,
  apiKey: string,
  config: ConfigEngineRequest,
): Promise<unknown> {
  return invoke("engine_config_engine", { baseUrl, apiKey, config });
}

export function engineConfigBackground(
  baseUrl: string,
  apiKey: string,
  config: ConfigBackgroundRequest,
): Promise<unknown> {
  return invoke("engine_config_background", { baseUrl, apiKey, config });
}

export function engineConfigMemory(
  baseUrl: string,
  apiKey: string,
  config: ConfigMemoryRequest,
): Promise<unknown> {
  return invoke("engine_config_memory", { baseUrl, apiKey, config });
}

export function engineConfigSafety(
  baseUrl: string,
  apiKey: string,
  config: ConfigSafetyRequest,
): Promise<unknown> {
  return invoke("engine_config_safety", { baseUrl, apiKey, config });
}

export function engineConfigResearch(
  baseUrl: string,
  apiKey: string,
  config: ConfigResearchRequest,
): Promise<unknown> {
  return invoke("engine_config_research", { baseUrl, apiKey, config });
}

export function engineConfigLlmDelete(
  baseUrl: string,
  apiKey: string,
  provider: string,
): Promise<unknown> {
  return invoke("engine_config_llm_delete", { baseUrl, apiKey, provider });
}

// ── Status & Usage ──────────────────────────────────────────────────────────

export function engineStatus(baseUrl: string, apiKey: string): Promise<unknown> {
  return invoke("engine_status", { baseUrl, apiKey });
}

export function engineUsage(baseUrl: string, apiKey: string): Promise<unknown> {
  return invoke("engine_usage", { baseUrl, apiKey });
}

export function engineGetConfig(baseUrl: string, apiKey: string): Promise<unknown> {
  return invoke("engine_get_config", { baseUrl, apiKey });
}

// ── Characters ──────────────────────────────────────────────────────────────

export function engineCharactersList(baseUrl: string, apiKey: string): Promise<unknown> {
  return invoke("engine_characters_list", { baseUrl, apiKey });
}

export function engineCharacterLoad(
  baseUrl: string,
  apiKey: string,
  slug: string,
): Promise<unknown> {
  return invoke("engine_character_load", { baseUrl, apiKey, slug });
}

export function engineCharacterUnload(
  baseUrl: string,
  apiKey: string,
  slug: string,
): Promise<unknown> {
  return invoke("engine_character_unload", { baseUrl, apiKey, slug });
}

export function engineCharacterActivity(
  baseUrl: string,
  apiKey: string,
  slug: string,
): Promise<unknown> {
  return invoke("engine_character_activity", { baseUrl, apiKey, slug });
}

export function engineCharacterTemplate(
  baseUrl: string,
  apiKey: string,
): Promise<unknown> {
  return invoke("engine_character_template", { baseUrl, apiKey });
}

export function engineCharacterBoost(
  baseUrl: string,
  apiKey: string,
  body: { name?: string; seed: string; era?: string },
): Promise<unknown> {
  return invoke("engine_character_boost", { baseUrl, apiKey, body });
}

export function engineCharacterCreate(
  baseUrl: string,
  apiKey: string,
  body: Record<string, unknown>,
): Promise<unknown> {
  return invoke("engine_character_create", { baseUrl, apiKey, body });
}

export function engineCharacterFull(
  baseUrl: string,
  apiKey: string,
  slug: string,
): Promise<unknown> {
  return invoke("engine_character_full", { baseUrl, apiKey, slug });
}

export function engineCharacterUpdate(
  baseUrl: string,
  apiKey: string,
  slug: string,
  body: Record<string, unknown>,
): Promise<unknown> {
  return invoke("engine_character_update", { baseUrl, apiKey, slug, body });
}

// ── Chat ───────────────────────────────────────────────────────────────────

export function engineChat(
  baseUrl: string,
  apiKey: string,
  slug: string,
  body: { message: string; user_id: string; user_name: string; user_description: string },
): Promise<unknown> {
  return invoke("engine_chat", { baseUrl, apiKey, slug, body });
}

export function engineChatHistory(
  baseUrl: string,
  apiKey: string,
  slug: string,
  userId: string,
  limit?: number,
): Promise<unknown> {
  return invoke("engine_chat_history", { baseUrl, apiKey, slug, userId, limit: limit ?? null });
}

// ── Character Delete ───────────────────────────────────────────────────────

export function engineCharacterDeleteCmd(
  baseUrl: string,
  apiKey: string,
  slug: string,
): Promise<unknown> {
  return invoke("engine_character_delete_cmd", { baseUrl, apiKey, slug });
}
