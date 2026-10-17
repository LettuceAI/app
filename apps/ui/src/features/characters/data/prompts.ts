import type { PromptSummary } from "@/api/generated/bindings";

export type CharacterPromptKind = "direct_chat" | "group_chat_roleplay" | "group_chat_conversational" | "companion_chat";

const builtInKeys: Record<CharacterPromptKind, string> = {
  direct_chat: "prompt_app_default",
  group_chat_roleplay: "prompt_app_group_chat_roleplay",
  group_chat_conversational: "prompt_app_group_chat",
  companion_chat: "prompt_app_companion",
};

export function inheritedPromptName(prompts: PromptSummary[], kind: CharacterPromptKind): string | null {
  const appDefault = kind === "direct_chat" || kind === "companion_chat" ? prompts.find((prompt) => prompt.app_default) : undefined;
  return (appDefault ?? prompts.find((prompt) => prompt.origin.type === "built_in" && prompt.origin.key === builtInKeys[kind]))?.name ?? null;
}
