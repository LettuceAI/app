import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Ear } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, icon, ListGroup, ListRow, ListRowButton, Segmented, Sheet, Skeleton, Switch, Text, toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { GroupAudienceScope, GroupMessageAudienceMemberView, ParticipantView, TimelineMessage } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { textOf } from "../data/messages";
import { createOperationIds } from "../memory/operation-ids";
import { groupMessageAudienceQuery } from "./queries";
import { useOverlayBack } from "./use-overlay-back";

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
  const [scope, setScope] = useState<GroupAudienceScope>("message");
  const [pending, setPending] = useState<string | null>(null);
  const [ids] = useState(createOperationIds);
  useOverlayBack(open, onClose);
  const author = participants.find((participant) => participant.id === message.author_participant_id)?.character_id ?? null;

  const change = async (member: GroupMessageAudienceMemberView, heard: boolean) => {
    const key = `${member.character_id}:${heard}:${scope}`;
    setPending(member.character_id);
    try {
      const next = await api.call("groupMessageAudienceSet", { conversation_id: conversationId, message_id: message.id, character_id: member.character_id, heard, scope, client_operation_id: ids.idFor(key) });
      ids.settle(key);
      client.setQueryData(query.queryKey, next);
      await client.invalidateQueries({ queryKey: queryKeys.companions.all });
    } catch (error) {
      if (isApiFailure(error) && (error.code === "conflict" || error.code === "invalid_input")) ids.settle(key);
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      setPending(null);
    }
  };

  return <Sheet open={open} onClose={onClose} placement="auto" size="md" title={t("companion.audience.title")} description={textOf(message) ? <span className="line-clamp-2">{t("companion.effect.quote", { text: textOf(message) })}</span> : undefined}>
    {audience.isPending ? <Skeleton className="h-20 w-full" /> : audience.isError ? <FailureState error={audience.error} onRetry={() => void audience.refetch()} /> : <div className="space-y-4 pb-2">
      <Text variant="hint">{t("companion.audience.body")}</Text>
      <ListGroup>
        <ListRow title={t("companion.audience.scope")} description={t(scope === "message" ? "companion.audience.scopeMessageBody" : "companion.audience.scopeOnwardBody")} trailing={<Segmented<GroupAudienceScope> size="sm" aria-label={t("companion.audience.scope")} value={scope} onChange={setScope} options={[{ value: "message", label: t("companion.audience.scopeMessage") }, { value: "onward", label: t("companion.audience.scopeOnward") }]} />} />
      </ListGroup>
      <ListGroup>
        {audience.data.map((member) => {
          const speaker = member.character_id === author;
          return <ListRow
            key={member.character_id}
            icon={<Avatar src={participants.find((participant) => participant.character_id === member.character_id)?.avatar?.url ?? null} name={member.display_name} size="sm" />}
            title={member.display_name}
            description={speaker ? t("companion.audience.speaker") : member.away_after ? t("companion.audience.awayAfter") : undefined}
            trailing={<Switch checked={member.heard} disabled={speaker || pending !== null} aria-label={t("companion.audience.heard", { name: member.display_name })} onChange={(heard) => void change(member, heard)} />}
          />;
        })}
      </ListGroup>
    </div>}
  </Sheet>;
}
