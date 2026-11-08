import { useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Sprout } from "lucide-react";
import { useTranslation } from "react-i18next";
import { AppBar, Avatar, Button, CrispStringsProvider, EmptyState, FilterBar, icon, ListGroup, ListRow, ListRowButton, LoadingState, relativeTime, Sheet, Skeleton, Tab, TabList, Table, Tabs, Text, useCompactLayout, type Column } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { ConversationView, GroupCompanionMemberView, GroupMemberRelationshipView } from "@/api/generated/bindings";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { FailureState } from "@/shared/ui/FailureState";
import { BondTiles, EmotionGroup, SectionHead } from "./presentation";
import { companionSoulQuery, emotionStatusQuery, groupCompanionStateQuery } from "./queries";
import { EmotionInstallNotice } from "./EmotionInstallNotice";
import { SoulTab } from "./SoulTab";
import { closenessBand, signed, stanceAxes } from "./values";
import { useOverlayBack } from "./use-overlay-back";

export type GroupCompanionTab = "relationship" | "soul";

interface Stance {
  from: GroupCompanionMemberView;
  to: GroupCompanionMemberView;
  value: GroupMemberRelationshipView;
}

function stanceOf(from: GroupCompanionMemberView, to: GroupCompanionMemberView): GroupMemberRelationshipView | null {
  return from.member_relationships.find((entry) => entry.target_character_id === to.character_id) ?? null;
}

