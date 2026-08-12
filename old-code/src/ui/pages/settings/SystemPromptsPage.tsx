import { useState, useEffect, useMemo, useRef } from "react";
import { useNavigate } from "react-router-dom";
import {
  Pencil,
  Trash2,
  Lock,
  Search,
  Copy,
  Star,
  FileText,
  Download,
  Upload,
  Sparkles,
  Brain,
  MessageSquare,
  Users,
  Image,
  Plus,
  X,
  ListFilter,
  Check,
  AlertTriangle,
  RefreshCw,
} from "lucide-react";
import { getPromptTypeName } from "./EditPromptTemplate";
import { cn, typography, radius, interactive } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import {
  listPromptTemplates,
  deletePromptTemplate,
  createPromptTemplate,
  exportPromptTemplateAsUsc,
  resetAllProtectedTemplates,
} from "../../../core/prompts/service";
import type {
  PromptTemplateType,
  SystemPromptEntry,
  SystemPromptTemplate,
} from "../../../core/storage/schemas";
import { listCharacters, readSettings, setPromptTemplate } from "../../../core/storage/repo";
import {
  APP_DEFAULT_TEMPLATE_ID,
  APP_LOCAL_ROLEPLAY_TEMPLATE_ID,
  APP_DYNAMIC_SUMMARY_TEMPLATE_ID,
  APP_DYNAMIC_MEMORY_TEMPLATE_ID,
  APP_HELP_ME_REPLY_TEMPLATE_ID,
  APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID,
  APP_GROUP_CHAT_TEMPLATE_ID,
  APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID,
  APP_AVATAR_GENERATION_TEMPLATE_ID,
  APP_AVATAR_EDIT_TEMPLATE_ID,
  APP_SCENE_GENERATION_TEMPLATE_ID,
  APP_DESIGN_REFERENCE_TEMPLATE_ID,
  APP_COMPANION_SOUL_WRITER_TEMPLATE_ID,
  isProtectedPromptTemplate,
  isSystemPromptTemplate,
  getPromptTypeLabel,
} from "../../../core/prompts/constants";
import { BottomMenu, PromptTemplateExportMenu } from "../../components";
import { describeUsage, promptFeatureRefs } from "../../../core/storage/usage";
import { toast } from "../../components/toast";
import { downloadJson, readFileAsText } from "../../../core/storage/personaTransfer";
import type { PromptTemplateExportFormat } from "../../components/PromptTemplateExportMenu";

type TemplateUsage = {
  characters: number;
  features: string[];
};

const ALL_PROMPT_TYPES: PromptTemplateType[] = [
  "directChat",
  "companionChat",
  "groupChatRoleplay",
  "groupChatConversational",
  "replyHelperRoleplay",
  "replyHelperConversational",
  "dynamicMemorySummarizer",
  "dynamicMemoryManager",
  "lorebookEntryWriter",
  "lorebookKeywordGenerator",
  "avatarGeneration",
  "avatarEditRequest",
  "sceneGeneration",
  "scenePromptWriter",
  "designReferenceWriter",
  "companionSoulWriter",
  "undefined",
];

type ExternalPromptEntry = {
  identifier?: string;
  name?: string;
  system_prompt?: boolean;
  marker?: boolean;
  content?: string;
  role?: string;
  injection_position?: number | string;
  injection_depth?: number;
  conditional_min_messages?: number;
  interval_turns?: number;
  forbid_overrides?: boolean;
  enabled?: boolean;
  prompt_entry_payload?: {
    type?: string;
    slot?: string;
  };
  promptEntryPayload?: {
    type?: string;
    slot?: string;
  };
};

type ExternalPromptExport = {
  impersonation_prompt?: string;
  new_chat_prompt?: string;
  new_group_chat_prompt?: string;
  new_example_chat_prompt?: string;
  continue_nudge_prompt?: string;
  scenario_format?: string;
  personality_format?: string;
  group_nudge_prompt?: string;
  wi_format?: string;
  prompts?: ExternalPromptEntry[];
  prompt_order?: Array<{
    character_id: number;
    order: Array<{ identifier: string; enabled?: boolean }>;
  }>;
};

type ImportedPromptTemplatePayload = {
  name: string;
  promptType: PromptTemplateType;
  content: string;
  entries: SystemPromptEntry[];
  condensePromptEntries: boolean;
};

