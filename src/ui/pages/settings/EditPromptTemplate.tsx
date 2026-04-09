import { useState, useEffect, useMemo, useRef } from "react";
import {
  AnimatePresence,
  Reorder,
  motion,
  useDragControls,
  useMotionValue,
  type PanInfo,
} from "framer-motion";
import { useParams } from "react-router-dom";
import {
  RotateCcw,
  Eye,
  Code2,
  Check,
  AlertTriangle,
  Sparkles,
  Copy,
  Lock,
  ChevronDown,
  ChevronUp,
  GripVertical,
  Plus,
  Trash2,
  X,
  Layers,
} from "lucide-react";
import { cn, radius, interactive } from "../../design-tokens";
import { MessageStructurePreview } from "./components/MessageStructurePreview";
import { BottomMenu } from "../../components";
import { confirmBottomMenu } from "../../components/ConfirmBottomMenu";
import { useI18n } from "../../../core/i18n/context";
import { Switch } from "../../components/Switch";
import { useNavigationManager } from "../../navigation";
import {
  createPromptTemplate,
  updatePromptTemplate,
  getPromptTemplate,
  getAppDefaultTemplateId,
  resetAppDefaultTemplate,
  resetLocalRoleplayTemplate,
  resetDynamicSummaryTemplate,
  resetDynamicMemoryTemplate,
  resetHelpMeReplyTemplate,
  resetAvatarGenerationTemplate,
  resetAvatarEditTemplate,
  resetSceneGenerationTemplate,
  resetDesignReferenceTemplate,
  renderPromptPreview,
  getRequiredTemplateVariables,
} from "../../../core/prompts/service";
import { listCharacters, listPersonas } from "../../../core/storage";
import type {
  Character,
  Persona,
  PromptEntryCondition,
  SystemPromptEntry,
} from "../../../core/storage/schemas";
import {
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
  isProtectedPromptTemplate,
} from "../../../core/prompts/constants";

type PromptType =
  | "system"
  | "local_roleplay"
  | "summary"
  | "memory"
  | "reply"
  | "avatar_generation"
  | "avatar_edit"
  | "scene_generation"
  | "design_reference"
  | "group_chat"
  | "group_chat_roleplay"
  | null;

type Variable = {
  var: string;
  label: string;
  desc: string;
};

type PromptEntryImageSlot = "character" | "persona" | "chatBackground" | "avatar" | "references";
type PromptEntryKind = "text" | "image";
type ImageCapablePromptType = Exclude<PromptType, null>;

const VARIABLES_BY_TYPE: Record<string, Variable[]> = {
  system: [
    { var: "{{char.name}}", label: "Character Name", desc: "The character's display name" },
    { var: "{{char.desc}}", label: "Character Definition", desc: "Full character definition" },
    { var: "{{scene}}", label: "Scene", desc: "Starting scene or scenario" },
    { var: "{{rules}}", label: "Rules", desc: "Character behavioral rules" },
    { var: "{{persona.name}}", label: "User Name", desc: "The user's persona name" },
    { var: "{{persona.desc}}", label: "User Description", desc: "User persona description" },
    { var: "{{context_summary}}", label: "Context Summary", desc: "Dynamic conversation summary" },
    { var: "{{key_memories}}", label: "Key Memories", desc: "List of relevant memories" },
  ],
  local_roleplay: [
    { var: "{{char.name}}", label: "Character Name", desc: "The character's display name" },
    { var: "{{char.desc}}", label: "Character Definition", desc: "Full character definition" },
    { var: "{{persona.name}}", label: "User Name", desc: "The user's persona name" },
    { var: "{{persona.desc}}", label: "User Description", desc: "User persona description" },
    { var: "{{scene}}", label: "Scene", desc: "Starting scene or scenario" },
    { var: "{{scene_direction}}", label: "Scene Direction", desc: "Optional scene direction" },
    { var: "{{context_summary}}", label: "Context Summary", desc: "Dynamic conversation summary" },
    { var: "{{key_memories}}", label: "Key Memories", desc: "List of relevant memories" },
    { var: "{{lorebook}}", label: "Lorebook", desc: "Matched lorebook content" },
  ],
  summary: [
    { var: "{{prev_summary}}", label: "Previous Summary", desc: "The cumulative summary" },
    { var: "{{character}}", label: "Character", desc: "Character placeholder" },
    { var: "{{persona}}", label: "Persona", desc: "Persona placeholder" },
  ],
  memory: [
    { var: "{{max_entries}}", label: "Max Entries", desc: "Maximum memory entries allowed" },
  ],
  reply: [
    { var: "{{char.name}}", label: "Character Name", desc: "The character's display name" },
    { var: "{{char.desc}}", label: "Character Definition", desc: "Full character definition" },
    { var: "{{persona.name}}", label: "User Name", desc: "The user's persona name" },
    { var: "{{persona.desc}}", label: "User Description", desc: "User persona description" },
    { var: "{{current_draft}}", label: "Current Draft", desc: "Content user started writing" },
  ],
  avatar_generation: [
    {
      var: "{{avatar_subject_name}}",
      label: "Avatar Subject Name",
      desc: "Name of the character or persona the avatar is for",
    },
    {
      var: "{{avatar_subject_description}}",
      label: "Avatar Subject Description",
      desc: "Description of the character or persona the avatar is for",
    },
    { var: "{{avatar_request}}", label: "Avatar Request", desc: "User request for the avatar" },
  ],
  avatar_edit: [
    {
      var: "{{avatar_subject_name}}",
      label: "Avatar Subject Name",
      desc: "Name of the character or persona the avatar is for",
    },
    {
      var: "{{avatar_subject_description}}",
      label: "Avatar Subject Description",
      desc: "Description of the character or persona the avatar is for",
    },
    {
      var: "{{current_avatar_prompt}}",
      label: "Current Avatar Prompt",
      desc: "The prompt used for the current avatar image",
    },
    { var: "{{edit_request}}", label: "Edit Request", desc: "Requested avatar changes" },
  ],
  scene_generation: [
    { var: "{{char.name}}", label: "Character Name", desc: "The character's display name" },
    { var: "{{char.desc}}", label: "Character Definition", desc: "Full character definition" },
    { var: "{{persona.name}}", label: "User Name", desc: "The user's persona name" },
    { var: "{{persona.desc}}", label: "User Description", desc: "User persona description" },
    {
      var: "{{image[character]}}",
      label: "Character Reference Image",
      desc: "Injected image block for the character avatar reference",
    },
    {
      var: "{{reference[character]}}",
      label: "Character Reference Text",
      desc: "Rendered text notes for the character design references",
    },
    {
      var: "{{image[persona]}}",
      label: "Persona Reference Image",
      desc: "Injected image block for the persona avatar reference",
    },
    {
      var: "{{image[chatBackground]}}",
      label: "Chat Background Image",
      desc: "Injected image block for the chat background/environment reference",
    },
    {
      var: "{{reference[chatBackground]}}",
      label: "Chat Background Text",
      desc: "Rendered text directions for using the chat background as a scene reference",
    },
    {
      var: "{{reference[persona]}}",
      label: "Persona Reference Text",
      desc: "Rendered text notes for the persona design references",
    },
    {
      var: "{{recent_messages}}",
      label: "Recent Messages",
      desc: "Recent chat lines used to derive the scene",
    },
    {
      var: "{{scene_request}}",
      label: "Scene Request",
      desc: "Manual or automatic scene image request",
    },
  ],
  design_reference: [
    { var: "{{subject_name}}", label: "Subject Name", desc: "Name of the subject being described" },
    {
      var: "{{subject_description}}",
      label: "Subject Context",
      desc: "Character or persona context that can inform the design notes",
    },
    {
      var: "{{current_description}}",
      label: "Current Notes",
      desc: "Existing design notes to refine when they match the images",
    },
    {
      var: "{{image[avatar]}}",
      label: "Avatar Image",
      desc: "Injected image block for the subject's avatar when available",
    },
    {
      var: "{{image[references]}}",
      label: "Reference Images",
      desc: "Injected image block for attached design reference images",
    },
  ],
  group_chat: [
    { var: "{{char.name}}", label: "Character Name", desc: "The character's display name" },
    { var: "{{char.desc}}", label: "Character Definition", desc: "Full character definition" },
    { var: "{{persona.name}}", label: "User Name", desc: "The user's persona name" },
    { var: "{{persona.desc}}", label: "User Description", desc: "User persona description" },
    { var: "{{group_characters}}", label: "Group Characters", desc: "List of group characters" },
  ],
  group_chat_roleplay: [
    { var: "{{scene}}", label: "Scene", desc: "Starting scene or scenario" },
    { var: "{{scene_direction}}", label: "Scene Direction", desc: "Optional scene direction" },
    { var: "{{char.name}}", label: "Character Name", desc: "The character's display name" },
    { var: "{{char.desc}}", label: "Character Definition", desc: "Full character definition" },
    { var: "{{persona.name}}", label: "User Name", desc: "The user's persona name" },
    { var: "{{persona.desc}}", label: "User Description", desc: "User persona description" },
    { var: "{{group_characters}}", label: "Group Characters", desc: "List of group characters" },
    { var: "{{context_summary}}", label: "Context Summary", desc: "Dynamic conversation summary" },
    { var: "{{key_memories}}", label: "Key Memories", desc: "List of relevant memories" },
  ],
  default: [
    { var: "{{char.name}}", label: "Character Name", desc: "The character's display name" },
    { var: "{{char.desc}}", label: "Character Definition", desc: "Full character definition" },
    { var: "{{scene}}", label: "Scene", desc: "Starting scene or scenario" },
    { var: "{{rules}}", label: "Rules", desc: "Character behavioral rules" },
    { var: "{{persona.name}}", label: "User Name", desc: "The user's persona name" },
    { var: "{{persona.desc}}", label: "User Description", desc: "User persona description" },
    { var: "{{context_summary}}", label: "Context Summary", desc: "Dynamic conversation summary" },
    { var: "{{key_memories}}", label: "Key Memories", desc: "List of relevant memories" },
  ],
};

const IMAGE_ENTRY_SLOT_LABELS: Record<PromptEntryImageSlot, string> = {
  character: "Character reference",
  persona: "Persona reference",
  chatBackground: "Chat background",
  avatar: "Avatar image",
  references: "Reference images",
};

const IMAGE_ENTRY_SLOT_TOKENS: Record<PromptEntryImageSlot, string> = {
  character: "{{image[character]}}",
  persona: "{{image[persona]}}",
  chatBackground: "{{image[chatBackground]}}",
  avatar: "{{image[avatar]}}",
  references: "{{image[references]}}",
};

const IMAGE_ENTRY_SLOT_OPTIONS_BY_PROMPT_TYPE: Partial<
  Record<ImageCapablePromptType, PromptEntryImageSlot[]>
> = {
  scene_generation: ["character", "persona", "chatBackground"],
  design_reference: ["avatar", "references"],
};

const ENTRY_ROLE_OPTIONS = [
  { value: "system", label: "System" },
  { value: "user", label: "User" },
  { value: "assistant", label: "Assistant" },
] as const;

const ENTRY_POSITION_OPTIONS = [
  { value: "relative", label: "Relative" },
  { value: "inChat", label: "In Chat" },
  { value: "conditional", label: "Conditional" },
  { value: "interval", label: "Interval" },
] as const;

const DRAG_HOLD_MS = 450;
const AUTO_SCROLL_EDGE_PX = 96;
const AUTO_SCROLL_MAX_SPEED_PX = 18;

function resolveScrollContainer(from: HTMLElement | null): HTMLElement | null {
  let current = from;
  while (current) {
    const style = getComputedStyle(current);
    const overflowY = style.overflowY;
    if (
      (overflowY === "auto" || overflowY === "scroll" || overflowY === "overlay") &&
      current.scrollHeight > current.clientHeight
    ) {
      return current;
    }
    current = current.parentElement;
  }
  return null;
}

function computeAutoScrollSpeed(pointerY: number, rectTop: number, rectBottom: number): number {
  const topEdge = rectTop + AUTO_SCROLL_EDGE_PX;
  const bottomEdge = rectBottom - AUTO_SCROLL_EDGE_PX;
  if (pointerY < topEdge) {
    const ratio = Math.min(1, (topEdge - pointerY) / AUTO_SCROLL_EDGE_PX);
    return -Math.ceil(ratio * AUTO_SCROLL_MAX_SPEED_PX);
  }
  if (pointerY > bottomEdge) {
    const ratio = Math.min(1, (pointerY - bottomEdge) / AUTO_SCROLL_EDGE_PX);
    return Math.ceil(ratio * AUTO_SCROLL_MAX_SPEED_PX);
  }
  return 0;
}

function useDragEdgeAutoScroll() {
  const containerRef = useRef<HTMLElement | null>(null);
  const pointerYRef = useRef<number | null>(null);
  const draggingRef = useRef(false);
  const rafRef = useRef<number | null>(null);

  const stop = () => {
    draggingRef.current = false;
    pointerYRef.current = null;
    if (rafRef.current !== null) {
      window.cancelAnimationFrame(rafRef.current);
      rafRef.current = null;
    }
  };

  const tick = () => {
    if (!draggingRef.current) {
      rafRef.current = null;
      return;
    }

    const pointerY = pointerYRef.current;
    const container = containerRef.current;
    if (pointerY == null || !container) {
      rafRef.current = window.requestAnimationFrame(tick);
      return;
    }

    const rect = container.getBoundingClientRect();
    const speed = computeAutoScrollSpeed(pointerY, rect.top, rect.bottom);
    if (speed !== 0) {
      const maxScrollTop = container.scrollHeight - container.clientHeight;
      const next = Math.max(0, Math.min(maxScrollTop, container.scrollTop + speed));
      if (next !== container.scrollTop) {
        container.scrollTop = next;
      }
    }

    rafRef.current = window.requestAnimationFrame(tick);
  };

  const start = (from: HTMLElement | null, pointerY: number) => {
    containerRef.current = resolveScrollContainer(from) ?? document.querySelector("main");
    pointerYRef.current = pointerY;
    draggingRef.current = true;
    if (rafRef.current === null) {
      rafRef.current = window.requestAnimationFrame(tick);
    }
  };

  const update = (pointerY: number) => {
    pointerYRef.current = pointerY;
  };

  useEffect(() => stop, []);

  return { start, update, stop };
}

const createEntryId = () =>
  typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : `entry_${Date.now()}_${Math.random().toString(16).slice(2)}`;

const DEFAULT_ENTRY_ROLE: SystemPromptEntry["role"] = "system";
const DEFAULT_ENTRY_POSITION: SystemPromptEntry["injectionPosition"] = "relative";
const DEFAULT_CONDITIONAL_MIN_MESSAGES = 6;
const DEFAULT_INTERVAL_TURNS = 3;
type ConditionJoin = "and" | "or";
type SimplePromptEntryCondition = Exclude<
  PromptEntryCondition,
  { type: "all" } | { type: "any" } | { type: "not" }
>;
type SimplePromptEntryConditionType = SimplePromptEntryCondition["type"];
type ConditionRow = {
  condition: SimplePromptEntryCondition;
  joinWithPrevious: ConditionJoin;
};
type ConditionDraft = {
  include: ConditionRow[];
  exclude: ConditionRow[];
};
type ConditionRowGroup = {
  join: ConditionJoin | null;
  rows: ConditionRow[];
};

