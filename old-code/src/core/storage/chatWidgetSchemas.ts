import { z } from "zod";

export type BoxVariant =
  | "default"
  | "subtle"
  | "info"
  | "warning"
  | "success"
  | "danger";

export type SelectorKind = "persona" | "model" | "author_note";

export type ButtonAction =
  | "regenerate"
  | "swap_places"
  | "new_session"
  | "continue"
  | "abort"
  | "view_history"
  | "open_memories"
  | "open_search"
  | "toggle_voice_autoplay";

export type ImageSource =
  | { kind: "character_avatar" }
  | { kind: "persona_avatar" }
  | { kind: "library"; path: string }
  | { kind: "upload"; path: string };

export type ImageShape = "auto" | "square" | "wide" | "circle";

export type WidgetDesign = "default" | "minimal" | "solid" | "outline";

interface NodeBase {
  id: string;
  design?: WidgetDesign;
}

export interface DividerNode extends NodeBase {
  type: "divider";
  style?: "line" | "space";
}

export interface BoxNode extends NodeBase {
  type: "box";
  variant?: BoxVariant;
  title?: string;
  description?: string;
  children: WidgetNode[];
}

export interface CharacterInfoNode extends NodeBase {
  type: "character_info";
  characterId?: string;
}

export interface PersonaInfoNode extends NodeBase {
  type: "persona_info";
}

export interface ScratchPadNode extends NodeBase {
  type: "scratch_pad";
  title?: string;
  description?: string;
  content?: string;
}

export interface ImageNode extends NodeBase {
  type: "image";
  title?: string;
  description?: string;
  source: ImageSource;
  shape?: ImageShape;
}

export interface SelectorNode extends NodeBase {
  type: "selector";
  kind: SelectorKind;
  title?: string;
  description?: string;
}

export interface ButtonNode extends NodeBase {
  type: "button";
  action: ButtonAction;
  title?: string;
  description?: string;
}

export interface StatItem {
  id: string;
  label: string;
  value: number;
  min?: number;
  max?: number;
}

export interface StatTrackerNode extends NodeBase {
  type: "stat_tracker";
  title?: string;
  description?: string;
  stats: StatItem[];
}

export interface SnippetItem {
  id: string;
  label: string;
  text: string;
}

export interface QuickSnippetsNode extends NodeBase {
  type: "quick_snippets";
  title?: string;
  description?: string;
  snippets: SnippetItem[];
}

export interface DiceNode extends NodeBase {
  type: "dice";
  title?: string;
  description?: string;
  notation?: string;
}

export interface MemoryNode extends NodeBase {
  type: "memory";
  title?: string;
  limit?: number;
}

export interface CompanionStateNode extends NodeBase {
  type: "companion_state";
  title?: string;
}

export interface SessionInfoNode extends NodeBase {
  type: "session_info";
  title?: string;
}

export interface AuthorNoteNode extends NodeBase {
  type: "author_note";
  title?: string;
  description?: string;
}

export interface TimeNode extends NodeBase {
  type: "time";
  title?: string;
  hourFormat?: "12h" | "24h";
  showSeconds?: boolean;
  showDate?: boolean;
}

export type WidgetNode =
  | DividerNode
  | BoxNode
  | CharacterInfoNode
  | PersonaInfoNode
  | ScratchPadNode
  | ImageNode
  | SelectorNode
  | ButtonNode
  | StatTrackerNode
  | QuickSnippetsNode
  | DiceNode
  | MemoryNode
  | CompanionStateNode
  | SessionInfoNode
  | AuthorNoteNode
  | TimeNode;

const imageSourceSchema: z.ZodType<ImageSource> = z.union([
  z.object({ kind: z.literal("character_avatar") }),
  z.object({ kind: z.literal("persona_avatar") }),
  z.object({ kind: z.literal("library"), path: z.string() }),
  z.object({ kind: z.literal("upload"), path: z.string() }),
]);