function normalizeImportedSystemEntry(
  input: unknown,
  fallbackIndex: number,
  importedPromptName: string,
): SystemPromptEntry | null {
  if (!input || typeof input !== "object") return null;
  const entry = input as Record<string, unknown>;
  const content = typeof entry.content === "string" ? entry.content : "";
  if (!content.trim()) return null;

  const id =
    typeof entry.id === "string" && entry.id.trim()
      ? entry.id
      : `imported_entry_${fallbackIndex}_${Math.random().toString(36).slice(2, 8)}`;
  const role = entry.role === "user" || entry.role === "assistant" ? entry.role : "system";
  const injectionPosition =
    entry.injectionPosition === "conditional" ||
    entry.injectionPosition === "interval" ||
    entry.injectionPosition === "inChat"
      ? entry.injectionPosition
      : "relative";
  const injectionDepth =
    typeof entry.injectionDepth === "number" && Number.isFinite(entry.injectionDepth)
      ? Math.max(0, Math.floor(entry.injectionDepth))
      : 0;
  const conditionalMinMessages =
    typeof entry.conditionalMinMessages === "number" &&
    Number.isFinite(entry.conditionalMinMessages)
      ? Math.max(1, Math.floor(entry.conditionalMinMessages))
      : null;
  const intervalTurns =
    typeof entry.intervalTurns === "number" && Number.isFinite(entry.intervalTurns)
      ? Math.max(1, Math.floor(entry.intervalTurns))
      : null;
  const payload =
    entry.promptEntryPayload && typeof entry.promptEntryPayload === "object"
      ? (entry.promptEntryPayload as Record<string, unknown>)
      : null;
  const payloadSlot = payload?.slot;
  const resolvedPayloadSlot:
    | "character"
    | "persona"
    | "chatBackground"
    | "avatar"
    | "references"
    | null =
    payloadSlot === "character" ||
    payloadSlot === "persona" ||
    payloadSlot === "chatBackground" ||
    payloadSlot === "avatar" ||
    payloadSlot === "references"
      ? payloadSlot
      : null;
  const promptEntryPayload =
    payload?.type === "imageSlot" && resolvedPayloadSlot
      ? { type: "imageSlot" as const, slot: resolvedPayloadSlot }
      : null;

  return {
    id,
    name: typeof entry.name === "string" && entry.name.trim() ? entry.name : importedPromptName,
    role,
    content,
    enabled: typeof entry.enabled === "boolean" ? entry.enabled : true,
    injectionPosition,
    injectionDepth,
    conditionalMinMessages,
    intervalTurns,
    systemPrompt: typeof entry.systemPrompt === "boolean" ? entry.systemPrompt : false,
    conditions:
      entry.conditions && typeof entry.conditions === "object" ? (entry.conditions as any) : null,
    promptEntryPayload,
  };
}

function normalizeImportedPromptTemplatePayload(
  input: {
    name?: unknown;
    promptType?: unknown;
    scope?: unknown;
    targetIds?: unknown;
    content?: unknown;
    entries?: unknown;
    condensePromptEntries?: unknown;
  },
  nameRequiredMessage: string,
  importedPromptName: string,
): ImportedPromptTemplatePayload {
  const name = typeof input.name === "string" ? input.name.trim() : "";
  if (!name) {
    throw new Error(nameRequiredMessage);
  }

  const promptType: PromptTemplateType =
    input.promptType === "directChat" ||
    input.promptType === "companionChat" ||
    input.promptType === "groupChatRoleplay" ||
    input.promptType === "groupChatConversational" ||
    input.promptType === "dynamicMemorySummarizer" ||
    input.promptType === "dynamicMemoryManager" ||
    input.promptType === "replyHelperRoleplay" ||
    input.promptType === "replyHelperConversational" ||
    input.promptType === "lorebookEntryWriter" ||
    input.promptType === "lorebookKeywordGenerator" ||
    input.promptType === "avatarGeneration" ||
    input.promptType === "avatarEditRequest" ||
    input.promptType === "sceneGeneration" ||
    input.promptType === "scenePromptWriter" ||
    input.promptType === "designReferenceWriter" ||
    input.promptType === "companionSoulWriter" ||
    input.promptType === "undefined"
      ? input.promptType
      : "undefined";
  const content = typeof input.content === "string" ? input.content : "";
  const entries = (Array.isArray(input.entries) ? input.entries : [])
    .map((entry, index) => normalizeImportedSystemEntry(entry, index, importedPromptName))
    .filter((entry): entry is SystemPromptEntry => entry !== null);

  return {
    name,
    promptType,
    content,
    entries,
    condensePromptEntries: Boolean(input.condensePromptEntries),
  };
}

function generatePromptTemplateExportFilename(
  templateName: string,
  format: PromptTemplateExportFormat,
) {
  const safeName = templateName.replace(/[^a-z0-9_-]/gi, "_").toLowerCase();
  const date = new Date().toISOString().split("T")[0];
  const extension = format === "usc" ? "usc" : "json";
  return `system_prompts_${safeName || "export"}_${date}.${extension}`;
}

type PromptOrderEntry = {
  identifier: string;
  enabled?: boolean;
};

const EXTERNAL_MARKER_IDENTIFIERS = new Set([
  "worldInfoBefore",
  "personaDescription",
  "charDescription",
  "charPersonality",
  "scenario",
  "worldInfoAfter",
  "dialogueExamples",
  "chatHistory",
]);

function normalizePromptVariables(content: string) {
  return content.replace(/{{scenario}}/g, "{{scene}}").replace(/{{personality}}/g, "{{char.desc}}");
}

function entryToExternal(entry: SystemPromptEntry): ExternalPromptEntry {
  const role: "system" | "user" | "assistant" =
    entry.role === "assistant" || entry.role === "user" ? entry.role : "system";
  const injectionPosition =
    entry.injectionPosition === "relative"
      ? 0
      : entry.injectionPosition === "inChat"
        ? 1
        : entry.injectionPosition;
  return {
    identifier: entry.id,
    name: entry.name,
    system_prompt: entry.systemPrompt,
    marker: false,
    content: normalizePromptVariables(entry.content),
    role,
    injection_position: injectionPosition,
    injection_depth: entry.injectionDepth,
    conditional_min_messages: entry.conditionalMinMessages ?? undefined,
    interval_turns: entry.intervalTurns ?? undefined,
    forbid_overrides: false,
    enabled: entry.enabled,
    prompt_entry_payload: entry.promptEntryPayload
      ? {
          type: entry.promptEntryPayload.type,
          slot: entry.promptEntryPayload.slot,
        }
      : undefined,
  };
}

function makeExternalMarkers(): ExternalPromptEntry[] {
  return [
    { identifier: "worldInfoBefore", name: "Lorebook Before", system_prompt: true, marker: true },
    {
      identifier: "personaDescription",
      name: "Persona Description",
      system_prompt: true,
      marker: true,
    },
    { identifier: "charDescription", name: "Char Description", system_prompt: true, marker: true },
    { identifier: "charPersonality", name: "Char Personality", system_prompt: true, marker: true },
    { identifier: "scenario", name: "Scenario", system_prompt: true, marker: true },
    { identifier: "worldInfoAfter", name: "Lorebook After", system_prompt: true, marker: true },
    { identifier: "dialogueExamples", name: "Chat Examples", system_prompt: true, marker: true },
    { identifier: "chatHistory", name: "Chat History", system_prompt: true, marker: true },
  ];
}

