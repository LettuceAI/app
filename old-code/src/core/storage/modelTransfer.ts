import { invoke } from "@tauri-apps/api/core";
import type { Model } from "./schemas";

type SafeModelExport = {
  id: string;
  name: string;
  providerId: string;
  providerLabel: string;
  displayName: string;
  createdAt: number;
  inputScopes: string[];
  outputScopes: string[];
  advancedModelSettings?: Model["advancedModelSettings"];
  promptTemplateId?: string | null;
  systemPrompt?: string | null;
};

type ImportedModelPayload = {
  name?: unknown;
  providerId?: unknown;
  providerLabel?: unknown;
  displayName?: unknown;
  createdAt?: unknown;
  inputScopes?: unknown;
  outputScopes?: unknown;
  advancedModelSettings?: unknown;
  promptTemplateId?: unknown;
  systemPrompt?: unknown;
};

function createId() {
  return globalThis.crypto?.randomUUID?.() ?? crypto.randomUUID();
}

function normalizeScopes(value: unknown): Array<"text" | "image" | "audio"> {
  const scopes = Array.isArray(value) ? value : [];
  const normalized = new Set<"text" | "image" | "audio">();
  const orderedScopes = ["text", "image", "audio"] as const;
  for (const scope of scopes) {
    if (scope === "text" || scope === "image" || scope === "audio") {
      normalized.add(scope);
    }
  }
  if (normalized.size === 0) {
    normalized.add("text");
  }
  return orderedScopes.filter((scope) => normalized.has(scope));
}

function normalizeImportedModel(input: ImportedModelPayload): Model {
  const name = typeof input.name === "string" ? input.name.trim() : "";
  if (!name) {
    throw new Error("Model name is required.");
  }

  const providerId = typeof input.providerId === "string" ? input.providerId.trim() : "";
  if (!providerId) {
    throw new Error("Provider ID is required.");
  }

  const providerLabel =
    typeof input.providerLabel === "string" && input.providerLabel.trim()
      ? input.providerLabel.trim()
      : providerId;
  const displayName =
    typeof input.displayName === "string" && input.displayName.trim()
      ? input.displayName.trim()
      : name;

  return {
    id: createId(),
    name,
    providerId,
    providerCredentialId: null,
    providerLabel,
    displayName,
    createdAt: typeof input.createdAt === "number" ? input.createdAt : Date.now(),
    inputScopes: normalizeScopes(input.inputScopes),
    outputScopes: normalizeScopes(input.outputScopes),
    advancedModelSettings:
      input.advancedModelSettings && typeof input.advancedModelSettings === "object"
        ? (input.advancedModelSettings as Model["advancedModelSettings"])
        : undefined,
    promptTemplateId: typeof input.promptTemplateId === "string" ? input.promptTemplateId : null,
    systemPrompt: typeof input.systemPrompt === "string" ? input.systemPrompt : null,
  };
}

export async function exportModelAsUsc(model: Model): Promise<string> {
  return await invoke<string>("model_export_as_usc", {
    modelJson: JSON.stringify(model),
  });
}

export function serializeModelExport(model: Model): string {
  const payload: SafeModelExport = {
    id: model.id,
    name: model.name,
    providerId: model.providerId,
    providerLabel: model.providerLabel,
    displayName: model.displayName,
    createdAt: model.createdAt,
    inputScopes: model.inputScopes,
    outputScopes: model.outputScopes,
  };

  if (model.advancedModelSettings != null) {
    payload.advancedModelSettings = model.advancedModelSettings;
  }
  if (model.promptTemplateId !== undefined) {
    payload.promptTemplateId = model.promptTemplateId ?? null;
  }
  if (model.systemPrompt !== undefined) {
    payload.systemPrompt = model.systemPrompt ?? null;
  }

  return JSON.stringify(payload, null, 2);
}

export function importModel(raw: string): Model {
  const parsed = JSON.parse(raw) as any;

  if (parsed?.schema?.name === "USC" && parsed?.kind === "model_profile" && parsed?.payload) {
    return normalizeImportedModel({
      name: parsed.payload.name,
      providerId: parsed.payload.providerId,
      providerLabel: parsed.payload.providerLabel,
      displayName: parsed.payload.displayName,
      createdAt: parsed.payload.createdAt,
      inputScopes: parsed.payload.inputScopes,
      outputScopes: parsed.payload.outputScopes,
      advancedModelSettings: parsed.payload.advancedModelSettings,
      promptTemplateId:
        typeof parsed.payload.systemPromptTemplate?.id === "string"
          ? parsed.payload.systemPromptTemplate.id
          : null,
      systemPrompt: parsed.payload.systemPrompt,
    });
  }

  if (parsed && typeof parsed === "object") {
    return normalizeImportedModel(parsed);
  }

  throw new Error("Unsupported model file.");
}

export function generateModelExportFilename(modelName: string, format: "json" | "usc"): string {
  const safeName = modelName.replace(/[^a-z0-9_-]/gi, "_").toLowerCase();
  const timestamp = new Date().toISOString().split("T")[0];
  return `model_${safeName || "export"}_${timestamp}.${format}`;
}
