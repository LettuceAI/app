import type { PromptCondition, PromptEntryInput, PromptInput, PromptKind, PromptOrigin, PromptView } from "@/api/generated/bindings";

export type Kind = Exclude<PromptKind, "runtime_text">;
export type Role = PromptEntryInput["role"];
export type Position = PromptEntryInput["position"];
export type ImageSlot = NonNullable<PromptEntryInput["image_slot"]>;
export type RuleType = Exclude<PromptCondition["type"], "all" | "any" | "not">;
export type RuleValue = boolean | number | string | string[];
export type Rule = { id: string; type: RuleType | "group"; value: RuleValue; join: "and" | "or"; condition?: PromptCondition };
export type Rules = { include: Rule[]; exclude: Rule[] };
export type Entry = {
  id: string; name: string; role: Role; content: string; enabled: boolean; position: Position; depth: number;
  minMessages: number | null; everyTurns: number | null; required: boolean; rules: Rules; imageSlot: ImageSlot | null;
  persisted: boolean; systemPrompt: boolean; originalCondition: PromptCondition | null; originalRules: string;
};
export type Prompt = {
  id: string; name: string; kind: Kind; origin: PromptOrigin; condense: boolean; behavior: PromptInput["behavior"];
  entries: Entry[]; revision: number | null; sourceId: string | null;
};
export type PreviewSelection = { id: string; name: string; description: string; image: string | null };
export type StoredPrompt = PromptView;
