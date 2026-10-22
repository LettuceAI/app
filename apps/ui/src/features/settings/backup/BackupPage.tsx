import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Download, FileArchive, FileUp } from "lucide-react";
import { EmptyState, fileSize, icon, ListGroup, ListRowButton, Page, Skeleton, toast, useLongPress } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import { isApiFailure } from "@/api/client";
import type { BackupHeaderView, BackupIndexView, FileSource } from "@/api/generated/bindings";
import { RestoreSheet } from "@/features/opened-files";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi, useSettingsLayout } from "../data";
import { BackupItemSheet } from "./BackupItemSheet";
import { backupsQuery, useBackupFailureText } from "./backup-data";
import { CreateBackupSheet } from "./CreateBackupSheet";

type Overlay = { kind: "create" } | { kind: "item"; id: string } | { kind: "restore"; source: FileSource; header: BackupHeaderView };

const notBackup: ReadonlySet<string> = new Set(["unsupported", "malformed", "invalid_input"]);

function BackupRow({ entry, onOpen }: { entry: BackupIndexView; onOpen: () => void }) {
  const { t, i18n } = useTranslation("settings");
  const press = useLongPress(onOpen);
  const created = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" }).format(entry.header.created_at);
  const detail = entry.availability === "available" ? `v${entry.header.app_version} · ${fileSize(entry.size, i18n.language)}` : t(`backup.saved.${entry.availability}`);
  return <ListRowButton icon={<FileArchive size={icon.lg} />} tone={entry.availability === "available" ? "neutral" : "warning"} title={created} description={detail} {...press.handlers} onClick={press.guardClick(onOpen)} />;
}

export function BackupPage() {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const { back } = useSettingsLayout();
  const failureText = useBackupFailureText();
  const backups = useQuery(backupsQuery(api));
  const status = useQuery(appStatusQuery(api));
  const [overlay, setOverlay] = useState<Overlay | null>(null);
  const [picking, setPicking] = useState(false);
  const entries = backups.data ?? [];
  const current = overlay?.kind === "item" ? entries.find((entry) => entry.id === overlay.id) ?? null : null;
  const item = useClosingValue(current);
  const create = useClosingValue(overlay?.kind === "create" ? true : null);
  const android = status.data?.platform === "android";

  const browse = async () => {
    setPicking(true);
    try {
      const picked = await api.call("filesPickOpen", { kinds: ["backup"], multiple: false });
      const source = picked.sources[0];
      if (!source) return;
      const header = await api.call("backupInspect", { source });
      setOverlay({ kind: "restore", source, header });
    } catch (error) {
      toast.error(isApiFailure(error) && notBackup.has(error.code) ? t("backup.restore.notBackup") : failureText.ofError(error));
    } finally {
      setPicking(false);
    }
  };

  return (
    <>
      <Page title={t("sections.backup.title")} {...back}>
        <div className="space-y-7">
          <ListGroup label={t("backup.create.group")}>
            <ListRowButton icon={<Download size={icon.lg} />} title={t("backup.create.title")} description={t("backup.create.body")} disabled={status.isPending} onClick={() => setOverlay({ kind: "create" })} />
          </ListGroup>
          <ListGroup label={t("backup.restore.group")}>
            <ListRowButton icon={<FileUp size={icon.lg} />} title={t("backup.restore.title")} description={t("backup.restore.body")} disabled={picking} onClick={() => void browse()} />
          </ListGroup>
          {backups.isPending && (
            <div className="space-y-3" role="status" aria-label={t("backup.loading")}>
              <Skeleton className="h-4 w-28" />
              <Skeleton className="h-16 w-full" />
            </div>
          )}
          {backups.isError && <FailureState error={backups.error} onRetry={() => void backups.refetch()} />}
          {backups.isSuccess && entries.length === 0 && (
            <EmptyState title={t("backup.saved.empty.title")} description={t("backup.saved.empty.body")} />
          )}
          {entries.length > 0 && (
            <ListGroup label={t("backup.saved.group")}>
              {entries.map((entry) => <BackupRow key={entry.id} entry={entry} onOpen={() => setOverlay({ kind: "item", id: entry.id })} />)}
            </ListGroup>
          )}
        </div>
      </Page>
      <CreateBackupSheet key={create.session} api={api} open={overlay?.kind === "create"} android={android} onClose={() => setOverlay(null)} />
      {item.shown && <BackupItemSheet key={item.shown.id} api={api} entry={item.shown} open={current !== null} onClose={() => setOverlay(null)} onRestore={(entry) => setOverlay({ kind: "restore", source: { uri: entry.uri }, header: entry.header })} />}
      {overlay?.kind === "restore" && <RestoreSheet api={api} source={overlay.source} header={overlay.header} onClose={() => setOverlay(null)} />}
    </>
  );
}
