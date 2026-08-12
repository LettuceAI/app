import { invoke } from "@tauri-apps/api/core";
import type { ChatTemplate, ChatTemplateMessage } from "./schemas";

export type ChatTemplateExportFormat = "json" | "usc";

type LegacyChatTemplateExport = {
  version: number;
  kind: "chat_template";
  template: {
    name: string;
    messages: Array<{ role: "user" | "assistant"; content: string }>;
    sceneId?: string | null;
    promptTemplateId?: string | null;
    lorebookIdsOverride?: string[] | null;
  };
};

function createId() {
  return globalThis.crypto?.randomUUID?.() ?? crypto.randomUUID();
}

function normalizeMessage(input: unknown): ChatTemplateMessage | null {
  if (!input || typeof input !== "object") return null;
  const role = (input as { role?: unknown }).role;
  const content = (input as { content?: unknown }).content;
  if ((role !== "user" && role !== "assistant") || typeof content !== "string") {
    return null;
  }
  return {
    id: createId(),
    role,
    content,
  };
}

function normalizeImportedTemplate(input: {
  name?: unknown;
  messages?: unknown;
  sceneId?: unknown;
  promptTemplateId?: unknown;
  lorebookIdsOverride?: unknown;
}): ChatTemplate {
  const name = typeof input.name === "string" ? input.name.trim() : "";
  if (!name) {
    throw new Error("Chat template name is required.");
  }

  const rawMessages = Array.isArray(input.messages) ? input.messages : [];
  const messages = rawMessages.map(normalizeMessage).filter((message): message is ChatTemplateMessage => message !== null);

  return {
    id: createId(),
    name,
    messages,
    sceneId: typeof input.sceneId === "string" ? input.sceneId : null,
    promptTemplateId: typeof input.promptTemplateId === "string" ? input.promptTemplateId : null,
    lorebookIdsOverride: Array.isArray(input.lorebookIdsOverride)
      ? input.lorebookIdsOverride.filter((id): id is string => typeof id === "string")
      : null,
    createdAt: Date.now(),
  };
}

export async function exportChatTemplateAsUsc(template: ChatTemplate): Promise<string> {
  return await invoke<string>("chat_template_export_as_usc", {
    templateJson: JSON.stringify(template),
  });
}

export function serializeChatTemplateExport(template: ChatTemplate): string {
  const payload: LegacyChatTemplateExport = {
    version: 1,
    kind: "chat_template",
    template: {
      name: template.name,
      messages: template.messages.map((message) => ({
        role: message.role,
        content: message.content,
      })),
      sceneId: template.sceneId ?? null,
      promptTemplateId: template.promptTemplateId ?? null,
      lorebookIdsOverride: Array.isArray(template.lorebookIdsOverride)
        ? template.lorebookIdsOverride
        : null,
    },
  };

  return JSON.stringify(payload, null, 2);
}

export function importChatTemplate(raw: string): ChatTemplate {
  const parsed = JSON.parse(raw) as any;

  if (parsed?.schema?.name === "USC" && parsed?.kind === "chat_template" && parsed?.payload) {
    return normalizeImportedTemplate({
      name: parsed.payload.name,
      messages: parsed.payload.messages,
      sceneId: parsed.payload.sceneId,
      promptTemplateId:
        typeof parsed.payload.systemPromptTemplate?.id === "string"
          ? parsed.payload.systemPromptTemplate.id
          : null,
      lorebookIdsOverride: parsed.payload.lorebookIdsOverride,
    });
  }

  if (parsed?.kind === "chat_template" && parsed?.template) {
    return normalizeImportedTemplate(parsed.template);
  }

  if (parsed && typeof parsed === "object") {
    return normalizeImportedTemplate(parsed);
  }

  throw new Error("Unsupported chat template file.");
}

export function generateChatTemplateExportFilename(
  templateName: string,
  format: ChatTemplateExportFormat,
): string {
  const safeName = templateName.replace(/[^a-z0-9_-]/gi, "_").toLowerCase();
  const date = new Date().toISOString().split("T")[0];
  return `chat_template_${safeName || "export"}_${date}.${format}`;
}