export function GroupCompanionScreen({ api, conversationId, view, tab, onTab, onBack }: { api: ApiClient; conversationId: string; view: ConversationView; tab: GroupCompanionTab; onTab: (tab: GroupCompanionTab) => void; onBack: () => void }) {
  const { t, i18n } = useTranslation("conversations");
  const phone = useCompactLayout();
  const main = useRef<HTMLElement>(null);
  const state = useQuery(groupCompanionStateQuery(api, conversationId));
  const emotion = useQuery(emotionStatusQuery(api));
  const [picked, setPicked] = useState<string | null>(null);
  const [stance, setStance] = useState<Stance | null>(null);
  const { shown: shownStance } = useClosingValue(stance);
  useOverlayBack(stance !== null, () => setStance(null));
  const members = state.data?.members ?? [];
  const member = members.find((entry) => entry.character_id === picked) ?? members[0] ?? null;
  const soul = useQuery(companionSoulQuery(api, conversationId, member?.character_id ?? null));
  const avatarOf = (characterId: string) => view.participants.find((participant) => participant.character_id === characterId)?.avatar?.url ?? null;
  const band = (value: GroupMemberRelationshipView | null) => value === null ? t("companion.group.noStance") : t(`companion.axes.closeness.${closenessBand(value.closeness ?? 0)}`);

  let body;
  if (state.isPending) body = <LoadingState />;
  else if (state.isError && state.data === undefined) body = <FailureState error={state.error} onRetry={() => void state.refetch()} />;
  else if (state.data === null || member === null) body = <EmptyState icon={<Sprout size={icon.xl} />} title={t("companion.unavailable")} description={t("companion.unavailableBody")} action={<Button onClick={onBack}>{t("companion.back")}</Button>} />;
  else if (tab === "soul") body = soul.isPending ? <LoadingState /> : soul.isError && soul.data === undefined ? <FailureState error={soul.error} onRetry={() => void soul.refetch()} /> : soul.data ? <SoulTab key={member.character_id} api={api} conversationId={conversationId} soul={soul.data} onSources={() => undefined} /> : null;
  else {
    const others = members.filter((entry) => entry.character_id !== member.character_id);
    const columns: Column<GroupCompanionMemberView>[] = [
      { key: "member", header: t("companion.group.matrixCorner"), cell: (row) => <span className="flex items-center gap-2"><Avatar src={avatarOf(row.character_id)} name={row.display_name} size="xs" /><span className="truncate">{row.display_name}</span></span> },
      ...members.map((target): Column<GroupCompanionMemberView> => ({
        key: target.character_id,
        header: <span className="flex items-center gap-2"><Avatar src={avatarOf(target.character_id)} name={target.display_name} size="xs" /><span className="truncate">{target.display_name}</span></span>,
        cell: (row) => {
          if (row.character_id === target.character_id) return null;
          const value = stanceOf(row, target);
          return value === null ? <Text as="span" variant="caption" className="text-fg-4">{band(null)}</Text> : <Button size="sm" variant="ghost" aria-label={t("companion.group.stanceLabel", { from: row.display_name, to: target.display_name, band: band(value) })} onClick={() => setStance({ from: row, to: target, value })}>{band(value)}</Button>;
        },
      })),
    ];
    body = <div className="space-y-7">
      <section>
        <SectionHead title={t("companion.bond")} right={member.relationship.last_interaction_at > 0 ? t("companion.lastTalked", { when: relativeTime(member.relationship.last_interaction_at, undefined, i18n.language).toLowerCase() }) : undefined} />
        {soul.isPending ? <Skeleton className="h-28 w-full" /> : soul.isError && soul.data === undefined ? <FailureState error={soul.error} onRetry={() => void soul.refetch()} size="sm" /> : soul.data ? <div className="grid grid-cols-2 gap-2 @2xl:grid-cols-3 @6xl:grid-cols-5"><BondTiles state={member} defaults={soul.data.config.relationship_defaults} narrow={phone} /></div> : null}
        {!member.has_checkpoint && <Text variant="hint" className="mt-2 px-1">{t("companion.group.noCheckpoint", { name: member.display_name })}</Text>}
      </section>
      <section>
        <SectionHead title={t("companion.engine")} />
        {emotion.isPending ? <Skeleton className="h-28 w-full" /> : emotion.isError && emotion.data === undefined ? <FailureState error={emotion.error} onRetry={() => void emotion.refetch()} size="sm" /> : emotion.data?.type !== "installed" ? <EmotionInstallNotice api={api} /> : <div className="grid gap-2 @2xl:grid-cols-2"><EmotionGroup group="expressed" values={member.expressed} /><EmotionGroup group="felt" values={member.felt} /></div>}
      </section>
      {others.length > 0 && <section>
        <SectionHead title={t("companion.group.withOthers")} />
        {phone
          ? <ListGroup label={t("companion.group.towardOthers", { name: member.display_name })}>{others.map((target) => {
            const value = stanceOf(member, target);
            return value === null
              ? <ListRow key={target.character_id} icon={<Avatar src={avatarOf(target.character_id)} name={target.display_name} size="sm" />} title={target.display_name} description={band(null)} />
              : <ListRowButton key={target.character_id} icon={<Avatar src={avatarOf(target.character_id)} name={target.display_name} size="sm" />} title={target.display_name} description={band(value)} onClick={() => setStance({ from: member, to: target, value })} />;
          })}</ListGroup>
          : <Table columns={columns} rows={members} rowKey={(row) => row.character_id} />}
      </section>}
    </div>;
  }

  return <CrispStringsProvider locale={i18n.language} strings={{ sheetClose: t("crisp.sheetClose"), dialogClose: t("companion.crisp.close"), searchFieldClear: t("companion.crisp.clearSearch") }}>
    <div className="relative flex h-full flex-col overflow-clip">
      <main ref={main} className="min-h-0 flex-1 overflow-y-auto">
        <AppBar title={<Text as="span" variant="title" className="min-w-0 truncate">{view.title}</Text>} {...(phone ? {} : { meta: t("companion.title") })} onBack={onBack} backLabel={t("companion.back")} scrollRef={main} filters={<Tabs value={tab} onChange={onTab}><TabList>{(["relationship", "soul"] as const).map((id) => <Tab key={id} value={id}>{t(phone && id === "relationship" ? "companion.tabs.bond" : `companion.tabs.${id}`)}</Tab>)}</TabList></Tabs>} />
        <div className="@container space-y-6 px-4 pb-10 pt-4 @xl:px-8">
          {members.length > 1 && <FilterBar aria-label={t("companion.group.members")} value={member?.character_id ?? ""} onChange={setPicked} options={members.map((entry) => ({ value: entry.character_id, label: entry.display_name, icon: <Avatar src={avatarOf(entry.character_id)} name={entry.display_name} size="xs" /> }))} />}
          {body}
        </div>
      </main>
    </div>
    <Sheet open={stance !== null} onClose={() => setStance(null)} placement="auto" size="sm" title={shownStance ? t("companion.group.stanceTitle", { from: shownStance.from.display_name, to: shownStance.to.display_name }) : undefined} description={shownStance ? band(shownStance.value) : undefined}>
      {shownStance && <ListGroup>{stanceAxes.map((axis) => {
        const value = shownStance.value[axis] ?? 0;
        return <ListRow key={axis} title={t(`companion.axes.${axis}.label`)} className="py-2.5" trailing={<Text as="span" variant="body" className="tabular-nums text-fg">{axis === "tension" ? new Intl.NumberFormat(i18n.language).format(Math.round(value * 100)) : signed(value, i18n.language)}</Text>} />;
      })}</ListGroup>}
    </Sheet>
  </CrispStringsProvider>;
}
