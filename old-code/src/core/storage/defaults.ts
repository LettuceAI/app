import { invoke } from "@tauri-apps/api/core";

/**
 * Get default character rules from Rust backend
 * Single source of truth for default rules
 * @param pureModeLevel - Pure Mode level ("off", "low", "standard", "strict")
 */
export async function getDefaultCharacterRules(pureModeLevel: string): Promise<string[]> {
  return invoke<string[]>("get_default_character_rules", { pureModeLevel });
}
