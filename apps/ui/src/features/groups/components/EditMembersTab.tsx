import { useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { MoreHorizontal, Sparkles, UserMinus, UserPlus, Users } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  Alert, Avatar, Button, Card, Checkbox, EmptyState, icon, IconButton,
  ListGroup, ListRow, ListRowButton, ReorderableList, SearchField, Sheet, Switch, Text, toast, useLongPress,
} from "@lettuceai/crisp";
import type { CharacterSummary } from "@/api/generated/bindings";
import { ModelChoiceSheet } from "@/features/characters";
import { modelsQuery } from "@/features/conversations";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import type { StepProps } from "./MembersStep";

export interface EditorMember extends Pick<CharacterSummary, "id" | "name" | "avatar" | "description"> {
  archived?: boolean;
}
type MemberPanel = { kind: "member" | "model"; id: string } | { kind: "add" };

function MemberCard({ member, handle, position, phone, detail, onMenu }: {
  member: EditorMember; handle: ReactNode; position: string; phone: boolean; detail: string; onMenu: () => void;
}) {
  const { t } = useTranslation("groups");
  const press = useLongPress(onMenu);
  return <Card padding="none" className="flex items-center gap-3 py-2 pl-2 pr-2" {...press.handlers}>
    {handle}
    <Avatar src={member.avatar?.url ?? null} name={member.name} size="sm" />
    <span className="min-w-0 flex-1">
      <span className="block truncate text-base font-medium text-fg">{member.name}</span>
      <span className="block truncate text-sm text-fg-3">{detail}</span>
    </span>
    {!phone && <span className="shrink-0 text-sm tabular-nums text-fg-3">{position}</span>}
    <IconButton label={t("edit.memberOptions", { name: member.name })} onClick={onMenu}><MoreHorizontal size={icon.md} /></IconButton>
  </Card>;
}

