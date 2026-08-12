import { invoke } from "@tauri-apps/api/core";
import type { Lorebook } from "./schemas";

export async function exportLorebook(lorebookId: string): Promise<string> {
  try {
    return await invoke<string>("lorebook_export", { lorebookId });
  } catch (error) {
    console.error("[exportLorebook] Failed to export lorebook:", error);
    throw new Error(typeof error === "string" ? error : "Failed to export lorebook");
  }
}

export async function exportLorebookAsUsc(lorebookId: string): Promise<string> {
  try {
    return await invoke<string>("lorebook_export_as_usc", { lorebookId });
  } catch (error) {
    console.error("[exportLorebookAsUsc] Failed to export lorebook:", error);
    throw new Error(typeof error === "string" ? error : "Failed to export lorebook as USC");
  }
}

function filenameToLorebookName(filename: string): string {
  const trimmed = filename.trim();
  if (!trimmed) return "";
  return trimmed.replace(/\.[^/.]+$/, "").trim();
}

export async function importLorebook(
  importJson: string,
  fallbackFilename?: string,
): Promise<Lorebook> {
  try {
    let normalizedImportJson = importJson;

    if (fallbackFilename && fallbackFilename.trim()) {
      const parsed = JSON.parse(importJson) as { name?: unknown };
      const hasName = typeof parsed.name === "string" && parsed.name.trim().length > 0;
      if (!hasName) {
        const fallbackName = filenameToLorebookName(fallbackFilename);
        if (fallbackName) {
          parsed.name = fallbackName;
          normalizedImportJson = JSON.stringify(parsed);
        }
      }
    }

    const lorebookJson = await invoke<string>("lorebook_import", {
      importJson: normalizedImportJson,
    });
    return JSON.parse(lorebookJson) as Lorebook;
  } catch (error) {
    console.error("[importLorebook] Failed to import lorebook:", error);
    throw new Error(typeof error === "string" ? error : "Failed to import lorebook");
  }
}

export async function downloadJson(json: string, filename: string): Promise<void> {
  try {
    const savedPath = await invoke<string>("save_json_to_downloads", {
      filename,
      jsonContent: json,
    });
    alert(`File saved to: ${savedPath}`);
    return;
  } catch (_error) {
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

export function readFileAsText(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(new Error("Failed to read file"));
    reader.readAsText(file);
  });
}

export function generateLorebookExportFilename(lorebookName: string): string {
  const safeName = lorebookName.replace(/[^a-z0-9_-]/gi, "_").toLowerCase();
  const timestamp = new Date().toISOString().split("T")[0];
  return `lorebook_${safeName || "export"}_${timestamp}.json`;
}

export function generateLorebookExportFilenameWithFormat(
  lorebookName: string,
  format: "legacy_json" | "usc",
): string {
  const safeName = lorebookName.replace(/[^a-z0-9_-]/gi, "_").toLowerCase();
  const timestamp = new Date().toISOString().split("T")[0];
  const extension = format === "usc" ? "usc" : "json";
  return `lorebook_${safeName || "export"}_${timestamp}.${extension}`;
}