const SIMPLE_CONDITION_OPTIONS: Array<{
  value: SimplePromptEntryConditionType;
  label: string;
  kind: "boolean" | "number" | "list" | "chatMode";
  placeholder?: string;
}> = [
  { value: "chatMode", label: "Chat mode", kind: "chatMode" },
  { value: "sceneGenerationEnabled", label: "Scene generation enabled", kind: "boolean" },
  { value: "avatarGenerationEnabled", label: "Avatar generation enabled", kind: "boolean" },
  { value: "hasScene", label: "Has scene", kind: "boolean" },
  { value: "hasSceneDirection", label: "Has scene direction", kind: "boolean" },
  { value: "hasPersona", label: "Has persona", kind: "boolean" },
  { value: "messageCountAtLeast", label: "Message count at least", kind: "number" },
  {
    value: "participantCountAtLeast",
    label: "Participant count at least",
    kind: "number",
  },
  { value: "keywordAny", label: "Keyword any", kind: "list", placeholder: "beach, sunset" },
  { value: "keywordAll", label: "Keyword all", kind: "list", placeholder: "storm, harbor" },
  { value: "keywordNone", label: "Keyword none", kind: "list", placeholder: "violence, gore" },
  { value: "dynamicMemoryEnabled", label: "Dynamic memory enabled", kind: "boolean" },
  { value: "hasMemorySummary", label: "Has memory summary", kind: "boolean" },
  { value: "hasKeyMemories", label: "Has key memories", kind: "boolean" },
  { value: "hasLorebookContent", label: "Has lorebook content", kind: "boolean" },
  { value: "hasSubjectDescription", label: "Has subject description", kind: "boolean" },
  { value: "hasCurrentDescription", label: "Has current description", kind: "boolean" },
  {
    value: "hasCharacterReferenceImages",
    label: "Has character reference images",
    kind: "boolean",
  },
  {
    value: "hasChatBackground",
    label: "Has chat background",
    kind: "boolean",
  },
  {
    value: "hasPersonaReferenceImages",
    label: "Has persona reference images",
    kind: "boolean",
  },
  {
    value: "hasCharacterReferenceText",
    label: "Has character reference text",
    kind: "boolean",
  },
  {
    value: "hasPersonaReferenceText",
    label: "Has persona reference text",
    kind: "boolean",
  },
  { value: "inputScopeAny", label: "Input scope any", kind: "list", placeholder: "text, image" },
  {
    value: "outputScopeAny",
    label: "Output scope any",
    kind: "list",
    placeholder: "text, image",
  },
  { value: "providerIdAny", label: "Provider any", kind: "list", placeholder: "openai, ollama" },
  { value: "reasoningEnabled", label: "Reasoning enabled", kind: "boolean" },
  { value: "visionEnabled", label: "Vision enabled", kind: "boolean" },
];

function isSimpleCondition(
  condition: PromptEntryCondition | null | undefined,
): condition is SimplePromptEntryCondition {
  return (
    !!condition && condition.type !== "all" && condition.type !== "any" && condition.type !== "not"
  );
}

function createDefaultCondition(
  type: SimplePromptEntryConditionType = "chatMode",
): SimplePromptEntryCondition {
  switch (type) {
    case "chatMode":
      return { type, value: "direct" };
    case "messageCountAtLeast":
      return { type, value: 1 };
    case "participantCountAtLeast":
      return { type, value: 2 };
    case "keywordAny":
    case "keywordAll":
    case "keywordNone":
    case "inputScopeAny":
    case "outputScopeAny":
    case "providerIdAny":
      return { type, values: [""] };
    default:
      return { type, value: true };
  }
}

