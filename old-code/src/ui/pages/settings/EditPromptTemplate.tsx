import { useState, useEffect, useMemo, useRef } from "react";
import {
  AnimatePresence,
  Reorder,
  motion,
  useDragControls,
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
  ChevronRight,
  ChevronDown,
  ChevronUp,
  GripVertical,
  Plus,
  Trash2,
  X,
  Layers,
  Wand2,
} from "lucide-react";
import { cn, radius, interactive } from "../../design-tokens";
import { MessageStructurePreview } from "./components/MessageStructurePreview";
import { BottomMenu, NumberInput } from "../../components";
import { confirmBottomMenu } from "../../components/ConfirmBottomMenu";
import { useI18n, type TranslationKey } from "../../../core/i18n/context";
import { Switch } from "../../components/Switch";
import { useNavigationManager } from "../../navigation";
import {
  listPromptTemplates,
  createPromptTemplate,
  updatePromptTemplate,
  getPromptTemplate,
  getAppDefaultTemplateId,
  resetAppDefaultTemplate,
  resetLocalRoleplayTemplate,
  resetCompanionTemplate,
  resetDynamicSummaryTemplate,
  resetDynamicMemoryTemplate,
  resetDynamicMemoryLocalTemplate,
  resetGroupChatTemplate,
  resetGroupChatRoleplayTemplate,
  resetHelpMeReplyTemplate,
  resetHelpMeReplyConversationalTemplate,
  resetLorebookEntryWriterTemplate,
  resetLorebookKeywordGeneratorTemplate,
  resetAvatarGenerationTemplate,
  resetAvatarEditTemplate,
  resetSceneGenerationTemplate,
  resetScenePromptWriterTemplate,
  resetDesignReferenceTemplate,
  resetCompanionSoulWriterTemplate,
  renderPromptPreview,
  getPromptParameterEngine,
} from "../../../core/prompts/service";
import { listCharacters, listPersonas } from "../../../core/storage";
import type {
  Character,
  Persona,
  PromptParameterEngine,
  PromptEntryCondition,
  PromptTemplateType,
  PromptTypeDefinition,
  SystemPromptEntry,
  SystemPromptTemplate,
} from "../../../core/storage/schemas";
import {
  APP_DEFAULT_TEMPLATE_ID,
  APP_LOCAL_ROLEPLAY_TEMPLATE_ID,
  APP_COMPANION_TEMPLATE_ID,
  APP_DYNAMIC_SUMMARY_TEMPLATE_ID,
  APP_DYNAMIC_MEMORY_TEMPLATE_ID,
  APP_DYNAMIC_MEMORY_LOCAL_TEMPLATE_ID,
  APP_HELP_ME_REPLY_TEMPLATE_ID,
  APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID,
  APP_LOREBOOK_ENTRY_WRITER_TEMPLATE_ID,
  LEGACY_APP_LOREBOOK_ENTRY_GENERATOR_TEMPLATE_ID,
  APP_LOREBOOK_KEYWORD_GENERATOR_TEMPLATE_ID,
  APP_GROUP_CHAT_TEMPLATE_ID,
  APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID,
  APP_AVATAR_GENERATION_TEMPLATE_ID,
  APP_AVATAR_EDIT_TEMPLATE_ID,
  APP_SCENE_GENERATION_TEMPLATE_ID,
  APP_SCENE_PROMPT_WRITER_TEMPLATE_ID,
  APP_DESIGN_REFERENCE_TEMPLATE_ID,
  APP_COMPANION_SOUL_WRITER_TEMPLATE_ID,
  isProtectedPromptTemplate,
} from "../../../core/prompts/constants";

type PromptEntryImageSlot = "character" | "persona" | "chatBackground" | "avatar" | "references";
type PromptEntryKind = "text" | "image";
type PromptType = PromptTemplateType;
type Translate = (key: TranslationKey, params?: Record<string, string | number>) => string;

const IMAGE_ENTRY_SLOT_LABEL_KEYS: Record<PromptEntryImageSlot, TranslationKey> = {
  character: "editPrompt.imageSlots.character",
  persona: "editPrompt.imageSlots.persona",
  chatBackground: "editPrompt.imageSlots.chatBackground",
  avatar: "editPrompt.imageSlots.avatar",
  references: "editPrompt.imageSlots.references",
};

const IMAGE_ENTRY_SLOT_TOKENS: Record<PromptEntryImageSlot, string> = {
  character: "{{image[character]}}",
  persona: "{{image[persona]}}",
  chatBackground: "{{image[chatBackground]}}",
  avatar: "{{image[avatar]}}",
  references: "{{image[references]}}",
};

const IMAGE_ENTRY_SLOT_OPTIONS_BY_PROMPT_TYPE: Partial<Record<PromptType, PromptEntryImageSlot[]>> =
  {
    undefined: ["character", "persona", "chatBackground", "avatar", "references"],
    sceneGeneration: ["character", "persona", "chatBackground"],
    scenePromptWriter: ["character", "persona", "chatBackground"],
    designReferenceWriter: ["avatar", "references"],
};

const ENTRY_ROLE_OPTIONS = [
  { value: "system", labelKey: "editPrompt.roles.system" },
  { value: "user", labelKey: "editPrompt.roles.user" },
  { value: "assistant", labelKey: "editPrompt.roles.assistant" },
] as const satisfies ReadonlyArray<{ value: string; labelKey: TranslationKey }>;

