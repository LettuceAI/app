import { invoke } from "@tauri-apps/api/core";
import type { AvatarCrop, Persona } from "./schemas";

/**
 * Persona export package format
 */
export interface PersonaExportPackage {
  version: number;
  exportedAt: number;
  persona: PersonaExportData;
  avatarData?: string;
}

/**
 * Persona data in export
 */
export interface PersonaExportData {
  title: string;
  description: string;
  nickname?: string;
  avatarCrop?: AvatarCrop;
  activeLorebookIds?: string[];
}

/**
 * Export a persona to a UEC package
 * Returns JSON string with all persona data and embedded avatar
 */
export async function exportPersona(personaId: string): Promise<string> {
  try {
    console.log("[exportPersona] Exporting persona:", personaId);
    const exportJson = await invoke<string>("persona_export", { personaId });
    console.log("[exportPersona] Export successful");
    return exportJson;
  } catch (error) {
    console.error("[exportPersona] Failed to export persona:", error);
    throw new Error(typeof error === "string" ? error : "Failed to export persona");
  }
}

/**
 * Import a persona from a UEC package
 * Creates a new persona with new ID
 * Returns the newly created persona
 */
export async function importPersona(importJson: string): Promise<Persona> {
  try {
    console.log("[importPersona] Importing persona");
    const personaJson = await invoke<string>("persona_import", { importJson });
    const persona = JSON.parse(personaJson) as Persona;
    console.log("[importPersona] Import successful:", persona.id);
    return persona;
  } catch (error) {
    console.error("[importPersona] Failed to import persona:", error);
    throw new Error(typeof error === "string" ? error : "Failed to import persona");
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
 * Read a file as text
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
export function generateExportFilename(personaTitle: string): string {
  const safeName = personaTitle.replace(/[^a-z0-9_-]/gi, "_").toLowerCase();
  const timestamp = new Date().toISOString().split("T")[0];
  return `persona_${safeName}_${timestamp}.uec`;
}