function normalizeListInput(value: string) {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

function logicalTypeForJoin(join: ConditionJoin): "all" | "any" {
  return join === "and" ? "all" : "any";
}

function joinForLogicalType(type: "all" | "any"): ConditionJoin {
  return type === "all" ? "and" : "or";
}

function decomposePositiveSequence(
  condition: PromptEntryCondition,
  joinWithPrevious: ConditionJoin = "and",
): ConditionRow[] {
  if (isSimpleCondition(condition)) {
    return [{ condition, joinWithPrevious }];
  }

  if (condition.type !== "all" && condition.type !== "any") {
    return [];
  }

  const join = joinForLogicalType(condition.type);
  return condition.conditions.flatMap((child, index) =>
    decomposePositiveSequence(child, index === 0 ? joinWithPrevious : join),
  );
}

function decomposeConditionTree(
  condition: PromptEntryCondition | null | undefined,
): ConditionDraft {
  if (!condition) {
    return { include: [], exclude: [] };
  }

  if (isSimpleCondition(condition)) {
    return { include: [{ condition, joinWithPrevious: "and" }], exclude: [] };
  }

  if (condition.type === "not") {
    return { include: [], exclude: decomposePositiveSequence(condition.condition) };
  }

  if (condition.type === "all") {
    const include: ConditionRow[] = [];
    const exclude: ConditionRow[] = [];
    condition.conditions.forEach((child) => {
      if (child.type === "not") {
        exclude.push(...decomposePositiveSequence(child.condition));
      } else {
        include.push(...decomposePositiveSequence(child));
      }
    });
    return { include, exclude };
  }

  return { include: decomposePositiveSequence(condition), exclude: [] };
}

function groupConditionRows(rows: ConditionRow[]): ConditionRowGroup[] {
  if (rows.length === 0) {
    return [];
  }

  const groups: ConditionRowGroup[] = [{ join: null, rows: [rows[0]] }];

  for (let index = 1; index < rows.length; index += 1) {
    const row = rows[index];
    const current = groups[groups.length - 1];

    if (current.join === null || current.join === row.joinWithPrevious) {
      current.join = row.joinWithPrevious;
      current.rows.push(row);
    } else {
      groups.push({ join: row.joinWithPrevious, rows: [row] });
    }
  }

  return groups;
}

function composeConditionSequence(rows: ConditionRow[]): PromptEntryCondition | null {
  const activeRows = rows.filter((row) => isSimpleCondition(row.condition));
  if (activeRows.length === 0) {
    return null;
  }

  const groups = groupConditionRows(activeRows);
  const composeGroup = (group: ConditionRowGroup): PromptEntryCondition => {
    if (group.rows.length === 1) {
      return group.rows[0].condition;
    }

    return {
      type: logicalTypeForJoin(group.join ?? "and"),
      conditions: group.rows.map((row) => row.condition),
    };
  };

  let expression: PromptEntryCondition = composeGroup(groups[0]);
  for (let index = 1; index < groups.length; index += 1) {
    expression = {
      type: logicalTypeForJoin(groups[index].join ?? "and"),
      conditions: [expression, composeGroup(groups[index])],
    };
  }

  return expression;
}

function composeConditionTree(draft: ConditionDraft): PromptEntryCondition | null {
  const include = composeConditionSequence(draft.include);
  const exclude = composeConditionSequence(draft.exclude);

  if (!include && !exclude) {
    return null;
  }

  if (include && !exclude) {
    return include;
  }

  if (!include && exclude) {
    return { type: "not", condition: exclude };
  }

  return {
    type: "all",
    conditions: [
      include as PromptEntryCondition,
      { type: "not", condition: exclude as PromptEntryCondition },
    ],
  };
}

function describeSimpleCondition(condition: SimplePromptEntryCondition): string {
  switch (condition.type) {
    case "chatMode":
      return `chat is ${condition.value}`;
    case "sceneGenerationEnabled":
      return `scene generation is ${condition.value ? "on" : "off"}`;
    case "avatarGenerationEnabled":
      return `avatar generation is ${condition.value ? "on" : "off"}`;
    case "hasScene":
      return condition.value ? "scene exists" : "scene missing";
    case "hasSceneDirection":
      return condition.value ? "scene has direction" : "scene has no direction";
    case "hasPersona":
      return condition.value ? "persona exists" : "persona missing";
    case "messageCountAtLeast":
      return `messages >= ${condition.value}`;
    case "participantCountAtLeast":
      return `participants >= ${condition.value}`;
    case "keywordAny":
      return `any keyword: ${condition.values.join(", ")}`;
    case "keywordAll":
      return `all keywords: ${condition.values.join(", ")}`;
    case "keywordNone":
      return `no keywords: ${condition.values.join(", ")}`;
    case "dynamicMemoryEnabled":
      return `dynamic memory is ${condition.value ? "on" : "off"}`;
    case "hasMemorySummary":
      return condition.value ? "memory summary exists" : "memory summary missing";
    case "hasKeyMemories":
      return condition.value ? "key memories exist" : "key memories missing";
    case "hasLorebookContent":
      return condition.value ? "lorebook content exists" : "lorebook content missing";
    case "hasSubjectDescription":
      return condition.value ? "subject description exists" : "subject description missing";
    case "hasCurrentDescription":
      return condition.value ? "current description exists" : "current description missing";
    case "hasCharacterReferenceImages":
      return condition.value
        ? "character reference images exist"
        : "character reference images missing";
    case "hasChatBackground":
      return condition.value ? "chat background exists" : "chat background missing";
    case "hasPersonaReferenceImages":
      return condition.value
        ? "persona reference images exist"
        : "persona reference images missing";
    case "hasCharacterReferenceText":
      return condition.value
        ? "character reference text exists"
        : "character reference text missing";
    case "hasPersonaReferenceText":
      return condition.value ? "persona reference text exists" : "persona reference text missing";
    case "inputScopeAny":
      return `input scope: ${condition.values.join(", ")}`;
    case "outputScopeAny":
      return `output scope: ${condition.values.join(", ")}`;
    case "providerIdAny":
      return `provider: ${condition.values.join(", ")}`;
    case "reasoningEnabled":
      return `reasoning is ${condition.value ? "on" : "off"}`;
    case "visionEnabled":
      return `vision is ${condition.value ? "on" : "off"}`;
  }
}

function describeConditionTree(condition: PromptEntryCondition | null | undefined): string {
  const draft = decomposeConditionTree(condition);
  const describeRows = (rows: ConditionRow[]) =>
    rows
      .map((row, index) => {
        const label = describeSimpleCondition(row.condition);
        return index === 0 ? label : `${row.joinWithPrevious.toUpperCase()} ${label}`;
      })
      .join(" ");

  const include = describeRows(draft.include);
  const exclude = describeRows(draft.exclude);
  if (!include && !exclude) {
    return "Always active";
  }
  if (include && exclude) {
    return `${include} · EXCLUDE ${exclude}`;
  }
  if (exclude) {
    return `EXCLUDE ${exclude}`;
  }
  return include;
}

function describeConditionSentence(condition: PromptEntryCondition | null | undefined): string {
  if (!condition) {
    return "This entry is always active.";
  }

  const draft = decomposeConditionTree(condition);

  const stripOuterParens = (value: string) => {
    if (!value.startsWith("(") || !value.endsWith(")")) {
      return value;
    }

    let depth = 0;
    for (let index = 0; index < value.length; index += 1) {
      const char = value[index];
      if (char === "(") {
        depth += 1;
      } else if (char === ")") {
        depth -= 1;
        if (depth === 0 && index < value.length - 1) {
          return value;
        }
      }
    }

    return value.slice(1, -1);
  };

  const describeGroupedSequence = (rows: ConditionRow[]) => {
    if (rows.length === 0) {
      return "";
    }

    const groups = groupConditionRows(rows);
    const describeGroup = (group: ConditionRowGroup) => {
      const connector = group.join === "or" ? " or " : " and ";
      const text = group.rows.map((row) => describeSimpleCondition(row.condition)).join(connector);
      return group.rows.length > 1 ? `(${text})` : text;
    };

    let expression = describeGroup(groups[0]);
    for (let index = 1; index < groups.length; index += 1) {
      const connector = groups[index].join === "or" ? "or" : "and";
      expression = `(${expression} ${connector} ${describeGroup(groups[index])})`;
    }

    return stripOuterParens(expression);
  };

  const include = describeGroupedSequence(draft.include);
  const exclude = describeGroupedSequence(draft.exclude);

  if (include && exclude) {
    return `This entry is active when ${include} and not (${exclude}).`;
  }

  if (exclude) {
    return `This entry is active unless ${exclude}.`;
  }

  return `This entry is active when ${include}.`;
}

function getConditionRowKey(row: ConditionRow): string {
  return `${getConditionIdentity(row.condition)}::${row.joinWithPrevious}`;
}

function normalizeConditionValues(values: string[]) {
  return [...new Set(values.map((value) => value.trim().toLowerCase()).filter(Boolean))].sort();
}

function getConditionIdentity(condition: SimplePromptEntryCondition): string {
  switch (condition.type) {
    case "keywordAny":
    case "keywordAll":
    case "keywordNone":
    case "inputScopeAny":
    case "outputScopeAny":
    case "providerIdAny":
      return `${condition.type}:${normalizeConditionValues(condition.values).join("|")}`;
    default:
      return `${condition.type}:${String(condition.value)}`;
  }
}

function getScalarConditionBucket(
  condition: SimplePromptEntryCondition,
): { type: SimplePromptEntryConditionType; value: string; label: string } | null {
  switch (condition.type) {
    case "chatMode":
      return {
        type: condition.type,
        value: condition.value,
        label: describeSimpleCondition(condition),
      };
    case "sceneGenerationEnabled":
    case "avatarGenerationEnabled":
    case "hasScene":
    case "hasSceneDirection":
    case "hasPersona":
    case "dynamicMemoryEnabled":
    case "hasMemorySummary":
    case "hasKeyMemories":
    case "hasLorebookContent":
    case "hasSubjectDescription":
    case "hasCurrentDescription":
    case "hasCharacterReferenceImages":
    case "hasChatBackground":
    case "hasPersonaReferenceImages":
    case "hasCharacterReferenceText":
    case "hasPersonaReferenceText":
    case "reasoningEnabled":
    case "visionEnabled":
      return {
        type: condition.type,
        value: String(condition.value),
        label: describeSimpleCondition(condition),
      };
    default:
      return null;
  }
}

function getConditionWarnings(draft: ConditionDraft): string[] {
  const warnings = new Set<string>();
  const includeLabels = new Map<string, string>();

  draft.include.forEach((row) => {
    includeLabels.set(getConditionIdentity(row.condition), describeSimpleCondition(row.condition));
  });

  draft.exclude.forEach((row) => {
    const label = includeLabels.get(getConditionIdentity(row.condition));
    if (label) {
      warnings.add(`This entry both requires and excludes "${label}".`);
    }
  });

  const scalarBuckets = new Map<SimplePromptEntryConditionType, Map<string, string>>();
  draft.include.forEach((row) => {
    const bucket = getScalarConditionBucket(row.condition);
    if (!bucket) {
      return;
    }

    const values = scalarBuckets.get(bucket.type) ?? new Map<string, string>();
    values.set(bucket.value, bucket.label);
    scalarBuckets.set(bucket.type, values);
  });

  scalarBuckets.forEach((values) => {
    if (values.size > 1) {
      warnings.add(
        `This entry requires mutually exclusive conditions: ${[...values.values()].join(" and ")}.`,
      );
    }
  });

  return [...warnings];
}

const createDefaultEntry = (
  content: string,
  overrides?: Partial<SystemPromptEntry>,
): SystemPromptEntry => ({
  id: createEntryId(),
  name: "System Prompt",
  role: DEFAULT_ENTRY_ROLE,
  content,
  enabled: true,
  injectionPosition: DEFAULT_ENTRY_POSITION,
  injectionDepth: 0,
  conditionalMinMessages: null,
  intervalTurns: null,
  systemPrompt: true,
  conditions: null,
  promptEntryPayload: null,
  ...overrides,
});

const createExtraEntry = (overrides?: Partial<SystemPromptEntry>) =>
  createDefaultEntry("", { name: "Prompt Entry", systemPrompt: false, ...overrides });

function getPromptEntryKind(entry: SystemPromptEntry): PromptEntryKind {
  return entry.promptEntryPayload?.type === "imageSlot" ? "image" : "text";
}

function getPromptEntryImageSlot(entry: SystemPromptEntry): PromptEntryImageSlot | null {
  if (entry.promptEntryPayload?.type !== "imageSlot") {
    return null;
  }
  return entry.promptEntryPayload.slot;
}

function getAllowedImageEntrySlots(
  promptType: PromptType,
  currentSlot?: PromptEntryImageSlot | null,
): PromptEntryImageSlot[] {
  const base = promptType ? (IMAGE_ENTRY_SLOT_OPTIONS_BY_PROMPT_TYPE[promptType] ?? []) : [];
  if (currentSlot && !base.includes(currentSlot)) {
    return [...base, currentSlot];
  }
  return base;
}

function entryHasEditableContent(entry: SystemPromptEntry) {
  return entry.content.trim().length > 0 || getPromptEntryKind(entry) === "image";
}

function getEntryKindSummary(entry: SystemPromptEntry) {
  const slot = getPromptEntryImageSlot(entry);
  if (!slot) {
    return "Text";
  }
  return `Image · ${IMAGE_ENTRY_SLOT_LABELS[slot]}`;
}

function getEntryPreviewText(entry: SystemPromptEntry) {
  const trimmed = entry.content.trim();
  if (trimmed) {
    return trimmed;
  }

  const slot = getPromptEntryImageSlot(entry);
  if (slot) {
    return `${IMAGE_ENTRY_SLOT_LABELS[slot]} attachment`;
  }

  return "Click to edit this entry.";
}

function getInjectionModeHint(position: SystemPromptEntry["injectionPosition"]) {
  switch (position) {
    case "relative":
      return "Before chat history (system context).";
    case "inChat":
      return "Always inject inside chat history.";
    case "conditional":
      return "Inject only after a minimum number of chat messages.";
    case "interval":
      return "Inject every N chat messages.";
    default:
      return "";
  }
}

function getEntryRoleLabel(role: SystemPromptEntry["role"]) {
  return ENTRY_ROLE_OPTIONS.find((option) => option.value === role)?.label ?? role;
}

function getEntryPositionLabel(position: SystemPromptEntry["injectionPosition"]) {
  return ENTRY_POSITION_OPTIONS.find((option) => option.value === position)?.label ?? position;
}

function getEntryBehaviorSummary(entry: SystemPromptEntry) {
  switch (entry.injectionPosition) {
    case "conditional":
      return `After ${entry.conditionalMinMessages ?? DEFAULT_CONDITIONAL_MIN_MESSAGES} messages`;
    case "interval":
      return `Every ${entry.intervalTurns ?? DEFAULT_INTERVAL_TURNS} messages`;
    case "inChat":
      return "Inline in chat history";
    case "relative":
    default:
      return "Before chat history";
  }
}

function getEntryActivationSummary(entry: SystemPromptEntry) {
  const conditionDraft = decomposeConditionTree(entry.conditions);
  const conditionCount = conditionDraft.include.length + conditionDraft.exclude.length;
  if (conditionCount === 0) {
    return "Always active";
  }
  return conditionCount === 1 ? describeConditionTree(entry.conditions) : `${conditionCount} rules`;
}

function SummaryField({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-[120px]">
      <p className="text-[10px] font-medium uppercase tracking-wide text-fg/35">{label}</p>
      <p className="mt-1 text-sm leading-snug text-fg/78">{value}</p>
    </div>
  );
}

function ConditionRuleRow({
  condition,
  onChange,
  onRemove,
}: {
  condition: SimplePromptEntryCondition;
  onChange: (next: SimplePromptEntryCondition) => void;
  onRemove: () => void;
}) {
  const meta =
    SIMPLE_CONDITION_OPTIONS.find((option) => option.value === condition.type) ??
    SIMPLE_CONDITION_OPTIONS[0];
  const [listInput, setListInput] = useState(
    "values" in condition ? condition.values.join(", ") : "",
  );

  useEffect(() => {
    if ("values" in condition) {
      setListInput(condition.values.join(", "));
    } else {
      setListInput("");
    }
  }, [condition.type]);

  const controlClasses = cn(
    "h-8 w-full rounded border border-fg/15 bg-surface-el/50 px-2 text-xs text-fg transition-all",
    "hover:border-fg/30 focus:border-accent/50 focus:outline-none focus:ring-1 focus:ring-accent/20",
  );

  return (
    <div className="group relative flex flex-col gap-2 rounded border border-fg/10 bg-fg/2 p-2 transition-colors hover:border-fg/20 sm:flex-row sm:items-center">
      <div className="flex-shrink-0 sm:w-[160px]">
        <select
          value={condition.type}
          onChange={(event) =>
            onChange(createDefaultCondition(event.target.value as SimplePromptEntryConditionType))
          }
          className={controlClasses}
        >
          {SIMPLE_CONDITION_OPTIONS.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
      </div>

      <div className="flex flex-1 items-center gap-2">
        {meta.kind === "chatMode" ? (
          <select
            value={condition.type === "chatMode" ? condition.value : "direct"}
            onChange={(event) =>
              onChange({
                type: "chatMode",
                value: event.target.value as "direct" | "group",
              })
            }
            className={controlClasses}
          >
            <option value="direct">Direct</option>
            <option value="group">Group</option>
          </select>
        ) : meta.kind === "boolean" ? (
          <div className="flex w-full gap-0.5 rounded border border-fg/15 bg-surface-el/50 p-0.5">
            {(["true", "false"] as const).map((val) => (
              <button
                key={val}
                type="button"
                onClick={() =>
                  onChange({
                    ...condition,
                    value: val === "true",
                  } as SimplePromptEntryCondition)
                }
                className={cn(
                  "flex-1 rounded-sm py-1 text-[10px] font-bold uppercase tracking-wider transition-colors",
                  ("value" in condition ? String(condition.value) : "false") === val
                    ? "bg-fg text-surface"
                    : "text-fg/40 hover:bg-fg/5 hover:text-fg/60",
                )}
              >
                {val === "true" ? "True" : "False"}
              </button>
            ))}
          </div>
        ) : meta.kind === "number" ? (
          <input
            type="number"
            min={condition.type === "messageCountAtLeast" ? 0 : 1}
            value={"value" in condition ? Number(condition.value) : 1}
            onChange={(event) =>
              onChange({
                ...condition,
                value: Math.max(
                  condition.type === "messageCountAtLeast" ? 0 : 1,
                  Number(event.target.value) || 0,
                ),
              } as SimplePromptEntryCondition)
            }
            className={controlClasses}
          />
        ) : (
          <input
            type="text"
            value={listInput}
            onChange={(event) => {
              const nextInput = event.target.value;
              setListInput(nextInput);
              onChange({
                ...condition,
                values: normalizeListInput(nextInput),
              } as SimplePromptEntryCondition);
            }}
            placeholder={meta.placeholder}
            className={controlClasses}
          />
        )}
      </div>

      <button
        type="button"
        onClick={onRemove}
        className={cn(
          "flex h-8 w-8 shrink-0 items-center justify-center rounded border border-fg/10 bg-surface-el/50 text-fg/40 transition-colors",
          "hover:border-danger/30 hover:bg-danger/5 hover:text-danger",
        )}
        title="Remove condition"
      >
        <X className="h-3.5 w-3.5" />
      </button>
    </div>
  );
}

function ConditionSequenceItem({
  showJoin,
  row,
  onJoinChange,
  onChange,
  onRemove,
}: {
  showJoin: boolean;
  row: ConditionRow;
  onJoinChange: (next: ConditionJoin) => void;
  onChange: (next: SimplePromptEntryCondition) => void;
  onRemove: () => void;
}) {
  return (
    <div className="list-none">
      <div className="space-y-3">
        {showJoin ? (
          <ConditionJoinRow value={row.joinWithPrevious} onChange={onJoinChange} />
        ) : null}
        <ConditionRuleRow condition={row.condition} onChange={onChange} onRemove={onRemove} />
      </div>
    </div>
  );
}

function ConditionJoinRow({
  value,
  onChange,
}: {
  value: ConditionJoin;
  onChange: (next: ConditionJoin) => void;
}) {
  return (
    <div className="relative flex items-center justify-center py-1">
      <div className="absolute inset-x-0 top-1/2 h-px -translate-y-1/2 bg-fg/10" />
      <select
        value={value}
        onChange={(event) => onChange(event.target.value as ConditionJoin)}
        className={cn(
          "relative z-10 flex h-6 items-center rounded-full border border-fg/15 bg-surface-el/80 px-3 text-[10px] font-bold uppercase tracking-wider text-fg/60",
          "hover:border-fg/30 hover:text-fg/80 focus:outline-none focus:ring-1 focus:ring-fg/20",
        )}
      >
        <option value="and">AND</option>
        <option value="or">OR</option>
      </select>
    </div>
  );
}

function PromptEntryConditionsPanel({
  entry,
  onUpdate,
}: {
  entry: SystemPromptEntry;
  onUpdate: (updates: Partial<SystemPromptEntry>) => void;
}) {
  const draft = useMemo(() => decomposeConditionTree(entry.conditions), [entry.conditions]);
  const warnings = useMemo(() => getConditionWarnings(draft), [draft]);
  const includeGroups = useMemo(() => groupConditionRows(draft.include), [draft.include]);
  const excludeGroups = useMemo(() => groupConditionRows(draft.exclude), [draft.exclude]);

  const commit = (next: ConditionDraft) => {
    onUpdate({ conditions: composeConditionTree(next) });
  };

  const addRule = (target: "include" | "exclude") => {
    const current = draft[target];
    commit({
      ...draft,
      [target]: [
        ...current,
        {
          condition: createDefaultCondition(),
          joinWithPrevious: "and",
        },
      ],
    });
  };

  const updateRule = (
    target: "include" | "exclude",
    index: number,
    nextCondition: SimplePromptEntryCondition,
  ) => {
    commit({
      ...draft,
      [target]: draft[target].map((row: ConditionRow, idx: number) =>
        idx === index ? { ...row, condition: nextCondition } : row,
      ),
    });
  };

  const updateJoin = (target: "include" | "exclude", index: number, nextJoin: ConditionJoin) => {
    commit({
      ...draft,
      [target]: draft[target].map((row: ConditionRow, idx: number) =>
        idx === index ? { ...row, joinWithPrevious: nextJoin } : row,
      ),
    });
  };

  const removeRule = (target: "include" | "exclude", index: number) => {
    commit({
      ...draft,
      [target]: draft[target].filter((_row: ConditionRow, idx: number) => idx !== index),
    });
  };

  return (
    <div className="space-y-8">
      {warnings.length > 0 && (
        <div className="rounded-lg border border-danger/20 bg-danger/5 px-3.5 py-3">
          <div className="flex items-start gap-2.5">
            <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-danger/75" />
            <div className="min-w-0 space-y-1">
              <p className="text-xs font-medium text-danger/85">
                Fix contradictory rules before saving.
              </p>
              {warnings.map((warning) => (
                <p key={warning} className="text-xs leading-relaxed text-fg/68">
                  {warning}
                </p>
              ))}
            </div>
          </div>
        </div>
      )}

      <section className="space-y-4">
        <div className="flex items-center justify-between">
          <h4 className="text-sm font-bold text-fg/60">Include Rules</h4>
          <button
            type="button"
            onClick={() => addRule("include")}
            className="flex items-center gap-1.5 rounded-lg border border-accent/20 bg-accent/5 px-3 py-1.5 text-xs font-bold text-accent transition-all hover:bg-accent/10"
          >
            <Plus className="h-3.5 w-3.5" />
            Add Rule
          </button>
        </div>

        <p className="max-w-[72ch] text-[11px] leading-relaxed text-fg/42">
          Adjacent rules with the same join are grouped together. Those groups are then evaluated
          from top to bottom.
        </p>

        <div className="space-y-3">
          {draft.include.length > 0 ? (
            includeGroups.map((group, groupIndex) => {
              const groupStartIndex = includeGroups
                .slice(0, groupIndex)
                .reduce((count, current) => count + current.rows.length, 0);

              return (
                <div key={`include-group-${entry.id}-${groupIndex}`} className="space-y-3">
                  {groupIndex > 0 && group.join ? (
                    <ConditionJoinRow
                      value={group.join}
                      onChange={(next) => updateJoin("include", groupStartIndex, next)}
                    />
                  ) : null}
                  <div className="rounded-xl border border-fg/10 bg-fg/3 p-3">
                    <div className="space-y-3">
                      {group.rows.map((row: ConditionRow, index: number) => {
                        const flatIndex = groupStartIndex + index;

                        return (
                          <ConditionSequenceItem
                            key={`include-${entry.id}-${getConditionRowKey(row)}`}
                            showJoin={index > 0}
                            row={row}
                            onJoinChange={(next) => updateJoin("include", flatIndex, next)}
                            onChange={(next) => updateRule("include", flatIndex, next)}
                            onRemove={() => removeRule("include", flatIndex)}
                          />
                        );
                      })}
                    </div>
                  </div>
                </div>
              );
            })
          ) : (
            <div className="rounded-xl border border-dashed border-fg/10 bg-fg/2 py-8 text-center">
              <p className="text-sm text-fg/30">Entry is active for all messages.</p>
            </div>
          )}
        </div>
      </section>

      <section className="space-y-4">
        <div className="flex items-center justify-between">
          <h4 className="text-sm font-bold text-fg/60">Exclusions</h4>
          <button
            type="button"
            onClick={() => addRule("exclude")}
            className="flex items-center gap-1.5 rounded-lg border border-danger/20 bg-danger/5 px-3 py-1.5 text-xs font-bold text-danger/70 transition-all hover:bg-danger/10"
          >
            <Plus className="h-3.5 w-3.5" />
            Add Exclusion
          </button>
        </div>

        <div className="space-y-3">
          {draft.exclude.length > 0 ? (
            excludeGroups.map((group, groupIndex) => {
              const groupStartIndex = excludeGroups
                .slice(0, groupIndex)
                .reduce((count, current) => count + current.rows.length, 0);

              return (
                <div key={`exclude-group-${entry.id}-${groupIndex}`} className="space-y-3">
                  {groupIndex > 0 && group.join ? (
                    <ConditionJoinRow
                      value={group.join}
                      onChange={(next) => updateJoin("exclude", groupStartIndex, next)}
                    />
                  ) : null}
                  <div className="rounded-xl border border-fg/10 bg-fg/3 p-3">
                    <div className="space-y-3">
                      {group.rows.map((row: ConditionRow, index: number) => {
                        const flatIndex = groupStartIndex + index;

                        return (
                          <ConditionSequenceItem
                            key={`exclude-${entry.id}-${getConditionRowKey(row)}`}
                            showJoin={index > 0}
                            row={row}
                            onJoinChange={(next) => updateJoin("exclude", flatIndex, next)}
                            onChange={(next) => updateRule("exclude", flatIndex, next)}
                            onRemove={() => removeRule("exclude", flatIndex)}
                          />
                        );
                      })}
                    </div>
                  </div>
                </div>
              );
            })
          ) : (
            <div className="rounded-xl border border-dashed border-fg/10 bg-fg/2 py-6 text-center">
              <p className="text-sm text-fg/30">No exclusions defined.</p>
            </div>
          )}
        </div>
      </section>

      <div className="border-t border-fg/10 pt-4">
        <div className="flex items-center gap-2">
          <Code2 className="h-3.5 w-3.5 text-fg/26" />
          <p className="text-[10px] font-medium uppercase tracking-wide text-fg/30">Evaluates As</p>
        </div>
        <p className="mt-2 max-w-[72ch] text-sm leading-relaxed text-fg/58">
          {describeConditionSentence(entry.conditions)}
        </p>
      </div>
    </div>
  );
}

function PromptEntryEditorForm({
  entry,
  promptType,
  onUpdate,
  onToggle,
  onTextareaRef,
  onTextareaFocus,
  contentRows = 20,
}: {
  entry: SystemPromptEntry;
  promptType: PromptType;
  onUpdate: (updates: Partial<SystemPromptEntry>) => void;
  onToggle?: () => void;
  onTextareaRef: (id: string, el: HTMLTextAreaElement | null) => void;
  onTextareaFocus: (id: string) => void;
  contentRows?: number;
}) {
  const toggleId = `entry-editor-toggle-${entry.id}`;
  const entryKind = getPromptEntryKind(entry);
  const currentSlot = getPromptEntryImageSlot(entry);
  const imageSlotOptions = getAllowedImageEntrySlots(promptType, currentSlot);
  const supportsImageEntries = imageSlotOptions.length > 0 || entryKind === "image";
  const canSelectImageKind = supportsImageEntries && (!entry.systemPrompt || entryKind === "image");
  const selectedImageSlot = currentSlot ?? imageSlotOptions[0] ?? "character";
  const roleValue = entryKind === "image" ? "user" : entry.role;
  const roleDescription =
    entryKind === "image"
      ? "Image attachment entries are always sent as user content."
      : "Which role the model receives for this entry.";
  const contentLabel = entryKind === "image" ? "Attachment Note" : "Prompt Content";
  const contentHint =
    entryKind === "image"
      ? "Optional text sent alongside the attached image slot. Leave blank if the image alone is enough."
      : "Write the prompt entry exactly as it should be sent.";
  const contentPlaceholder =
    entryKind === "image"
      ? "Optional note for this image attachment..."
      : "Write the prompt entry...";
  const showsLegacyImageToken = Object.values(IMAGE_ENTRY_SLOT_TOKENS).includes(
    entry.content.trim(),
  );

  return (
    <div className="space-y-5">
      <div className="flex items-center justify-between gap-3 rounded-lg border border-fg/10 bg-fg/4 px-3 py-2.5">
        <div>
          <p className="text-sm font-medium text-fg">Entry State</p>
          <p className="text-xs text-fg/45">
            {entry.systemPrompt
              ? "System prompt entries are always enabled."
              : "Controls whether this entry can be injected."}
          </p>
        </div>
        <div className="flex items-center gap-3">
          <Switch
            id={toggleId}
            checked={entry.enabled || entry.systemPrompt}
            onChange={() => onToggle?.()}
            disabled={entry.systemPrompt || !onToggle}
          />
          <span className="text-xs text-fg/55">
            {entry.systemPrompt ? "Required" : entry.enabled ? "Enabled" : "Disabled"}
          </span>
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <div className="space-y-1.5">
          <label className="text-xs font-medium text-fg/55">Entry Name</label>
          <input
            value={entry.name}
            onChange={(event) => onUpdate({ name: event.target.value })}
            className="w-full rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-sm text-fg"
            placeholder="Entry name"
          />
          <p className="text-[11px] text-fg/45">Used for organization and quick scanning.</p>
        </div>

        <div className="space-y-1.5">
          <label className="text-xs font-medium text-fg/55">Entry Kind</label>
          <select
            value={entryKind}
            onChange={(event) => {
              const nextKind = event.target.value as PromptEntryKind;
              if (nextKind === "image") {
                const nextSlot = imageSlotOptions[0] ?? "character";
                onUpdate({
                  promptEntryPayload: {
                    type: "imageSlot",
                    slot: nextSlot,
                  },
                  role: "user",
                  injectionPosition:
                    entry.injectionPosition === "relative" ? "inChat" : entry.injectionPosition,
                  content:
                    entry.content.trim().length > 0 &&
                    !Object.values(IMAGE_ENTRY_SLOT_TOKENS).includes(entry.content.trim())
                      ? entry.content
                      : "",
                });
                return;
              }

              onUpdate({
                promptEntryPayload: null,
                content: Object.values(IMAGE_ENTRY_SLOT_TOKENS).includes(entry.content.trim())
                  ? ""
                  : entry.content,
              });
            }}
            className="h-10 w-full rounded-lg border border-fg/10 bg-fg/5 px-3 text-sm text-fg"
          >
            <option value="text">Text</option>
            {canSelectImageKind ? <option value="image">Image attachment</option> : null}
          </select>
          <p className="text-[11px] text-fg/45">
            {entryKind === "image"
              ? "Attach a runtime image slot instead of relying on raw {{image[...]}} tokens."
              : canSelectImageKind
                ? "Standard text content. Switch to image attachment for reference-image entries."
                : "Standard text content for this prompt type."}
          </p>
        </div>
      </div>

      {entryKind === "image" ? (
        <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_220px]">
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-fg/55">Attachment Slot</label>
            <select
              value={selectedImageSlot}
              onChange={(event) => {
                const nextSlot = event.target.value as PromptEntryImageSlot;
                onUpdate({
                  promptEntryPayload: {
                    type: "imageSlot",
                    slot: nextSlot,
                  },
                  role: "user",
                  content:
                    entry.content.trim().length === 0 ||
                    Object.values(IMAGE_ENTRY_SLOT_TOKENS).includes(entry.content.trim())
                      ? ""
                      : entry.content,
                });
              }}
              className="h-10 w-full rounded-lg border border-fg/10 bg-fg/5 px-3 text-sm text-fg"
            >
              {imageSlotOptions.map((slot) => (
                <option key={slot} value={slot}>
                  {IMAGE_ENTRY_SLOT_LABELS[slot]}
                </option>
              ))}
            </select>
            <p className="text-[11px] text-fg/45">
              Picks which runtime image source this entry attaches.
            </p>
          </div>

          <div className="rounded-lg border border-fg/10 bg-fg/4 px-3 py-2.5">
            <p className="text-[10px] font-medium uppercase tracking-wide text-fg/35">
              Attachment Behavior
            </p>
            <p className="mt-1.5 text-sm leading-relaxed text-fg/72">
              Sends the selected image slot as user content. Any note below is sent with it.
            </p>
          </div>
        </div>
      ) : null}

      <div className="grid gap-4 md:grid-cols-2">
        <div className="space-y-1.5">
          <label className="text-xs font-medium text-fg/55">Role</label>
          <select
            value={roleValue}
            onChange={(event) => onUpdate({ role: event.target.value as any })}
            disabled={entryKind === "image"}
            className="h-10 w-full rounded-lg border border-fg/10 bg-fg/5 px-3 text-sm text-fg disabled:cursor-not-allowed disabled:opacity-60"
          >
            {ENTRY_ROLE_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
          <p className="text-[11px] text-fg/45">{roleDescription}</p>
        </div>

        <div className="space-y-1.5">
          <label className="text-xs font-medium text-fg/55">Placement</label>
          <select
            value={entry.injectionPosition}
            onChange={(event) => {
              const nextPosition = event.target.value as SystemPromptEntry["injectionPosition"];
              onUpdate({
                injectionPosition: nextPosition,
                conditionalMinMessages:
                  nextPosition === "conditional"
                    ? (entry.conditionalMinMessages ?? DEFAULT_CONDITIONAL_MIN_MESSAGES)
                    : (entry.conditionalMinMessages ?? null),
                intervalTurns:
                  nextPosition === "interval"
                    ? (entry.intervalTurns ?? DEFAULT_INTERVAL_TURNS)
                    : (entry.intervalTurns ?? null),
              });
            }}
            className="h-10 w-full rounded-lg border border-fg/10 bg-fg/5 px-3 text-sm text-fg"
          >
            {ENTRY_POSITION_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
          <p className="text-[11px] text-fg/45">{getInjectionModeHint(entry.injectionPosition)}</p>
        </div>
      </div>

      <div
        className={cn(
          "grid gap-4",
          entry.injectionPosition === "conditional" || entry.injectionPosition === "interval"
            ? "md:grid-cols-2"
            : "md:grid-cols-1",
        )}
      >
        <div className="space-y-1.5">
          <label className="text-xs font-medium text-fg/55">Insertion Depth</label>
          <input
            type="number"
            min={0}
            value={entry.injectionDepth}
            onChange={(event) => onUpdate({ injectionDepth: Number(event.target.value) })}
            className="h-10 w-full rounded-lg border border-fg/10 bg-fg/5 px-3 text-sm text-fg"
            placeholder="0"
          />
          <p className="text-[11px] text-fg/45">
            Depth 0 is newest; higher numbers insert earlier.
          </p>
        </div>

        {entry.injectionPosition === "conditional" ? (
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-fg/55">Min Messages</label>
            <input
              type="number"
              min={1}
              value={entry.conditionalMinMessages ?? DEFAULT_CONDITIONAL_MIN_MESSAGES}
              onChange={(event) =>
                onUpdate({
                  conditionalMinMessages: Math.max(1, Number(event.target.value) || 1),
                })
              }
              className="h-10 w-full rounded-lg border border-fg/10 bg-fg/5 px-3 text-sm text-fg"
            />
            <p className="text-[11px] text-fg/45">
              Inject only when at least this many messages are present.
            </p>
          </div>
        ) : entry.injectionPosition === "interval" ? (
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-fg/55">Every N Messages</label>
            <input
              type="number"
              min={1}
              value={entry.intervalTurns ?? DEFAULT_INTERVAL_TURNS}
              onChange={(event) =>
                onUpdate({
                  intervalTurns: Math.max(1, Number(event.target.value) || 1),
                })
              }
              className="h-10 w-full rounded-lg border border-fg/10 bg-fg/5 px-3 text-sm text-fg"
            />
            <p className="text-[11px] text-fg/45">Inject every N context turns.</p>
          </div>
        ) : null}
      </div>

      <PromptEntryConditionsPanel entry={entry} onUpdate={onUpdate} />

      <div className="space-y-1.5">
        <label className="text-xs font-medium text-fg/55">{contentLabel}</label>
        <textarea
          ref={(el) => {
            onTextareaRef(entry.id, el);
          }}
          value={entry.content}
          onChange={(event) => onUpdate({ content: event.target.value })}
          onFocus={() => onTextareaFocus(entry.id)}
          rows={contentRows}
          className="w-full resize-none rounded-xl border border-fg/10 bg-surface-el/30 px-3.5 py-3 font-mono text-sm leading-relaxed text-fg placeholder-fg/30"
          placeholder={contentPlaceholder}
        />
        <p className="text-[11px] text-fg/45">{contentHint}</p>
        {entryKind === "image" && showsLegacyImageToken ? (
          <p className="text-[11px] text-fg/38">
            Legacy token detected. It is no longer required once this entry has an image attachment.
          </p>
        ) : null}
      </div>
    </div>
  );
}

function DesktopEntryEditorDrawer({
  entry,
  promptType,
  isOpen,
  onClose,
  onUpdate,
  onToggle,
  onTextareaRef,
  onTextareaFocus,
}: {
  entry: SystemPromptEntry | null;
  promptType: PromptType;
  isOpen: boolean;
  onClose: () => void;
  onUpdate: (id: string, updates: Partial<SystemPromptEntry>) => void;
  onToggle: (id: string) => void;
  onTextareaRef: (id: string, el: HTMLTextAreaElement | null) => void;
  onTextareaFocus: (id: string) => void;
}) {
  useEffect(() => {
    if (!isOpen) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, onClose]);

  return (
    <AnimatePresence>
      {isOpen && entry ? (
        <>
          <motion.button
            type="button"
            className="fixed inset-0 z-40 hidden bg-black/45 lg:block"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={onClose}
          />
          <motion.aside
            className="fixed inset-y-0 left-0 z-50 hidden w-[min(560px,48vw)] border-r border-fg/10 bg-surface lg:flex lg:flex-col"
            initial={{ x: "-100%" }}
            animate={{ x: 0 }}
            exit={{ x: "-100%" }}
            transition={{ duration: 0.18, ease: "easeOut" }}
          >
            <div className="flex items-center justify-between border-b border-fg/10 px-5 py-4">
              <div>
                <h2 className="text-base font-semibold text-fg">Edit Entry</h2>
                <p className="text-xs text-fg/45">{entry.name || "Prompt Entry"}</p>
              </div>
              <button
                type="button"
                onClick={onClose}
                className="rounded-md border border-fg/10 px-3 py-1.5 text-xs font-medium text-fg/65 transition hover:bg-fg/8 hover:text-fg"
              >
                Close
              </button>
            </div>

            <div className="flex-1 overflow-y-auto px-5 py-5">
              <PromptEntryEditorForm
                entry={entry}
                promptType={promptType}
                onUpdate={(updates) => onUpdate(entry.id, updates)}
                onToggle={() => onToggle(entry.id)}
                onTextareaRef={onTextareaRef}
                onTextareaFocus={onTextareaFocus}
                contentRows={20}
              />
            </div>
          </motion.aside>
        </>
      ) : null}
    </AnimatePresence>
  );
}

function MobileEntryEditorPage({
  entry,
  promptType,
  isOpen,
  onClose,
  onUpdate,
  onToggle,
  onTextareaRef,
  onTextareaFocus,
}: {
  entry: SystemPromptEntry | null;
  promptType: PromptType;
  isOpen: boolean;
  onClose: () => void;
  onUpdate: (id: string, updates: Partial<SystemPromptEntry>) => void;
  onToggle: (id: string) => void;
  onTextareaRef: (id: string, el: HTMLTextAreaElement | null) => void;
  onTextareaFocus: (id: string) => void;
}) {
  return (
    <AnimatePresence>
      {isOpen && entry ? (
        <motion.div
          className="fixed inset-0 z-50 flex flex-col bg-surface lg:hidden"
          style={{ paddingTop: "env(safe-area-inset-top)" }}
          initial={{ x: "100%" }}
          animate={{ x: 0 }}
          exit={{ x: "100%" }}
          transition={{ duration: 0.18, ease: "easeOut" }}
        >
          <div className="flex items-center justify-between border-b border-fg/10 px-4 py-3">
            <div>
              <h2 className="text-base font-semibold text-fg">Edit Entry</h2>
              <p className="text-xs text-fg/45">{entry.name || "Prompt Entry"}</p>
            </div>
            <button
              type="button"
              onClick={onClose}
              className="rounded-md border border-fg/10 px-3 py-1.5 text-xs font-medium text-fg/65 transition hover:bg-fg/8 hover:text-fg"
            >
              Done
            </button>
          </div>

          <div className="flex-1 overflow-y-auto px-4 py-4">
            <PromptEntryEditorForm
              entry={entry}
              promptType={promptType}
              onUpdate={(updates) => onUpdate(entry.id, updates)}
              onToggle={() => onToggle(entry.id)}
              onTextareaRef={onTextareaRef}
              onTextareaFocus={onTextareaFocus}
              contentRows={20}
            />
          </div>
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}

const entriesToContent = (entries: SystemPromptEntry[]) =>
  entries
    .map((entry) => entry.content.trim())
    .filter(Boolean)
    .join("\n\n");

function entriesToValidationSource(entries: SystemPromptEntry[]) {
  return entries
    .flatMap((entry) => {
      const parts: string[] = [];
      const trimmed = entry.content.trim();
      if (trimmed) {
        parts.push(trimmed);
      }

      const slot = getPromptEntryImageSlot(entry);
      if (slot) {
        parts.push(IMAGE_ENTRY_SLOT_TOKENS[slot]);
      }

      return parts;
    })
    .join("\n\n");
}

const ensureSystemEntry = (entries: SystemPromptEntry[]) => {
  if (entries.length === 0) return [createDefaultEntry("")];
  if (entries.some((entry) => entry.systemPrompt)) return entries;
  return [{ ...entries[0], systemPrompt: true, enabled: true }, ...entries.slice(1)];
};

function PromptEntryCard({
  entry,
  onUpdate,
  onDelete,
  onToggle,
  onToggleCollapse,
  collapsed,
  highlighted,
  onOpenEditor,
}: {
  entry: SystemPromptEntry;
  onUpdate: (id: string, updates: Partial<SystemPromptEntry>) => void;
  onDelete: (id: string) => void;
  onToggle: (id: string) => void;
  onToggleCollapse: (id: string) => void;
  collapsed: boolean;
  highlighted?: boolean;
  onOpenEditor: () => void;
}) {
  const { t } = useI18n();
  const controls = useDragControls();
  const autoScroll = useDragEdgeAutoScroll();
  const toggleId = `prompt-entry-${entry.id}`;
  const conditionSummary = getEntryActivationSummary(entry);
  const contentPreview = getEntryPreviewText(entry);
  const isImageEntry = getPromptEntryKind(entry) === "image";

  return (
    <Reorder.Item
      id={`prompt-entry-row-${entry.id}`}
      value={entry}
      layout="position"
      dragListener={false}
      dragControls={controls}
      dragMomentum={false}
      dragElastic={0}
      whileDrag={{
        zIndex: 50,
        boxShadow:
          "0 24px 48px rgba(0,0,0,0.45), 0 8px 16px rgba(0,0,0,0.3), 0 0 0 1px rgba(255,255,255,0.08)",
      }}
      transition={{ layout: { duration: 0.2, ease: "easeOut" } }}
      style={{ position: "relative", zIndex: 0 }}
      onDragStart={(event, info) => {
        autoScroll.start(event.currentTarget as HTMLElement, info.point.y);
      }}
      onDrag={(_event, info) => {
        autoScroll.update(info.point.y);
      }}
      onDragEnd={() => {
        autoScroll.stop();
      }}
      className={cn(
        "rounded-xl border bg-fg/5 p-4 space-y-3 cursor-default transition-all",
        highlighted
          ? "border-accent/50 ring-2 ring-accent/30 ring-offset-1 ring-offset-black"
          : "border-fg/10",
      )}
    >
      <div className="flex flex-wrap items-center gap-2">
        <button
          onPointerDown={(event) => controls.start(event)}
          className={cn(
            "flex h-8 w-8 items-center justify-center rounded-lg cursor-grab active:cursor-grabbing",
            "border border-fg/10 bg-fg/5 text-fg/40",
          )}
          style={{ touchAction: "none" }}
          title="Drag to reorder"
        >
          <GripVertical className="h-4 w-4" />
        </button>

        <button
          onClick={() => onToggleCollapse(entry.id)}
          className={cn(
            "flex h-8 w-8 items-center justify-center rounded-lg",
            "border border-fg/10 bg-fg/5 text-fg/40",
          )}
          title={collapsed ? "Expand entry" : "Collapse entry"}
        >
          {collapsed ? <ChevronDown className="h-4 w-4" /> : <ChevronUp className="h-4 w-4" />}
        </button>

        <input
          value={entry.name}
          onChange={(event) => onUpdate(entry.id, { name: event.target.value })}
          className="flex-1 rounded-lg border border-fg/10 bg-surface-el/30 px-3 py-2 text-sm text-fg"
          placeholder="Entry name"
        />

        <div className="flex items-center gap-2">
          <div className="flex items-center gap-3">
            <span
              onClick={(event) => event.stopPropagation()}
              title={entry.systemPrompt ? "System prompt entries are always enabled" : "Toggle"}
            >
              <Switch
                id={toggleId}
                checked={entry.enabled || entry.systemPrompt}
                onChange={() => onToggle(entry.id)}
                disabled={entry.systemPrompt}
              />
            </span>
            <span className="text-xs text-fg/50">
              {entry.systemPrompt ? "Required" : entry.enabled ? "Enabled" : "Disabled"}
            </span>
          </div>

          {!entry.systemPrompt && (
            <button
              onClick={() => onDelete(entry.id)}
              className={cn(
                "rounded-lg border border-fg/10 p-2 text-fg/40",
                "hover:border-danger/40 hover:bg-danger/10 hover:text-danger/80",
              )}
              title={t("common.buttons.delete")}
            >
              <Trash2 className="h-4 w-4" />
            </button>
          )}
        </div>
      </div>

      <AnimatePresence initial={false}>
        {!collapsed && (
          <motion.div
            key={`prompt-entry-body-${entry.id}`}
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.18, ease: "easeOut" }}
            className="overflow-hidden"
          >
            <div className="space-y-3 pt-0.5">
              <div className="rounded-lg border border-fg/10 bg-surface-el/16 px-3.5 py-3">
                <div className="flex flex-wrap gap-x-6 gap-y-3">
                  <SummaryField
                    label="Role"
                    value={
                      getEntryKindSummary(entry) === "Text" ? getEntryRoleLabel(entry.role) : "User"
                    }
                  />
                  <SummaryField label="Kind" value={getEntryKindSummary(entry)} />
                  <SummaryField
                    label="Placement"
                    value={getEntryPositionLabel(entry.injectionPosition)}
                  />
                  <SummaryField label="Behavior" value={getEntryBehaviorSummary(entry)} />
                  <SummaryField label="Activation" value={conditionSummary} />
                  {entry.injectionDepth > 0 ? (
                    <SummaryField label="Depth" value={String(entry.injectionDepth)} />
                  ) : null}
                </div>
              </div>

              <p className="text-[11px] text-fg/50">
                {getInjectionModeHint(entry.injectionPosition)}
              </p>

              {isImageEntry ? (
                <div className="flex items-center justify-between gap-3 rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3">
                  <div className="min-w-0">
                    <p className="text-[11px] font-medium uppercase tracking-wide text-fg/40">
                      Prompt Content
                    </p>
                    <p className="mt-1 truncate text-sm text-fg/72">
                      {entry.content.trim() || "No note added."}
                    </p>
                  </div>
                  <button
                    type="button"
                    onClick={onOpenEditor}
                    className={cn(
                      "shrink-0 rounded-md border border-fg/10 px-2.5 py-1.5 text-xs font-medium text-fg/62 transition-colors",
                      "hover:border-fg/20 hover:bg-surface-el/30 hover:text-fg",
                    )}
                  >
                    Edit
                  </button>
                </div>
              ) : (
                <button
                  type="button"
                  onClick={onOpenEditor}
                  className={cn(
                    "block w-full rounded-xl border border-fg/10 bg-surface-el/20 p-3 text-left transition-colors",
                    "hover:border-fg/20 hover:bg-surface-el/30",
                  )}
                >
                  <div className="flex items-center justify-between gap-3">
                    <span className="text-[11px] font-medium uppercase tracking-wide text-fg/40">
                      Prompt Content
                    </span>
                    <span className="text-[11px] text-fg/45">Open editor</span>
                  </div>
                  <div className="mt-3 max-h-64 overflow-auto whitespace-pre-wrap rounded-lg border border-fg/10 bg-black/10 px-3.5 py-3 font-mono text-sm leading-relaxed text-fg/82">
                    {contentPreview}
                  </div>
                </button>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </Reorder.Item>
  );
}

function PromptEntryListItem({
  entry,
  onToggle,
  onDelete,
  onEdit,
}: {
  entry: SystemPromptEntry;
  onToggle: (id: string) => void;
  onDelete: (id: string) => void;
  onEdit: (id: string) => void;
}) {
  const { t } = useI18n();
  const controls = useDragControls();
  const autoScroll = useDragEdgeAutoScroll();
  const conditionSummary = describeConditionTree(entry.conditions);
  const dragTimeoutRef = useRef<number | null>(null);
  const draggingRef = useRef(false);
  const pendingEventRef = useRef<PointerEvent | null>(null);
  const scrollLockRef = useRef<{
    el: HTMLElement;
    overflow: string;
    touchAction: string;
  } | null>(null);
  const toggleId = `prompt-entry-mobile-${entry.id}`;

  const scheduleDragStart = (event: React.PointerEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();
    pendingEventRef.current = event.nativeEvent;
    if (dragTimeoutRef.current) {
      window.clearTimeout(dragTimeoutRef.current);
    }
    dragTimeoutRef.current = window.setTimeout(() => {
      dragTimeoutRef.current = null;
      const pendingEvent = pendingEventRef.current;
      if (pendingEvent) {
        draggingRef.current = true;
        controls.start(pendingEvent);
      }
    }, DRAG_HOLD_MS);
  };

  const cancelDragStart = () => {
    if (dragTimeoutRef.current) {
      window.clearTimeout(dragTimeoutRef.current);
      dragTimeoutRef.current = null;
    }
  };

  const cancelDragStartWithRelease = () => {
    cancelDragStart();
    draggingRef.current = false;
    pendingEventRef.current = null;
  };

  const lockScrollContainer = () => {
    const scrollEl = document.querySelector("main") as HTMLElement | null;
    if (!scrollEl || scrollLockRef.current) return;
    scrollLockRef.current = {
      el: scrollEl,
      overflow: scrollEl.style.overflow,
      touchAction: scrollEl.style.touchAction,
    };
    scrollEl.style.overflow = "hidden";
    scrollEl.style.touchAction = "none";
  };

  const unlockScrollContainer = () => {
    if (!scrollLockRef.current) return;
    const { el, overflow, touchAction } = scrollLockRef.current;
    el.style.overflow = overflow;
    el.style.touchAction = touchAction;
    scrollLockRef.current = null;
  };

  useEffect(() => {
    return () => {
      unlockScrollContainer();
      if (draggingRef.current) {
        document.body.style.overflow = "";
        document.body.style.touchAction = "";
        draggingRef.current = false;
      }
    };
  }, []);

  return (
    <Reorder.Item
      id={`prompt-entry-row-mobile-${entry.id}`}
      value={entry}
      layout
      dragListener={false}
      dragControls={controls}
      dragMomentum={false}
      dragElastic={0}
      whileDrag={{
        zIndex: 50,
        boxShadow:
          "0 24px 48px rgba(0,0,0,0.45), 0 8px 16px rgba(0,0,0,0.3), 0 0 0 1px rgba(255,255,255,0.08)",
      }}
      transition={{ layout: { duration: 0.2, ease: "easeOut" } }}
      style={{ position: "relative", zIndex: 0 }}
      onDragStart={(event, info: PanInfo) => {
        draggingRef.current = true;
        document.body.style.overflow = "hidden";
        document.body.style.touchAction = "none";
        lockScrollContainer();
        autoScroll.start(event.currentTarget as HTMLElement, info.point.y);
      }}
      onDrag={(_event, info: PanInfo) => {
        autoScroll.update(info.point.y);
      }}
      onDragEnd={() => {
        draggingRef.current = false;
        document.body.style.overflow = "";
        document.body.style.touchAction = "";
        unlockScrollContainer();
        autoScroll.stop();
      }}
      onPointerMove={(event) => {
        if (dragTimeoutRef.current) {
          pendingEventRef.current = event.nativeEvent;
        }
        if (draggingRef.current) {
          event.preventDefault();
        }
      }}
      onPointerUp={() => {
        draggingRef.current = false;
        pendingEventRef.current = null;
        unlockScrollContainer();
        autoScroll.stop();
      }}
      onPointerCancel={() => {
        draggingRef.current = false;
        pendingEventRef.current = null;
        unlockScrollContainer();
        autoScroll.stop();
      }}
      className={cn("rounded-xl border border-fg/10 bg-fg/5 p-3 select-none", "space-y-2")}
    >
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2 min-w-0">
          <button
            onPointerDown={scheduleDragStart}
            onPointerUp={cancelDragStartWithRelease}
            onPointerLeave={cancelDragStartWithRelease}
            onPointerCancel={cancelDragStartWithRelease}
            onContextMenu={(event) => event.preventDefault()}
            className={cn(
              "flex h-8 w-8 items-center justify-center rounded-lg",
              "border border-fg/10 bg-fg/5 text-fg/40",
            )}
            style={{ touchAction: "none" }}
            title="Drag to reorder"
          >
            <GripVertical className="h-4 w-4" />
          </button>
          <div className="min-w-0">
            <p className="text-sm font-medium text-fg truncate">{entry.name}</p>
            <p className="text-[11px] text-fg/40 uppercase tracking-wide">
              {getEntryKindSummary(entry)} · {entry.injectionPosition}
            </p>
            {entry.conditions && (
              <p className="mt-0.5 text-[11px] text-fg/35 truncate">{conditionSummary}</p>
            )}
          </div>
        </div>

        <div className="flex items-center gap-2">
          <div className="flex items-center gap-2">
            <span
              onClick={(event) => event.stopPropagation()}
              title={entry.systemPrompt ? "System prompt entries are always enabled" : "Toggle"}
            >
              <Switch
                id={toggleId}
                checked={entry.enabled || entry.systemPrompt}
                onChange={() => onToggle(entry.id)}
                disabled={entry.systemPrompt}
              />
            </span>
          </div>

          <button
            onClick={() => onEdit(entry.id)}
            className={cn(
              "rounded-lg border border-fg/10 px-3 py-1.5 text-xs font-medium text-fg/70",
              "hover:bg-fg/10 hover:text-fg",
            )}
          >
            {t("common.buttons.edit")}
          </button>

          {!entry.systemPrompt && (
            <button
              onClick={() => onDelete(entry.id)}
              className={cn(
                "rounded-lg border border-fg/10 p-2 text-fg/40",
                "hover:border-danger/40 hover:bg-danger/10 hover:text-danger/80",
              )}
              title={t("common.buttons.delete")}
            >
              <Trash2 className="h-4 w-4" />
            </button>
          )}
        </div>
      </div>

      <p className="text-xs text-fg/50 line-clamp-2">
        {entry.content?.trim() || t("common.labels.none")}
      </p>
    </Reorder.Item>
  );
}

function getPromptTypeName(type: PromptType): string {
  switch (type) {
    case "system":
      return "System Prompt";
    case "local_roleplay":
      return "Local RP Default";
    case "summary":
      return "Dynamic Summary";
    case "memory":
      return "Dynamic Memory";
    case "reply":
      return "Reply Helper";
    case "avatar_generation":
      return "Avatar Generation";
    case "avatar_edit":
      return "Avatar Image Edit";
    case "scene_generation":
      return "Scene Generation";
    case "design_reference":
      return "Design Reference Writer";
    case "group_chat":
      return "Group Chat";
    case "group_chat_roleplay":
      return "Group Chat RP";
    default:
      return "Custom Prompt";
  }
}

function LoadingSkeleton() {
  return (
    <div className="flex h-full flex-col pb-16">
      <main className="flex-1 overflow-y-auto px-4 pt-4">
        <div className="mx-auto w-full max-w-5xl space-y-4">
          <div className="h-12 w-full animate-pulse rounded-xl bg-fg/10" />
          <div className="h-80 w-full animate-pulse rounded-xl bg-fg/10" />
        </div>
      </main>
    </div>
  );
}

export function EditPromptTemplate() {
  const { go } = useNavigationManager();
  const { id } = useParams<{ id: string }>();
  const isEditing = !!id;
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const sidebarRef = useRef<HTMLDivElement>(null);
  const entryTextareaRefs = useRef<Record<string, HTMLTextAreaElement | null>>({});
  const activeEntryIdRef = useRef<string | null>(null);
  const entriesRef = useRef<SystemPromptEntry[]>([]);
  const nameRef = useRef("");
  const contentRef = useRef("");
  const savingRef = useRef(false);
  const initialRef = useRef<{
    name: string;
    content: string;
    entries: string;
    condensePromptEntries: boolean;
  } | null>(null);

  // Form state
  const [name, setName] = useState("");
  const [content, setContent] = useState("");
  const [entries, setEntries] = useState<SystemPromptEntry[]>([]);
  const [condensePromptEntries, setCondensePromptEntries] = useState(false);

  // Preview state
  const [characters, setCharacters] = useState<Character[]>([]);
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [previewCharacterId, setPreviewCharacterId] = useState<string | null>(null);
  const [previewPersonaId, setPreviewPersonaId] = useState<string | null>(null);
  const [preview, setPreview] = useState<string>("");
  const [previewEntries, setPreviewEntries] = useState<SystemPromptEntry[]>([]);
  const [previewing, setPreviewing] = useState(false);
  const [previewMode, setPreviewMode] = useState<"rendered" | "raw">("rendered");
  const [previewExpanded, setPreviewExpanded] = useState(false);
  const [collapsedEntries, setCollapsedEntries] = useState<Record<string, boolean>>({});
  const [mobileEntryEditorId, setMobileEntryEditorId] = useState<string | null>(null);
  const [desktopEntryEditorId, setDesktopEntryEditorId] = useState<string | null>(null);

  // UI state
  const [loading, setLoading] = useState(isEditing);
  const [saving, setSaving] = useState(false);
  const [showVariables, setShowVariables] = useState(false);
  const [showMobilePreview, setShowMobilePreview] = useState(false);
  const [editorView, setEditorView] = useState<"entries" | "structure">("entries");
  const [mobilePreviewTab, setMobilePreviewTab] = useState<"content" | "structure">("content");
  const [copiedVar, setCopiedVar] = useState<string | null>(null);
  const [highlightedEntryId, setHighlightedEntryId] = useState<string | null>(null);

  // Template metadata
  const [isAppDefault, setIsAppDefault] = useState(false);
  const [promptType, setPromptType] = useState<PromptType>(null);
  const [resetting, setResetting] = useState(false);
  const [requiredVariables, setRequiredVariables] = useState<string[]>([]);
  const [missingVariables, setMissingVariables] = useState<string[]>([]);

  const canReset =
    isAppDefault &&
    (promptType === "system" ||
      promptType === "summary" ||
      promptType === "memory" ||
      promptType === "reply" ||
      promptType === "avatar_generation" ||
      promptType === "avatar_edit" ||
      promptType === "scene_generation" ||
      promptType === "design_reference");

  const usesEntryEditor = true;
  const quickInsertY = useMotionValue(0);
  const [scrollListenerMounted, setScrollListenerMounted] = useState(false);

  // Trigger scroll listener setup after component mounts
  useEffect(() => {
    setScrollListenerMounted(true);
  }, []);

  useEffect(() => {
    if (!scrollListenerMounted) return;

    const getScrollParent = (node: HTMLElement | null): HTMLElement | null => {
      let current = node?.parentElement ?? null;
      while (current) {
        const style = getComputedStyle(current);
        const overflowY = style.overflowY;
        if (
          (overflowY === "auto" || overflowY === "scroll" || overflowY === "overlay") &&
          current.scrollHeight > current.clientHeight
        ) {
          return current;
        }
        current = current.parentElement;
      }
      return null;
    };

    const target = sidebarRef.current;
    if (!target) return;

    const scrollParent = getScrollParent(target);

    const handleScroll = () => {
      const scrollTop = scrollParent ? scrollParent.scrollTop : window.scrollY;
      quickInsertY.set(scrollTop);
    };

    const options: AddEventListenerOptions = { passive: true };
    (scrollParent ?? window).addEventListener("scroll", handleScroll, options);
    handleScroll();
    return () => (scrollParent ?? window).removeEventListener("scroll", handleScroll, options);
  }, [scrollListenerMounted, quickInsertY]);

  const variables = VARIABLES_BY_TYPE[promptType || "default"] || VARIABLES_BY_TYPE.default;

  const contentValue = usesEntryEditor ? entriesToContent(entries) : content;
  const charCount = contentValue.length;
  const charCountColor =
    charCount > 8000 ? "text-danger/80" : charCount > 5000 ? "text-warning/80" : "text-fg/40";

  const hasEntryContent = entries.some((entry) => entryHasEditableContent(entry));
  const hasContent = content.trim().length > 0;
  const conditionValidationErrors = useMemo(
    () =>
      entries.flatMap((entry, index) =>
        getConditionWarnings(decomposeConditionTree(entry.conditions)).map((warning) => {
          const label = entry.name.trim() || `Entry ${index + 1}`;
          return `${label}: ${warning}`;
        }),
      ),
    [entries],
  );
  const serializeEntries = (items: SystemPromptEntry[]) =>
    JSON.stringify(
      items.map((entry) => ({
        id: entry.id,
        name: entry.name,
        role: entry.role,
        content: entry.content,
        enabled: entry.enabled,
        injectionPosition: entry.injectionPosition,
        injectionDepth: entry.injectionDepth,
        conditionalMinMessages: entry.conditionalMinMessages ?? null,
        intervalTurns: entry.intervalTurns ?? null,
        systemPrompt: entry.systemPrompt,
        conditions: entry.conditions ?? null,
        promptEntryPayload: entry.promptEntryPayload ?? null,
      })),
    );
  const isDirty =
    !loading &&
    initialRef.current !== null &&
    (name.trim() !== initialRef.current.name ||
      content !== initialRef.current.content ||
      serializeEntries(entries) !== initialRef.current.entries ||
      condensePromptEntries !== initialRef.current.condensePromptEntries);
  const canSave =
    isDirty &&
    name.trim().length > 0 &&
    (hasEntryContent || hasContent) &&
    conditionValidationErrors.length === 0;

  // Expose save state to TopNav via window globals
  useEffect(() => {
    const globalWindow = window as any;
    globalWindow.__savePromptCanSave = canSave && !saving;
    globalWindow.__savePromptSaving = saving;

    return () => {
      delete globalWindow.__savePromptCanSave;
      delete globalWindow.__savePromptSaving;
    };
  }, [canSave, saving]);

  useEffect(() => {
    const globalWindow = window as any;
    const handleDiscard = () => resetToInitial();
    globalWindow.__discardChanges = handleDiscard;
    window.addEventListener("unsaved:discard", handleDiscard);
    return () => {
      if (globalWindow.__discardChanges === handleDiscard) {
        delete globalWindow.__discardChanges;
      }
      window.removeEventListener("unsaved:discard", handleDiscard);
    };
  }, [id]);

  useEffect(() => {
    initialRef.current = null;
  }, [id]);

  useEffect(() => {
    entriesRef.current = entries;
  }, [entries]);

  useEffect(() => {
    nameRef.current = name;
  }, [name]);

  useEffect(() => {
    contentRef.current = content;
  }, [content]);

  useEffect(() => {
    savingRef.current = saving;
  }, [saving]);

  // Listen for save event from TopNav
  useEffect(() => {
    const handleSave = () => {
      if (canSave && !savingRef.current) {
        handleSave_internal();
      }
    };

    window.addEventListener("prompt:save", handleSave);
    return () => window.removeEventListener("prompt:save", handleSave);
  }, [canSave]);

  useEffect(() => {
    loadData();
  }, []);

  useEffect(() => {
    if (isAppDefault && requiredVariables.length > 0) {
      const source = usesEntryEditor ? entriesToValidationSource(entries) : content;
      const missing = requiredVariables.filter((v) => !source.includes(v));
      setMissingVariables(missing);
    }
  }, [content, entries, requiredVariables, isAppDefault, usesEntryEditor]);

  async function loadData() {
    try {
      const [chars, pers] = await Promise.all([listCharacters(), listPersonas()]);
      setCharacters(chars);
      setPersonas(pers);
      setPreviewCharacterId(chars[0]?.id ?? null);
      setPreviewPersonaId(pers.find((p) => p.isDefault)?.id ?? null);

      if (isEditing && id) {
        const [template, appDefaultId] = await Promise.all([
          getPromptTemplate(id),
          getAppDefaultTemplateId(),
        ]);

        if (template) {
          setName(template.name);
          setContent(template.content);
          const isProtected =
            template.id === appDefaultId || isProtectedPromptTemplate(template.id);
          setIsAppDefault(isProtected);

          let detectedType: PromptType = null;
          if (template.id === appDefaultId) {
            detectedType = "system";
          } else if (template.id === APP_LOCAL_ROLEPLAY_TEMPLATE_ID) {
            detectedType = "local_roleplay";
          } else if (template.id === APP_DYNAMIC_SUMMARY_TEMPLATE_ID) {
            detectedType = "summary";
          } else if (template.id === APP_DYNAMIC_MEMORY_TEMPLATE_ID) {
            detectedType = "memory";
          } else if (template.id === APP_HELP_ME_REPLY_TEMPLATE_ID) {
            detectedType = "reply";
          } else if (template.id === APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID) {
            detectedType = "reply";
          } else if (template.id === APP_AVATAR_GENERATION_TEMPLATE_ID) {
            detectedType = "avatar_generation";
          } else if (template.id === APP_AVATAR_EDIT_TEMPLATE_ID) {
            detectedType = "avatar_edit";
          } else if (template.id === APP_SCENE_GENERATION_TEMPLATE_ID) {
            detectedType = "scene_generation";
          } else if (template.id === APP_DESIGN_REFERENCE_TEMPLATE_ID) {
            detectedType = "design_reference";
          } else if (template.id === APP_GROUP_CHAT_TEMPLATE_ID) {
            detectedType = "group_chat";
          } else if (template.id === APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID) {
            detectedType = "group_chat_roleplay";
          }
          setPromptType(detectedType);

          const nextEntries =
            template.entries?.length > 0
              ? template.entries
              : [createDefaultEntry(template.content)];
          const normalizedEntries = ensureSystemEntry(nextEntries);
          setEntries(normalizedEntries);
          setCondensePromptEntries(Boolean(template.condensePromptEntries));
          setCollapsedEntries(
            Object.fromEntries(normalizedEntries.map((entry) => [entry.id, true])),
          );
          initialRef.current = {
            name: template.name,
            content: template.content,
            entries: serializeEntries(normalizedEntries),
            condensePromptEntries: Boolean(template.condensePromptEntries),
          };

          if (isProtected) {
            const required = await getRequiredTemplateVariables(template.id);
            setRequiredVariables(required);
          }
        }
      } else {
        setContent("");
        setEntries([]);
        setCondensePromptEntries(false);
        setCollapsedEntries({});
        initialRef.current = {
          name: "",
          content: "",
          entries: serializeEntries([]),
          condensePromptEntries: false,
        };
      }
    } catch (error) {
      console.error("Failed to load data:", error);
    } finally {
      setLoading(false);
    }
  }

  const handleEntryUpdate = (id: string, updates: Partial<SystemPromptEntry>) => {
    setEntries((prev) => prev.map((entry) => (entry.id === id ? { ...entry, ...updates } : entry)));
  };

  const handleToggleEntryCollapse = (id: string) => {
    setCollapsedEntries((prev) => ({ ...prev, [id]: !prev[id] }));
  };

  const handleEntryDelete = (id: string) => {
    setEntries((prev) => prev.filter((entry) => entry.id !== id));
  };

  const handleEntryToggle = (id: string) => {
    setEntries((prev) =>
      prev.map((entry) => {
        if (entry.id !== id || entry.systemPrompt) return entry;
        return { ...entry, enabled: !entry.enabled };
      }),
    );
  };

  const handleAddEntry = () => {
    const entry = createExtraEntry();
    setEntries((prev) => [...prev, entry]);
    setCollapsedEntries((prev) => ({ ...prev, [entry.id]: false }));
    window.setTimeout(() => {
      const isMobile = window.matchMedia("(max-width: 1023px)").matches;
      const targetId = isMobile
        ? `prompt-entry-row-mobile-${entry.id}`
        : `prompt-entry-row-${entry.id}`;
      const target = document.getElementById(targetId);
      if (target) {
        target.scrollIntoView({ behavior: "smooth", block: "center" });
      }
    }, 150);
  };

  const handleStructureEdit = (entryId: string) => {
    setEditorView("entries");
    setCollapsedEntries((prev) => ({ ...prev, [entryId]: false }));
    window.setTimeout(() => {
      const isMobile = window.matchMedia("(max-width: 1023px)").matches;
      const targetId = isMobile
        ? `prompt-entry-row-mobile-${entryId}`
        : `prompt-entry-row-${entryId}`;
      document.getElementById(targetId)?.scrollIntoView({ behavior: "smooth", block: "center" });
      window.setTimeout(() => {
        entryTextareaRefs.current[entryId]?.focus();
      }, 300);
    }, 200);
  };

  const handleStructureDelete = (entryId: string) => {
    handleEntryDelete(entryId);
  };

  const handleStructureReorder = (entryId: string) => {
    setEditorView("entries");
    setHighlightedEntryId(entryId);
    window.setTimeout(() => {
      const isMobile = window.matchMedia("(max-width: 1023px)").matches;
      const targetId = isMobile
        ? `prompt-entry-row-mobile-${entryId}`
        : `prompt-entry-row-${entryId}`;
      document.getElementById(targetId)?.scrollIntoView({ behavior: "smooth", block: "center" });
    }, 200);
    window.setTimeout(() => setHighlightedEntryId(null), 4000);
  };

  const selectedMobileEntry = mobileEntryEditorId
    ? (entries.find((entry) => entry.id === mobileEntryEditorId) ?? null)
    : null;
  const selectedDesktopEntry = desktopEntryEditorId
    ? (entries.find((entry) => entry.id === desktopEntryEditorId) ?? null)
    : null;

  async function handleSave_internal() {
    const entriesSnapshot = entriesRef.current;
    const nameSnapshot = nameRef.current.trim();
    const contentSnapshot = contentRef.current;
    const validationSource = usesEntryEditor
      ? entriesToValidationSource(entriesSnapshot)
      : contentSnapshot;
    const hasContent = usesEntryEditor
      ? entriesSnapshot.some((entry) => entryHasEditableContent(entry))
      : contentSnapshot.trim().length > 0;
    if (!nameSnapshot || !hasContent) return;

    const currentMissingVariables =
      isAppDefault && requiredVariables.length > 0
        ? requiredVariables.filter((variable) => !validationSource.includes(variable))
        : [];

    if (isAppDefault && id && currentMissingVariables.length > 0) {
      alert(`Cannot save: Missing required variables: ${currentMissingVariables.join(", ")}`);
      return;
    }

    if (conditionValidationErrors.length > 0) {
      alert(
        `Cannot save: Fix contradictory activation rules.\n\n${conditionValidationErrors.join("\n")}`,
      );
      return;
    }

    setSaving(true);
    try {
      const contentToSave = usesEntryEditor
        ? entriesToContent(entriesSnapshot)
        : contentSnapshot.trim();
      let savedTemplate;
      if (isEditing && id) {
        savedTemplate = await updatePromptTemplate(id, {
          name: nameSnapshot,
          content: contentToSave,
          entries: usesEntryEditor ? entriesSnapshot : undefined,
          condensePromptEntries,
        });
      } else {
        savedTemplate = await createPromptTemplate(
          nameSnapshot,
          "appWide" as any,
          [],
          contentToSave,
          usesEntryEditor ? entriesSnapshot : undefined,
          condensePromptEntries,
        );
      }

      const normalizedEntries =
        usesEntryEditor && savedTemplate.entries?.length
          ? ensureSystemEntry(savedTemplate.entries)
          : entriesSnapshot;

      setName(savedTemplate.name);
      setContent(savedTemplate.content);
      if (usesEntryEditor) {
        setEntries(normalizedEntries);
        setCollapsedEntries((prev) =>
          Object.fromEntries(normalizedEntries.map((entry) => [entry.id, prev[entry.id] ?? true])),
        );
      }

      initialRef.current = {
        name: savedTemplate.name,
        content: savedTemplate.content,
        entries: serializeEntries(normalizedEntries),
        condensePromptEntries: Boolean(savedTemplate.condensePromptEntries),
      };

      if (!isEditing) {
        go(`/settings/prompts/${savedTemplate.id}`, { replace: true });
      }
    } catch (error) {
      console.error("Failed to save template:", error);
      alert("Failed to save template: " + String(error));
    } finally {
      setSaving(false);
    }
  }

  async function handleReset() {
    if (!isAppDefault || !promptType) return;
    if (
      ![
        "system",
        "local_roleplay",
        "summary",
        "memory",
        "reply",
        "avatar_generation",
        "avatar_edit",
        "scene_generation",
        "design_reference",
      ].includes(promptType)
    ) {
      return;
    }

    const promptTypeName = getPromptTypeName(promptType);
    const confirmed = await confirmBottomMenu({
      title: `Reset ${promptTypeName}?`,
      message: `Reset to the original default ${promptTypeName}? This cannot be undone.`,
      confirmLabel: "Reset",
      destructive: true,
    });
    if (!confirmed) return;

    setResetting(true);
    try {
      let updated;
      if (promptType === "system") {
        updated = await resetAppDefaultTemplate();
      } else if (promptType === "local_roleplay") {
        updated = await resetLocalRoleplayTemplate();
      } else if (promptType === "summary") {
        updated = await resetDynamicSummaryTemplate();
      } else if (promptType === "memory") {
        updated = await resetDynamicMemoryTemplate();
      } else if (promptType === "reply") {
        updated = await resetHelpMeReplyTemplate();
      } else if (promptType === "avatar_generation") {
        updated = await resetAvatarGenerationTemplate();
      } else if (promptType === "scene_generation") {
        updated = await resetSceneGenerationTemplate();
      } else if (promptType === "design_reference") {
        updated = await resetDesignReferenceTemplate();
      } else {
        updated = await resetAvatarEditTemplate();
      }
      setContent(updated.content);
      setCondensePromptEntries(Boolean(updated.condensePromptEntries));
      if (usesEntryEditor) {
        const nextEntries =
          updated.entries?.length > 0 ? updated.entries : [createDefaultEntry(updated.content)];
        const normalizedEntries = ensureSystemEntry(nextEntries);
        setEntries(normalizedEntries);
        setCollapsedEntries(Object.fromEntries(normalizedEntries.map((entry) => [entry.id, true])));
      }
    } catch (error) {
      console.error("Failed to reset template:", error);
      alert("Failed to reset template");
    } finally {
      setResetting(false);
    }
  }

  const resetToInitial = () => {
    if (!initialRef.current) return;
    try {
      const nextEntries = JSON.parse(initialRef.current.entries) as SystemPromptEntry[];
      setName(initialRef.current.name);
      setContent(initialRef.current.content);
      setEntries(nextEntries);
      setCondensePromptEntries(initialRef.current.condensePromptEntries);
      setCollapsedEntries(Object.fromEntries(nextEntries.map((entry) => [entry.id, true])));
      setMobileEntryEditorId(null);
    } catch (error) {
      console.error("Failed to reset prompt editor:", error);
    }
  };

  async function handlePreview() {
    if (!previewCharacterId) return;
    setPreviewing(true);
    try {
      if (usesEntryEditor) {
        if (previewMode === "raw") {
          setPreviewEntries(entries);
        } else {
          const renderedEntries = await Promise.all(
            entries.map(async (entry) => {
              const rendered = await renderPromptPreview(entry.content, {
                characterId: previewCharacterId,
                personaId: previewPersonaId ?? undefined,
              });
              return { ...entry, content: rendered };
            }),
          );
          setPreviewEntries(renderedEntries);
        }
      } else {
        const rendered = await renderPromptPreview(content, {
          characterId: previewCharacterId,
          personaId: previewPersonaId ?? undefined,
        });
        setPreview(rendered);
      }
    } catch (e) {
      console.error("Preview failed", e);
      setPreview("<failed to render preview>");
      if (usesEntryEditor) {
        setPreviewEntries([]);
      }
    } finally {
      setPreviewing(false);
    }
  }

  async function copyVariable(variable: string) {
    await navigator.clipboard.writeText(variable);
    setCopiedVar(variable);
    setTimeout(() => setCopiedVar(null), 2000);
  }

  function insertVariable(variable: string) {
    if (usesEntryEditor) {
      const targetId = activeEntryIdRef.current;
      const targetEl = targetId ? entryTextareaRefs.current[targetId] : null;
      if (targetId && targetEl) {
        const start = targetEl.selectionStart ?? 0;
        const end = targetEl.selectionEnd ?? start;
        setEntries((prev) =>
          prev.map((entry) => {
            if (entry.id !== targetId) return entry;
            const nextContent =
              entry.content.substring(0, start) + variable + entry.content.substring(end);
            return { ...entry, content: nextContent };
          }),
        );
        setTimeout(() => {
          const el = entryTextareaRefs.current[targetId];
          if (!el) return;
          el.focus();
          const newPos = start + variable.length;
          el.setSelectionRange(newPos, newPos);
        }, 0);
        return;
      }
      setEntries((prev) => {
        if (prev.length === 0) return prev;
        const targetIndex = prev.findIndex((entry) => entry.systemPrompt);
        const index = targetIndex >= 0 ? targetIndex : 0;
        const next = [...prev];
        next[index] = {
          ...next[index],
          content: `${next[index].content}${next[index].content ? "\n" : ""}${variable}`,
        };
        return next;
      });
      return;
    }
    if (!textareaRef.current) return;

    const textarea = textareaRef.current;
    const start = textarea.selectionStart;
    const end = textarea.selectionEnd;

    const newContent = content.substring(0, start) + variable + content.substring(end);
    setContent(newContent);

    setTimeout(() => {
      textarea.focus();
      const newPos = start + variable.length;
      textarea.setSelectionRange(newPos, newPos);
    }, 0);
  }

  if (loading) {
    return <LoadingSkeleton />;
  }

  // Preview Panel Component (used in both desktop inline and mobile sheet)
  const PreviewPanel = ({ isMobile = false }: { isMobile?: boolean }) => (
    <div className={cn("space-y-3", isMobile ? "" : "")}>
      {/* Mode Toggle */}
      <div className="flex items-center gap-1 p-1 rounded-lg border border-fg/10 bg-fg/5">
        <button
          onClick={() => setPreviewMode("rendered")}
          className={cn(
            "flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5",
            radius.md,
            "text-xs font-medium transition",
            previewMode === "rendered"
              ? "bg-accent/20 text-accent/80"
              : "text-fg/50 hover:text-fg/70",
          )}
        >
          <Sparkles className="h-3.5 w-3.5" />
          Rendered
        </button>
        <button
          onClick={() => setPreviewMode("raw")}
          className={cn(
            "flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5",
            radius.md,
            "text-xs font-medium transition",
            previewMode === "raw" ? "bg-accent/20 text-accent/80" : "text-fg/50 hover:text-fg/70",
          )}
        >
          <Code2 className="h-3.5 w-3.5" />
          Raw
        </button>
      </div>

      {/* Character/Persona Selection */}
      {previewMode === "rendered" && (
        <>
          <div className="grid grid-cols-2 gap-2">
            <select
              value={previewCharacterId ?? ""}
              onChange={(e) => setPreviewCharacterId(e.target.value || null)}
              className={cn(
                "w-full px-3 py-2",
                radius.md,
                "border border-fg/10 bg-fg/5",
                "text-sm text-fg",
                "focus:border-fg/20 focus:outline-none",
              )}
            >
              <option value="">Select character…</option>
              {characters.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>

            <select
              value={previewPersonaId ?? ""}
              onChange={(e) => setPreviewPersonaId(e.target.value || null)}
              className={cn(
                "w-full px-3 py-2",
                radius.md,
                "border border-fg/10 bg-fg/5",
                "text-sm text-fg",
                "focus:border-fg/20 focus:outline-none",
              )}
            >
              <option value="">Select persona…</option>
              {personas.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.title}
                </option>
              ))}
            </select>
          </div>

          <button
            onClick={handlePreview}
            disabled={!previewCharacterId || previewing}
            className={cn(
              "w-full py-2",
              radius.md,
              "border text-sm font-medium transition",
              !previewCharacterId || previewing
                ? "border-fg/10 bg-fg/5 text-fg/30 cursor-not-allowed"
                : "border-accent/40 bg-accent/15 text-accent/80 hover:bg-accent/25",
            )}
          >
            {previewing ? "Rendering…" : "Generate Preview"}
          </button>
        </>
      )}

      {/* Preview Output */}
      <div
        className={cn(
          "overflow-auto",
          radius.lg,
          "border border-fg/10 bg-surface-el/30 p-4",
          isMobile ? "max-h-80" : "max-h-64",
        )}
      >
        {usesEntryEditor ? (
          (() => {
            const entriesToShow = previewMode === "rendered" ? previewEntries : entries;
            if (previewMode === "rendered" && entriesToShow.length === 0) {
              return (
                <div className="flex flex-col items-center justify-center h-full py-8 text-center">
                  <Eye className="h-8 w-8 text-fg/20 mb-2" />
                  <p className="text-sm text-fg/50">No preview yet</p>
                  <p className="text-xs text-fg/30">Select a character and generate</p>
                </div>
              );
            }
            if (entriesToShow.length === 0) {
              return <p className="text-xs text-fg/40">No entries to preview</p>;
            }
            return (
              <div className="space-y-4">
                {entriesToShow.map((entry) => (
                  <div key={entry.id} className="space-y-1">
                    <div className="text-[11px] uppercase tracking-wide text-fg/40">
                      {getEntryKindSummary(entry)} · {entry.name}
                    </div>
                    <pre className="whitespace-pre-wrap text-xs leading-relaxed text-fg/80 font-mono">
                      {entry.content || getEntryPreviewText(entry)}
                    </pre>
                  </div>
                ))}
              </div>
            );
          })()
        ) : previewMode === "rendered" ? (
          preview ? (
            <pre className="whitespace-pre-wrap text-xs leading-relaxed text-fg/80 font-mono">
              {preview}
            </pre>
          ) : (
            <div className="flex flex-col items-center justify-center h-full py-8 text-center">
              <Eye className="h-8 w-8 text-fg/20 mb-2" />
              <p className="text-sm text-fg/50">No preview yet</p>
              <p className="text-xs text-fg/30">Select a character and generate</p>
            </div>
          )
        ) : (
          <pre className="whitespace-pre-wrap text-xs leading-relaxed text-fg/80 font-mono">
            {content || "No content to preview"}
          </pre>
        )}
      </div>
    </div>
  );

  return (
    <div className="flex h-full flex-col pb-16">
      <main className="flex-1 overflow-y-auto px-4 pt-4">
        <div className="mx-auto w-full max-w-5xl">
          {/* Desktop: Two column layout */}
          <div className="flex flex-col lg:flex-row lg:gap-6">
            {/* Main Editor Column */}
            <div className="flex-1 space-y-4 min-w-0">
              {/* Protected Template Notice */}
              {isAppDefault && (
                <div className={cn(radius.lg, "border border-warning/30 bg-warning/10 p-3")}>
                  <div className="flex items-center justify-between gap-3">
                    <div className="flex items-center gap-2 min-w-0">
                      <Lock className="h-4 w-4 text-warning/80 shrink-0" />
                      <div className="min-w-0">
                        <span className="text-sm font-medium text-warning/80">Protected</span>
                        {promptType && (
                          <span className="text-xs text-warning/70 ml-2">
                            {getPromptTypeName(promptType)}
                          </span>
                        )}
                      </div>
                    </div>
                    {canReset && (
                      <button
                        onClick={handleReset}
                        disabled={resetting}
                        className={cn(
                          "flex items-center gap-1.5 px-3 py-1.5 shrink-0",
                          radius.md,
                          "text-xs font-medium text-warning/80",
                          "hover:bg-warning/20",
                          interactive.transition.fast,
                          "disabled:opacity-50",
                        )}
                      >
                        <RotateCcw className={cn("h-3.5 w-3.5", resetting && "animate-spin")} />
                        Reset
                      </button>
                    )}
                  </div>
                </div>
              )}

              {/* Validation Warnings */}
              <AnimatePresence>
                {conditionValidationErrors.length > 0 && (
                  <motion.div
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: "auto" }}
                    exit={{ opacity: 0, height: 0 }}
                    className={cn(radius.lg, "border border-danger/30 bg-danger/10 p-3")}
                  >
                    <div className="flex items-start gap-2">
                      <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-danger/80" />
                      <div className="space-y-1">
                        <p className="text-sm font-medium text-danger/80">
                          Contradictory Activation Rules
                        </p>
                        {conditionValidationErrors.map((error) => (
                          <p key={error} className="text-xs text-danger/70">
                            {error}
                          </p>
                        ))}
                      </div>
                    </div>
                  </motion.div>
                )}

                {isAppDefault && missingVariables.length > 0 && (
                  <motion.div
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: "auto" }}
                    exit={{ opacity: 0, height: 0 }}
                    className={cn(radius.lg, "border border-danger/30 bg-danger/10 p-3")}
                  >
                    <div className="flex items-start gap-2">
                      <AlertTriangle className="h-4 w-4 text-danger/80 shrink-0 mt-0.5" />
                      <div>
                        <p className="text-sm font-medium text-danger/80">
                          Missing Required Variables
                        </p>
                        <p className="text-xs text-danger/70 mt-0.5">
                          Include: <span className="font-mono">{missingVariables.join(", ")}</span>
                        </p>
                      </div>
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>

              {/* Name Input */}
              <div className="space-y-2">
                <label className="text-xs font-medium uppercase tracking-wider text-fg/50">
                  Template Name
                </label>
                <input
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="e.g., Creative Roleplay"
                  className={cn(
                    "w-full px-4 py-3",
                    radius.lg,
                    "border border-fg/10 bg-fg/5",
                    "text-sm text-fg placeholder-fg/30",
                    interactive.transition.fast,
                    "focus:border-fg/20 focus:bg-fg/10 focus:outline-none",
                  )}
                />
              </div>

              {/* Content Editor */}
              <div className="space-y-2">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  {usesEntryEditor ? (
                    <div className="flex items-center gap-1 p-0.5 rounded-md border border-fg/10 bg-surface-el/20">
                      <button
                        onClick={() => setEditorView("entries")}
                        className={cn(
                          "px-2.5 py-1 text-xs font-medium",
                          radius.sm,
                          "transition",
                          editorView === "entries"
                            ? "bg-fg/10 text-fg"
                            : "text-fg/40 hover:text-fg/60",
                        )}
                      >
                        Entries
                      </button>
                      <button
                        onClick={() => setEditorView("structure")}
                        className={cn(
                          "flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium",
                          radius.sm,
                          "transition",
                          editorView === "structure"
                            ? "bg-fg/10 text-fg"
                            : "text-fg/40 hover:text-fg/60",
                        )}
                      >
                        <Layers className="h-3 w-3" />
                        Structure
                      </button>
                    </div>
                  ) : (
                    <label className="text-xs font-medium uppercase tracking-wider text-fg/50">
                      Prompt Content
                    </label>
                  )}
                  {usesEntryEditor && (
                    <div className="flex items-center gap-3 rounded-lg border border-fg/10 bg-surface-el/20 px-2.5 py-1.5">
                      <Switch
                        id="condense-prompt-entries"
                        checked={condensePromptEntries}
                        onChange={(next) => setCondensePromptEntries(next)}
                      />
                      <span className="text-xs text-fg/70">Send entries as one system message</span>
                    </div>
                  )}
                  <div className="flex flex-wrap items-center gap-2">
                    {usesEntryEditor && (
                      <button
                        onClick={handleAddEntry}
                        className={cn(
                          "flex items-center gap-1.5 px-2.5 py-1.5",
                          radius.md,
                          "border border-accent/30 bg-accent/10",
                          "text-xs font-medium text-accent/80",
                          interactive.transition.fast,
                          "hover:bg-accent/20",
                        )}
                      >
                        <Plus className="h-3.5 w-3.5" />
                        Add Entry
                      </button>
                    )}
                    <button
                      onClick={() => setShowVariables(true)}
                      className={cn(
                        "flex items-center gap-1.5 px-2.5 py-1.5",
                        radius.md,
                        "border border-info/30 bg-info/10",
                        "text-xs font-medium text-info/80",
                        interactive.transition.fast,
                        "hover:bg-info/20",
                      )}
                    >
                      <Sparkles className="h-3.5 w-3.5" />
                      Variables
                    </button>
                    <button
                      onClick={() => setShowMobilePreview(true)}
                      className={cn(
                        "flex items-center gap-1.5 px-2.5 py-1.5 lg:hidden",
                        radius.md,
                        "border border-fg/10 bg-fg/5",
                        "text-xs font-medium text-fg/70",
                        interactive.transition.fast,
                        "hover:bg-fg/10",
                      )}
                    >
                      <Eye className="h-3.5 w-3.5" />
                      Preview
                    </button>
                  </div>
                </div>

                {usesEntryEditor ? (
                  <AnimatePresence mode="wait" initial={false}>
                    {editorView === "structure" ? (
                      <motion.div
                        key="structure-view"
                        initial={{ opacity: 0 }}
                        animate={{ opacity: 1 }}
                        exit={{ opacity: 0 }}
                        transition={{ duration: 0.15 }}
                      >
                        <MessageStructurePreview
                          entries={entries}
                          condensePromptEntries={condensePromptEntries}
                          onEditEntry={handleStructureEdit}
                          onDeleteEntry={handleStructureDelete}
                          onReorderEntry={handleStructureReorder}
                        />
                      </motion.div>
                    ) : (
                      <motion.div
                        key="entries-view"
                        initial={{ opacity: 0 }}
                        animate={{ opacity: 1 }}
                        exit={{ opacity: 0 }}
                        transition={{ duration: 0.15 }}
                        className="space-y-3"
                      >
                        <Reorder.Group
                          axis="y"
                          values={entries}
                          onReorder={setEntries}
                          className="hidden lg:flex lg:flex-col gap-3"
                        >
                          {entries.map((entry) => (
                            <PromptEntryCard
                              key={entry.id}
                              entry={entry}
                              onUpdate={handleEntryUpdate}
                              onDelete={handleEntryDelete}
                              onToggle={handleEntryToggle}
                              onToggleCollapse={handleToggleEntryCollapse}
                              collapsed={collapsedEntries[entry.id] ?? true}
                              highlighted={highlightedEntryId === entry.id}
                              onOpenEditor={() => setDesktopEntryEditorId(entry.id)}
                            />
                          ))}
                        </Reorder.Group>

                        <Reorder.Group
                          axis="y"
                          values={entries}
                          onReorder={setEntries}
                          className="flex flex-col gap-2 lg:hidden"
                        >
                          {entries.map((entry) => (
                            <PromptEntryListItem
                              key={entry.id}
                              entry={entry}
                              onToggle={handleEntryToggle}
                              onDelete={handleEntryDelete}
                              onEdit={(id) => setMobileEntryEditorId(id)}
                            />
                          ))}
                        </Reorder.Group>

                        <div className="flex items-center justify-end">
                          <span
                            className={cn(
                              "px-2 py-1 rounded-md bg-surface-el/60",
                              "text-xs font-medium",
                              charCountColor,
                            )}
                          >
                            {charCount.toLocaleString()}
                          </span>
                        </div>
                      </motion.div>
                    )}
                  </AnimatePresence>
                ) : (
                  <div className="relative">
                    <textarea
                      ref={textareaRef}
                      value={content}
                      onChange={(e) => setContent(e.target.value)}
                      placeholder="You are a creative and engaging AI assistant..."
                      rows={20}
                      className={cn(
                        "w-full px-4 py-3 resize-none",
                        radius.lg,
                        "border border-fg/10 bg-fg/5",
                        "font-mono text-sm leading-relaxed text-fg placeholder-fg/30",
                        interactive.transition.fast,
                        "focus:border-fg/20 focus:bg-fg/10 focus:outline-none",
                      )}
                    />
                    <div className="absolute bottom-3 right-3 pointer-events-none">
                      <span
                        className={cn(
                          "px-2 py-1 rounded-md bg-surface-el/60",
                          "text-xs font-medium",
                          charCountColor,
                        )}
                      >
                        {charCount.toLocaleString()}
                      </span>
                    </div>
                  </div>
                )}
              </div>

              {/* Collapsible Preview Panel (Desktop - below content) */}
              <div className={cn(radius.lg, "border border-fg/10 bg-fg/5 hidden lg:block")}>
                {/* Collapsed Header / Toggle */}
                <button
                  onClick={() => setPreviewExpanded(!previewExpanded)}
                  className={cn(
                    "w-full flex items-center justify-between px-4 py-3",
                    "text-left",
                    interactive.transition.fast,
                    "hover:bg-fg/5",
                    previewExpanded ? "border-b border-fg/10" : "",
                  )}
                >
                  <div className="flex items-center gap-2">
                    <Eye className="h-4 w-4 text-fg/50" />
                    <span className="text-sm font-medium text-fg">Preview</span>
                    {!previewExpanded && preview && (
                      <span className="text-xs text-fg/40 ml-2">(has generated preview)</span>
                    )}
                  </div>
                  <div className="flex items-center gap-2">
                    {previewExpanded ? (
                      <ChevronUp className="h-4 w-4 text-fg/50" />
                    ) : (
                      <ChevronDown className="h-4 w-4 text-fg/50" />
                    )}
                  </div>
                </button>

                {/* Expanded Content */}
                <AnimatePresence>
                  {previewExpanded && (
                    <motion.div
                      initial={{ height: 0, opacity: 0 }}
                      animate={{ height: "auto", opacity: 1 }}
                      exit={{ height: 0, opacity: 0 }}
                      transition={{ duration: 0.2 }}
                      className="overflow-hidden"
                    >
                      <div className="p-4">
                        <PreviewPanel />
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>
            </div>

            {/* Desktop Sidebar - Quick Insert */}
            <motion.div
              ref={sidebarRef}
              style={{ y: quickInsertY }}
              className="hidden lg:block w-80 shrink-0 space-y-4 self-start relative z-20"
            >
              <div className={cn(radius.lg, "border border-fg/10 bg-fg/5 p-4")}>
                <h3 className="text-sm font-medium text-fg mb-1">Quick Insert</h3>
                <p className="text-xs text-fg/40 mb-3">Click to insert at cursor</p>

                <div className="space-y-1.5 max-h-[60vh] overflow-y-auto">
                  {variables.map((v) => {
                    const isRequired = requiredVariables.includes(v.var);
                    const isMissing = missingVariables.includes(v.var);
                    return (
                      <button
                        key={v.var}
                        onClick={() => insertVariable(v.var)}
                        className={cn(
                          "w-full text-left p-2",
                          radius.md,
                          "border",
                          isMissing
                            ? "border-danger/30 bg-danger/10"
                            : isRequired
                              ? "border-warning/30 bg-warning/10"
                              : "border-fg/10 bg-fg/5",
                          interactive.transition.fast,
                          "hover:bg-fg/10",
                        )}
                      >
                        <div className="flex items-center gap-2">
                          {isRequired && (
                            <span
                              className={cn(
                                "text-xs",
                                isMissing ? "text-danger/80" : "text-warning/80",
                              )}
                            >
                              ★
                            </span>
                          )}
                          <code
                            className={cn(
                              "text-xs font-medium",
                              isMissing ? "text-danger/80" : "text-accent/80",
                            )}
                          >
                            {v.var}
                          </code>
                        </div>
                        <p className="text-[10px] text-fg/40 mt-0.5">{v.desc}</p>
                      </button>
                    );
                  })}
                </div>

                <div className="flex items-start gap-2 mt-3 pt-3 border-t border-fg/10">
                  <span className="text-fg/30 text-xs mt-0.5">ⓘ</span>
                  <p className="text-xs text-fg/40 leading-relaxed">
                    Variables are replaced with actual values when the prompt is used.
                  </p>
                </div>
              </div>
            </motion.div>
          </div>
        </div>
      </main>

      {/* Variables Bottom Sheet (Mobile) */}
      <BottomMenu
        isOpen={showVariables}
        onClose={() => setShowVariables(false)}
        title="Template Variables"
      >
        <div className="space-y-4">
          <p className="text-xs text-fg/50">Tap to insert a variable into your prompt</p>

          {isAppDefault && requiredVariables.length > 0 && (
            <div className={cn(radius.lg, "border border-warning/30 bg-warning/10 p-3")}>
              <p className="text-xs text-warning/80">
                <span className="font-semibold">Required:</span> Variables marked with ★ must be
                included
              </p>
            </div>
          )}

          <div className="max-h-[50vh] overflow-y-auto space-y-2">
            {variables.map((item) => {
              const isRequired = requiredVariables.includes(item.var);
              const isMissing = missingVariables.includes(item.var);
              return (
                <div
                  key={item.var}
                  className={cn(
                    radius.lg,
                    "border p-3",
                    isMissing
                      ? "border-danger/40 bg-danger/10"
                      : isRequired
                        ? "border-warning/30 bg-warning/10"
                        : "border-fg/10 bg-fg/5",
                  )}
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 mb-1">
                        {isRequired && (
                          <span className={isMissing ? "text-danger/80" : "text-warning/80"}>
                            ★
                          </span>
                        )}
                        <code
                          className={cn(
                            "text-sm font-semibold",
                            isMissing ? "text-danger/80" : "text-accent/80",
                          )}
                        >
                          {item.var}
                        </code>
                        {copiedVar === item.var && (
                          <span className="flex items-center gap-1 text-xs text-accent/80">
                            <Check className="h-3 w-3" />
                            Copied
                          </span>
                        )}
                      </div>
                      <p className="text-sm text-fg/80">{item.label}</p>
                      <p className="text-xs text-fg/50">{item.desc}</p>
                    </div>

                    <div className="flex items-center gap-2 shrink-0">
                      <button
                        onClick={() => copyVariable(item.var)}
                        className={cn(
                          "flex items-center justify-center h-8 w-8",
                          radius.md,
                          "border border-fg/10 bg-fg/5",
                          "text-fg/50",
                          interactive.transition.fast,
                          "hover:bg-fg/10 hover:text-fg",
                        )}
                        title="Copy"
                      >
                        <Copy className="h-4 w-4" />
                      </button>
                      <button
                        onClick={() => {
                          insertVariable(item.var);
                          setShowVariables(false);
                        }}
                        className={cn(
                          "flex items-center gap-1.5 px-3 py-1.5",
                          radius.md,
                          "border border-accent/30 bg-accent/15",
                          "text-xs font-medium text-accent/80",
                          interactive.transition.fast,
                          "hover:bg-accent/25",
                        )}
                      >
                        Insert
                      </button>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </BottomMenu>

      {/* Preview Bottom Sheet (Mobile only) */}
      <BottomMenu
        isOpen={showMobilePreview}
        onClose={() => setShowMobilePreview(false)}
        title="Preview"
      >
        {usesEntryEditor && (
          <div className="flex items-center gap-1 p-1 rounded-lg border border-fg/10 bg-fg/5 mb-3">
            <button
              onClick={() => setMobilePreviewTab("content")}
              className={cn(
                "flex-1 flex items-center justify-center px-3 py-1.5",
                radius.md,
                "text-xs font-medium transition",
                mobilePreviewTab === "content"
                  ? "bg-accent/20 text-accent/80"
                  : "text-fg/50 hover:text-fg/70",
              )}
            >
              Content
            </button>
            <button
              onClick={() => setMobilePreviewTab("structure")}
              className={cn(
                "flex-1 flex items-center justify-center px-3 py-1.5",
                radius.md,
                "text-xs font-medium transition",
                mobilePreviewTab === "structure"
                  ? "bg-accent/20 text-accent/80"
                  : "text-fg/50 hover:text-fg/70",
              )}
            >
              Structure
            </button>
          </div>
        )}
        {mobilePreviewTab === "content" || !usesEntryEditor ? (
          <PreviewPanel isMobile />
        ) : (
          <MessageStructurePreview
            entries={entries}
            condensePromptEntries={condensePromptEntries}
            onEditEntry={handleStructureEdit}
            onDeleteEntry={handleStructureDelete}
            onReorderEntry={handleStructureReorder}
          />
        )}
      </BottomMenu>

      <MobileEntryEditorPage
        entry={selectedMobileEntry}
        promptType={promptType}
        isOpen={!!mobileEntryEditorId}
        onClose={() => setMobileEntryEditorId(null)}
        onUpdate={handleEntryUpdate}
        onToggle={handleEntryToggle}
        onTextareaRef={(id, el) => {
          entryTextareaRefs.current[id] = el;
        }}
        onTextareaFocus={(id) => {
          activeEntryIdRef.current = id;
        }}
      />

      <DesktopEntryEditorDrawer
        entry={selectedDesktopEntry}
        promptType={promptType}
        isOpen={!!desktopEntryEditorId}
        onClose={() => setDesktopEntryEditorId(null)}
        onUpdate={handleEntryUpdate}
        onToggle={handleEntryToggle}
        onTextareaRef={(id, el) => {
          entryTextareaRefs.current[id] = el;
        }}
        onTextareaFocus={(id) => {
          activeEntryIdRef.current = id;
        }}
      />
    </div>
  );
}