const ENTRY_POSITION_OPTIONS = [
  { value: "relative", labelKey: "editPrompt.positions.relative" },
  { value: "inChat", labelKey: "editPrompt.positions.inChat" },
  { value: "conditional", labelKey: "editPrompt.positions.conditional" },
  { value: "interval", labelKey: "editPrompt.positions.interval" },
] as const satisfies ReadonlyArray<{ value: string; labelKey: TranslationKey }>;

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
  labelKey: TranslationKey;
  kind: "boolean" | "number" | "list" | "chatMode" | "infoSource";
  placeholderKey?: TranslationKey;
}> = [
  { value: "chatMode", labelKey: "editPrompt.conditions.chatMode", kind: "chatMode" },
  { value: "infoSource", labelKey: "editPrompt.conditions.infoSource", kind: "infoSource" },
  {
    value: "sceneGenerationEnabled",
    labelKey: "editPrompt.conditions.sceneGenerationEnabled",
    kind: "boolean",
  },
  {
    value: "avatarGenerationEnabled",
    labelKey: "editPrompt.conditions.avatarGenerationEnabled",
    kind: "boolean",
  },
  {
    value: "isLocalImageGenerationModel",
    labelKey: "editPrompt.conditions.isLocalImageGenerationModel",
    kind: "boolean",
  },
  {
    value: "isSceneGenerationLocalImageModel",
    labelKey: "editPrompt.conditions.isSceneGenerationLocalImageModel",
    kind: "boolean",
  },
  { value: "hasScene", labelKey: "editPrompt.conditions.hasScene", kind: "boolean" },
  {
    value: "hasSceneDirection",
    labelKey: "editPrompt.conditions.hasSceneDirection",
    kind: "boolean",
  },
  { value: "hasPersona", labelKey: "editPrompt.conditions.hasPersona", kind: "boolean" },
  {
    value: "messageCountAtLeast",
    labelKey: "editPrompt.conditions.messageCountAtLeast",
    kind: "number",
  },
  {
    value: "participantCountAtLeast",
    labelKey: "editPrompt.conditions.participantCountAtLeast",
    kind: "number",
  },
  {
    value: "keywordAny",
    labelKey: "editPrompt.conditions.keywordAny",
    kind: "list",
    placeholderKey: "editPrompt.conditionPlaceholders.keywordAny",
  },
  {
    value: "keywordAll",
    labelKey: "editPrompt.conditions.keywordAll",
    kind: "list",
    placeholderKey: "editPrompt.conditionPlaceholders.keywordAll",
  },
  {
    value: "keywordNone",
    labelKey: "editPrompt.conditions.keywordNone",
    kind: "list",
    placeholderKey: "editPrompt.conditionPlaceholders.keywordNone",
  },
  {
    value: "dynamicMemoryEnabled",
    labelKey: "editPrompt.conditions.dynamicMemoryEnabled",
    kind: "boolean",
  },
  { value: "hasMemorySummary", labelKey: "editPrompt.conditions.hasMemorySummary", kind: "boolean" },
  { value: "hasKeyMemories", labelKey: "editPrompt.conditions.hasKeyMemories", kind: "boolean" },
  {
    value: "hasLorebookContent",
    labelKey: "editPrompt.conditions.hasLorebookContent",
    kind: "boolean",
  },
  {
    value: "doesAuthorNoteExists",
    labelKey: "editPrompt.conditions.doesAuthorNoteExists",
    kind: "boolean",
  },
  {
    value: "hasActiveScheduledNote",
    labelKey: "editPrompt.conditions.hasActiveScheduledNote",
    kind: "boolean",
  },
  {
    value: "hasSubjectDescription",
    labelKey: "editPrompt.conditions.hasSubjectDescription",
    kind: "boolean",
  },
  {
    value: "hasCurrentDescription",
    labelKey: "editPrompt.conditions.hasCurrentDescription",
    kind: "boolean",
  },
  {
    value: "hasCharacterReferenceImages",
    labelKey: "editPrompt.conditions.hasCharacterReferenceImages",
    kind: "boolean",
  },
  {
    value: "hasChatBackground",
    labelKey: "editPrompt.conditions.hasChatBackground",
    kind: "boolean",
  },
  {
    value: "hasPersonaReferenceImages",
    labelKey: "editPrompt.conditions.hasPersonaReferenceImages",
    kind: "boolean",
  },
  {
    value: "hasCharacterReferenceText",
    labelKey: "editPrompt.conditions.hasCharacterReferenceText",
    kind: "boolean",
  },
  {
    value: "hasPersonaReferenceText",
    labelKey: "editPrompt.conditions.hasPersonaReferenceText",
    kind: "boolean",
  },
  {
    value: "inputScopeAny",
    labelKey: "editPrompt.conditions.inputScopeAny",
    kind: "list",
    placeholderKey: "editPrompt.conditionPlaceholders.inputScopeAny",
  },
  {
    value: "outputScopeAny",
    labelKey: "editPrompt.conditions.outputScopeAny",
    kind: "list",
    placeholderKey: "editPrompt.conditionPlaceholders.outputScopeAny",
  },
  {
    value: "providerIdAny",
    labelKey: "editPrompt.conditions.providerIdAny",
    kind: "list",
    placeholderKey: "editPrompt.conditionPlaceholders.providerIdAny",
  },
  { value: "reasoningEnabled", labelKey: "editPrompt.conditions.reasoningEnabled", kind: "boolean" },
  { value: "visionEnabled", labelKey: "editPrompt.conditions.visionEnabled", kind: "boolean" },
  {
    value: "isTimeAwarenessEnabled",
    labelKey: "editPrompt.conditions.isTimeAwarenessEnabled",
    kind: "boolean",
  },
  { value: "isCompanionMode", labelKey: "editPrompt.conditions.isCompanionMode", kind: "boolean" },
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
    case "infoSource":
      return { type, value: "messages" };
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

function describeSimpleCondition(t: Translate, condition: SimplePromptEntryCondition): string {
  switch (condition.type) {
    case "chatMode":
      return t("editPrompt.describe.chatMode", {
        value: t(
          condition.value === "group"
            ? "editPrompt.conditionValues.chatModeGroup"
            : "editPrompt.conditionValues.chatModeDirect",
        ),
      });
    case "infoSource":
      return t("editPrompt.describe.infoSource", {
        value: t(
          condition.value === "memory"
            ? "editPrompt.conditionValues.infoSourceMemory"
            : condition.value === "mixed"
              ? "editPrompt.conditionValues.infoSourceMixed"
              : "editPrompt.conditionValues.infoSourceMessages",
        ),
      });
    case "sceneGenerationEnabled":
      return t(
        condition.value
          ? "editPrompt.describe.sceneGenerationOn"
          : "editPrompt.describe.sceneGenerationOff",
      );
    case "avatarGenerationEnabled":
      return t(
        condition.value
          ? "editPrompt.describe.avatarGenerationOn"
          : "editPrompt.describe.avatarGenerationOff",
      );
    case "isLocalImageGenerationModel":
      return t(
        condition.value
          ? "editPrompt.describe.localImageGenerationModel"
          : "editPrompt.describe.remoteImageGenerationModel",
      );
    case "isSceneGenerationLocalImageModel":
      return t(
        condition.value
          ? "editPrompt.describe.sceneGenerationLocalImageModel"
          : "editPrompt.describe.sceneGenerationRemoteImageModel",
      );
    case "hasScene":
      return t(condition.value ? "editPrompt.describe.sceneExists" : "editPrompt.describe.sceneMissing");
    case "hasSceneDirection":
      return t(
        condition.value
          ? "editPrompt.describe.sceneHasDirection"
          : "editPrompt.describe.sceneHasNoDirection",
      );
    case "hasPersona":
      return t(
        condition.value ? "editPrompt.describe.personaExists" : "editPrompt.describe.personaMissing",
      );
    case "messageCountAtLeast":
      return t("editPrompt.describe.messagesAtLeast", { count: condition.value });
    case "participantCountAtLeast":
      return t("editPrompt.describe.participantsAtLeast", { count: condition.value });
    case "keywordAny":
      return t("editPrompt.describe.anyKeyword", { values: condition.values.join(", ") });
    case "keywordAll":
      return t("editPrompt.describe.allKeywords", { values: condition.values.join(", ") });
    case "keywordNone":
      return t("editPrompt.describe.noKeywords", { values: condition.values.join(", ") });
    case "dynamicMemoryEnabled":
      return t(
        condition.value ? "editPrompt.describe.dynamicMemoryOn" : "editPrompt.describe.dynamicMemoryOff",
      );
    case "hasMemorySummary":
      return t(
        condition.value
          ? "editPrompt.describe.memorySummaryExists"
          : "editPrompt.describe.memorySummaryMissing",
      );
    case "hasKeyMemories":
      return t(
        condition.value
          ? "editPrompt.describe.keyMemoriesExist"
          : "editPrompt.describe.keyMemoriesMissing",
      );
    case "hasLorebookContent":
      return t(
        condition.value
          ? "editPrompt.describe.lorebookContentExists"
          : "editPrompt.describe.lorebookContentMissing",
      );
    case "doesAuthorNoteExists":
      return t(
        condition.value
          ? "editPrompt.describe.authorNoteExists"
          : "editPrompt.describe.authorNoteMissing",
      );
    case "hasActiveScheduledNote":
      return t(
        condition.value
          ? "editPrompt.describe.scheduledNoteExists"
          : "editPrompt.describe.scheduledNoteMissing",
      );
    case "hasSubjectDescription":
      return t(
        condition.value
          ? "editPrompt.describe.subjectDescriptionExists"
          : "editPrompt.describe.subjectDescriptionMissing",
      );
    case "hasCurrentDescription":
      return t(
        condition.value
          ? "editPrompt.describe.currentDescriptionExists"
          : "editPrompt.describe.currentDescriptionMissing",
      );
    case "hasCharacterReferenceImages":
      return t(
        condition.value
          ? "editPrompt.describe.characterReferenceImagesExist"
          : "editPrompt.describe.characterReferenceImagesMissing",
      );
    case "hasChatBackground":
      return t(
        condition.value
          ? "editPrompt.describe.chatBackgroundExists"
          : "editPrompt.describe.chatBackgroundMissing",
      );
    case "hasPersonaReferenceImages":
      return t(
        condition.value
          ? "editPrompt.describe.personaReferenceImagesExist"
          : "editPrompt.describe.personaReferenceImagesMissing",
      );
    case "hasCharacterReferenceText":
      return t(
        condition.value
          ? "editPrompt.describe.characterReferenceTextExists"
          : "editPrompt.describe.characterReferenceTextMissing",
      );
    case "hasPersonaReferenceText":
      return t(
        condition.value
          ? "editPrompt.describe.personaReferenceTextExists"
          : "editPrompt.describe.personaReferenceTextMissing",
      );
    case "inputScopeAny":
      return t("editPrompt.describe.inputScope", { values: condition.values.join(", ") });
    case "outputScopeAny":
      return t("editPrompt.describe.outputScope", { values: condition.values.join(", ") });
    case "providerIdAny":
      return t("editPrompt.describe.provider", { values: condition.values.join(", ") });
    case "reasoningEnabled":
      return t(
        condition.value ? "editPrompt.describe.reasoningOn" : "editPrompt.describe.reasoningOff",
      );
    case "visionEnabled":
      return t(condition.value ? "editPrompt.describe.visionOn" : "editPrompt.describe.visionOff");
    case "isTimeAwarenessEnabled":
      return t(
        condition.value
          ? "editPrompt.describe.timeAwarenessOn"
          : "editPrompt.describe.timeAwarenessOff",
      );
    case "isCompanionMode":
      return t(
        condition.value
          ? "editPrompt.describe.companionModeOn"
          : "editPrompt.describe.companionModeOff",
      );
  }
}

function describeConditionTree(
  t: Translate,
  condition: PromptEntryCondition | null | undefined,
): string {
  const draft = decomposeConditionTree(condition);
  const describeRows = (rows: ConditionRow[]) =>
    rows
      .map((row, index) => {
        const label = describeSimpleCondition(t, row.condition);
        return index === 0 ? label : `${row.joinWithPrevious.toUpperCase()} ${label}`;
      })
      .join(" ");

  const include = describeRows(draft.include);
  const exclude = describeRows(draft.exclude);
  if (!include && !exclude) {
    return t("editPrompt.describe.alwaysActive");
  }
  if (include && exclude) {
    return t("editPrompt.describe.includeExclude", { include, exclude });
  }
  if (exclude) {
    return t("editPrompt.describe.excludePrefix", { value: exclude });
  }
  return include;
}

function describeConditionSentence(
  t: Translate,
  condition: PromptEntryCondition | null | undefined,
): string {
  if (!condition) {
    return t("editPrompt.describe.sentenceAlways");
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
      const text = group.rows
        .map((row) => describeSimpleCondition(t, row.condition))
        .join(connector);
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
    return t("editPrompt.describe.sentenceWhenAndNot", { include, exclude });
  }

  if (exclude) {
    return t("editPrompt.describe.sentenceUnless", { exclude });
  }

  return t("editPrompt.describe.sentenceWhen", { include });
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
  t: Translate,
  condition: SimplePromptEntryCondition,
): { type: SimplePromptEntryConditionType; value: string; label: string } | null {
  switch (condition.type) {
    case "chatMode":
    case "infoSource":
      return {
        type: condition.type,
        value: condition.value,
        label: describeSimpleCondition(t, condition),
      };
    case "sceneGenerationEnabled":
    case "avatarGenerationEnabled":
    case "isLocalImageGenerationModel":
    case "isSceneGenerationLocalImageModel":
    case "hasScene":
    case "hasSceneDirection":
    case "hasPersona":
    case "dynamicMemoryEnabled":
    case "hasMemorySummary":
    case "hasKeyMemories":
    case "hasLorebookContent":
    case "doesAuthorNoteExists":
    case "hasActiveScheduledNote":
    case "hasSubjectDescription":
    case "hasCurrentDescription":
    case "hasCharacterReferenceImages":
    case "hasChatBackground":
    case "hasPersonaReferenceImages":
    case "hasCharacterReferenceText":
    case "hasPersonaReferenceText":
    case "reasoningEnabled":
    case "visionEnabled":
    case "isTimeAwarenessEnabled":
    case "isCompanionMode":
      return {
        type: condition.type,
        value: String(condition.value),
        label: describeSimpleCondition(t, condition),
      };
    default:
      return null;
  }
}

function getConditionWarnings(t: Translate, draft: ConditionDraft): string[] {
  const warnings = new Set<string>();
  const includeLabels = new Map<string, string>();

  draft.include.forEach((row) => {
    includeLabels.set(
      getConditionIdentity(row.condition),
      describeSimpleCondition(t, row.condition),
    );
  });

  draft.exclude.forEach((row) => {
    const label = includeLabels.get(getConditionIdentity(row.condition));
    if (label) {
      warnings.add(t("editPrompt.warnings.requiresAndExcludes", { label }));
    }
  });

  const scalarBuckets = new Map<SimplePromptEntryConditionType, Map<string, string>>();
  draft.include.forEach((row) => {
    const bucket = getScalarConditionBucket(t, row.condition);
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
        t("editPrompt.warnings.mutuallyExclusive", { values: [...values.values()].join(" and ") }),
      );
    }
  });

  return [...warnings];
}

