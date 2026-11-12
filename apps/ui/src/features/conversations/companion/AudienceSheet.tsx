import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Ear } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, Badge, Button, icon, ListGroup, ListRow, ListRowButton, Sheet, Skeleton, Text, toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { GroupAudienceScope, GroupMessageAudienceMemberView, ParticipantView, TimelineMessage } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { textOf } from "../data/messages";
import { createOperationIds } from "../memory/operation-ids";
import { groupMessageAudienceQuery } from "./queries";
import { useOverlayBack } from "@/shared/hooks/useOverlayBack";

export function AudienceRow({ api, conversationId, message, onOpen }: { api: ApiClient; conversationId: string; message: TimelineMessage; onOpen: () => void }) {
  const { t, i18n } = useTranslation("conversations");
  const audience = useQuery(groupMessageAudienceQuery(api, conversationId, message.id));
  if (audience.isPending) return <Skeleton className="h-10 w-full" />;
  if (audience.isError || audience.data.length === 0) return null;
  const heard = audience.data.filter((member) => member.heard).map((member) => member.display_name);
  const description = heard.length === 0 ? t("companion.audience.nobody") : new Intl.ListFormat(i18n.language, { style: "short", type: "conjunction" }).format(heard);
  return <ListGroup><ListRowButton icon={<Ear size={icon.lg} />} title={t("companion.audience.title")} description={description} onClick={onOpen} /></ListGroup>;
}

export function AudienceSheet({ api, conversationId, message, participants, open, onClose }: { api: ApiClient; conversationId: string; message: TimelineMessage; participants: readonly ParticipantView[]; open: boolean; onClose: () => void }) {
  const { t } = useTranslation("conversations");
  const { t: common } = useTranslation("common");
  const client = useQueryClient();
  const query = groupMessageAudienceQuery(api, conversationId, message.id);
  const audience = useQuery({ ...query, enabled: open });
  const [expanded, setExpanded] = useState<string | null>(null);
  const [pending, setPending] = useState<string | null>(null);
  const [ids] = useState(createOperationIds);
  useOverlayBack(open, onClose);
  const author = participants.find((participant) => participant.id === message.author_participant_id)?.character_id ?? null;
  const excerpt = textOf(message).trim();

  const change = async (member: GroupMessageAudienceMemberView, heard: boolean, scope: GroupAudienceScope) => {
    const key = `${member.character_id}:${heard}:${scope}`;
    setPending(key);
    try {
      const next = await api.call("groupMessageAudienceSet", { conversation_id: conversationId, message_id: message.id, character_id: member.character_id, heard, scope, client_operation_id: ids.idFor(key) });
      ids.settle(key);
      client.setQueryData(query.queryKey, next);
      setExpanded(null);
      await client.invalidateQueries({ queryKey: queryKeys.companions.all });
    } catch (error) {
      if (isApiFailure(error) && (error.code === "conflict" || error.code === "invalid_input")) ids.settle(key);
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      setPending(null);
    }
  };

  const status = (member: GroupMessageAudienceMemberView) => [t(member.heard ? "companion.audience.statusHeard" : "companion.audience.statusMissed"), ...(member.away_after ? [t("companion.audience.awayAfter")] : [])].join(" · ");

  return <Sheet open={open} onClose={onClose} placement="auto" size="sm" title={t("companion.audience.title")} description={excerpt ? <span className="line-clamp-2 italic">{excerpt}</span> : undefined}>
    {audience.isPending ? <Skeleton className="h-20 w-full" /> : audience.isError ? <FailureState error={audience.error} onRetry={() => void audience.refetch()} /> : <div className="space-y-3 pb-2">
      <ListGroup>
        {audience.data.map((member) => {
          const face = <Avatar src={participants.find((participant) => participant.character_id === member.character_id)?.avatar?.url ?? null} name={member.display_name} size="sm" />;
          if (member.character_id === author) return <ListRow key={member.character_id} icon={face} title={member.display_name} description={t("companion.audience.speaker")} />;
          const badge = <Badge tone={member.heard ? "accent" : "neutral"}>{t(member.heard ? "companion.audience.heardBadge" : "companion.audience.missedBadge")}</Badge>;
          const actions: { heard: boolean; scope: GroupAudienceScope; label: string }[] = member.heard
            ? [{ heard: false, scope: "message", label: t("companion.audience.missOne") }, { heard: false, scope: "onward", label: t("companion.audience.missOnward") }]
            : [{ heard: true, scope: "message", label: t("companion.audience.hearOne") }, { heard: true, scope: "onward", label: t("companion.audience.hearOnward") }];
          return <div key={member.character_id}>
            <ListRowButton icon={face} title={member.display_name} description={status(member)} trailing={badge} chevron={false} aria-expanded={expanded === member.character_id} onClick={() => setExpanded((current) => current === member.character_id ? null : member.character_id)} />
            {expanded === member.character_id && <div className="flex flex-wrap gap-2 px-4 pb-3 pt-1">
              {actions.map((action) => <Button key={action.scope} size="sm" variant="secondary" loading={pending === `${member.character_id}:${action.heard}:${action.scope}`} disabled={pending !== null} onClick={() => void change(member, action.heard, action.scope)}>{action.label}</Button>)}
            </div>}
          </div>;
        })}
      </ListGroup>
      <Text variant="hint" className="px-1">{t("companion.audience.hint")}</Text>
    </div>}
  </Sheet>;
}
