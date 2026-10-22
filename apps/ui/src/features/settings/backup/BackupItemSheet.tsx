import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { ListX, Trash2, Upload } from "lucide-react";
import { Alert, confirm, fileSize, icon, KeyValue, ListGroup, ListRowButton, Sheet } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { BackupIndexView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";

export function BackupItemSheet({ api, entry, open, onClose, onRestore }: { api: ApiClient; entry: BackupIndexView; open: boolean; onClose: () => void; onRestore: (entry: BackupIndexView) => void }) {
  const { t, i18n } = useTranslation("settings");
  const { t: common } = useTranslation();
  const queryClient = useQueryClient();
  const [failed, setFailed] = useState<unknown>(null);
  const remove = useMutation({
    mutationFn: (file: boolean) => api.call(file ? "backupDelete" : "backupForget", { id: entry.id, client_operation_id: crypto.randomUUID() }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.backups.all });
      onClose();
    },
    onError: setFailed,
  });

  const available = entry.availability === "available";
  const created = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" }).format(entry.header.created_at);
  const items = [
    { label: t("backup.item.version"), value: entry.header.app_version },
    ...(available ? [{ label: t("backup.item.size"), value: fileSize(entry.size, i18n.language) }] : []),
    ...(entry.uri.includes("://") ? [] : [{ label: t("backup.item.location"), value: <span className="break-all font-mono text-xs">{entry.uri}</span> }]),
  ];

  const forget = () => { setFailed(null); remove.mutate(false); };
  const erase = async () => {
    const sure = await confirm({ title: t("backup.item.deleteTitle"), message: t("backup.item.deleteConfirmBody"), confirmLabel: t("backup.item.deleteConfirm"), cancelLabel: common("actions.cancel"), destructive: true });
    if (!sure) return;
    setFailed(null);
    remove.mutate(true);
  };

  return (
    <Sheet open={open} onClose={onClose} size="sm" title={created}>
      <div className="space-y-5 pb-1">
        {entry.availability !== "available" && <Alert tone="warning" title={t(`backup.saved.${entry.availability}`)} />}
        <KeyValue items={items} />
        <ListGroup>
          {available && <ListRowButton icon={<Upload size={icon.lg} />} title={t("backup.item.restore")} description={t("backup.item.restoreBody")} onClick={() => onRestore(entry)} />}
          <ListRowButton icon={<ListX size={icon.lg} />} title={t("backup.item.forget")} description={t("backup.item.forgetBody")} chevron={false} disabled={remove.isPending} onClick={forget} />
          {available && <ListRowButton icon={<Trash2 size={icon.lg} />} tone="danger" title={t("backup.item.delete")} description={t("backup.item.deleteBody")} chevron={false} disabled={remove.isPending} onClick={() => void erase()} />}
        </ListGroup>
        {failed !== null && <FailureState error={failed} size="sm" />}
      </div>
    </Sheet>
  );
}