const createDefaultEntry = (
  t: Translate,
  content: string,
  overrides?: Partial<SystemPromptEntry>,
): SystemPromptEntry => ({
  id: createEntryId(),
  name: t("editPrompt.defaults.systemPromptName"),
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

const createExtraEntry = (t: Translate, overrides?: Partial<SystemPromptEntry>) =>
  createDefaultEntry(t, "", {
    name: t("editPrompt.defaults.promptEntryName"),
    systemPrompt: false,
    ...overrides,
  });

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

function getEntryKindSummary(t: Translate, entry: SystemPromptEntry) {
  const slot = getPromptEntryImageSlot(entry);
  if (!slot) {
    return t("editPrompt.entryKind.text");
  }
  return t("editPrompt.entryKind.imageWithSlot", { slot: t(IMAGE_ENTRY_SLOT_LABEL_KEYS[slot]) });
}

function getEntryPreviewText(t: Translate, entry: SystemPromptEntry) {
  const trimmed = entry.content.trim();
  if (trimmed) {
    return trimmed;
  }

  const slot = getPromptEntryImageSlot(entry);
  if (slot) {
    return t("editPrompt.entryKind.attachment", { slot: t(IMAGE_ENTRY_SLOT_LABEL_KEYS[slot]) });
  }

  return t("editPrompt.entryKind.clickToEdit");
}

function getInjectionModeHint(t: Translate, position: SystemPromptEntry["injectionPosition"]) {
  switch (position) {
    case "relative":
      return t("editPrompt.behavior.relativeHint");
    case "inChat":
      return t("editPrompt.behavior.inChatHint");
    case "conditional":
      return t("editPrompt.behavior.conditionalHint");
    case "interval":
      return t("editPrompt.behavior.intervalHint");
    default:
      return "";
  }
}

function getEntryRoleLabel(t: Translate, role: SystemPromptEntry["role"]) {
  const option = ENTRY_ROLE_OPTIONS.find((opt) => opt.value === role);
  return option ? t(option.labelKey) : role;
}

function getEntryPositionLabel(t: Translate, position: SystemPromptEntry["injectionPosition"]) {
  const option = ENTRY_POSITION_OPTIONS.find((opt) => opt.value === position);
  return option ? t(option.labelKey) : position;
}

function getEntryBehaviorSummary(t: Translate, entry: SystemPromptEntry) {
  switch (entry.injectionPosition) {
    case "conditional":
      return t("editPrompt.behavior.afterMessages", {
        count: entry.conditionalMinMessages ?? DEFAULT_CONDITIONAL_MIN_MESSAGES,
      });
    case "interval":
      return t("editPrompt.behavior.everyMessages", {
        count: entry.intervalTurns ?? DEFAULT_INTERVAL_TURNS,
      });
    case "inChat":
      return t("editPrompt.behavior.inlineInChat");
    case "relative":
    default:
      return t("editPrompt.behavior.beforeChatHistory");
  }
}

function getEntryActivationSummary(t: Translate, entry: SystemPromptEntry) {
  const conditionDraft = decomposeConditionTree(entry.conditions);
  const conditionCount = conditionDraft.include.length + conditionDraft.exclude.length;
  if (conditionCount === 0) {
    return t("editPrompt.activation.alwaysActive");
  }
  return conditionCount === 1
    ? describeConditionTree(t, entry.conditions)
    : t("editPrompt.activation.ruleCount", { count: conditionCount });
}

function MetaPill({
  label,
  value,
  title,
}: {
  label: string;
  value: string;
  title?: string;
}) {
  return (
    <span
      title={title}
      className="inline-flex items-center gap-1.5 rounded-full border border-fg/10 bg-surface-el/30 px-2.5 py-1 text-[11px]"
    >
      <span className="text-fg/40">{label}</span>
      <span className="font-medium text-fg/80">{value}</span>
    </span>
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
  const { t } = useI18n();
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
      <div className="shrink-0 sm:w-40">
        <select
          value={condition.type}
          onChange={(event) =>
            onChange(createDefaultCondition(event.target.value as SimplePromptEntryConditionType))
          }
          className={controlClasses}
        >
          {SIMPLE_CONDITION_OPTIONS.map((option) => (
            <option key={option.value} value={option.value}>
              {t(option.labelKey)}
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
            <option value="direct">{t("editPrompt.conditionValues.chatModeDirect")}</option>
            <option value="group">{t("editPrompt.conditionValues.chatModeGroup")}</option>
          </select>
        ) : meta.kind === "infoSource" ? (
          <select
            value={condition.type === "infoSource" ? condition.value : "messages"}
            onChange={(event) =>
              onChange({
                type: "infoSource",
                value: event.target.value as "messages" | "memory" | "mixed",
              })
            }
            className={controlClasses}
          >
            <option value="messages">{t("editPrompt.conditionValues.infoSourceMessages")}</option>
            <option value="memory">{t("editPrompt.conditionValues.infoSourceMemory")}</option>
            <option value="mixed">{t("editPrompt.conditionValues.infoSourceMixed")}</option>
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
                {val === "true"
                  ? t("editPrompt.conditionValues.true")
                  : t("editPrompt.conditionValues.false")}
              </button>
            ))}
          </div>
        ) : meta.kind === "number" ? (
          <NumberInput
            min={condition.type === "messageCountAtLeast" ? 0 : 1}
            value={"value" in condition ? Number(condition.value) : 1}
            onChange={(next) =>
              onChange({
                ...condition,
                value: next,
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
            placeholder={meta.placeholderKey ? t(meta.placeholderKey) : undefined}
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
        title={t("editPrompt.conditionsPanel.removeCondition")}
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
  const { t } = useI18n();
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
        <option value="and">{t("editPrompt.conditionValues.and")}</option>
        <option value="or">{t("editPrompt.conditionValues.or")}</option>
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
  const { t } = useI18n();
  const draft = useMemo(() => decomposeConditionTree(entry.conditions), [entry.conditions]);
  const warnings = useMemo(() => getConditionWarnings(t, draft), [t, draft]);
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
                {t("editPrompt.warnings.fixContradictory")}
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
          <h4 className="text-sm font-bold text-fg/60">
            {t("editPrompt.conditionsPanel.includeRules")}
          </h4>
          <button
            type="button"
            onClick={() => addRule("include")}
            className="flex items-center gap-1.5 rounded-lg border border-accent/20 bg-accent/5 px-3 py-1.5 text-xs font-bold text-accent transition-all hover:bg-accent/10"
          >
            <Plus className="h-3.5 w-3.5" />
            {t("editPrompt.conditionsPanel.addRule")}
          </button>
        </div>

        <p className="max-w-[72ch] text-[11px] leading-relaxed text-fg/42">
          {t("editPrompt.conditionsPanel.groupingHint")}
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
              <p className="text-sm text-fg/30">
                {t("editPrompt.conditionsPanel.activeForAll")}
              </p>
            </div>
          )}
        </div>
      </section>

      <section className="space-y-4">
        <div className="flex items-center justify-between">
          <h4 className="text-sm font-bold text-fg/60">
            {t("editPrompt.conditionsPanel.exclusions")}
          </h4>
          <button
            type="button"
            onClick={() => addRule("exclude")}
            className="flex items-center gap-1.5 rounded-lg border border-danger/20 bg-danger/5 px-3 py-1.5 text-xs font-bold text-danger/70 transition-all hover:bg-danger/10"
          >
            <Plus className="h-3.5 w-3.5" />
            {t("editPrompt.conditionsPanel.addExclusion")}
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
              <p className="text-sm text-fg/30">
                {t("editPrompt.conditionsPanel.noExclusions")}
              </p>
            </div>
          )}
        </div>
      </section>

      <div className="border-t border-fg/10 pt-4">
        <div className="flex items-center gap-2">
          <Code2 className="h-3.5 w-3.5 text-fg/26" />
          <p className="text-[10px] font-medium uppercase tracking-wide text-fg/30">
            {t("editPrompt.conditionsPanel.evaluatesAs")}
          </p>
        </div>
        <p className="mt-2 max-w-[72ch] text-sm leading-relaxed text-fg/58">
          {describeConditionSentence(t, entry.conditions)}
        </p>
      </div>
    </div>
  );
}

function SectionHeader({
  label,
  trailing,
}: {
  label: string;
  trailing?: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-3 border-t border-fg/8 pt-4">
      <h3 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
        {label}
      </h3>
      {trailing}
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
  const { t } = useI18n();
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
      ? t("editPrompt.form.roleImageHint")
      : t("editPrompt.form.roleHint");
  const contentLabel =
    entryKind === "image" ? t("editPrompt.form.attachmentNote") : t("editPrompt.form.promptContent");
  const contentHint =
    entryKind === "image"
      ? t("editPrompt.form.imageContentHint")
      : t("editPrompt.form.textContentHint");
  const contentPlaceholder =
    entryKind === "image"
      ? t("editPrompt.form.imageContentPlaceholder")
      : t("editPrompt.form.textContentPlaceholder");
  const showsLegacyImageToken = Object.values(IMAGE_ENTRY_SLOT_TOKENS).includes(
    entry.content.trim(),
  );
  const conditionDraft = useMemo(
    () => decomposeConditionTree(entry.conditions),
    [entry.conditions],
  );
  const conditionsCount = conditionDraft.include.length + conditionDraft.exclude.length;
  const [conditionsOpen, setConditionsOpen] = useState(conditionsCount > 0);

  return (
    <div className="space-y-5">
      <div className="flex items-center justify-between gap-3 rounded-lg border border-fg/10 bg-fg/4 px-3 py-2.5">
        <div>
          <p className="text-sm font-medium text-fg">{t("editPrompt.form.entryState")}</p>
          <p className="text-xs text-fg/45">
            {entry.systemPrompt
              ? t("editPrompt.form.systemAlwaysEnabled")
              : t("editPrompt.form.controlsInjection")}
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
            {entry.systemPrompt
              ? t("editPrompt.card.required")
              : entry.enabled
                ? t("common.labels.enabled")
                : t("common.labels.disabled")}
          </span>
        </div>
      </div>

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
          <p className="text-[11px] text-fg/38">{t("editPrompt.form.legacyTokenDetected")}</p>
        ) : null}
      </div>

      <SectionHeader label={t("editPrompt.form.basic")} />

      <div className="grid gap-4 md:grid-cols-2">
        <div className="space-y-1.5">
          <label className="text-xs font-medium text-fg/55">{t("editPrompt.form.entryName")}</label>
          <input
            value={entry.name}
            onChange={(event) => onUpdate({ name: event.target.value })}
            className="w-full rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-sm text-fg"
            placeholder={t("editPrompt.form.entryNamePlaceholder")}
          />
          <p className="text-[11px] text-fg/45">{t("editPrompt.form.entryNameHint")}</p>
        </div>

        <div className="space-y-1.5">
          <label className="text-xs font-medium text-fg/55">{t("editPrompt.form.entryKind")}</label>
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
            <option value="text">{t("editPrompt.form.kindText")}</option>
            {canSelectImageKind ? (
              <option value="image">{t("editPrompt.form.kindImage")}</option>
            ) : null}
          </select>
          <p className="text-[11px] text-fg/45">
            {entryKind === "image"
              ? t("editPrompt.form.kindImageHint")
              : canSelectImageKind
                ? t("editPrompt.form.kindTextHintImageAllowed")
                : t("editPrompt.form.kindTextHint")}
          </p>
        </div>
      </div>

      {entryKind === "image" ? (
        <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_220px]">
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-fg/55">
              {t("editPrompt.form.attachmentSlot")}
            </label>
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
                  {t(IMAGE_ENTRY_SLOT_LABEL_KEYS[slot])}
                </option>
              ))}
            </select>
            <p className="text-[11px] text-fg/45">{t("editPrompt.form.attachmentSlotHint")}</p>
          </div>

          <div className="rounded-lg border border-fg/10 bg-fg/4 px-3 py-2.5">
            <p className="text-[10px] font-medium uppercase tracking-wide text-fg/35">
              {t("editPrompt.form.attachmentBehavior")}
            </p>
            <p className="mt-1.5 text-sm leading-relaxed text-fg/72">
              {t("editPrompt.form.attachmentBehaviorHint")}
            </p>
          </div>
        </div>
      ) : null}

      <SectionHeader label={t("editPrompt.form.injection")} />

      <div className="grid gap-4 md:grid-cols-2">
        <div className="space-y-1.5">
          <label className="text-xs font-medium text-fg/55">{t("editPrompt.form.role")}</label>
          <select
            value={roleValue}
            onChange={(event) => onUpdate({ role: event.target.value as any })}
            disabled={entryKind === "image"}
            className="h-10 w-full rounded-lg border border-fg/10 bg-fg/5 px-3 text-sm text-fg disabled:cursor-not-allowed disabled:opacity-60"
          >
            {ENTRY_ROLE_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {t(option.labelKey)}
              </option>
            ))}
          </select>
          <p className="text-[11px] text-fg/45">{roleDescription}</p>
        </div>

        <div className="space-y-1.5">
          <label className="text-xs font-medium text-fg/55">{t("editPrompt.form.placement")}</label>
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
                {t(option.labelKey)}
              </option>
            ))}
          </select>
          <p className="text-[11px] text-fg/45">
            {getInjectionModeHint(t, entry.injectionPosition)}
          </p>
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
          <label className="text-xs font-medium text-fg/55">
            {t("editPrompt.form.insertionDepth")}
          </label>
          <NumberInput
            min={0}
            value={entry.injectionDepth}
            onChange={(next) => onUpdate({ injectionDepth: next ?? 0 })}
            className="h-10 w-full rounded-lg border border-fg/10 bg-fg/5 px-3 text-sm text-fg"
            placeholder="0"
          />
          <p className="text-[11px] text-fg/45">{t("editPrompt.form.insertionDepthHint")}</p>
        </div>

        {entry.injectionPosition === "conditional" ? (
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-fg/55">
              {t("editPrompt.form.minMessages")}
            </label>
            <NumberInput
              min={1}
              value={entry.conditionalMinMessages ?? DEFAULT_CONDITIONAL_MIN_MESSAGES}
              onChange={(next) => onUpdate({ conditionalMinMessages: next })}
              className="h-10 w-full rounded-lg border border-fg/10 bg-fg/5 px-3 text-sm text-fg"
            />
            <p className="text-[11px] text-fg/45">{t("editPrompt.form.minMessagesHint")}</p>
          </div>
        ) : entry.injectionPosition === "interval" ? (
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-fg/55">
              {t("editPrompt.form.everyNMessages")}
            </label>
            <NumberInput
              min={1}
              value={entry.intervalTurns ?? DEFAULT_INTERVAL_TURNS}
              onChange={(next) => onUpdate({ intervalTurns: next })}
              className="h-10 w-full rounded-lg border border-fg/10 bg-fg/5 px-3 text-sm text-fg"
            />
            <p className="text-[11px] text-fg/45">{t("editPrompt.form.everyNMessagesHint")}</p>
          </div>
        ) : null}
      </div>

      <SectionHeader
        label={t("editPrompt.form.conditionsSection")}
        trailing={
          <button
            type="button"
            onClick={() => setConditionsOpen((v) => !v)}
            className="inline-flex items-center gap-1.5 rounded-full border border-fg/10 bg-fg/5 px-2.5 py-1 text-[11px] text-fg/60 hover:border-fg/20 hover:text-fg/80"
          >
            <span>
              {conditionsCount > 0
                ? conditionsCount === 1
                  ? t("editPrompt.form.conditionsCount", { count: conditionsCount })
                  : t("editPrompt.form.conditionsCountPlural", { count: conditionsCount })
                : t("editPrompt.form.conditionsNone")}
            </span>
            {conditionsOpen ? (
              <ChevronUp className="h-3 w-3" />
            ) : (
              <ChevronDown className="h-3 w-3" />
            )}
          </button>
        }
      />

      {conditionsOpen ? (
        <PromptEntryConditionsPanel entry={entry} onUpdate={onUpdate} />
      ) : null}
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
  const { t } = useI18n();
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
            className="fixed bottom-0 left-0 top-[var(--titlebar-h,0px)] z-50 hidden w-[min(560px,48vw)] border-r border-fg/10 bg-surface lg:flex lg:flex-col"
            initial={{ x: "-100%" }}
            animate={{ x: 0 }}
            exit={{ x: "-100%" }}
            transition={{ duration: 0.18, ease: "easeOut" }}
          >
            <div className="flex items-center justify-between border-b border-fg/10 px-5 py-4">
              <div>
                <h2 className="text-base font-semibold text-fg">{t("editPrompt.drawer.editEntry")}</h2>
                <p className="text-xs text-fg/45">
                  {entry.name || t("editPrompt.defaults.promptEntryName")}
                </p>
              </div>
              <button
                type="button"
                onClick={onClose}
                className="rounded-md border border-fg/10 px-3 py-1.5 text-xs font-medium text-fg/65 transition hover:bg-fg/8 hover:text-fg"
              >
                {t("editPrompt.drawer.close")}
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
  const { t } = useI18n();
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
              <h2 className="text-base font-semibold text-fg">{t("editPrompt.drawer.editEntry")}</h2>
              <p className="text-xs text-fg/45">
                {entry.name || t("editPrompt.defaults.promptEntryName")}
              </p>
            </div>
            <button
              type="button"
              onClick={onClose}
              className="rounded-md border border-fg/10 px-3 py-1.5 text-xs font-medium text-fg/65 transition hover:bg-fg/8 hover:text-fg"
            >
              {t("common.buttons.done")}
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

