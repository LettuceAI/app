import type { LibraryChatWidgetSlots_Serialize, LibraryWidgetNode_Serialize, SettingsWidgetNode } from "@/api/generated/bindings";

export type WidgetNode = LibraryWidgetNode_Serialize;
export type WidgetType = WidgetNode["type"];
export type WidgetSlots = LibraryChatWidgetSlots_Serialize;

/** The names a new widget starts with: its type's label, or the name of its first stat or snippet. */
export type DefaultLabel = WidgetType | "stat" | "snippet";

/** Maps the global (camelCase, library or upload image) widget tree onto the per-chat shape. */
export function fromSettingsNode(node: SettingsWidgetNode): WidgetNode {
  switch (node.type) {
    case "box":
      return { ...node, children: node.children.map(fromSettingsNode) };
    case "character_info": {
      const { characterId, ...rest } = node;
      return { ...rest, character_id: characterId };
    }
    case "image":
      return {
        ...node,
        source:
          node.source.kind === "library" || node.source.kind === "upload"
            ? { kind: "unresolved_legacy", token: node.source.path }
            : node.source,
      };
    case "time": {
      const { hourFormat, showSeconds, showDate, ...rest } = node;
      return {
        ...rest,
        hour_format: hourFormat === "12h" ? "h12" : hourFormat === "24h" ? "h24" : null,
        show_seconds: showSeconds,
        show_date: showDate,
      };
    }
    default:
      return node;
  }
}

export function createWidgetNode(type: WidgetType, label: (key: DefaultLabel) => string): WidgetNode {
  const id = crypto.randomUUID();
  switch (type) {
    case "divider":
      return { type, id, design: null, style: "line" };
    case "box":
      return { type, id, design: null, variant: "default", title: label("box"), description: null, children: [] };
    case "character_info":
      return { type, id, design: null, character_id: null };
    case "persona_info":
      return { type, id, design: null };
    case "scratch_pad":
      return { type, id, design: null, title: label("scratch_pad"), description: null, content: "" };
    case "image":
      return { type, id, design: null, title: null, description: null, source: { kind: "character_avatar" }, shape: null };
    case "selector":
      return { type, id, design: null, kind: "persona", title: null, description: null };
    case "button":
      return { type, id, design: null, action: "regenerate", title: null, description: null };
    case "stat_tracker":
      return {
        type,
        id,
        design: null,
        title: label("stat_tracker"),
        description: null,
        stats: [{ id: crypto.randomUUID(), label: label("stat"), value: 0, min: null, max: null }],
      };
    case "quick_snippets":
      return {
        type,
        id,
        design: null,
        title: label("quick_snippets"),
        description: null,
        snippets: [{ id: crypto.randomUUID(), label: label("snippet"), text: "" }],
      };
    case "dice":
      return { type, id, design: null, title: label("dice"), description: null, notation: "1d20" };
    case "memory":
      return { type, id, design: null, title: label("memory"), limit: 10 };
    case "companion_state":
      return { type, id, design: null, title: label("companion_state") };
    case "session_info":
      return { type, id, design: null, title: label("session_info") };
    case "author_note":
      return { type, id, design: null, title: label("author_note"), description: null };
    case "time":
      return { type, id, design: null, title: label("time"), hour_format: "h24", show_seconds: false, show_date: true };
  }
}

/** Replaces the node with this id wherever it sits, including inside boxes. */
export function updateNode(nodes: WidgetNode[], id: string, fn: (node: WidgetNode) => WidgetNode): WidgetNode[] {
  return nodes.map((node) =>
    node.id === id ? fn(node) : node.type === "box" ? { ...node, children: updateNode(node.children, id, fn) } : node,
  );
}

export interface WidgetPlace {
  companion: boolean;
  group: boolean;
}

/** Inverse of `fold`: splits a folded panel's list back by position so a drop always sticks. */
export function splitFolded(current: WidgetSlots, shown: WidgetNode[], visibleSide: "left" | "right"): WidgetSlots {
  const hidden = new Set(current[visibleSide === "left" ? "right" : "left"].map((node) => node.id));
  let cut = 0;
  shown.forEach((node, index) => {
    if (!hidden.has(node.id)) cut = index + 1;
  });
  const front = shown.slice(0, cut);
  const back = shown.slice(cut);
  return visibleSide === "left" ? { left: front, right: back } : { left: back, right: front };
}

/** The "Actions" preset of the type picker: a box of the three buttons people reach for most. */
export function createActionsPreset(label: string): WidgetNode {
  const button = (action: "regenerate" | "continue" | "new_session"): WidgetNode => ({ type: "button", id: crypto.randomUUID(), design: null, action, title: null, description: null });
  return { type: "box", id: crypto.randomUUID(), design: null, variant: "default", title: label, description: null, children: [button("regenerate"), button("continue"), button("new_session")] };
}

/** Companion state only exists in a direct companion chat. */
export function typeAvailable(type: WidgetType, where: WidgetPlace): boolean {
  return type !== "companion_state" || (where.companion && !where.group);
}

/** What a chat can show: only the types it supports, and not the voice autoplay button, which has no per-chat setting in this app. */
export function availableNodes(nodes: WidgetNode[], where: WidgetPlace): WidgetNode[] {
  const shown = nodes.filter((node) => typeAvailable(node.type, where) && !(node.type === "button" && node.action === "toggle_voice_autoplay"));
  return shown.map((node) => (node.type === "box" ? { ...node, children: availableNodes(node.children, where) } : node));
}

/** A single visible panel carries every widget, so the other side's widgets are not lost from view. */
export function fold(slots: WidgetSlots, layout: { showLeft: boolean; showRight: boolean }): WidgetSlots {
  if (layout.showLeft && layout.showRight) return slots;
  return layout.showLeft ? { left: [...slots.left, ...slots.right], right: [] } : { left: [], right: [...slots.right, ...slots.left] };
}
