import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { GitBranch, GitMerge, History, MessageSquare, X } from "lucide-react";
import { EmptyState, icon, ListGroup, ListRowButton, Page, relativeTime, Skeleton, toast } from "@lettuceai/crisp";
import type { SyncConflictsList, SyncForkView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { ActionRow } from "../voices/ActionRow";
import { ConflictSheet, LegacyConflictSheet } from "./ConflictSheets";
import { describeValue, openConflictCount, syncConflictsQuery, useSyncFailureText } from "./sync-data";

type Overlay = { kind: "conflict"; id: string } | { kind: "legacy"; id: string };

function ForkRow({ fork }: { fork: SyncForkView }) {
  const { t } = useTranslation("sync");
  const api = useSettingsApi();
  const client = useQueryClient();
  const navigate = useNavigate();
  const failureText = useSyncFailureText();
  const conversation = useQuery({
    queryKey: [...queryKeys.conversations.detail(fork.conversation_id), "sync-title"],
    queryFn: () => api.call("conversationOpen", { conversation_id: fork.conversation_id }),
    select: (view) => view.title,
  });
  const dismiss = useMutation({
    mutationFn: () => api.call("syncForkResolve", { conversation_id: fork.conversation_id, branch_id: fork.branch_id, client_operation_id: crypto.randomUUID() }),
    onSuccess: () => client.invalidateQueries({ queryKey: queryKeys.sync.conflicts() }),
    onError: (error) => toast.error(failureText(error)),
  });
  const open = () => void navigate({ to: "/chat/$conversationId/branches", params: { conversationId: fork.conversation_id } });
  return (
    <ActionRow
      icon={<GitBranch size={icon.lg} />}
      title={conversation.data || t("conflicts.forkUntitled")}
      description={`${t("conflicts.forkBody")} ${t(fork.holds_local ? "conflicts.forkLocal" : "conflicts.forkOther")}`}
      menuLabel={t("conflicts.menu")}
      onClick={open}
      actions={[
        { label: t("conflicts.openBranches"), icon: <MessageSquare size={icon.md} />, onSelect: open },
        { label: t("conflicts.dismiss"), icon: <X size={icon.md} />, disabled: dismiss.isPending, onSelect: () => dismiss.mutate() },
      ]}
    />
  );
}

function Lists({ list }: { list: SyncConflictsList }) {
  const { t, i18n } = useTranslation("sync");
  const [overlay, setOverlay] = useState<Overlay | null>(null);
  const conflict = overlay?.kind === "conflict" ? list.conflicts.find((entry) => entry.conflict_id === overlay.id) ?? null : null;
  const legacy = overlay?.kind === "legacy" ? list.legacy_conflicts.find((entry) => `${entry.run_id}:${entry.source_key}` === overlay.id) ?? null : null;
  const conflictShown = useClosingValue(conflict);
  const legacyShown = useClosingValue(legacy);
  const when = (at: number) => relativeTime(at, undefined, i18n.language);
  const kindName = (kind: string) => (i18n.exists(`conflicts.kind.${kind}`, { ns: "sync" }) ? t(`conflicts.kind.${kind}` as never) : kind);
  return (
    <>
      {list.conflicts.length > 0 && (
        <ListGroup label={t("conflicts.edits")}>
          {list.conflicts.map((entry) => {
            const candidate = entry.current.value.type === "deleted" ? entry.other : entry.current;
            return (
              <ListRowButton
                key={entry.conflict_id}
                icon={<GitMerge size={icon.lg} />}
                title={describeValue(candidate.value, t).name}
                description={t("conflicts.described", { kind: kindName(entry.entity_kind), when: when(entry.detected_at) })}
                onClick={() => setOverlay({ kind: "conflict", id: entry.conflict_id })}
              />
            );
          })}
        </ListGroup>
      )}
      {list.forks.length > 0 && (
        <ListGroup label={t("conflicts.forks")}>
          {list.forks.map((fork) => <ForkRow key={`${fork.conversation_id}:${fork.branch_id}`} fork={fork} />)}
        </ListGroup>
      )}
      {list.legacy_conflicts.length > 0 && (
        <ListGroup label={t("conflicts.legacy")}>
          {list.legacy_conflicts.map((entry) => (
            <ListRowButton
              key={`${entry.run_id}:${entry.source_key}`}
              icon={<History size={icon.lg} />}
              title={kindName(entry.entity_kind)}
              description={when(entry.detected_at)}
              onClick={() => setOverlay({ kind: "legacy", id: `${entry.run_id}:${entry.source_key}` })}
            />
          ))}
        </ListGroup>
      )}
      {conflictShown.shown && <ConflictSheet key={conflictShown.shown.conflict_id} conflict={conflictShown.shown} open={conflict !== null} onClose={() => setOverlay(null)} />}
      {legacyShown.shown && <LegacyConflictSheet key={`${legacyShown.shown.run_id}:${legacyShown.shown.source_key}`} conflict={legacyShown.shown} open={legacy !== null} onClose={() => setOverlay(null)} />}
    </>
  );
}

export function SyncConflictsPage() {
  const { t } = useTranslation("sync");
  const { t: settings } = useTranslation("settings");
  const api = useSettingsApi();
  const navigate = useNavigate();
  const conflicts = useQuery(syncConflictsQuery(api));
  return (
    <Page
      title={t("conflicts.title")}
      backLabel={settings("sections.sync.title")}
      onBack={() => void navigate({ to: "/settings/$section", params: { section: "sync" } })}
    >
      <div className="space-y-7">
        {conflicts.isPending && <div role="status" aria-label={t("loading")}><Skeleton className="h-16 w-full" /></div>}
        {conflicts.isError && <FailureState error={conflicts.error} onRetry={() => void conflicts.refetch()} />}
        {conflicts.isSuccess && openConflictCount(conflicts.data) === 0 && <EmptyState title={t("conflicts.emptyTitle")} description={t("conflicts.emptyBody")} />}
        {conflicts.isSuccess && <Lists list={conflicts.data} />}
      </div>
    </Page>
  );
}