function toSystemEntry(
  input: ExternalPromptEntry,
  fallbackIndex: number,
  importedPromptName: string,
): SystemPromptEntry | null {
  if (input.marker || (input.identifier && EXTERNAL_MARKER_IDENTIFIERS.has(input.identifier))) {
    return null;
  }
  const content = typeof input.content === "string" ? normalizePromptVariables(input.content) : "";
  if (!content.trim()) return null;
  const id =
    typeof input.identifier === "string" && input.identifier.trim()
      ? input.identifier
      : `imported_${fallbackIndex}_${Math.random().toString(36).slice(2, 8)}`;
  const role = input.role === "user" || input.role === "assistant" ? input.role : "system";
  const injectionPosition =
    input.injection_position === "conditional"
      ? "conditional"
      : input.injection_position === "interval"
        ? "interval"
        : input.injection_position === 1 || input.injection_position === "inChat"
          ? "inChat"
          : "relative";
  const injectionDepth =
    typeof input.injection_depth === "number" && Number.isFinite(input.injection_depth)
      ? input.injection_depth
      : 0;
  return {
    id,
    name: typeof input.name === "string" && input.name.trim() ? input.name : importedPromptName,
    role,
    content,
    enabled: input.enabled ?? true,
    injectionPosition,
    injectionDepth,
    conditionalMinMessages:
      typeof input.conditional_min_messages === "number" &&
      Number.isFinite(input.conditional_min_messages)
        ? Math.max(1, Math.floor(input.conditional_min_messages))
        : null,
    intervalTurns:
      typeof input.interval_turns === "number" && Number.isFinite(input.interval_turns)
        ? Math.max(1, Math.floor(input.interval_turns))
        : null,
    // Imported entries should stay user-editable/deletable.
    systemPrompt: false,
    conditions: null,
    promptEntryPayload:
      (input.prompt_entry_payload?.type ?? input.promptEntryPayload?.type) === "imageSlot" &&
      ["character", "persona", "avatar", "references"].includes(
        input.prompt_entry_payload?.slot ?? input.promptEntryPayload?.slot ?? "",
      )
        ? {
            type: "imageSlot",
            slot: (input.prompt_entry_payload?.slot ?? input.promptEntryPayload?.slot) as
              | "character"
              | "persona"
              | "avatar"
              | "references",
          }
        : null,
  };
}

function flattenPromptOrder(input: unknown): PromptOrderEntry[] {
  const flattened: PromptOrderEntry[] = [];
  const visit = (node: unknown) => {
    if (!node) return;
    if (Array.isArray(node)) {
      node.forEach(visit);
      return;
    }
    if (typeof node !== "object") return;

    const obj = node as Record<string, unknown>;
    const identifier = typeof obj.identifier === "string" ? obj.identifier.trim() : "";
    const enabled = typeof obj.enabled === "boolean" ? obj.enabled : undefined;
    if (identifier) {
      flattened.push({ identifier, enabled });
    }

    if (obj.order) visit(obj.order);
  };

  visit(input);
  return flattened;
}

function collectPromptOrderBlocks(input: unknown): PromptOrderEntry[][] {
  const blocks: PromptOrderEntry[][] = [];
  const visit = (node: unknown) => {
    if (!node) return;
    if (Array.isArray(node)) {
      node.forEach(visit);
      return;
    }
    if (typeof node !== "object") return;

    const obj = node as Record<string, unknown>;
    if (obj.order) {
      const block = flattenPromptOrder(obj.order);
      if (block.length > 0) {
        blocks.push(block);
      }
    }

    Object.values(obj).forEach(visit);
  };

  visit(input);
  return blocks;
}

function getTemplateIcon(templateId: string) {
  switch (templateId) {
    case APP_DEFAULT_TEMPLATE_ID:
    case APP_LOCAL_ROLEPLAY_TEMPLATE_ID:
      return Sparkles;
    case APP_DYNAMIC_SUMMARY_TEMPLATE_ID:
    case APP_DYNAMIC_MEMORY_TEMPLATE_ID:
      return Brain;
    case APP_HELP_ME_REPLY_TEMPLATE_ID:
    case APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID:
      return MessageSquare;
    case APP_GROUP_CHAT_TEMPLATE_ID:
    case APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID:
      return Users;
    case APP_AVATAR_GENERATION_TEMPLATE_ID:
    case APP_AVATAR_EDIT_TEMPLATE_ID:
    case APP_SCENE_GENERATION_TEMPLATE_ID:
    case APP_DESIGN_REFERENCE_TEMPLATE_ID:
      return Image;
    case APP_COMPANION_SOUL_WRITER_TEMPLATE_ID:
      return Brain;
    default:
      return FileText;
  }
}

function getTemplatePreviewText(template: SystemPromptTemplate) {
  if (template.entries && template.entries.length > 0) {
    return template.entries.map((entry) => entry.content).join("\n");
  }
  return template.content;
}

