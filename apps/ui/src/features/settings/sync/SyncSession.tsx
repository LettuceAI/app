import { useTranslation } from "react-i18next";
import { useNavigate } from "@tanstack/react-router";
import { useQuery } from "@tanstack/react-query";
import { Alert, Button, fileSize, KeyValue, ListGroup, ListRowButton, Panel, Progress, Skeleton } from "@lettuceai/crisp";
import type { SyncStatus } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { ApprovalSheet, ReadyToSync, Sharing } from "./SyncSharing";
import { SyncStart } from "./SyncStart";
import { syncStatusQuery, useSyncAction, useSyncFailureText } from "./sync-data";

type Of<T extends SyncStatus["type"]> = Extract<SyncStatus, { type: T }>;

function Transfer({ status }: { status: Of<"connecting"> | Of<"syncing"> }) {
  const { t, i18n } = useTranslation("sync");
  const cancel = useSyncAction();
  const syncing = status.type === "syncing";
  const progress = syncing ? status.progress : null;
  const total = progress?.items_total ?? 0;
  const bytesTotal = progress?.bytes_total ?? 0;
  const language = i18n.language;
  const lines = progress
    ? [
        total > 0 ? t("progress.items", { done: progress.items_done, total }) : t("progress.itemsOnly", { done: progress.items_done }),
        bytesTotal > 0
          ? t("progress.bytes", { done: fileSize(progress.bytes_done, language), total: fileSize(bytesTotal, language) })
          : t("progress.bytesOnly", { done: fileSize(progress.bytes_done, language) }),
      ]
    : [];
  return (
    <Panel
      title={syncing ? t("progress.syncing") : t("progress.connecting")}
      description={syncing ? t(`progress.phase.${status.phase}`) : t("progress.connectingBody")}
    >
      <div role="status" className="space-y-2">
        <Progress label={syncing ? t("progress.syncing") : t("progress.connecting")} {...(progress && total > 0 ? { value: Math.min(100, (progress.items_done / total) * 100) } : {})} />
        {lines.map((line) => <p key={line} className="text-sm text-fg-3 tabular-nums">{line}</p>)}
      </div>
      <div className="mt-4">
        <Button variant="secondary" loading={cancel.isPending} onClick={() => cancel.mutate((client) => client.call("syncStop"))}>{t("progress.cancel")}</Button>
      </div>
    </Panel>
  );
}

function Completed({ status }: { status: Of<"completed"> }) {
  const { t, i18n } = useTranslation("sync");
  const navigate = useNavigate();
  const done = useSyncAction();
  const { counts } = status;
  const row = (label: string, value: number, format: (value: number) => string = String) => (value === 0 ? [] : [{ label, value: format(value) }]);
  const items = [
    ...row(t("completed.sentChanges"), counts.sent_changes),
    ...row(t("completed.receivedChanges"), counts.received_changes),
    ...row(t("completed.receivedMedia"), counts.received_assets),
    ...row(t("completed.receivedBytes"), counts.received_bytes, (bytes) => fileSize(bytes, i18n.language)),
    ...row(t("completed.receivedSecrets"), counts.received_secrets),
    ...row(t("completed.skippedSecrets"), counts.skipped_secrets),
    ...row(t("completed.unavailableMedia"), counts.unavailable_assets),
  ];
  const open = status.conflicts + status.forks;
  return (
    <Panel title={t("completed.title")} {...(items.length === 0 ? { description: t("completed.upToDate") } : {})}>
      <div className="space-y-4">
        {items.length > 0 && <KeyValue items={items} />}
        {open > 0 && (
          <ListGroup>
            <ListRowButton title={t("conflicts.needChoice", { count: open })} onClick={() => void navigate({ to: "/settings/sync/conflicts" })} />
          </ListGroup>
        )}
        <Button variant="primary" loading={done.isPending} onClick={() => done.mutate((client) => client.call("syncStop"))}>{t("completed.done")}</Button>
      </div>
    </Panel>
  );
}

function Failed({ status }: { status: Of<"failed"> }) {
  const { t } = useTranslation("sync");
  const failureText = useSyncFailureText();
  const retry = useSyncAction();
  return (
    <div className="space-y-3">
      <Alert tone="danger" title={t("failed.title")} description={failureText(status.reason)} />
      <Button variant="secondary" loading={retry.isPending} onClick={() => retry.mutate((client) => client.call("syncStop"))}>{t("failed.retry")}</Button>
    </div>
  );
}

/**
 * The sync session in whatever state the backend is in. `joinOnly` leaves out hosting, for the first-run setup,
 * where this device has nothing to share yet.
 */
export function SyncSession({ joinOnly = false }: { joinOnly?: boolean }) {
  const { t } = useTranslation("sync");
  const api = useSettingsApi();
  const status = useQuery(syncStatusQuery(api));
  if (status.isPending) return <div role="status" aria-label={t("loading")}><Skeleton className="h-32 w-full" /></div>;
  if (status.isError) return <FailureState error={status.error} onRetry={() => void status.refetch()} />;
  const current = status.data;
  return (
    <>
      {current.type === "idle" && <SyncStart joinOnly={joinOnly} />}
      {current.type === "sharing" && <Sharing addresses={current.addresses} port={current.port} pin={current.pin} />}
      {current.type === "pending_approval" && <Alert tone="info" title={t("request.waiting")} />}
      {current.type === "pending_start" && <ReadyToSync name={current.device_name} />}
      {(current.type === "connecting" || current.type === "syncing") && <Transfer status={current} />}
      {current.type === "completed" && <Completed status={current} />}
      {current.type === "failed" && <Failed status={current} />}
      <ApprovalSheet name={current.type === "pending_approval" ? current.device_name : null} />
    </>
  );
}