const ensureSystemEntry = (t: Translate, entries: SystemPromptEntry[]) => {
  if (entries.length === 0) return [createDefaultEntry(t, "")];
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
  const conditionSummary = getEntryActivationSummary(t, entry);
  const contentPreview = getEntryPreviewText(t, entry);
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
          title={t("editPrompt.card.dragToReorder")}
        >
          <GripVertical className="h-4 w-4" />
        </button>

        <button
          onClick={() => onToggleCollapse(entry.id)}
          className={cn(
            "flex h-8 w-8 items-center justify-center rounded-lg",
            "border border-fg/10 bg-fg/5 text-fg/40",
          )}
          title={collapsed ? t("editPrompt.card.expandEntry") : t("editPrompt.card.collapseEntry")}
        >
          {collapsed ? <ChevronDown className="h-4 w-4" /> : <ChevronUp className="h-4 w-4" />}
        </button>

        <input
          value={entry.name}
          onChange={(event) => onUpdate(entry.id, { name: event.target.value })}
          className="flex-1 rounded-lg border border-fg/10 bg-surface-el/30 px-3 py-2 text-sm text-fg"
          placeholder={t("editPrompt.card.entryNamePlaceholder")}
        />

        <div className="flex items-center gap-2">
          <div className="flex items-center gap-3">
            <span
              onClick={(event) => event.stopPropagation()}
              title={
                entry.systemPrompt
                  ? t("editPrompt.card.systemAlwaysEnabledTitle")
                  : t("editPrompt.card.toggle")
              }
            >
              <Switch
                id={toggleId}
                checked={entry.enabled || entry.systemPrompt}
                onChange={() => onToggle(entry.id)}
                disabled={entry.systemPrompt}
              />
            </span>
            <span className="text-xs text-fg/50">
              {entry.systemPrompt
                ? t("editPrompt.card.required")
                : entry.enabled
                  ? t("common.labels.enabled")
                  : t("common.labels.disabled")}
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
              <div className="flex flex-wrap items-center gap-1.5">
                <MetaPill
                  label={t("editPrompt.meta.role")}
                  value={
                    getPromptEntryKind(entry) === "image"
                      ? t("editPrompt.roles.user")
                      : getEntryRoleLabel(t, entry.role)
                  }
                />
                <MetaPill label={t("editPrompt.meta.kind")} value={getEntryKindSummary(t, entry)} />
                <MetaPill
                  label={t("editPrompt.meta.placement")}
                  value={getEntryPositionLabel(t, entry.injectionPosition)}
                  title={getInjectionModeHint(t, entry.injectionPosition)}
                />
                <MetaPill
                  label={t("editPrompt.meta.behavior")}
                  value={getEntryBehaviorSummary(t, entry)}
                />
                <MetaPill label={t("editPrompt.meta.activation")} value={conditionSummary} />
                {entry.injectionDepth > 0 ? (
                  <MetaPill label={t("editPrompt.meta.depth")} value={String(entry.injectionDepth)} />
                ) : null}
              </div>

              {isImageEntry ? (
                <div className="flex items-center justify-between gap-3 rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3">
                  <div className="min-w-0">
                    <p className="text-[11px] font-medium uppercase tracking-wide text-fg/40">
                      {t("editPrompt.card.promptContent")}
                    </p>
                    <p className="mt-1 truncate text-sm text-fg/72">
                      {entry.content.trim() || t("editPrompt.card.noNoteAdded")}
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
                    {t("common.buttons.edit")}
                  </button>
                </div>
              ) : (
                <button
                  type="button"
                  onClick={onOpenEditor}
                  className={cn(
                    "group block w-full rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3 text-left transition-colors",
                    "hover:border-fg/20 hover:bg-surface-el/30",
                  )}
                >
                  <div className="flex items-center justify-between gap-3">
                    <span className="text-[11px] font-medium uppercase tracking-wide text-fg/40">
                      {t("editPrompt.card.promptContent")}
                    </span>
                    <span className="inline-flex items-center gap-1 text-[11px] text-fg/40 transition-colors group-hover:text-fg/70">
                      {t("editPrompt.card.openEditor")}
                      <ChevronRight className="h-3 w-3" />
                    </span>
                  </div>
                  <p className="mt-2 line-clamp-3 whitespace-pre-wrap text-sm leading-relaxed text-fg/80">
                    {contentPreview}
                  </p>
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
  const conditionSummary = describeConditionTree(t, entry.conditions);
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
            title={t("editPrompt.card.dragToReorder")}
          >
            <GripVertical className="h-4 w-4" />
          </button>
          <div className="min-w-0">
            <p className="text-sm font-medium text-fg truncate">{entry.name}</p>
            <p className="text-[11px] text-fg/40 uppercase tracking-wide">
              {getEntryKindSummary(t, entry)} · {entry.injectionPosition}
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
              title={
                entry.systemPrompt
                  ? t("editPrompt.card.systemAlwaysEnabledTitle")
                  : t("editPrompt.card.toggle")
              }
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

const PROMPT_TYPE_NAME_KEYS: Partial<Record<PromptType, TranslationKey>> = {
  undefined: "editPrompt.promptTypes.undefined",
  directChat: "editPrompt.promptTypes.directChat",
  companionChat: "editPrompt.promptTypes.companionChat",
  groupChatRoleplay: "editPrompt.promptTypes.groupChatRoleplay",
  groupChatConversational: "editPrompt.promptTypes.groupChatConversational",
  dynamicMemorySummarizer: "editPrompt.promptTypes.dynamicMemorySummarizer",
  dynamicMemoryManager: "editPrompt.promptTypes.dynamicMemoryManager",
  replyHelperRoleplay: "editPrompt.promptTypes.replyHelperRoleplay",
  replyHelperConversational: "editPrompt.promptTypes.replyHelperConversational",
  avatarGeneration: "editPrompt.promptTypes.avatarGeneration",
  avatarEditRequest: "editPrompt.promptTypes.avatarEditRequest",
  sceneGeneration: "editPrompt.promptTypes.sceneGeneration",
  scenePromptWriter: "editPrompt.promptTypes.scenePromptWriter",
  designReferenceWriter: "editPrompt.promptTypes.designReferenceWriter",
  companionSoulWriter: "editPrompt.promptTypes.companionSoulWriter",
  lorebookEntryWriter: "editPrompt.promptTypes.lorebookEntryWriter",
  lorebookKeywordGenerator: "editPrompt.promptTypes.lorebookKeywordGenerator",
};

export function getPromptTypeNameKey(type: PromptType): TranslationKey {
  return PROMPT_TYPE_NAME_KEYS[type] ?? "editPrompt.promptTypes.undefined";
}

const PROMPT_TYPE_NAME_FALLBACKS: Partial<Record<PromptType, string>> = {
  undefined: "Undefined",
  directChat: "Direct Chat",
  companionChat: "Companion Chat",
  groupChatRoleplay: "Group Chat (Roleplay)",
  groupChatConversational: "Group Chat (Conversation)",
  dynamicMemorySummarizer: "Dynamic Memory Summarizer",
  dynamicMemoryManager: "Dynamic Memory Manager",
  replyHelperRoleplay: "Reply Helper (Roleplay)",
  replyHelperConversational: "Reply Helper (Conversational)",
  avatarGeneration: "Avatar Generation",
  avatarEditRequest: "Avatar Edit Request",
  sceneGeneration: "Scene Generation",
  scenePromptWriter: "Scene Prompt Writer",
  designReferenceWriter: "Design Reference Writer",
  companionSoulWriter: "Companion Soul Writer",
  lorebookEntryWriter: "Lorebook Entry Writer",
  lorebookKeywordGenerator: "Lorebook Keyword Generator",
};

export function getPromptTypeName(type: PromptType): string;
export function getPromptTypeName(t: Translate, type: PromptType): string;
export function getPromptTypeName(arg1: Translate | PromptType, arg2?: PromptType): string {
  if (typeof arg1 === "function") {
    return arg1(getPromptTypeNameKey(arg2 as PromptType));
  }
  return PROMPT_TYPE_NAME_FALLBACKS[arg1] ?? PROMPT_TYPE_NAME_FALLBACKS.undefined ?? "Undefined";
}

function cloneTemplateEntries(entries: SystemPromptEntry[]): SystemPromptEntry[] {
  return entries.map((entry) => ({
    ...entry,
    conditions: entry.conditions ? JSON.parse(JSON.stringify(entry.conditions)) : null,
    promptEntryPayload: entry.promptEntryPayload
      ? JSON.parse(JSON.stringify(entry.promptEntryPayload))
      : null,
  }));
}

function LoadingSkeleton() {
  return (
    <div className="flex h-full flex-col">
      <main className="flex-1 overflow-y-auto px-4 pt-4 pb-6">
        <div className="mx-auto w-full max-w-5xl space-y-4">
          <div className="h-12 w-full animate-pulse rounded-xl bg-fg/10" />
          <div className="h-80 w-full animate-pulse rounded-xl bg-fg/10" />
        </div>
      </main>
    </div>
  );
}

export function EditPromptTemplate() {
  const { t } = useI18n();
  const { go } = useNavigationManager();
  const { id } = useParams<{ id: string }>();
  const isEditing = !!id;
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const entryTextareaRefs = useRef<Record<string, HTMLTextAreaElement | null>>({});
  const activeEntryIdRef = useRef<string | null>(null);
  const entriesRef = useRef<SystemPromptEntry[]>([]);
  const nameRef = useRef("");
  const contentRef = useRef("");
  const savingRef = useRef(false);
  const initialRef = useRef<{
    name: string;
    promptType: PromptType;
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
  const [showTemplatePicker, setShowTemplatePicker] = useState(false);
  const [editorView, setEditorView] = useState<"entries" | "structure">("entries");
  const [mobilePreviewTab, setMobilePreviewTab] = useState<"content" | "structure">("content");
  const [copiedVar, setCopiedVar] = useState<string | null>(null);
  const [highlightedEntryId, setHighlightedEntryId] = useState<string | null>(null);

  // Template metadata
  const [isAppDefault, setIsAppDefault] = useState(false);
  const [promptType, setPromptType] = useState<PromptType>("undefined");
  const [parameterEngine, setParameterEngine] = useState<PromptParameterEngine | null>(null);
  const [protectedTemplates, setProtectedTemplates] = useState<SystemPromptTemplate[]>([]);
  const [resetting, setResetting] = useState(false);
  const [requiredVariables, setRequiredVariables] = useState<string[]>([]);
  const [missingVariables, setMissingVariables] = useState<string[]>([]);

  const canReset = isAppDefault && Boolean(id);

  const usesEntryEditor = true;

  const promptTypeDefinitions = useMemo(() => {
    return new Map<PromptType, PromptTypeDefinition>(
      (parameterEngine?.promptTypes ?? []).map((definition) => [definition.promptType, definition]),
    );
  }, [parameterEngine]);
  const currentPromptTypeDefinition = promptTypeDefinitions.get(promptType) ?? null;
  const variables = currentPromptTypeDefinition?.allowedVariables ?? [];

  const contentValue = usesEntryEditor ? entriesToContent(entries) : content;
  const charCount = contentValue.length;
  const charCountColor =
    charCount > 8000 ? "text-danger/80" : charCount > 5000 ? "text-warning/80" : "text-fg/40";

  const hasEntryContent = entries.some((entry) => entryHasEditableContent(entry));
  const hasContent = content.trim().length > 0;
  const conditionValidationErrors = useMemo(
    () =>
      entries.flatMap((entry, index) =>
        getConditionWarnings(t, decomposeConditionTree(entry.conditions)).map((warning) => {
          const label =
            entry.name.trim() || t("editPrompt.validation.entryFallback", { index: index + 1 });
          return `${label}: ${warning}`;
        }),
      ),
    [t, entries],
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
      promptType !== initialRef.current.promptType ||
      content !== initialRef.current.content ||
      serializeEntries(entries) !== initialRef.current.entries ||
      condensePromptEntries !== initialRef.current.condensePromptEntries);
  const canSave =
    isDirty &&
    name.trim().length > 0 &&
    (hasEntryContent || hasContent) &&
    conditionValidationErrors.length === 0;
  const showTemplateEmptyState = !isEditing && usesEntryEditor && entries.length === 0;

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
    const source = usesEntryEditor ? entriesToValidationSource(entries) : content;
    const missing = requiredVariables.filter((v) => !source.includes(v));
    setMissingVariables(missing);
  }, [content, entries, requiredVariables, usesEntryEditor]);

  useEffect(() => {
    const required = promptTypeDefinitions.get(promptType)?.requiredVariables ?? [];
    setRequiredVariables(required);
  }, [promptType, promptTypeDefinitions]);

  async function loadData() {
    try {
      const [chars, pers, nextParameterEngine, promptTemplates] = await Promise.all([
        listCharacters(),
        listPersonas(),
        getPromptParameterEngine(),
        listPromptTemplates(),
      ]);
      setCharacters(chars);
      setPersonas(pers);
      setParameterEngine(nextParameterEngine);
      setProtectedTemplates(
        promptTemplates.filter((template) => isProtectedPromptTemplate(template.id)),
      );
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
          setPromptType(template.promptType);

          const nextEntries =
            template.entries?.length > 0
              ? template.entries
              : [createDefaultEntry(t, template.content)];
          const normalizedEntries = ensureSystemEntry(t, nextEntries);
          setEntries(normalizedEntries);
          setCondensePromptEntries(Boolean(template.condensePromptEntries));
          setCollapsedEntries(
            Object.fromEntries(normalizedEntries.map((entry) => [entry.id, true])),
          );
          initialRef.current = {
            name: template.name,
            promptType: template.promptType,
            content: template.content,
            entries: serializeEntries(normalizedEntries),
            condensePromptEntries: Boolean(template.condensePromptEntries),
          };
          const required =
            nextParameterEngine.promptTypes.find(
              (definition) => definition.promptType === template.promptType,
            )?.requiredVariables ?? [];
          setRequiredVariables(required);
        }
      } else {
        setContent("");
        setEntries([]);
        setCondensePromptEntries(false);
        setCollapsedEntries({});
        setPromptType("undefined");
        setRequiredVariables(
          nextParameterEngine.promptTypes.find((definition) => definition.promptType === "undefined")
            ?.requiredVariables ?? [],
        );
        initialRef.current = {
          name: "",
          promptType: "undefined",
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
    const entry = createExtraEntry(t);
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

  const applyProtectedTemplate = (template: SystemPromptTemplate) => {
    const nextEntries =
      template.entries?.length > 0
        ? cloneTemplateEntries(template.entries)
        : [createDefaultEntry(t, template.content)];
    const normalizedEntries = ensureSystemEntry(t, nextEntries);

    setName((currentName) => (currentName.trim().length > 0 ? currentName : template.name));
    setContent(template.content);
    setPromptType(template.promptType);
    setEntries(normalizedEntries);
    setCondensePromptEntries(Boolean(template.condensePromptEntries));
    setCollapsedEntries(Object.fromEntries(normalizedEntries.map((entry) => [entry.id, true])));
    setEditorView("entries");
    setMobileEntryEditorId(null);
    setDesktopEntryEditorId(null);
    setShowTemplatePicker(false);
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
      requiredVariables.length > 0
        ? requiredVariables.filter((variable) => !validationSource.includes(variable))
        : [];

    if (currentMissingVariables.length > 0) {
      alert(
        t("editPrompt.alerts.missingVariables", {
          variables: currentMissingVariables.join(", "),
        }),
      );
      return;
    }

    if (conditionValidationErrors.length > 0) {
      alert(
        t("editPrompt.alerts.contradictoryRules", {
          errors: conditionValidationErrors.join("\n"),
        }),
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
          promptType,
          content: contentToSave,
          entries: usesEntryEditor ? entriesSnapshot : undefined,
          condensePromptEntries,
        });
      } else {
        savedTemplate = await createPromptTemplate(
          nameSnapshot,
          promptType,
          contentToSave,
          usesEntryEditor ? entriesSnapshot : undefined,
          condensePromptEntries,
        );
      }

      const normalizedEntries =
        usesEntryEditor && savedTemplate.entries?.length
          ? ensureSystemEntry(t, savedTemplate.entries)
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
        promptType: savedTemplate.promptType,
        content: savedTemplate.content,
        entries: serializeEntries(normalizedEntries),
        condensePromptEntries: Boolean(savedTemplate.condensePromptEntries),
      };

      if (!isEditing) {
        go(`/settings/prompts/${savedTemplate.id}`, { replace: true });
      }
    } catch (error) {
      console.error("Failed to save template:", error);
      alert(t("editPrompt.alerts.saveFailed", { error: String(error) }));
    } finally {
      setSaving(false);
    }
  }

  async function handleReset() {
    if (!isAppDefault || !id) {
      return;
    }

    const promptTypeName = name.trim() || getPromptTypeName(t, promptType);
    const confirmed = await confirmBottomMenu({
      title: t("editPrompt.reset.title", { name: promptTypeName }),
      message: t("editPrompt.reset.message", { name: promptTypeName }),
      confirmLabel: t("editPrompt.reset.confirmLabel"),
      destructive: true,
    });
    if (!confirmed) return;

    setResetting(true);
    try {
      let updated;
      if (id === APP_DEFAULT_TEMPLATE_ID) {
        updated = await resetAppDefaultTemplate();
      } else if (id === APP_LOCAL_ROLEPLAY_TEMPLATE_ID) {
        updated = await resetLocalRoleplayTemplate();
      } else if (id === APP_COMPANION_TEMPLATE_ID) {
        updated = await resetCompanionTemplate();
      } else if (id === APP_DYNAMIC_SUMMARY_TEMPLATE_ID) {
        updated = await resetDynamicSummaryTemplate();
      } else if (id === APP_DYNAMIC_MEMORY_TEMPLATE_ID) {
        updated = await resetDynamicMemoryTemplate();
      } else if (id === APP_DYNAMIC_MEMORY_LOCAL_TEMPLATE_ID) {
        updated = await resetDynamicMemoryLocalTemplate();
      } else if (id === APP_GROUP_CHAT_TEMPLATE_ID) {
        updated = await resetGroupChatTemplate();
      } else if (id === APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID) {
        updated = await resetGroupChatRoleplayTemplate();
      } else if (id === APP_HELP_ME_REPLY_TEMPLATE_ID) {
        updated = await resetHelpMeReplyTemplate();
      } else if (id === APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID) {
        updated = await resetHelpMeReplyConversationalTemplate();
      } else if (id === APP_LOREBOOK_ENTRY_WRITER_TEMPLATE_ID) {
        updated = await resetLorebookEntryWriterTemplate();
      } else if (id === LEGACY_APP_LOREBOOK_ENTRY_GENERATOR_TEMPLATE_ID) {
        updated = await resetLorebookEntryWriterTemplate();
      } else if (id === APP_LOREBOOK_KEYWORD_GENERATOR_TEMPLATE_ID) {
        updated = await resetLorebookKeywordGeneratorTemplate();
      } else if (id === APP_AVATAR_GENERATION_TEMPLATE_ID) {
        updated = await resetAvatarGenerationTemplate();
      } else if (id === APP_AVATAR_EDIT_TEMPLATE_ID) {
        updated = await resetAvatarEditTemplate();
      } else if (id === APP_SCENE_GENERATION_TEMPLATE_ID) {
        updated = await resetSceneGenerationTemplate();
      } else if (id === APP_SCENE_PROMPT_WRITER_TEMPLATE_ID) {
        updated = await resetScenePromptWriterTemplate();
      } else if (id === APP_DESIGN_REFERENCE_TEMPLATE_ID) {
        updated = await resetDesignReferenceTemplate();
      } else if (id === APP_COMPANION_SOUL_WRITER_TEMPLATE_ID) {
        updated = await resetCompanionSoulWriterTemplate();
      } else {
        return;
      }
      setContent(updated.content);
      setPromptType(updated.promptType);
      setCondensePromptEntries(Boolean(updated.condensePromptEntries));
      if (usesEntryEditor) {
        const nextEntries =
          updated.entries?.length > 0 ? updated.entries : [createDefaultEntry(t, updated.content)];
        const normalizedEntries = ensureSystemEntry(t, nextEntries);
        setEntries(normalizedEntries);
        setCollapsedEntries(Object.fromEntries(normalizedEntries.map((entry) => [entry.id, true])));
      }
    } catch (error) {
      console.error("Failed to reset template:", error);
      alert(t("editPrompt.reset.failed"));
    } finally {
      setResetting(false);
    }
  }

  const resetToInitial = () => {
    if (!initialRef.current) return;
    try {
      const nextEntries = JSON.parse(initialRef.current.entries) as SystemPromptEntry[];
      setName(initialRef.current.name);
      setPromptType(initialRef.current.promptType);
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
      setPreview(t("editPrompt.alerts.previewFailed"));
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
          {t("editPrompt.preview.rendered")}
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
          {t("editPrompt.preview.raw")}
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
              <option value="">{t("editPrompt.preview.selectCharacter")}</option>
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
              <option value="">{t("editPrompt.preview.selectPersona")}</option>
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
            {previewing
              ? t("editPrompt.preview.rendering")
              : t("editPrompt.preview.generatePreview")}
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
                  <p className="text-sm text-fg/50">{t("editPrompt.preview.noPreviewYet")}</p>
                  <p className="text-xs text-fg/30">{t("editPrompt.preview.selectAndGenerate")}</p>
                </div>
              );
            }
            if (entriesToShow.length === 0) {
              return (
                <p className="text-xs text-fg/40">{t("editPrompt.preview.noEntriesToPreview")}</p>
              );
            }
            return (
              <div className="space-y-4">
                {entriesToShow.map((entry) => (
                  <div key={entry.id} className="space-y-1">
                    <div className="text-[11px] uppercase tracking-wide text-fg/40">
                      {getEntryKindSummary(t, entry)} · {entry.name}
                    </div>
                    <pre className="whitespace-pre-wrap text-xs leading-relaxed text-fg/80 font-mono">
                      {entry.content || getEntryPreviewText(t, entry)}
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
              <p className="text-sm text-fg/50">{t("editPrompt.preview.noPreviewYet")}</p>
              <p className="text-xs text-fg/30">{t("editPrompt.preview.selectAndGenerate")}</p>
            </div>
          )
        ) : (
          <pre className="whitespace-pre-wrap text-xs leading-relaxed text-fg/80 font-mono">
            {content || t("editPrompt.preview.noContentToPreview")}
          </pre>
        )}
      </div>
    </div>
  );

  return (
    <div className="flex h-full flex-col">
      <main className="flex-1 overflow-y-auto px-4 pt-4 pb-6">
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
                        <span className="text-sm font-medium text-warning/80">
                          {t("editPrompt.page.protected")}
                        </span>
                        {promptType && (
                          <span className="text-xs text-warning/70 ml-2">
                            {getPromptTypeName(t, promptType)}
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
                        {t("editPrompt.page.reset")}
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
                          {t("editPrompt.page.contradictoryRules")}
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

                {missingVariables.length > 0 && (
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
                          {t("editPrompt.page.missingRequiredVariables")}
                        </p>
                        <p className="text-xs text-danger/70 mt-0.5">
                          {t("editPrompt.page.include")}{" "}
                          <span className="font-mono">{missingVariables.join(", ")}</span>
                        </p>
                      </div>
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>

              {/* Name Input */}
              <div className="space-y-2">
                <label className="text-xs font-medium uppercase tracking-wider text-fg/50">
                  {t("editPrompt.page.templateName")}
                </label>
                <input
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder={t("editPrompt.page.templateNamePlaceholder")}
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

              <div className="space-y-2">
                <label className="text-xs font-medium uppercase tracking-wider text-fg/50">
                  {t("editPrompt.page.promptType")}
                </label>
                <select
                  value={promptType}
                  onChange={(e) => setPromptType(e.target.value as PromptType)}
                  disabled={isAppDefault}
                  className={cn(
                    "w-full px-4 py-3",
                    radius.lg,
                    "border border-fg/10 bg-fg/5",
                    "text-sm text-fg",
                    interactive.transition.fast,
                    "focus:border-fg/20 focus:bg-fg/10 focus:outline-none",
                    "disabled:cursor-not-allowed disabled:opacity-60",
                  )}
                >
                  {(parameterEngine?.promptTypes ?? []).map((definition) => (
                    <option key={definition.promptType} value={definition.promptType}>
                      {definition.label}
                    </option>
                  ))}
                </select>
                <p className="text-xs leading-relaxed text-fg/42">
                  {currentPromptTypeDefinition
                    ? t("editPrompt.page.promptTypeVariablesSummary", {
                        required: currentPromptTypeDefinition.requiredVariables.length,
                        available: currentPromptTypeDefinition.allowedVariables.length,
                      })
                    : t("editPrompt.page.promptTypeFallback")}
                </p>
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
                        {t("editPrompt.page.entries")}
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
                        {t("editPrompt.page.structure")}
                      </button>
                    </div>
                  ) : (
                    <label className="text-xs font-medium uppercase tracking-wider text-fg/50">
                      {t("editPrompt.page.promptContent")}
                    </label>
                  )}
                  {usesEntryEditor && (
                    <div className="flex items-center gap-3 rounded-lg border border-fg/10 bg-surface-el/20 px-2.5 py-1.5">
                      <Switch
                        id="condense-prompt-entries"
                        checked={condensePromptEntries}
                        onChange={(next) => setCondensePromptEntries(next)}
                      />
                      <span className="text-xs text-fg/70">
                        {t("editPrompt.page.condenseEntries")}
                      </span>
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
                        {t("editPrompt.page.addEntry")}
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
                      {t("editPrompt.page.variables")}
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
                      {t("editPrompt.page.preview")}
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
                        {entries.length === 0 ? (
                          <div
                            className={cn(
                              radius.lg,
                              "border border-dashed border-fg/10 bg-fg/2",
                              "px-4 py-8 text-center",
                            )}
                          >
                            <div className="mx-auto max-w-md">
                              <div className="mb-3 flex justify-center text-fg/35">
                                <Layers className="h-5 w-5" />
                              </div>
                              <p className="text-sm font-medium text-fg/75">
                                {t("editPrompt.page.noEntriesYet")}
                              </p>
                              <p className="mt-1 text-sm text-fg/45">
                                {t("editPrompt.page.noEntriesHint")}
                              </p>
                              <div className="mt-4 flex flex-wrap items-center justify-center gap-2">
                                <button
                                  onClick={handleAddEntry}
                                  className={cn(
                                    "inline-flex items-center gap-2 px-3 py-2",
                                    radius.lg,
                                    "border border-fg/10 bg-fg/5 text-sm text-fg/80",
                                    interactive.transition.fast,
                                    "hover:bg-fg/10",
                                  )}
                                >
                                  <Plus className="h-4 w-4" />
                                  {t("editPrompt.page.addEntryLower")}
                                </button>
                                {showTemplateEmptyState && protectedTemplates.length > 0 && (
                                  <button
                                    onClick={() => setShowTemplatePicker(true)}
                                    className={cn(
                                      "inline-flex items-center gap-2 px-3 py-2",
                                      radius.lg,
                                      "border border-fg/10 bg-fg/5 text-sm text-fg/80",
                                      interactive.transition.fast,
                                      "hover:bg-fg/10",
                                    )}
                                  >
                                    <Wand2 className="h-4 w-4" />
                                    {t("editPrompt.page.useATemplate")}
                                  </button>
                                )}
                              </div>
                            </div>
                          </div>
                        ) : (
                          <>
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
                          </>
                        )}

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
                      placeholder={t("editPrompt.page.contentPlaceholder")}
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
                    <span className="text-sm font-medium text-fg">
                      {t("editPrompt.page.preview")}
                    </span>
                    {!previewExpanded && preview && (
                      <span className="text-xs text-fg/40 ml-2">
                        {t("editPrompt.page.hasGeneratedPreview")}
                      </span>
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

          </div>
        </div>
      </main>

      {/* Variables Bottom Sheet (Mobile) */}
      <BottomMenu
        isOpen={showVariables}
        onClose={() => setShowVariables(false)}
        title={t("editPrompt.variablesSheet.title")}
      >
        <div className="space-y-4">
          <p className="text-xs text-fg/50">{t("editPrompt.variablesSheet.tapToInsert")}</p>

          {requiredVariables.length > 0 && (
            <div className={cn(radius.lg, "border border-warning/30 bg-warning/10 p-3")}>
              <p className="text-xs text-warning/80">
                <span className="font-semibold">{t("editPrompt.variablesSheet.requiredNote")}</span>{" "}
                {t("editPrompt.variablesSheet.requiredNoteRest")}
              </p>
            </div>
          )}

          <div className="max-h-[50vh] overflow-y-auto space-y-2">
            {variables.map((item) => {
              const isRequired = requiredVariables.includes(item.variable);
              const isMissing = missingVariables.includes(item.variable);
              return (
                <div
                  key={item.variable}
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
                          {item.variable}
                        </code>
                        {copiedVar === item.variable && (
                          <span className="flex items-center gap-1 text-xs text-accent/80">
                            <Check className="h-3 w-3" />
                            {t("editPrompt.variablesSheet.copied")}
                          </span>
                        )}
                      </div>
                      <p className="text-sm text-fg/80">{item.label}</p>
                      <p className="text-xs text-fg/50">{item.description}</p>
                    </div>

                    <div className="flex items-center gap-2 shrink-0">
                      <button
                        onClick={() => copyVariable(item.variable)}
                        className={cn(
                          "flex items-center justify-center h-8 w-8",
                          radius.md,
                          "border border-fg/10 bg-fg/5",
                          "text-fg/50",
                          interactive.transition.fast,
                          "hover:bg-fg/10 hover:text-fg",
                        )}
                        title={t("editPrompt.variablesSheet.copy")}
                      >
                        <Copy className="h-4 w-4" />
                      </button>
                      <button
                        onClick={() => {
                          insertVariable(item.variable);
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
                        {t("editPrompt.variablesSheet.insert")}
                      </button>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </BottomMenu>

      <BottomMenu
        isOpen={showTemplatePicker}
        onClose={() => setShowTemplatePicker(false)}
        title={t("editPrompt.templatePicker.title")}
      >
        <div className="space-y-4">
          <p className="text-sm text-fg/50">{t("editPrompt.templatePicker.description")}</p>

          <div className="space-y-2">
            {protectedTemplates.map((template) => {
              return (
                <button
                  key={template.id}
                  onClick={() => applyProtectedTemplate(template)}
                  className={cn(
                    "flex w-full items-center justify-between gap-3 px-4 py-3 text-left",
                    radius.lg,
                    "border border-fg/10 bg-fg/5",
                    interactive.transition.fast,
                    "hover:bg-fg/10",
                  )}
                >
                  <div className="min-w-0 flex-1">
                    <p className="text-sm font-medium text-fg">{template.name}</p>
                    <p className="mt-1 text-xs text-fg/45">
                      {getPromptTypeName(t, template.promptType)}
                    </p>
                  </div>
                  <ChevronRight className="h-4 w-4 shrink-0 text-fg/35" />
                </button>
              );
            })}
          </div>
        </div>
      </BottomMenu>

      {/* Preview Bottom Sheet (Mobile only) */}
      <BottomMenu
        isOpen={showMobilePreview}
        onClose={() => setShowMobilePreview(false)}
        title={t("editPrompt.page.preview")}
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
              {t("editPrompt.preview.content")}
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
              {t("editPrompt.preview.structure")}
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
