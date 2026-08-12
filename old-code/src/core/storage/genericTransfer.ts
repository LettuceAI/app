import { invoke } from "@tauri-apps/api/core";
import type { Character, Persona } from "./schemas";

/**
 * Result from generic import that includes type detection
 */
export interface GenericImportResult {
  importType: "character" | "persona";
  data: Character | Persona;
}

/**
 * Import either a character or persona from a JSON package
 * Automatically detects the type based on the "type" field in the JSON
 * Returns the imported entity with type information
 */
export async function importPackage(importJson: string): Promise<GenericImportResult> {
  try {
    console.log("[importPackage] Importing package with auto-detection");
    const resultJson = await invoke<string>("import_package", { importJson });
    const result = JSON.parse(resultJson);

    console.log("[importPackage] Import successful, type:", result.importType);

    return {
      importType: result.importType,
      data: result,
    };
  } catch (error) {
    console.error("[importPackage] Failed to import package:", error);
    throw new Error(typeof error === "string" ? error : "Failed to import package");
  }
}

/**
 * Convert a legacy export package to a UEC JSON string.
 */
export async function convertImportToUec(importJson: string): Promise<string> {
  try {
    const convertedJson = await invoke<string>("convert_export_to_uec", { importJson });
    return convertedJson;
  } catch (error) {
    console.error("[convertImportToUec] Failed to convert package:", error);
    throw new Error(typeof error === "string" ? error : "Failed to convert package");
  }
}

/**
 * Convert a character export package to a target format.
 */
export async function convertImportToFormat(
  importJson: string,
  targetFormat: "chara_card_v2" | "chara_card_v3" | "uec",
): Promise<string> {
  try {
    const convertedJson = await invoke<string>("convert_export_to_format", {
      importJson,
      targetFormat,
    });
    return convertedJson;
  } catch (error) {
    console.error("[convertImportToFormat] Failed to convert package:", error);
    throw new Error(typeof error === "string" ? error : "Failed to convert package");
  }
}

/**
 * Helper to check if import JSON is a character export
 */
export function isCharacterExport(importJson: string): boolean {
  try {
    const parsed = JSON.parse(importJson);
    if (parsed?.schema?.name === "UEC") {
      return parsed.kind === "character";
    }
    return parsed.type === "character";
  } catch {
    return false;
  }
}

/**
 * Helper to check if import JSON is a persona export
 */
export function isPersonaExport(importJson: string): boolean {
  try {
    const parsed = JSON.parse(importJson);
    if (parsed?.schema?.name === "UEC") {
      return parsed.kind === "persona";
    }
    return parsed.type === "persona";
  } catch {
    return false;
  }
}
