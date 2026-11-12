import { useQuery } from "@tanstack/react-query";
import { Sprout } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, cn, icon, ListGroup, ListRow, ListRowButton, Sheet, Skeleton, Text } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { GroupCompanionEffectView, GroupCompanionMemberEffectView, ParticipantView, TimelineMessage } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { textOf } from "../data/messages";
import { groupCompanionEffectsQuery } from "./queries";
import { changedAxes, changesSomething, numericEntries, signed } from "./values";
import { useOverlayBack } from "@/shared/hooks/useOverlayBack";

function visibleEffects(members: readonly GroupCompanionMemberEffectView[]): GroupCompanionMemberEffectView[] {
  return members.map((member) => ({ ...member, effects: member.effects.filter(changesSomething) })).filter((member) => member.effects.length > 0);
}

export function GroupEffectRow({ api, conversationId, message, onOpen }: { api: ApiClient; conversationId: string; message: TimelineMessage; onOpen: () => void }) {
  const { t, i18n } = useTranslation("conversations");
  const effects = useQuery(groupCompanionEffectsQuery(api, conversationId, message.id));
  if (effects.isPending) return <Skeleton className="h-10 w-full" />;
  if (effects.isError) return <FailureState error={effects.error} onRetry={() => void effects.refetch()} size="sm" />;
  const changed = visibleEffects(effects.data);
  if (changed.length === 0) return null;
  const names = new Intl.ListFormat(i18n.language, { style: "short", type: "conjunction" }).format(changed.map((member) => member.display_name));
  return <ListGroup><ListRowButton icon={<Sprout size={icon.lg} />} title={t("companion.effect.title")} description={t("companion.group.effectSummary", { count: changed.length, names })} onClick={onOpen} /></ListGroup>;
}

function EffectBlock({ effect, participants }: { effect: GroupCompanionEffectView; participants: readonly ParticipantView[] }) {
  const { t, i18n } = useTranslation("conversations");
  const target = effect.target === null ? null : effect.target.kind === "persona" ? t("companion.group.you") : participants.find((participant) => participant.character_id === (effect.target?.kind === "character" ? effect.target.character_id : null))?.name ?? t("companion.group.someone");
  const bond = effect.relationship === null ? [] : changedAxes(effect.relationship);
  const felt = numericEntries(effect.felt).filter(([, value]) => value !== 0);
  const reason = effect.reason === "social_event" ? t("companion.group.reason.social_event", { kind: effect.social_event === null ? "" : t(`companion.group.socialEvent.${effect.social_event}`, { defaultValue: effect.social_event }) }) : t(`companion.group.reason.${effect.reason}`);
  return <ListGroup label={reason}>
    {bond.map(([axis, value]) => <ListRow key={axis} title={t("companion.group.axisToward", { axis: t(`companion.axes.${axis}.label`), target: target ?? "" })} className="py-2.5" trailing={<Text as="span" variant="body" className={cn("tabular-nums", (axis === "tension" ? value < 0 : value > 0) ? "text-accent" : "text-danger")}>{signed(value, i18n.language)}</Text>} />)}
    {felt.map(([emotion, value]) => <ListRow key={emotion} title={t("companion.group.feels", { emotion: t(`companion.emotions.${emotion}`) })} className="py-2.5" trailing={<Text as="span" variant="body" className={cn("tabular-nums", value > 0 ? "text-fg" : "text-fg-3")}>{signed(value, i18n.language)}</Text>} />)}
  </ListGroup>;
}

export function GroupEffectSheet({ api, conversationId, message, participants, open, onClose }: { api: ApiClient; conversationId: string; message: TimelineMessage; participants: readonly ParticipantView[]; open: boolean; onClose: () => void }) {
  const { t } = useTranslation("conversations");
  const effects = useQuery({ ...groupCompanionEffectsQuery(api, conversationId, message.id), enabled: open });
  useOverlayBack(open, onClose);
  return <Sheet open={open} onClose={onClose} placement="auto" size="md" title={t("companion.effect.title")} description={textOf(message).trim() ? <span className="line-clamp-2 italic">{textOf(message).trim()}</span> : undefined}>
    {effects.isPending ? <Skeleton className="h-20 w-full" /> : effects.isError ? <FailureState error={effects.error} onRetry={() => void effects.refetch()} /> : visibleEffects(effects.data).length === 0 ? <Text variant="hint">{t("companion.effect.none")}</Text> : <div className="space-y-5 pb-2">
      {visibleEffects(effects.data).map((member) => <section key={member.character_id} className="space-y-2">
        <div className="flex items-center gap-2 px-1"><Avatar src={participants.find((participant) => participant.character_id === member.character_id)?.avatar?.url ?? null} name={member.display_name} size="sm" /><Text variant="label">{member.display_name}</Text></div>
        {member.effects.map((effect, index) => <EffectBlock key={index} effect={effect} participants={participants} />)}
      </section>)}
    </div>}
  </Sheet>;
}
