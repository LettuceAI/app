import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { BookOpen, UserRound } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, icon, ListGroup, ListRow, ListRowButton, Skeleton, Switch } from "@lettuceai/crisp";
import { ModelChoiceSheet } from "@/features/characters";
import { modelsQuery, useMissingSoulWarning } from "@/features/conversations";
import { LorebookChoiceSheet } from "@/entities/lorebook";
import { PersonaSheet } from "@/entities/persona";
import { castQuery, groupPersonasQuery } from "../data/queries";
import { GroupNotesGroup } from "./GroupNotesGroup";
import type { StepProps } from "./MembersStep";

export function ExtrasStep({ api, form, update, libraryOnly = false, saved }: StepProps & { libraryOnly?: boolean; saved?: { id: string; memberIds: string[] } }) {
  const { t, i18n } = useTranslation("groups");
  const [picking, setPicking] = useState<string | null>(null);
  const personas = useQuery(groupPersonasQuery(api));
  const cast = useQuery(castQuery(api));
  const models = useQuery(modelsQuery(api));
  const soulWarning = useMissingSoulWarning(api, { editable: saved !== undefined });
  const persona = form.draft.persona;

  const modelMember = form.draft.members.find((member) => picking === `model:${member.character_id}`);
  const personaName = persona.kind === "inherit" ? t("extras.defaultPersona") : persona.kind === "disabled" ? t("extras.none") : personas.isPending ? <Skeleton className="mt-1 h-3.5 w-32" /> : personas.data?.items.find((item) => item.id === persona.value)?.title ?? t("unavailable");
  const choose = (value: string | null) => setPicking(value);
  const noteMembers = (saved ? saved.memberIds : form.draft.members.map((member) => member.character_id)).map((id) => ({ id, name: cast.data?.find((item) => item.id === id)?.name ?? t("unavailable") }));
  return <div className="space-y-6">
    {!libraryOnly && <p className="text-base text-fg-2">{t("extras.body")}</p>}
    <div className={libraryOnly ? "space-y-6" : "grid grid-cols-1 gap-6 @4xl:grid-cols-2 @4xl:gap-x-10"}>
      <div className="space-y-6">
        <ListGroup><ListRowButton icon={<UserRound size={icon.lg} />} title={t("extras.persona")} description={personaName} onClick={() => choose("persona")} /></ListGroup>
        <ListGroup label={t("extras.lorebooks")}><ListRowButton icon={<BookOpen size={icon.lg} />} title={t("extras.groupLorebooks")} description={form.lorebookIds.length ? t("extras.attached", { count: form.lorebookIds.length, formatted: new Intl.NumberFormat(i18n.language).format(form.lorebookIds.length) }) : t("extras.noneAttached")} onClick={() => choose("lorebooks")} /><ListRow title={t("extras.memberLorebooks")} description={t(form.draft.disable_character_lorebooks ? "extras.memberLorebooksOff" : "extras.memberLorebooksOn")} trailing={<Switch checked={!form.draft.disable_character_lorebooks} aria-label={t("extras.memberLorebooks")} onChange={(enabled) => update((current) => ({ ...current, draft: { ...current.draft, disable_character_lorebooks: !enabled } }))} />} /></ListGroup>
        <ListGroup label={t("extras.time")}><ListRow title={t("extras.timeAwareness")} description={t(form.draft.time_awareness ? "extras.timeAwarenessOn" : "extras.timeAwarenessOff")} trailing={<Switch checked={form.draft.time_awareness} aria-label={t("extras.timeAwareness")} onChange={(time_awareness) => update((current) => ({ ...current, draft: { ...current.draft, time_awareness } }))} />} /></ListGroup>
        <ListGroup><ListRow title={t("extras.companionMode")} description={t(form.draft.companion_mode ? "extras.companionModeOn" : "extras.companionModeOff")} trailing={<Switch checked={form.draft.companion_mode} aria-label={t("extras.companionMode")} onChange={(companion_mode) => { update((current) => ({ ...current, draft: { ...current.draft, companion_mode } })); if (companion_mode) void soulWarning.warn(form.draft.members.map((member) => member.character_id)); }} />} />{form.draft.companion_mode && <>
          <ListRow title={t("extras.personalMemories")} description={t(form.draft.companion_personal_memories ? "extras.personalMemoriesOn" : "extras.personalMemoriesOff")} trailing={<Switch checked={form.draft.companion_personal_memories} aria-label={t("extras.personalMemories")} onChange={(companion_personal_memories) => update((current) => ({ ...current, draft: { ...current.draft, companion_personal_memories } }))} />} />
          <ListRow title={t("extras.memoryWriteBack")} description={t(form.draft.companion_memory_write_back ? "extras.memoryWriteBackOn" : "extras.memoryWriteBackOff")} trailing={<Switch checked={form.draft.companion_memory_write_back} aria-label={t("extras.memoryWriteBack")} onChange={(companion_memory_write_back) => update((current) => ({ ...current, draft: { ...current.draft, companion_memory_write_back } }))} />} />
        </>}</ListGroup>
        <GroupNotesGroup
          api={api}
          groupId={saved?.id ?? null}
          members={noteMembers}
          draftNotes={form.notes ?? []}
          onDraftNotes={(notes) => update((current) => ({ ...current, notes }))}
          leading={<ListRow title={t("extras.characterNotes")} description={t(form.draft.disable_character_notes ? "extras.characterNotesOff" : "extras.characterNotesOn")} trailing={<Switch checked={!form.draft.disable_character_notes} aria-label={t("extras.characterNotes")} onChange={(enabled) => update((current) => ({ ...current, draft: { ...current.draft, disable_character_notes: !enabled } }))} />} />}
        />
      </div>
      {!libraryOnly && <ListGroup label={t("extras.models")} className="self-start">{form.draft.members.map((member) => {
        const character = cast.data?.find((item) => item.id === member.character_id);
        return <ListRowButton key={member.character_id} icon={<Avatar src={character?.avatar?.url ?? null} name={character?.name ?? ""} size="sm" />} title={<span className="block truncate">{character?.name}</span>} description={member.model_profile_override === null ? t("extras.characterModel") : models.isPending ? <Skeleton className="mt-1 h-3.5 w-32" /> : models.data?.models.find((model) => model.id === member.model_profile_override)?.display_name ?? t("unavailable")} onClick={() => choose(`model:${member.character_id}`)} />;
      })}</ListGroup>}
    </div>
    {soulWarning.dialog}
    <PersonaSheet
      open={picking === "persona"}
      personas={personas.data}
      status={personas.status}
      error={personas.error}
      onRetry={() => void personas.refetch()}
      selectedId={persona.kind === "explicit" ? persona.value : null}
      defaultChoice={{ title: t("extras.useDefault"), description: t("extras.defaultPersona"), selected: persona.kind === "inherit", onSelect: () => { update((current) => ({ ...current, draft: { ...current.draft, persona: { kind: "inherit" } } })); choose(null); } }}
      onClose={() => choose(null)}
      onSelect={(id) => { update((current) => ({ ...current, draft: { ...current.draft, persona: id === null ? { kind: "disabled" } : { kind: "explicit", value: id } } })); choose(null); }}
    />
    <LorebookChoiceSheet api={api} open={picking === "lorebooks"} value={form.lorebookIds} onChange={(lorebookIds) => update((current) => ({ ...current, lorebookIds }))} onClose={() => choose(null)} />
    {modelMember && <ModelChoiceSheet api={api} open value={modelMember.model_profile_override} defaultChoice={{ title: t("extras.characterModel"), description: t("extras.characterModelBody") }} onChange={(id) => update((current) => ({ ...current, draft: { ...current.draft, members: current.draft.members.map((member) => member.character_id === modelMember.character_id ? { ...member, model_profile_override: id } : member) } }))} onClose={() => choose(null)} />}
  </div>;
}
