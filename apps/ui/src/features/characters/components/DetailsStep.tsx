import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Check, ChevronDown } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge, Button, Chip, Field, icon, ListGroup, ListRowButton, Textarea } from "@lettuceai/crisp";
import { useChatModels } from "@/features/conversations";
import { wordCount } from "../data/form";
import { dynamicMemoryEnabledQuery } from "../data/queries";
import { PromptSetting } from "./PromptSetting";
import { ModelChoiceSheet } from "./Pickers";
import type { StepProps } from "./step-props";

export function DetailsStep({ api, form, update }: StepProps) {
  const { t, i18n } = useTranslation("characters");
  const [sheet, setSheet] = useState<"model" | null>(null);
  const chat = useChatModels(api);
  const { data: dynamicMemory = false } = useQuery(dynamicMemoryEnabledQuery(api));
  const { profile, defaults } = form.draft;
  const words = wordCount(profile.definition ?? "");
  const setProfile = (patch: Partial<typeof profile>) => update((current) => ({ ...current, draft: { ...current.draft, profile: { ...current.draft.profile, ...patch } } }));
  const setDefaults = (patch: Partial<typeof defaults>) => update((current) => ({ ...current, draft: { ...current.draft, defaults: { ...current.draft.defaults, ...patch } } }));
  const modelName = defaults.model_profile_id === null ? t("details.modelDefault") : (chat.find(defaults.model_profile_id)?.name ?? (chat.models.length > 0 ? t("details.modelMissing") : ""));

  return <>
    <div className="grid gap-6 @4xl:grid-cols-[3fr_2fr] @4xl:gap-x-10">
      <div className="min-w-0 space-y-6">
        <div className="space-y-3">
          <Field label={t("details.definition")} required hint={
            <span className="flex flex-wrap justify-between gap-x-4">
              <span>{t("details.definitionHint", { char: "{{char}}", user: "{{user}}" })}</span>
              <span className="tabular-nums">{t("details.words", { count: words, formattedCount: new Intl.NumberFormat(i18n.language).format(words) })}</span>
            </span>
          }>
            <Textarea value={profile.definition ?? ""} rows={8} className="@4xl:min-h-96" autoGrow maxRows={24} placeholder={t("details.definitionPlaceholder")} onChange={(event) => setProfile({ definition: event.target.value })} data-autofocus />
          </Field>
          <div className="flex flex-wrap gap-2">
            {["{{char}}", "{{user}}"].map((placeholder) => <Chip key={placeholder} onClick={() => setProfile({ definition: `${profile.definition ?? ""}${profile.definition && !profile.definition.endsWith(" ") ? " " : ""}${placeholder}` })}><span className="font-mono text-xs">{placeholder}</span></Chip>)}
          </div>
        </div>
        <Field label={t("details.description")} hint={t("details.descriptionHint")}>
          <Textarea value={profile.description ?? ""} rows={3} autoGrow maxRows={12} placeholder={t("details.descriptionPlaceholder")} onChange={(event) => setProfile({ description: event.target.value })} />
        </Field>
      </div>
      <div className="min-w-0 space-y-5">
        <Field label={t("details.model")} hint={t("details.modelHint")}>
          <Button block variant="secondary" aria-label={t("details.model")} className="grid grid-cols-1 text-left" trailing={<ChevronDown size={icon.sm} className="shrink-0" />} onClick={() => setSheet("model")}><span className="min-w-0 flex-1 truncate">{modelName}</span></Button>
        </Field>
        <ListGroup label={t("details.prompts")}>
          <PromptSetting api={api} promptKind="direct_chat" label={t("details.directPrompt")} value={defaults.direct_prompt_id} onChange={(direct_prompt_id) => setDefaults({ direct_prompt_id })} />
          <PromptSetting api={api} promptKind="group_chat_roleplay" label={t("details.groupRoleplayPrompt")} value={defaults.group_roleplay_prompt_id} onChange={(group_roleplay_prompt_id) => setDefaults({ group_roleplay_prompt_id })} />
          <PromptSetting api={api} promptKind="group_chat_conversational" label={t("details.groupConversationPrompt")} value={defaults.group_conversation_prompt_id} onChange={(group_conversation_prompt_id) => setDefaults({ group_conversation_prompt_id })} />
          {defaults.interaction_mode === "companion" && <PromptSetting api={api} promptKind="companion_chat" label={t("details.companionPrompt")} value={defaults.companion_soul?.prompting.prompt_template_id ?? null} onChange={(prompt_template_id) => update((current) => {
            const config = current.draft.defaults.companion_soul;
            if (!config) return current;
            return { ...current, draft: { ...current.draft, defaults: { ...current.draft.defaults, companion_soul: { ...config, prompting: { ...config.prompting, prompt_template_id } } } } };
          })} />}
        </ListGroup>
        <ListGroup label={t("details.memory")}>
          {(["dynamic", "manual"] as const).map((policy) => {
            const selected = defaults.memory_policy === policy;
            const key = policy === "dynamic" ? "memoryDynamic" : "memoryManual";
            const unavailable = policy === "dynamic" && !dynamicMemory;
            return <ListRowButton key={policy} role="radio" aria-checked={selected} disabled={unavailable} chevron={false} title={policy === "dynamic" && dynamicMemory ? <span className="flex items-center gap-2">{t(`details.${key}`)}<Badge tone="accent">{t("details.recommended")}</Badge></span> : t(`details.${key}`)} description={t(unavailable ? "details.memoryDynamicOff" : `details.${key}Body`)} trailing={selected ? <Check size={icon.md} className="text-accent" /> : undefined} onClick={() => setDefaults({ memory_policy: policy })} />;
          })}
        </ListGroup>
      </div>
    </div>
    <ModelChoiceSheet api={api} open={sheet === "model"} value={defaults.model_profile_id} onChange={(model_profile_id) => setDefaults({ model_profile_id })} onClose={() => setSheet(null)} />

  </>;
}
