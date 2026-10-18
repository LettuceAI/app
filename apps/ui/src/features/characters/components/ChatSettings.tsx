import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { BookOpen, Check } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge, icon, ListGroup, ListRowButton } from "@lettuceai/crisp";
import { dynamicMemoryEnabledQuery } from "../data/queries";
import { draftSetters } from "./draft-setters";
import { LorebookChoiceSheet } from "./Pickers";
import { PromptSetting } from "./PromptSetting";
import type { StepProps } from "./step-props";

export function PromptsGroup({ api, form, update }: StepProps) {
  const { t } = useTranslation("characters");
  const { setDefaults } = draftSetters(update);
  const { defaults } = form.draft;
  return <ListGroup label={t("details.prompts")}>
    <PromptSetting api={api} promptKind="direct_chat" label={t("details.directPrompt")} value={defaults.direct_prompt_id} onChange={(direct_prompt_id) => setDefaults({ direct_prompt_id })} />
    <PromptSetting api={api} promptKind="group_chat_roleplay" label={t("details.groupRoleplayPrompt")} value={defaults.group_roleplay_prompt_id} onChange={(group_roleplay_prompt_id) => setDefaults({ group_roleplay_prompt_id })} />
    <PromptSetting api={api} promptKind="group_chat_conversational" label={t("details.groupConversationPrompt")} value={defaults.group_conversation_prompt_id} onChange={(group_conversation_prompt_id) => setDefaults({ group_conversation_prompt_id })} />
    {defaults.interaction_mode === "companion" && <PromptSetting api={api} promptKind="companion_chat" label={t("details.companionPrompt")} value={defaults.companion_soul?.prompting.prompt_template_id ?? null} onChange={(prompt_template_id) => update((current) => {
      const config = current.draft.defaults.companion_soul;
      if (!config) return current;
      return { ...current, draft: { ...current.draft, defaults: { ...current.draft.defaults, companion_soul: { ...config, prompting: { ...config.prompting, prompt_template_id } } } } };
    })} />}
  </ListGroup>;
}

export function MemoryGroup({ api, form, update }: StepProps) {
  const { t } = useTranslation("characters");
  const { data: dynamicMemory = false } = useQuery(dynamicMemoryEnabledQuery(api));
  const { setDefaults } = draftSetters(update);
  const { defaults } = form.draft;
  return <ListGroup label={t("details.memory")}>
    {(["dynamic", "manual"] as const).map((policy) => {
      const selected = defaults.memory_policy === policy;
      const key = policy === "dynamic" ? "memoryDynamic" : "memoryManual";
      const unavailable = policy === "dynamic" && !dynamicMemory;
      return <ListRowButton key={policy} role="radio" aria-checked={selected} disabled={unavailable} chevron={false} title={policy === "dynamic" && dynamicMemory ? <span className="flex items-center gap-2">{t(`details.${key}`)}<Badge tone="accent">{t("details.recommended")}</Badge></span> : t(`details.${key}`)} description={t(unavailable ? "details.memoryDynamicOff" : `details.${key}Body`)} trailing={selected ? <Check size={icon.md} className="text-accent" /> : undefined} onClick={() => setDefaults({ memory_policy: policy })} />;
    })}
  </ListGroup>;
}

export function LorebooksRow({ api, form, update }: StepProps) {
  const { t, i18n } = useTranslation("characters");
  const [picking, setPicking] = useState(false);
  const count = form.lorebookIds.length;
  return <>
    <ListGroup>
      <ListRowButton
        icon={<BookOpen size={icon.lg} />}
        title={t("extras.lorebooks")}
        description={count === 0 ? t("extras.lorebooksNone") : t("extras.lorebooksCount", { count, formattedCount: new Intl.NumberFormat(i18n.language).format(count) })}
        onClick={() => setPicking(true)}
      />
    </ListGroup>
    <LorebookChoiceSheet api={api} open={picking} value={form.lorebookIds} onChange={(lorebookIds) => update((current) => ({ ...current, lorebookIds }))} onClose={() => setPicking(false)} />
  </>;
}