export const widgetNodeSchema: z.ZodType<WidgetNode> = z.lazy(() =>
  z.discriminatedUnion("type", [
    z.object({
      id: z.string(),
      design: z.enum(["default", "minimal", "solid", "outline"]).optional(),
      type: z.literal("divider"),
      style: z.enum(["line", "space"]).optional(),
    }),
    z.object({
      id: z.string(),
      design: z.enum(["default", "minimal", "solid", "outline"]).optional(),
      type: z.literal("box"),
      variant: z
        .enum(["default", "subtle", "info", "warning", "success", "danger"])
        .optional(),
      title: z.string().optional(),
      description: z.string().optional(),
      children: z.array(widgetNodeSchema),
    }),
    z.object({
      id: z.string(),
      design: z.enum(["default", "minimal", "solid", "outline"]).optional(),
      type: z.literal("character_info"),
      characterId: z.string().optional(),
    }),
    z.object({
      id: z.string(),
      design: z.enum(["default", "minimal", "solid", "outline"]).optional(),
      type: z.literal("persona_info"),
    }),
    z.object({
      id: z.string(),
      design: z.enum(["default", "minimal", "solid", "outline"]).optional(),
      type: z.literal("scratch_pad"),
      title: z.string().optional(),
      description: z.string().optional(),
      content: z.string().optional(),
    }),
    z.object({
      id: z.string(),
      design: z.enum(["default", "minimal", "solid", "outline"]).optional(),
      type: z.literal("image"),
      title: z.string().optional(),
      description: z.string().optional(),
      source: imageSourceSchema,
      shape: z.enum(["auto", "square", "wide", "circle"]).optional(),
    }),
    z.object({
      id: z.string(),
      design: z.enum(["default", "minimal", "solid", "outline"]).optional(),
      type: z.literal("selector"),
      kind: z.enum(["persona", "model", "author_note"]),
      title: z.string().optional(),
      description: z.string().optional(),
    }),
    z.object({
      id: z.string(),
      design: z.enum(["default", "minimal", "solid", "outline"]).optional(),
      type: z.literal("button"),
      action: z.enum([
        "regenerate",
        "swap_places",
        "new_session",
        "continue",
        "abort",
        "view_history",
        "open_memories",
        "open_search",
        "toggle_voice_autoplay",
      ]),
      title: z.string().optional(),
      description: z.string().optional(),
    }),
    z.object({
      id: z.string(),
      design: z.enum(["default", "minimal", "solid", "outline"]).optional(),
      type: z.literal("stat_tracker"),
      title: z.string().optional(),
      description: z.string().optional(),
      stats: z
        .array(
          z.object({
            id: z.string(),
            label: z.string(),
            value: z.number(),
            min: z.number().optional(),
            max: z.number().optional(),
          }),
        )
        .default([]),
    }),
    z.object({
      id: z.string(),
      design: z.enum(["default", "minimal", "solid", "outline"]).optional(),
      type: z.literal("quick_snippets"),
      title: z.string().optional(),
      description: z.string().optional(),
      snippets: z
        .array(
          z.object({
            id: z.string(),
            label: z.string(),
            text: z.string(),
          }),
        )
        .default([]),
    }),
    z.object({
      id: z.string(),
      design: z.enum(["default", "minimal", "solid", "outline"]).optional(),
      type: z.literal("dice"),
      title: z.string().optional(),
      description: z.string().optional(),
      notation: z.string().optional(),
    }),
    z.object({
      id: z.string(),
      design: z.enum(["default", "minimal", "solid", "outline"]).optional(),
      type: z.literal("memory"),
      title: z.string().optional(),
      limit: z.number().int().min(1).max(100).optional(),
    }),
    z.object({
      id: z.string(),
      design: z.enum(["default", "minimal", "solid", "outline"]).optional(),
      type: z.literal("companion_state"),
      title: z.string().optional(),
    }),
    z.object({
      id: z.string(),
      design: z.enum(["default", "minimal", "solid", "outline"]).optional(),
      type: z.literal("session_info"),
      title: z.string().optional(),
    }),
    z.object({
      id: z.string(),
      design: z.enum(["default", "minimal", "solid", "outline"]).optional(),
      type: z.literal("author_note"),
      title: z.string().optional(),
      description: z.string().optional(),
    }),
    z.object({
      id: z.string(),
      design: z.enum(["default", "minimal", "solid", "outline"]).optional(),
      type: z.literal("time"),
      title: z.string().optional(),
      hourFormat: z.enum(["12h", "24h"]).optional(),
      showSeconds: z.boolean().optional(),
      showDate: z.boolean().optional(),
    }),
  ]),
);
