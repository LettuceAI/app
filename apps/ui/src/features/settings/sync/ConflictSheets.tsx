import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Alert, Button, KeyValue, Panel, Sheet, toast } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import type { LegacyConflictView, SyncConflictChoice, SyncConflictView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { useSettingsApi } from "../data";
import { describeValue, useSyncFailureText } from "./sync-data";

export function ConflictSheet({ conflict, open, onClose }: { conflict: SyncConflictView; open: boolean; onClose: () => void }) {
  const { t, i18n } = useTranslation("sync");
  const api = useSettingsApi();
  const client = useQueryClient();
  const failureText = useSyncFailureText();
  const resolve = useMutation({
    mutationFn: (choice: SyncConflictChoice) =>
      api.call("syncConflictResolve", {
        conflict_id: conflict.conflict_id,
        expected_current_change: conflict.current.change_id,
        choice,
        client_operation_id: crypto.randomUUID(),
      }),
    onSuccess: async () => {
      toast.success(t("conflicts.resolved"));
      onClose();
      await client.invalidateQueries();
    },
    onError: () => client.invalidateQueries({ queryKey: queryKeys.sync.conflicts() }),
  });
  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" });
  const side = (title: string, candidate: SyncConflictView["current"]) => {
    const { name, detail } = describeValue(candidate.value, t);
    const when = typeof candidate.timestamp === "number" ? date.format(candidate.timestamp) : t("conflicts.noTime");
    return (
      <Panel title={title} description={when}>
        <p className="font-medium text-fg">{name}</p>
        {detail && <p className="mt-1 line-clamp-3 text-sm text-fg-2">{detail}</p>}
      </Panel>
    );
  };
  return (
    <Sheet
      open={open}
      onClose={onClose}
      size="lg"
      title={t("conflicts.sheetTitle")}
      footer={
        <div className="flex flex-col gap-2 sm:flex-row">
          <Button variant="secondary" className="flex-1" disabled={resolve.isPending} onClick={() => resolve.mutate("current")}>{t("conflicts.keepThis")}</Button>
          <Button variant="primary" className="flex-1" loading={resolve.isPending} onClick={() => resolve.mutate("other")}>{t("conflicts.useOther")}</Button>
        </div>
      }
    >
      <div className="space-y-4 pb-1">
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          {side(t("conflicts.thisDevice"), conflict.current)}
          {side(t("conflicts.otherDevice"), conflict.other)}
        </div>
        {resolve.isError && (
          <Alert tone={isApiFailure(resolve.error) && resolve.error.code === "conflict" ? "warning" : "danger"} title={isApiFailure(resolve.error) && resolve.error.code === "conflict" ? t("conflicts.changedMeanwhile") : failureText(resolve.error)} />
        )}
      </div>
    </Sheet>
  );
}

const shown = (value: unknown) => {
  const text = typeof value === "string" ? value : (JSON.stringify(value) ?? "");
  return text.length > 200 ? `${text.slice(0, 200)}...` : text;
};

export function LegacyConflictSheet({ conflict, open, onClose }: { conflict: LegacyConflictView; open: boolean; onClose: () => void }) {
  const { t } = useTranslation("sync");
  const api = useSettingsApi();
  const client = useQueryClient();
  const failureText = useSyncFailureText();
  const dismiss = useMutation({
    mutationFn: () => api.call("legacyConflictDismiss", { run_id: conflict.run_id, source_key: conflict.source_key, client_operation_id: crypto.randomUUID() }),
    onSuccess: async () => {
      onClose();
      await client.invalidateQueries({ queryKey: queryKeys.sync.conflicts() });
    },
  });
  return (
    <Sheet
      open={open}
      onClose={onClose}
      size="lg"
      title={t("conflicts.legacyTitle")}
      footer={<Button variant="primary" block loading={dismiss.isPending} onClick={() => dismiss.mutate()}>{t("conflicts.dismiss")}</Button>}
    >
      <div className="space-y-4 pb-1">
        {conflict.fields.map((field) => (
          <Panel key={field.key} title={field.key}>
            <KeyValue
              items={[
                { label: t("conflicts.legacyCurrent"), value: field.current.type === "value" ? shown(field.current.value) : t("conflicts.notMapped") },
                { label: t("conflicts.legacyLocal"), value: shown(field.local) },
                { label: t("conflicts.legacyIncoming"), value: shown(field.incoming) },
              ]}
            />
          </Panel>
        ))}
        {dismiss.isError && <Alert tone="danger" title={failureText(dismiss.error)} />}
      </div>
    </Sheet>
  );
}