function PromptCardSkeleton() {
  return (
    <div className="grid grid-cols-1 lg:grid-cols-2 gap-3">
      {[1, 2, 3, 4].map((i) => (
        <div key={i} className="rounded-xl border border-fg/10 bg-fg/5 p-4">
          <div className="flex items-start gap-3">
            <div className="h-9 w-9 animate-pulse rounded-lg bg-fg/10" />
            <div className="flex-1 space-y-2">
              <div className="h-4 w-32 animate-pulse rounded bg-fg/10" />
              <div className="h-3 w-full animate-pulse rounded bg-fg/5" />
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

function EmptyState({ onCreate }: { onCreate: () => void }) {
  const { t } = useI18n();
  return (
    <div className="flex flex-col items-center justify-center py-16 px-6">
      <div className="flex h-14 w-14 items-center justify-center rounded-xl border border-fg/10 bg-fg/5 mb-4">
        <FileText className="h-7 w-7 text-fg/30" />
      </div>
      <h3 className={cn(typography.h2.size, typography.h2.weight, "text-fg mb-2")}>
        {t("systemPrompts.empty.title")}
      </h3>
      <p className={cn(typography.body.size, "text-fg/50 text-center max-w-xs mb-6")}>
        {t("systemPrompts.empty.description")}
      </p>
      <button
        onClick={onCreate}
        className={cn(
          "flex items-center gap-2 px-5 py-2.5",
          radius.lg,
          "border border-accent/40 bg-accent/20",
          "text-sm font-medium text-accent/90",
          interactive.transition.default,
          "hover:bg-accent/30",
          "active:scale-[0.98]",
        )}
      >
        <Plus className="h-4 w-4" />
        {t("systemPrompts.empty.createButton")}
      </button>
    </div>
  );
}

function PromptCard({
  template,
  isActiveDefault,
  usage,
  onEdit,
  onDelete,
  onDuplicate,
  onExport,
  onSetDefault,
}: {
  template: SystemPromptTemplate;
  isActiveDefault: boolean;
  usage: TemplateUsage;
  onEdit: () => void;
  onDelete: () => void;
  onDuplicate: () => void;
  onExport: () => void;
  onSetDefault: () => void;
}) {
  const { t } = useI18n();
  const isProtected = isProtectedPromptTemplate(template.id);
  const isSystem = isSystemPromptTemplate(template.id);
  const typeLabel = getPromptTypeLabel(template.id);
  const Icon = getTemplateIcon(template.id);

  return (
    <div
      className={cn(
        "group relative",
        radius.lg,
        "border border-fg/10 bg-fg/5",
        "hover:border-fg/20 hover:bg-fg/[0.07]",
        interactive.transition.fast,
      )}
    >
      <div className="p-4">
        {/* Header Row */}
        <div className="flex items-start gap-3">
          {/* Icon */}
          <div
            className={cn(
              "flex h-9 w-9 shrink-0 items-center justify-center",
              radius.lg,
              isActiveDefault ? "bg-accent/20 text-accent" : "bg-fg/10 text-fg/50",
            )}
          >
            <Icon className="h-4 w-4" />
          </div>

          {/* Title + Type */}
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <h3 className="text-sm font-medium text-fg truncate">{template.name}</h3>
              {isActiveDefault && <Star className="h-3.5 w-3.5 text-accent fill-accent shrink-0" />}
              {isProtected && <Lock className="h-3.5 w-3.5 text-warning shrink-0" />}
            </div>
            <p className="text-xs text-fg/40 mt-0.5">{typeLabel}</p>
          </div>

          {/* Actions */}
          <div className="flex items-center gap-1 shrink-0">
            <button
              onClick={onEdit}
              className={cn(
                "p-1.5",
                radius.md,
                "text-fg/40 hover:text-fg hover:bg-fg/10",
                interactive.transition.fast,
              )}
              title={t("systemPrompts.card.edit")}
            >
              <Pencil className="h-4 w-4" />
            </button>
            <button
              onClick={onDuplicate}
              className={cn(
                "p-1.5",
                radius.md,
                "text-fg/40 hover:text-fg hover:bg-fg/10",
                interactive.transition.fast,
              )}
              title={t("systemPrompts.card.duplicate")}
            >
              <Copy className="h-4 w-4" />
            </button>
            <button
              onClick={onExport}
              className={cn(
                "p-1.5",
                radius.md,
                "text-fg/40 hover:text-accent/80 hover:bg-accent/10",
                interactive.transition.fast,
              )}
              title={t("systemPrompts.card.export")}
            >
              <Download className="h-4 w-4" />
            </button>
            {!isProtected && (
              <button
                onClick={onDelete}
                className={cn(
                  "p-1.5",
                  radius.md,
                  "text-fg/40 hover:text-danger hover:bg-danger/10",
                  interactive.transition.fast,
                )}
                title={t("systemPrompts.card.delete")}
              >
                <Trash2 className="h-4 w-4" />
              </button>
            )}
          </div>
        </div>

        {/* Content Preview */}
        <p className="text-xs text-fg/40 line-clamp-2 mt-3 leading-relaxed">
          {getTemplatePreviewText(template)}
        </p>

        {/* Footer */}
        <div className="flex items-center justify-between mt-3 pt-3 border-t border-fg/5">
          <div className="text-[11px] text-fg/30">
            {isSystem
              ? usage.characters === 1
                ? t("systemPrompts.card.charCountOne", { count: usage.characters })
                : t("systemPrompts.card.charCountOther", { count: usage.characters })
              : t("systemPrompts.card.internalFeature")}
          </div>

          {isSystem && !isActiveDefault && (
            <button
              onClick={onSetDefault}
              className={cn(
                "flex items-center gap-1 px-2 py-1",
                radius.md,
                "text-[11px] font-medium text-accent",
                "hover:bg-accent/10",
                interactive.transition.fast,
              )}
            >
              <Star className="h-3 w-3" />
              {t("systemPrompts.card.setDefault")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

export function SystemPromptsPage() {
  const navigate = useNavigate();
  const { t } = useI18n();
  const [templates, setTemplates] = useState<SystemPromptTemplate[]>([]);
  const [loading, setLoading] = useState(true);
  const [activeDefaultId, setActiveDefaultId] = useState<string>(APP_DEFAULT_TEMPLATE_ID);
  const [usageById, setUsageById] = useState<Record<string, TemplateUsage>>({});
  const [search, setSearch] = useState("");
  const [selectedTypes, setSelectedTypes] = useState<Set<PromptTemplateType>>(new Set());
  const [typeFilterOpen, setTypeFilterOpen] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [templateToDelete, setTemplateToDelete] = useState<SystemPromptTemplate | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [exportMenuOpen, setExportMenuOpen] = useState(false);
  const [exportTarget, setExportTarget] = useState<SystemPromptTemplate | null>(null);
  const [importing, setImporting] = useState(false);
  const [showUpdateConfirm, setShowUpdateConfirm] = useState(false);
  const [updatingProtected, setUpdatingProtected] = useState(false);
  const importInputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    loadData();

    const globalWindow = window as any;
    globalWindow.__openAddPromptTemplate = () => {
      navigate("/settings/prompts/new");
    };

    const handleAdd = () => navigate("/settings/prompts/new");
    window.addEventListener("prompts:add", handleAdd);

    return () => {
      delete globalWindow.__openAddPromptTemplate;
      window.removeEventListener("prompts:add", handleAdd);
    };
  }, [navigate]);

  async function handleExportTemplate(format: PromptTemplateExportFormat) {
    if (!exportTarget || exporting) return;
    setExporting(true);
    try {
      if (format === "usc") {
        const exportJson = await exportPromptTemplateAsUsc(exportTarget.id);
        await downloadJson(
          exportJson,
          generatePromptTemplateExportFilename(exportTarget.name, format),
        );
        setExportMenuOpen(false);
        setExportTarget(null);
        return;
      }

      const entries =
        exportTarget.entries && exportTarget.entries.length > 0
          ? exportTarget.entries
          : [
              {
                id: "main",
                name: "Main Prompt",
                role: "system",
                content: normalizePromptVariables(exportTarget.content),
                enabled: true,
                injectionPosition: "relative",
                injectionDepth: 0,
                systemPrompt: true,
              },
            ];

      const prompts = [
        ...entries.map((entry) => entryToExternal(entry as SystemPromptEntry)),
        ...makeExternalMarkers(),
      ];
      const exportPayload: ExternalPromptExport = {
        impersonation_prompt:
          "[Write your next reply from the point of view of {{user}}, using the chat history so far as a guideline for the writing style of {{user}}. Write 1 reply only in internet RP style. Don't write as {{char}} or system. Don't describe actions of {{char}}.]",
        new_chat_prompt: "[Start a new Chat]",
        new_group_chat_prompt: "[Start a new group chat. Group members: {{group}}]",
        new_example_chat_prompt: "[Example Chat]",
        continue_nudge_prompt:
          "[Continue the following message. Do not include ANY parts of the original message. Use capitalization and punctuation as if your reply is a part of the original message: {{lastChatMessage}}]",
        scenario_format: "{{scene}}",
        personality_format: "{{char.desc}}",
        group_nudge_prompt: "[Write the next reply only as {{char}}.]",
        wi_format: "{0}",
        prompts,
        prompt_order: [
          {
            character_id: 100001,
            order: prompts.map((prompt) => ({
              identifier: prompt.identifier || "",
              enabled: prompt.enabled ?? true,
            })),
          },
        ],
      };

      const json = JSON.stringify(exportPayload, null, 2);
      await downloadJson(json, generatePromptTemplateExportFilename(exportTarget.name, format));
      setExportMenuOpen(false);
      setExportTarget(null);
    } catch (error) {
      console.error("Failed to export system prompts:", error);
      toast.error(t("systemPrompts.toasts.exportFailedTitle"), String(error));
    } finally {
      setExporting(false);
    }
  }

  async function handleImport(file: File) {
    if (importing) return;
    setImporting(true);
    try {
      const raw = await readFileAsText(file);
      const parsed = JSON.parse(raw) as ExternalPromptExport;

      if (
        parsed &&
        typeof parsed === "object" &&
        (parsed as any).schema?.name === "USC" &&
        (parsed as any).kind === "system_prompt_template" &&
        (parsed as any).payload
      ) {
        const imported = normalizeImportedPromptTemplatePayload(
          (parsed as any).payload,
          t("systemPrompts.errors.nameRequired"),
          t("systemPrompts.importedPromptName"),
        );
        await createPromptTemplate(
          imported.name,
          imported.promptType,
          imported.content,
          imported.entries,
          imported.condensePromptEntries,
        );
        await loadData();
        toast.success(
          t("systemPrompts.toasts.importedTitle"),
          t("systemPrompts.toasts.importedMessage", { name: imported.name }),
        );
        return;
      }

      const promptEntries = Array.isArray(parsed.prompts) ? parsed.prompts : [];
      const promptIdentifiers = new Set(
        promptEntries
          .map((prompt) => (typeof prompt.identifier === "string" ? prompt.identifier.trim() : ""))
          .filter(Boolean),
      );
      const orderBlocks = collectPromptOrderBlocks(parsed.prompt_order);
      const orderedRefs =
        orderBlocks.length > 0
          ? orderBlocks.slice().sort((a, b) => {
              const aMatches = a.filter((entry) => promptIdentifiers.has(entry.identifier)).length;
              const bMatches = b.filter((entry) => promptIdentifiers.has(entry.identifier)).length;
              if (aMatches !== bMatches) return bMatches - aMatches;
              return b.length - a.length;
            })[0]
          : flattenPromptOrder(parsed.prompt_order);
      const orderIndexById = new Map<string, number>();
      const enabledById = new Map<string, boolean>();
      orderedRefs.forEach((entry, index) => {
        if (!orderIndexById.has(entry.identifier)) {
          orderIndexById.set(entry.identifier, index);
        }
        if (typeof entry.enabled === "boolean") {
          enabledById.set(entry.identifier, entry.enabled);
        }
      });

      const importedEntries = promptEntries
        .map((prompt, index) => toSystemEntry(prompt, index, t("systemPrompts.importedPromptName")))
        .map((entry, index) => ({ entry, index }))
        .filter((item): item is { entry: SystemPromptEntry; index: number } => Boolean(item.entry))
        .map((item) => {
          const overrideEnabled = enabledById.get(item.entry.id);
          if (typeof overrideEnabled === "boolean") {
            item.entry.enabled = overrideEnabled;
          }
          return item;
        })
        .sort((a, b) => {
          const aOrder = orderIndexById.get(a.entry.id) ?? Number.MAX_SAFE_INTEGER;
          const bOrder = orderIndexById.get(b.entry.id) ?? Number.MAX_SAFE_INTEGER;
          if (aOrder !== bOrder) return aOrder - bOrder;
          return a.index - b.index;
        })
        .map((item) => item.entry);

      if (importedEntries.length === 0) {
        alert(t("systemPrompts.toasts.noImportablePrompts"));
        return;
      }

      const baseName =
        file.name.replace(/\.[^/.]+$/, "") || t("systemPrompts.importedPromptSetName");
      await createPromptTemplate(baseName, "undefined", "", importedEntries, false);
      await loadData();
      toast.success(
        t("systemPrompts.toasts.importedTitle"),
        t("systemPrompts.toasts.importedMessage", { name: baseName }),
      );
    } catch (error) {
      console.error("Failed to import system prompts:", error);
      toast.error(t("systemPrompts.toasts.importFailedTitle"), String(error));
    } finally {
      setImporting(false);
    }
  }

  async function loadData() {
    try {
      const [data, settings, characters] = await Promise.all([
        listPromptTemplates(),
        readSettings(),
        listCharacters(),
      ]);

      const usage: Record<string, TemplateUsage> = {};
      const ensure = (id: string): TemplateUsage => {
        if (!usage[id]) {
          usage[id] = { characters: 0, features: [] };
        }
        return usage[id];
      };

      characters.forEach((character) => {
        const referenced = new Set(
          [
            character.promptTemplateId,
            character.groupChatPromptTemplateId,
            character.groupChatRoleplayPromptTemplateId,
          ].filter((id): id is string => Boolean(id)),
        );
        referenced.forEach((id) => {
          ensure(id).characters += 1;
        });
      });

      promptFeatureRefs(settings).forEach(({ id, label }) => {
        const entry = ensure(id);
        if (!entry.features.includes(label)) {
          entry.features.push(label);
        }
      });

      const activeDefault = settings.promptTemplateId ?? APP_DEFAULT_TEMPLATE_ID;

      const sorted = data.sort((a, b) => {
        if (a.id === activeDefault) return -1;
        if (b.id === activeDefault) return 1;
        if (a.id === APP_DEFAULT_TEMPLATE_ID) return -1;
        if (b.id === APP_DEFAULT_TEMPLATE_ID) return 1;
        return b.createdAt - a.createdAt;
      });

      setTemplates(sorted);
      setActiveDefaultId(activeDefault);
      setUsageById(usage);
    } catch (error) {
      console.error("Failed to load templates:", error);
    } finally {
      setLoading(false);
    }
  }

  async function handleUpdateProtected() {
    setUpdatingProtected(true);
    try {
      await resetAllProtectedTemplates();
      await loadData();
      setShowUpdateConfirm(false);
      toast.success(
        t("systemPrompts.updateProtected.successTitle"),
        t("systemPrompts.updateProtected.successMessage"),
      );
    } catch (error) {
      console.error("Failed to update protected templates:", error);
      toast.error(
        t("systemPrompts.updateProtected.failTitle"),
        String(error),
      );
    } finally {
      setUpdatingProtected(false);
    }
  }

  async function handleDelete() {
    if (!templateToDelete) return;
    if (isProtectedPromptTemplate(templateToDelete.id)) {
      alert(t("systemPrompts.errors.protectedDelete"));
      return;
    }

    setDeleting(true);
    try {
      await deletePromptTemplate(templateToDelete.id);
      setTemplates((prev) => prev.filter((t) => t.id !== templateToDelete.id));
      setShowDeleteConfirm(false);
      setTemplateToDelete(null);
    } catch (error) {
      console.error("Failed to delete template:", error);
      alert(t("systemPrompts.errors.deleteFailed", { error: String(error) }));
    } finally {
      setDeleting(false);
    }
  }

  async function handleDuplicate(template: SystemPromptTemplate) {
    try {
      const name = t("systemPrompts.duplicateSuffix", { name: template.name });
      const contentToSave = template.content.trim()
        ? template.content
        : getTemplatePreviewText(template);
      await createPromptTemplate(
        name,
        template.promptType,
        contentToSave,
        template.entries,
        Boolean(template.condensePromptEntries),
      );
      await loadData();
    } catch (error) {
      console.error("Failed to duplicate template:", error);
      alert(t("systemPrompts.errors.duplicateFailed", { error: String(error) }));
    }
  }

  async function handleSetDefault(templateId: string) {
    try {
      const next = templateId === APP_DEFAULT_TEMPLATE_ID ? null : templateId;
      await setPromptTemplate(next);
      await loadData();
    } catch (error) {
      console.error("Failed to set default template:", error);
      alert(t("systemPrompts.errors.setDefaultFailed", { error: String(error) }));
    }
  }

  const filtered = useMemo(() => {
    return templates.filter((t) => {
      if (selectedTypes.size > 0 && !selectedTypes.has(t.promptType)) return false;

      const q = search.trim().toLowerCase();
      if (!q) return true;
      return (
        t.name.toLowerCase().includes(q) || getTemplatePreviewText(t).toLowerCase().includes(q)
      );
    });
  }, [templates, selectedTypes, search]);

  return (
    <div className="flex h-full flex-col">
      <main className="flex-1 overflow-y-auto px-4 pt-4 pb-6">
        <div className="mx-auto w-full max-w-5xl space-y-4">
          {/* Search and Filters */}
          <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
            <div className="relative sm:flex-1">
              <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg/30" />
              <input
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder={t("systemPrompts.page.searchPlaceholder")}
                className={cn(
                  "w-full py-2.5 pl-10 pr-10",
                  radius.lg,
                  "border border-fg/10 bg-fg/5",
                  "text-sm text-fg placeholder-fg/30",
                  interactive.transition.fast,
                  "focus:border-fg/20 focus:bg-fg/10 focus:outline-none",
                )}
              />
              {search && (
                <button
                  onClick={() => setSearch("")}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-fg/40 hover:text-fg/70"
                >
                  <X className="h-4 w-4" />
                </button>
              )}
            </div>

            <div className="flex items-center gap-2 sm:shrink-0">
              <input
                ref={importInputRef}
                type="file"
                className="hidden"
                onChange={(event) => {
                  const file = event.target.files?.[0];
                  if (file) {
                    void handleImport(file);
                  }
                  event.currentTarget.value = "";
                }}
              />
              <button
                onClick={() => setTypeFilterOpen(true)}
                className={cn(
                  "flex items-center gap-1.5 px-3 py-2.5",
                  radius.lg,
                  "text-xs font-medium",
                  interactive.transition.fast,
                  selectedTypes.size > 0
                    ? "border border-accent/40 bg-accent/15 text-accent/80"
                    : "border border-fg/10 bg-fg/5 text-fg/50 hover:bg-fg/10 hover:text-fg/70",
                )}
              >
                <ListFilter className="h-3.5 w-3.5" />
                <span>{t("systemPrompts.page.typeFilter")}</span>
                {selectedTypes.size > 0 && (
                  <span className="rounded-full bg-accent/25 px-1.5 text-[10px] font-semibold tabular-nums">
                    {selectedTypes.size}
                  </span>
                )}
              </button>
              <button
                onClick={() => setShowUpdateConfirm(true)}
                disabled={updatingProtected}
                title={t("systemPrompts.updateProtected.button")}
                className={cn(
                  "flex items-center gap-2 px-3 py-2.5",
                  radius.lg,
                  "border border-fg/10 bg-fg/5",
                  "text-xs font-medium text-fg/70",
                  interactive.transition.fast,
                  "hover:bg-fg/10 hover:text-fg",
                  "disabled:opacity-50",
                )}
              >
                <RefreshCw
                  className={cn("h-3.5 w-3.5", updatingProtected && "animate-spin")}
                />
                <span className="hidden sm:inline">
                  {updatingProtected
                    ? t("systemPrompts.updateProtected.updating")
                    : t("systemPrompts.updateProtected.button")}
                </span>
              </button>
              <button
                onClick={() => importInputRef.current?.click()}
                disabled={importing}
                className={cn(
                  "flex items-center gap-2 px-3 py-2.5",
                  radius.lg,
                  "border border-fg/10 bg-fg/5",
                  "text-xs font-medium text-fg/70",
                  interactive.transition.fast,
                  "hover:bg-fg/10 hover:text-fg",
                  "disabled:opacity-50",
                )}
              >
                <Upload className="h-3.5 w-3.5" />
                {importing ? t("systemPrompts.page.importing") : t("systemPrompts.page.import")}
              </button>
            </div>
          </div>

          {/* Templates Grid */}
          {loading ? (
            <PromptCardSkeleton />
          ) : filtered.length === 0 ? (
            search || selectedTypes.size > 0 ? (
              <div className="flex flex-col items-center justify-center py-12 px-6">
                <p className="text-sm text-fg/50 mb-1">{t("systemPrompts.page.noMatchingTitle")}</p>
                <p className="text-xs text-fg/30">{t("systemPrompts.page.noMatchingHint")}</p>
              </div>
            ) : (
              <EmptyState onCreate={() => navigate("/settings/prompts/new")} />
            )
          ) : (
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-3">
              {filtered.map((template) => (
                <PromptCard
                  key={template.id}
                  template={template}
                  isActiveDefault={template.id === activeDefaultId}
                  usage={usageById[template.id] || { characters: 0 }}
                  onEdit={() => navigate(`/settings/prompts/${template.id}`)}
                  onDelete={() => {
                    setTemplateToDelete(template);
                    setShowDeleteConfirm(true);
                  }}
                  onDuplicate={() => handleDuplicate(template)}
                  onExport={() => {
                    setExportTarget(template);
                    setExportMenuOpen(true);
                  }}
                  onSetDefault={() => handleSetDefault(template.id)}
                />
              ))}
            </div>
          )}
        </div>
      </main>

      <PromptTemplateExportMenu
        isOpen={exportMenuOpen}
        onClose={() => {
          if (exporting) return;
          setExportMenuOpen(false);
          setExportTarget(null);
        }}
        onSelect={(format) => {
          void handleExportTemplate(format);
        }}
        exporting={exporting}
      />

      <BottomMenu
        isOpen={typeFilterOpen}
        onClose={() => setTypeFilterOpen(false)}
        title={t("systemPrompts.page.filterByTypeTitle")}
        rightAction={
          selectedTypes.size > 0 ? (
            <button
              type="button"
              onClick={() => setSelectedTypes(new Set())}
              className="text-xs font-medium text-fg/55 hover:text-fg/80"
            >
              {t("systemPrompts.page.clear")}
            </button>
          ) : null
        }
      >
        <div className="space-y-1">
          {ALL_PROMPT_TYPES.map((type) => {
            const active = selectedTypes.has(type);
            return (
              <button
                key={type}
                type="button"
                onClick={() => {
                  setSelectedTypes((prev) => {
                    const next = new Set(prev);
                    if (next.has(type)) {
                      next.delete(type);
                    } else {
                      next.add(type);
                    }
                    return next;
                  });
                }}
                className={cn(
                  "flex w-full items-center justify-between gap-3 px-3 py-2.5 text-left",
                  radius.md,
                  interactive.transition.fast,
                  active
                    ? "bg-accent/15 text-accent/90"
                    : "text-fg/75 hover:bg-fg/5 hover:text-fg",
                )}
              >
                <span className="text-sm">{getPromptTypeName(type)}</span>
                <span
                  className={cn(
                    "flex h-5 w-5 shrink-0 items-center justify-center rounded-md border",
                    active
                      ? "border-accent/60 bg-accent/30 text-accent"
                      : "border-fg/15 bg-fg/5",
                  )}
                >
                  {active && <Check className="h-3 w-3" />}
                </span>
              </button>
            );
          })}
        </div>
      </BottomMenu>

      {/* Delete Confirmation */}
      <BottomMenu
        isOpen={showDeleteConfirm}
        onClose={() => {
          setShowDeleteConfirm(false);
          setTemplateToDelete(null);
        }}
        title={t("systemPrompts.delete.title")}
      >
        <div className="space-y-4">
          <div className={cn(radius.lg, "border border-fg/10 bg-fg/5 p-3")}>
            <p className="text-sm font-medium text-fg">{templateToDelete?.name}</p>
            <p className="text-xs text-fg/50 mt-1 line-clamp-2">{templateToDelete?.content}</p>
          </div>

          {(() => {
            const warning = templateToDelete
              ? describeUsage(
                  usageById[templateToDelete.id] ?? { characters: 0, features: [] },
                  "prompt",
                )
              : null;
            return warning ? (
              <div className={cn(radius.lg, "border border-warning/30 bg-warning/10 p-3")}>
                <div className="flex items-center gap-2 text-warning">
                  <AlertTriangle className="h-4 w-4" />
                  <p className="text-sm font-semibold">{warning.title}</p>
                </div>
                <p className="mt-1.5 text-xs leading-relaxed text-fg/70">{warning.body}</p>
              </div>
            ) : (
              <p className="text-sm text-fg/60">
                {t("systemPrompts.delete.fallbackWarning")}
              </p>
            );
          })()}

          <div className="flex gap-3">
            <button
              onClick={() => {
                setShowDeleteConfirm(false);
                setTemplateToDelete(null);
              }}
              disabled={deleting}
              className={cn(
                "flex-1 py-3",
                radius.lg,
                "border border-fg/10 bg-fg/5",
                "text-sm font-medium text-fg",
                interactive.transition.fast,
                "hover:bg-fg/10",
                "disabled:opacity-50",
              )}
            >
              {t("common.buttons.cancel")}
            </button>
            <button
              onClick={handleDelete}
              disabled={deleting}
              className={cn(
                "flex-1 py-3",
                radius.lg,
                "border border-danger/30 bg-danger/15",
                "text-sm font-medium text-danger/80",
                interactive.transition.fast,
                "hover:bg-danger/25",
                "disabled:opacity-50",
              )}
            >
              {deleting ? t("systemPrompts.delete.deleting") : t("common.buttons.delete")}
            </button>
          </div>
        </div>
      </BottomMenu>

      {/* Update Protected Confirmation */}
      <BottomMenu
        isOpen={showUpdateConfirm}
        onClose={() => setShowUpdateConfirm(false)}
        title={t("systemPrompts.updateProtected.title")}
      >
        <div className="space-y-4">
          <div className={cn(radius.lg, "border border-warning/30 bg-warning/10 p-3")}>
            <div className="flex items-center gap-2 text-warning">
              <AlertTriangle className="h-4 w-4" />
              <p className="text-sm font-semibold">
                {t("systemPrompts.updateProtected.warningTitle")}
              </p>
            </div>
            <p className="mt-1.5 text-xs leading-relaxed text-fg/70">
              {t("systemPrompts.updateProtected.warningBody")}
            </p>
          </div>

          <div className="flex gap-3">
            <button
              onClick={() => setShowUpdateConfirm(false)}
              disabled={updatingProtected}
              className={cn(
                "flex-1 py-3",
                radius.lg,
                "border border-fg/10 bg-fg/5",
                "text-sm font-medium text-fg",
                interactive.transition.fast,
                "hover:bg-fg/10",
                "disabled:opacity-50",
              )}
            >
              {t("common.buttons.cancel")}
            </button>
            <button
              onClick={handleUpdateProtected}
              disabled={updatingProtected}
              className={cn(
                "flex-1 py-3",
                radius.lg,
                "border border-accent/30 bg-accent/15",
                "text-sm font-medium text-accent/80",
                interactive.transition.fast,
                "hover:bg-accent/25",
                "disabled:opacity-50",
              )}
            >
              {updatingProtected
                ? t("systemPrompts.updateProtected.updating")
                : t("systemPrompts.updateProtected.confirm")}
            </button>
          </div>
        </div>
      </BottomMenu>
    </div>
  );
}