export function EditMembersTab({ api, form, update, cast, phone }: StepProps & { cast: EditorMember[]; phone: boolean }) {
  const { t, i18n } = useTranslation("groups");
  const models = useQuery(modelsQuery(api));
  const [panel, setPanel] = useState<MemberPanel | null>(null);
  const { shown } = useClosingValue(panel);
  const [picked, setPicked] = useState<string[]>([]);
  const [search, setSearch] = useState("");
  const members = form.draft.members;
  const byId = (id: string) => cast.find((member) => member.id === id);
  const member = shown && shown.kind !== "add" ? byId(shown.id) : undefined;
  const link = members.find((item) => item.character_id === member?.id);
  const modelName = (id: string | null) => id === null ? t("extras.characterModel") : models.data?.models.find((item) => item.id === id)?.display_name ?? t("unavailable");
  const count = (value: number) => new Intl.NumberFormat(i18n.language).format(value);
  const answering = members.filter((item) => !item.muted).length;
  const skipped = members.length - answering;
  const candidates = cast.filter((item) => !item.archived && !members.some((member) => member.character_id === item.id));
  const needle = search.trim().toLocaleLowerCase();
  const visible = candidates.filter((item) => [item.name, item.description].some((text) => text?.toLocaleLowerCase().includes(needle)));
  const patchMember = (id: string, patch: Partial<typeof members[number]>) => update((current) => ({ ...current, draft: { ...current.draft, members: current.draft.members.map((item) => item.character_id === id ? { ...item, ...patch } : item) } }));
  const remove = () => {
    if (!member || members.length <= 2) return;
    const before = members;
    update((current) => ({ ...current, draft: { ...current.draft, members: current.draft.members.filter((item) => item.character_id !== member.id) } }));
    setPanel(null);
    toast.success(t("edit.memberRemoved", { name: member.name }), undefined, {
      action: { label: t("edit.undo"), onClick: () => update((current) => ({ ...current, draft: { ...current.draft, members: before } })) },
    });
  };
  return <div className="max-w-3xl space-y-4">
    {members.length < 2 && <Alert tone="warning" title={t(members.length ? "edit.oneLeft" : "edit.noneLeft")} description={t("edit.membersLost")} />}
    <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
      <Text variant="hint" className="tabular-nums" aria-live="polite">{t("memberCount", { count: members.length, formatted: count(members.length) })}{skipped > 0 && t("edit.skippedCount", { count: skipped, formatted: count(skipped) })}</Text>
      <Button size="sm" variant="tonal" leading={<UserPlus size={icon.sm} />} onClick={() => { setPicked([]); setSearch(""); setPanel({ kind: "add" }); }}>{t("edit.addMembers")}</Button>
    </div>
    {members.length > 0 && <ReorderableList aria-label={t("setup.order")} items={members} keyOf={(item) => item.character_id} onReorder={(next) => update((current) => ({ ...current, draft: { ...current.draft, members: next.map((item, ordinal) => ({ ...item, ordinal })) } }))}>
      {(item, handle, position) => {
        const character = byId(item.character_id);
        if (!character) return null;
        const detail = [character.archived ? t("edit.archived") : null, item.muted ? t("edit.skipped") : null, modelName(item.model_profile_override)].filter(Boolean).join(" · ");
        return <MemberCard member={character} handle={handle} position={count(position + 1)} phone={phone} detail={detail} onMenu={() => setPanel({ kind: "member", id: item.character_id })} />;
      }}
    </ReorderableList>}
    <Text variant="hint" className="block px-1">{t("setup.orderBody")} {t("edit.membersFollow")}</Text>
    <Sheet open={panel?.kind === "member"} onClose={() => setPanel(null)} size="sm" title={member?.name} description={link && t(member?.archived ? "edit.orderArchived" : "edit.orderNumber", { number: count(members.indexOf(link) + 1) })} leftAction={member && <Avatar src={member.avatar?.url ?? null} name={member.name} size="md" />}>
      {member && link && <div className="space-y-4 pb-1">
        <ListGroup>
          <ListRowButton icon={<Sparkles size={icon.lg} />} title={t("edit.model")} description={modelName(link.model_profile_override)} onClick={() => setPanel({ kind: "model", id: member.id })} />
          <ListRow title={t("edit.answers")} description={t(!link.muted && answering === 1 ? "edit.lastAnswering" : link.muted ? "edit.answersOff" : "edit.answersOn")} trailing={<Switch checked={!link.muted} disabled={!link.muted && answering === 1} aria-label={t("edit.answers")} onChange={(enabled) => patchMember(member.id, { muted: !enabled })} />} />
        </ListGroup>
        <ListGroup><ListRowButton icon={<UserMinus size={icon.lg} />} title={t("edit.removeMember")} description={t(members.length <= 2 ? "edit.needTwo" : "edit.staysInLibrary")} tone="danger" chevron={false} disabled={members.length <= 2} onClick={remove} /></ListGroup>
      </div>}
    </Sheet>
    {panel?.kind === "model" && link && <ModelChoiceSheet api={api} open value={link.model_profile_override} defaultChoice={{ title: t("extras.characterModel"), description: t("extras.characterModelBody") }} onChange={(id) => patchMember(link.character_id, { model_profile_override: id })} onClose={() => setPanel(null)} />}
    <Sheet open={panel?.kind === "add"} onClose={() => setPanel(null)} size="sm" title={t("edit.addMembers")} description={t("edit.addMembersBody")} footer={<Button variant="primary" block disabled={picked.length === 0} onClick={() => {
      update((current) => ({ ...current, draft: { ...current.draft, members: [...current.draft.members, ...picked.filter((id) => !current.draft.members.some((item) => item.character_id === id)).map((id, index) => ({ character_id: id, ordinal: current.draft.members.length + index, muted: false, model_profile_override: null }))] } }));
      setPanel(null);
    }}>{picked.length === 0 ? t("edit.pickToAdd") : t("edit.addCount", { count: picked.length, formatted: count(picked.length) })}</Button>}>
      <div className="space-y-3 pb-1">
        {(candidates.length > 8 || search) && <SearchField value={search} onChange={setSearch} placeholder={t("members.search")} aria-label={t("members.search")} size="sm" />}
        {candidates.length === 0 ? <EmptyState size="sm" icon={<Users size={icon.xl} />} title={t("edit.everyoneHere")} description={t("edit.everyoneHereBody")} /> : visible.length === 0 ? <EmptyState size="sm" title={t("noMatch", { query: search.trim() })} action={<Button variant="secondary" onClick={() => setSearch("")}>{t("clear")}</Button>} /> :
          <ListGroup>{visible.map((item) => <ListRow key={item.id} icon={<Avatar src={item.avatar?.url ?? null} name={item.name} size="sm" />} title={<span className="block truncate">{item.name}</span>} description={<span className="line-clamp-1">{item.description}</span>} trailing={<Checkbox checked={picked.includes(item.id)} aria-label={item.name} onChange={(on) => setPicked((current) => on ? [...current, item.id] : current.filter((id) => id !== item.id))} />} />)}</ListGroup>}
      </div>
    </Sheet>
  </div>;
}
