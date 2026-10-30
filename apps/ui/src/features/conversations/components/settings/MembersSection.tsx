import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { UserMinus, UserPlus } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, confirm, icon, IconButton, ListGroup, ListRow, ListRowButton, LoadingState, Meter, Switch } from "@lettuceai/crisp";
import { queryKeys } from "@/api/query-keys";
import { ModelChoiceSheet } from "@/features/characters";
import { modelsQuery } from "../../data/queries";
import { FailureState } from "@/shared/ui/FailureState";
import { participationQuery } from "../../data/settings-queries";
import { AddMemberSheet } from "./AddMemberSheet";
import type { PanelCtx } from "./context";
import { SettingRow } from "./SettingRow";

const noThresholds = { warning: 2, danger: 2 };

export function MembersSection({ ctx }: { ctx: PanelCtx }) {
  const { t, i18n } = useTranslation("conversations");
  const { api, queryClient, conversationId, view } = ctx;
  const stats = useQuery(participationQuery(api, conversationId, true));
  const [adding, setAdding] = useState(false);
  const [modelMember, setModelMember] = useState<string | null>(null);
  const models = useQuery(modelsQuery(api));
  if (stats.isPending) return <LoadingState />;
  if (stats.isError) return <FailureState error={stats.error} onRetry={() => void stats.refetch()} />;
  const members = stats.data.items.filter((member) => member.enabled);
  const active = members.filter((member) => !member.muted).length;
  const percent = new Intl.NumberFormat(i18n.language, { style: "percent", maximumFractionDigits: 0 });
  const refresh = () => queryClient.invalidateQueries({ queryKey: queryKeys.conversations.detail(conversationId) });
  const update = (participantId: string, change: { enabled: boolean | null; muted: boolean | null }) =>
    ctx.run(async () => {
      await api.call("conversationParticipantUpdate", {
        conversation_id: conversationId,
        participant_id: participantId,
        ...change,
        model: null,
        client_operation_id: crypto.randomUUID(),
      });
      await refresh();
    });
  const add = (characterId: string) =>
    ctx.run(async () => {
      await api.call("conversationParticipantAdd", {
        conversation_id: conversationId,
        character_id: characterId,
        client_operation_id: crypto.randomUUID(),
      });
      await refresh();
    });
  const remove = async (participantId: string, name: string) => {
    const sure = await confirm({
      title: t("members.remove.title", { name }),
      message: t("members.remove.body"),
      confirmLabel: t("members.remove.confirm"),
      cancelLabel: t("confirm.cancel"),
      destructive: true,
    });
    if (sure) await update(participantId, { enabled: false, muted: null });
  };
  const taken = new Set(members.flatMap((member) => (member.character_id ? [member.character_id] : [])));
  const selection = ctx.settings?.members;
  const chosen = selection?.participants.find((member) => member.participant_id === modelMember);
  const setModel = async (participantId: string, id: string | null) => (await ctx.run(async () => {
    await api.call("conversationParticipantUpdate", {
      conversation_id: conversationId, participant_id: participantId, enabled: null, muted: null,
      model: id ? { type: "set", id } : { type: "reset" }, client_operation_id: crypto.randomUUID(),
    });
    await refresh();
    return true;
  })) === true;
  return (
    <>
    <ListGroup label={t("settings.sections.members")}>
      {members.map((member) => {
        const last = !member.muted && active === 1;
        const avatar = view.participants.find((entry) => entry.id === member.participant_id)?.avatar?.url ?? null;
        return (
          <ListRow
            key={member.participant_id}
            icon={<Avatar src={avatar} name={member.name} size="sm" />}
            title={member.name}
            description={
              <span className="block space-y-1.5">
                <span className="block">
                  {last
                    ? t("settings.members.last")
                    : member.muted
                      ? t("settings.members.muted")
                      : t("settings.members.replies", { count: member.message_count, percent: percent.format(member.percent / 100) })}
                </span>
                {stats.data.total_messages > 0 && (
                  <Meter value={member.percent} max={100} size="sm" thresholds={noThresholds} label={undefined} />
                )}
              </span>
            }
            trailing={
              <span className="flex items-center gap-1">
                <Switch
                  checked={!member.muted}
                  disabled={ctx.busy || last}
                  aria-label={t("settings.members.answers", { name: member.name })}
                  onChange={(on) => void update(member.participant_id, { enabled: null, muted: !on })}
                />
                <IconButton
                  label={t("members.remove.label", { name: member.name })}
                  size="sm"
                  disabled={ctx.busy || last}
                  onClick={() => void remove(member.participant_id, member.name)}
                >
                  <UserMinus size={icon.md} />
                </IconButton>
              </span>
            }
          />
        );
      })}
      <ListRowButton
        icon={<UserPlus size={icon.lg} />}
        title={t("members.add.row")}
        chevron={false}
        disabled={ctx.busy}
        onClick={() => setAdding(true)}
      />
      <AddMemberSheet api={api} open={adding} taken={taken} busy={ctx.busy} onClose={() => setAdding(false)} onPick={(id) => void add(id)} />
      {selection && (selection.members_source === "conversation" || selection.muted_source === "conversation") && <ListRowButton title={t("members.followGroup")} description={t("members.followGroupHint")} chevron={false} disabled={ctx.busy} onClick={() => void ctx.save({ reset_members: true })} />}
    </ListGroup>
    <ListGroup label={t("members.models")}>
      {selection?.participants.filter((member) => member.enabled).map((member) => {
        const id = member.model.id ?? models.data?.default_model_id;
        const modelName = models.isPending ? t("settings.loading") : models.isError ? t("settings.loadFailed") : models.data?.models.find((model) => model.id === id)?.display_name ?? t("settings.model.none");
        return <SettingRow key={member.participant_id} icon={<Avatar src={view.participants.find((entry) => entry.id === member.participant_id)?.avatar?.url ?? null} name={member.name} size="sm" />} title={member.name} value={modelName} source={member.model.source} disabled={ctx.busy} onOpen={() => { setModelMember(member.participant_id); ctx.openSheet("member_model"); }} onReset={() => void setModel(member.participant_id, null)} />;
      })}
      {selection?.models_source === "conversation" && <ListRowButton title={t("members.followModels")} description={t("members.followModelsHint")} chevron={false} disabled={ctx.busy} onClick={() => void ctx.save({ reset_member_models: true })} />}
    </ListGroup>
    {chosen && ctx.activeSheet === "member_model" && <ModelChoiceSheet api={api} open busy={ctx.busy} value={chosen.model.source === "conversation" ? chosen.model.id : null} defaultChoice={{ title: t("members.characterModel"), description: t("members.characterModelHint") }} onChange={(id) => setModel(chosen.participant_id, id)} onClose={() => { setModelMember(null); ctx.closeSheet(); }} />}
    </>
  );
}
