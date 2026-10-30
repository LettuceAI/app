import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Avatar, Card, Field, Input, ReorderableList, Segmented, Text } from "@lettuceai/crisp";
import type { GroupChatMode } from "@/api/generated/bindings";
import { castQuery } from "../data/queries";
import { BackgroundGroup, SpeakerGroup, MemoryGroup } from "./GroupFields";
import type { StepProps } from "./MembersStep";

export function SetupStep({ api, form, update, fallbackName, dynamicMemory }: StepProps & { fallbackName: string; dynamicMemory: boolean }) {
  const { t } = useTranslation("groups");
  const cast = useQuery(castQuery(api));
  const draft = form.draft;
  const setDraft = (patch: Partial<typeof draft>) => update((current) => ({ ...current, draft: { ...current.draft, ...patch } }));
  return <div className="grid grid-cols-1 gap-6 @4xl:grid-cols-2 @4xl:gap-x-10">
    <div className="space-y-6">
      <Field label={t("setup.name")} hint={t("setup.nameHint")}><Input data-autofocus value={draft.name} placeholder={fallbackName} onChange={(event) => setDraft({ name: event.target.value })} /></Field>
      <Field label={t("setup.mode")} hint={t(`setup.${draft.chat_mode}Body`)}><Segmented<GroupChatMode> block aria-label={t("setup.mode")} value={draft.chat_mode} onChange={(chat_mode) => setDraft({ chat_mode })} options={[{ value: "conversation", label: t("setup.conversation") }, { value: "roleplay", label: t("setup.roleplay") }]} /></Field>
      <BackgroundGroup api={api} form={form} update={update} />
    </div>
    <div className="space-y-6">
      <SpeakerGroup api={api} form={form} update={update} />
      <section className="space-y-2" aria-label={t("setup.order")}>
        <Text variant="label" className="block px-1">{t("setup.order")}</Text>
        <ReorderableList aria-label={t("setup.order")} items={draft.members} keyOf={(member) => member.character_id} onReorder={(members) => setDraft({ members: members.map((member, ordinal) => ({ ...member, ordinal })) })}>{(member, handle, position) => {
          const character = cast.data?.find((item) => item.id === member.character_id);
          return <Card padding="none" className="flex items-center gap-3 py-2 pl-2 pr-4">{handle}<Avatar src={character?.avatar?.url ?? null} name={character?.name ?? ""} size="sm" /><span className="min-w-0 flex-1 truncate text-base font-medium text-fg">{character?.name}</span><span className="shrink-0 text-sm tabular-nums text-fg-3">{position + 1}</span></Card>;
        }}</ReorderableList>
        <Text variant="hint" className="px-1">{t("setup.orderBody")}</Text>
      </section>
      <MemoryGroup api={api} form={form} update={update} dynamicMemory={dynamicMemory} />
    </div>
  </div>;
}
