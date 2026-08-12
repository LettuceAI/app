import { uuidv4 } from "../../../../../../core/storage/repo";
import type { WidgetNode } from "../../../../../../core/storage/chatWidgetSchemas";
import type { TranslationKey } from "../../../../../../core/i18n/context";

export type WidgetType = WidgetNode["type"];

export const WIDGET_TYPE_LABEL = {
  divider: "chats.widgets.types.divider.label",
  box: "chats.widgets.types.box.label",
  character_info: "chats.widgets.types.character_info.label",
  persona_info: "chats.widgets.types.persona_info.label",
  scratch_pad: "chats.widgets.types.scratch_pad.label",
  image: "chats.widgets.types.image.label",
  selector: "chats.widgets.types.selector.label",
  button: "chats.widgets.types.button.label",
  stat_tracker: "chats.widgets.types.stat_tracker.label",
  quick_snippets: "chats.widgets.types.quick_snippets.label",
  dice: "chats.widgets.types.dice.label",
  memory: "chats.widgets.types.memory.label",
  companion_state: "chats.widgets.types.companion_state.label",
  session_info: "chats.widgets.types.session_info.label",
  author_note: "chats.widgets.types.author_note.label",
  time: "chats.widgets.types.time.label",
} satisfies Record<WidgetType, TranslationKey>;

export const WIDGET_TYPE_DESC = {
  divider: "chats.widgets.types.divider.desc",
  box: "chats.widgets.types.box.desc",
  character_info: "chats.widgets.types.character_info.desc",
  persona_info: "chats.widgets.types.persona_info.desc",
  scratch_pad: "chats.widgets.types.scratch_pad.desc",
  image: "chats.widgets.types.image.desc",
  selector: "chats.widgets.types.selector.desc",
  button: "chats.widgets.types.button.desc",
  stat_tracker: "chats.widgets.types.stat_tracker.desc",
  quick_snippets: "chats.widgets.types.quick_snippets.desc",
  dice: "chats.widgets.types.dice.desc",
  memory: "chats.widgets.types.memory.desc",
  companion_state: "chats.widgets.types.companion_state.desc",
  session_info: "chats.widgets.types.session_info.desc",
  author_note: "chats.widgets.types.author_note.desc",
  time: "chats.widgets.types.time.desc",
} satisfies Record<WidgetType, TranslationKey>;

const COMPANION_ONLY_TYPES: readonly WidgetType[] = ["companion_state"];

export interface WidgetAvailability {
  isCompanion: boolean;
  isGroup: boolean;
}

export function isWidgetTypeAvailable(
  type: WidgetType,
  availability: WidgetAvailability,
): boolean {
  if (COMPANION_ONLY_TYPES.includes(type)) {
    return availability.isCompanion && !availability.isGroup;
  }
  return true;
}

export function filterAvailableWidgets(
  nodes: WidgetNode[],
  availability: WidgetAvailability,
): WidgetNode[] {
  const kept = nodes.filter((node) => isWidgetTypeAvailable(node.type, availability));
  return kept.some((node) => node.type === "box")
    ? kept.map((node) =>
        node.type === "box"
          ? { ...node, children: filterAvailableWidgets(node.children, availability) }
          : node,
      )
    : kept;
}

export function createWidgetNode(type: WidgetType): WidgetNode {
  const id = uuidv4();
  switch (type) {
    case "divider":
      return { id, type: "divider", style: "line" };
    case "box":
      return {
        id,
        type: "box",
        variant: "default",
        title: "Untitled",
        children: [],
      };
    case "character_info":
      return { id, type: "character_info" };
    case "persona_info":
      return { id, type: "persona_info" };
    case "scratch_pad":
      return { id, type: "scratch_pad", title: "Notes", content: "" };
    case "image":
      return { id, type: "image", source: { kind: "character_avatar" } };
    case "selector":
      return { id, type: "selector", kind: "persona", title: "Persona" };
    case "button":
      return {
        id,
        type: "button",
        action: "regenerate",
        title: "Regenerate last reply",
      };
    case "stat_tracker":
      return {
        id,
        type: "stat_tracker",
        title: "Stats",
        stats: [{ id: uuidv4(), label: "Affection", value: 0 }],
      };
    case "quick_snippets":
      return {
        id,
        type: "quick_snippets",
        title: "Quick snippets",
        snippets: [{ id: uuidv4(), label: "Continue", text: "Continue the scene." }],
      };
    case "dice":
      return { id, type: "dice", title: "Dice", notation: "1d20" };
    case "memory":
      return { id, type: "memory", title: "Memories", limit: 10 };
    case "companion_state":
      return { id, type: "companion_state", title: "Companion" };
    case "session_info":
      return { id, type: "session_info", title: "Session" };
    case "author_note":
      return { id, type: "author_note", title: "Author note" };
    case "time":
      return {
        id,
        type: "time",
        title: "Time",
        hourFormat: "24h",
        showSeconds: false,
        showDate: true,
      };
  }
}

export function setLibraryImageOnNode(
  nodes: WidgetNode[],
  id: string,
  imageId: string,
): WidgetNode[] {
  return nodes.map((n) => {
    if (n.id === id && n.type === "image") {
      return { ...n, source: { kind: "library", path: imageId } };
    }
    if (n.type === "box") {
      return { ...n, children: setLibraryImageOnNode(n.children, id, imageId) };
    }
    return n;
  });
}

export function setScratchPadContentOnNode(
  nodes: WidgetNode[],
  id: string,
  content: string,
): WidgetNode[] {
  return nodes.map((n) => {
    if (n.id === id && n.type === "scratch_pad") {
      return { ...n, content };
    }
    if (n.type === "box") {
      return { ...n, children: setScratchPadContentOnNode(n.children, id, content) };
    }
    return n;
  });
}

export function patchWidgetNode(
  nodes: WidgetNode[],
  id: string,
  patch: Partial<WidgetNode>,
): WidgetNode[] {
  return nodes.map((n) => {
    if (n.id === id) {
      return { ...n, ...patch } as WidgetNode;
    }
    if (n.type === "box") {
      return { ...n, children: patchWidgetNode(n.children, id, patch) };
    }
    return n;
  });
}

export function widgetSummary(node: WidgetNode): string {
  switch (node.type) {
    case "divider":
      return node.style === "space" ? "Space" : "Line";
    case "box":
      return node.title || "Untitled box";
    case "character_info":
    case "persona_info":
      return WIDGET_TYPE_LABEL[node.type];
    case "scratch_pad":
      return node.title || "Scratch pad";
    case "image":
      return node.title || `Image: ${node.source.kind.replace("_", " ")}`;
    case "selector":
      return node.title || `Selector: ${node.kind.replace("_", " ")}`;
    case "button":
      return node.title || `Button: ${node.action.replace("_", " ")}`;
    case "stat_tracker":
      return node.title || `Stats (${node.stats.length})`;
    case "quick_snippets":
      return node.title || `Snippets (${node.snippets.length})`;
    case "dice":
      return node.title || `Dice ${node.notation ?? "1d20"}`;
    case "memory":
      return node.title || "Memories";
    case "companion_state":
      return node.title || "Companion state";
    case "session_info":
      return node.title || "Session info";
    case "author_note":
      return node.title || "Author note";
    case "time":
      return node.title || "Time";
  }
}
