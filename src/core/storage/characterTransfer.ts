import { invoke } from "@tauri-apps/api/core";
import type { AvatarCrop, Character, ChatTemplate } from "./schemas";

export type CharacterFileFormat =
  | "uec"
  | "legacy_json"
  | "chara_card_v3"
  | "chara_card_v2"
  | "chara_card_v1";

export interface CharacterFormatInfo {
  id: CharacterFileFormat;
  label: string;
  extension: string;
  canExport: boolean;
  canImport: boolean;
  readOnly: boolean;
}

export interface SceneExport {
  id: string;
  content: string;
  direction?: string;
  createdAt?: number;
  selectedVariantId?: string;
  variants: SceneVariantExport[];
}

export interface SceneVariantExport {
  id: string;
  content: string;
  direction?: string;
  createdAt?: number;
}

export interface CharacterImportPreview {
  name: string;
  description: string;
  definition: string;
  scenario?: string;
  nickname?: string;
  creator?: string;
  creatorNotes?: string;
  creatorNotesMultilingual?: Record<string, unknown> | null;
  source?: string[];
  tags?: string[];
  characterBook?: CharacterBookImport | null;
  scenes: SceneExport[];
  chatTemplates: ChatTemplate[];
  defaultSceneId: string | null;
  defaultChatTemplateId: string | null;
  promptTemplateId: string | null;
  memoryType: "manual" | "dynamic";
  disableAvatarGradient: boolean;
  fileFormat?: CharacterFileFormat;
  avatarData?: string | null;
  avatarCrop?: AvatarCrop;
  backgroundImageData?: string | null;
}

export interface CharacterBookEntryImport {
  name?: string;
  keys?: string[];
  secondary_keys?: string[];
  content?: string;
  enabled?: boolean;
  insertion_order?: number;
  case_sensitive?: boolean;
  priority?: number;
  constant?: boolean;
}

export interface CharacterBookImport {
  name?: string;
  description?: string;
  entries: CharacterBookEntryImport[];
}

/**
 * Export a character to a UEC package
 * Returns JSON string with all character data and embedded images
 */
export async function exportCharacter(characterId: string): Promise<string> {
  try {
    console.log("[exportCharacter] Exporting character:", characterId);
    const exportJson = await invoke<string>("character_export_with_format", {
      characterId,
      format: "uec",
    });
    console.log("[exportCharacter] Export successful");
    return exportJson;
  } catch (error) {
    console.error("[exportCharacter] Failed to export character:", error);
    throw new Error(typeof error === "string" ? error : "Failed to export character");
  }
}

/**
 * Export a character in a specific file format
 */
export async function exportCharacterWithFormat(
  characterId: string,
  format: CharacterFileFormat,
): Promise<string> {
  try {
    console.log("[exportCharacterWithFormat] Exporting character:", characterId, format);
    const exportJson = await invoke<string>("character_export_with_format", {
      characterId,
      format,
    });
    console.log("[exportCharacterWithFormat] Export successful");
    return exportJson;
  } catch (error) {
    console.error("[exportCharacterWithFormat] Failed to export character:", error);
    throw new Error(typeof error === "string" ? error : "Failed to export character");
  }
}

/**
 * Import a character from a UEC package
 * Creates a new character with new IDs
 * Returns the newly created character
 */
export async function importCharacter(importJson: string): Promise<Character> {
  try {
    console.log("[importCharacter] Importing character");
    const characterJson = await invoke<string>("character_import", { importJson });
    const character = JSON.parse(characterJson) as Character;
    console.log("[importCharacter] Import successful:", character.id);
    return character;
  } catch (error) {
    console.error("[importCharacter] Failed to import character:", error);
    throw new Error(typeof error === "string" ? error : "Failed to import character");
  }
}

/**
 * Parse an import file into a preview payload for the character form
 */
export async function previewCharacterImport(importJson: string): Promise<CharacterImportPreview> {
  try {
    const previewJson = await invoke<string>("character_import_preview", { importJson });
    return JSON.parse(previewJson) as CharacterImportPreview;
  } catch (error) {
    console.error("[previewCharacterImport] Failed to parse character:", error);
    throw new Error(typeof error === "string" ? error : "Failed to parse character");
  }
}

export async function previewCharacterImportFile(file: File): Promise<CharacterImportPreview> {
  try {
    const buffer = await file.arrayBuffer();
    const previewJson = await invoke<string>("character_import_preview_from_bytes", {
      filename: file.name,
      data: Array.from(new Uint8Array(buffer)),
    });
    return JSON.parse(previewJson) as CharacterImportPreview;
  } catch (error) {
    console.error("[previewCharacterImportFile] Failed to parse character:", error);
    throw new Error(typeof error === "string" ? error : "Failed to parse character");
  }
}

/**
 * Download a JSON string as a file
 * On mobile (Android/iOS), saves to the Downloads folder
 * On web/desktop, triggers a browser download
 */
export async function downloadJson(json: string, filename: string): Promise<void> {
  try {
    console.log("[downloadJson] Attempting to save via Tauri command");
    const savedPath = await invoke<string>("save_json_to_downloads", {
      filename,
      jsonContent: json,
    });
    console.log(`[downloadJson] File saved to: ${savedPath}`);
    alert(`File saved to: ${savedPath}`);
    return;
  } catch (error) {
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = filename;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
  }
}

/**
 * Read a file as UTF-8 text
 */
export function readFileAsText(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(new Error("Failed to read file"));
    reader.readAsText(file);
  });
}

/**
 * Generate a safe filename for export
 */
export function generateExportFilename(characterName: string): string {
  const safeName = characterName.replace(/[^a-z0-9_-]/gi, "_").toLowerCase();
  const timestamp = new Date().toISOString().split("T")[0];
  return `character_${safeName}_${timestamp}.uec`;
}

export function generateExportFilenameWithFormat(
  characterName: string,
  format: CharacterFileFormat,
): string {
  const safeName = characterName.replace(/[^a-z0-9_-]/gi, "_").toLowerCase();
  const timestamp = new Date().toISOString().split("T")[0];
  const extension = format === "uec" ? "uec" : "json";
  return `character_${safeName}_${timestamp}.${extension}`;
}

/**
 * List supported character file formats
 */
export async function listCharacterFormats(): Promise<CharacterFormatInfo[]> {
  try {
    return await invoke<CharacterFormatInfo[]>("character_list_formats");
  } catch (error) {
    console.error("[listCharacterFormats] Failed to load formats:", error);
    throw new Error(typeof error === "string" ? error : "Failed to load formats");
  }
}

/**
 * Detect character file format from JSON content
 */
export async function detectCharacterFormat(importJson: string): Promise<CharacterFormatInfo> {
  try {
    return await invoke<CharacterFormatInfo>("character_detect_format", { importJson });
  } catch (error) {
    console.error("[detectCharacterFormat] Failed to detect format:", error);
    throw new Error(typeof error === "string" ? error : "Failed to detect format");
  }
}
