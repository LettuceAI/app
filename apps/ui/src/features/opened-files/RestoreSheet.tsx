import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Alert, Button, confirm, Field, KeyValue, Progress, SecretInput, Sheet, toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { BackupHeaderView, FileSource, JobView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { watchJob } from "@/entities/job";

export function RestoreSheet({ api, source, header, onClose }: { api: ApiClient; source: FileSource; header: BackupHeaderView; onClose: () => void }) {
  const { t, i18n } = useTranslation("opened-files");
  const { t: common } = useTranslation();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [password, setPassword] = useState("");
  const [running, setRunning] = useState(false);
  const [job, setJob] = useState<JobView | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const controller = useRef<AbortController | null>(null);

  useEffect(() => () => controller.current?.abort(), []);

  const reasonText = (view: JobView) => {
    const reason = view.failure?.reason;
    return reason && i18n.exists(`restore.failure.${reason}`, { ns: "opened-files" }) ? t(`restore.failure.${reason}` as never) : t("restore.failure.unknown");
  };

  const afterRestore = async (view: JobView) => {
    onClose();
    queryClient.removeQueries({ queryKey: queryKeys.app.status() });
    await queryClient.invalidateQueries();
    await navigate({ to: "/", replace: true });
    const warnings = view.result?.type === "backup_restored" ? view.result.warnings.length : 0;
    if (warnings > 0) toast.warning(t("restore.warnings", { count: warnings }));
    else toast.success(t("restore.done"));
  };

  const restore = async () => {
    if (running || !password) return;
    const sure = await confirm({ title: t("restore.confirmTitle"), message: t("restore.confirmBody"), confirmLabel: t("restore.confirm"), cancelLabel: t("cancel"), destructive: true });
    if (!sure) return;
    const abort = new AbortController();
    controller.current = abort;
    setRunning(true);
    setFailure(null);
    setJob(null);
    let jobId: string | null = null;
    try {
      const accepted = await api.call(header.version === 1 ? "legacyBackupRestore" : "backupRestore", { source, password, client_operation_id: crypto.randomUUID() });
      jobId = accepted.job_id;
      const settled = await watchJob(api, accepted.job_id, setJob, abort.signal);
      if (settled.state === "succeeded") await afterRestore(settled);
      else if (settled.state === "failed") setFailure(reasonText(settled));
    } catch (error) {
      if (abort.signal.aborted) return;
      const settled = jobId ? await api.call("jobGet", { job_id: jobId }).catch(() => null) : null;
      if (settled?.state === "succeeded") await afterRestore(settled);
      else if (settled?.state === "failed") setFailure(reasonText(settled));
      else setFailure(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      if (controller.current === abort) controller.current = null;
      setRunning(false);
    }
  };

  const stop = () => {
    if (job) void api.call("jobCancel", { job_id: job.id }).catch(() => undefined);
  };

  const active = job !== null && job.progress.total ? Math.min(100, (job.progress.current / job.progress.total) * 100) : undefined;
  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" }).format(header.created_at);

  return (
    <Sheet
      open
      dismissible={!running}
      onClose={() => { if (!running) onClose(); }}
      size="sm"
      title={t("restore.title")}
      footer={<div className="flex gap-2">
        {running
          ? <Button variant="secondary" className="flex-1" disabled={job === null} onClick={stop}>{t("restore.stop")}</Button>
          : <Button variant="secondary" className="flex-1" onClick={onClose}>{t("cancel")}</Button>}
        <Button variant="primary" className="flex-1" loading={running} disabled={!password} onClick={() => void restore()}>{t("restore.action")}</Button>
      </div>}
    >
      <div className="space-y-4 pb-1">
        <KeyValue items={[{ label: t("restore.created"), value: date }, { label: t("restore.version"), value: header.app_version }]} />
        <Field label={t("restore.password")} required>
          <SecretInput value={password} disabled={running} autoComplete="off" onChange={setPassword} />
        </Field>
        <Alert tone="warning" title={t("restore.warning")} />
        {running && <div role="status"><Progress {...(active !== undefined ? { value: active } : {})} label={t("restore.running")} /></div>}
        {failure && <Alert tone="danger" title={failure} />}
      </div>
    </Sheet>
  );
}
