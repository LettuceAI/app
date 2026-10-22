import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Alert, Button, Field, Progress, SecretInput, Sheet, Text, toast } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { FileTarget, JobView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { watchJob } from "@/entities/job";
import { minPasswordLength, useBackupFailureText } from "./backup-data";

function suggestedName(now: Date) {
  const pad = (value: number) => String(value).padStart(2, "0");
  return `lettuce_backup_${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}_${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}.lettuce`;
}

export function CreateBackupSheet({ api, open, android, onClose }: { api: ApiClient; open: boolean; android: boolean; onClose: () => void }) {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation();
  const failureText = useBackupFailureText();
  const queryClient = useQueryClient();
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [running, setRunning] = useState(false);
  const [job, setJob] = useState<JobView | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const controller = useRef<AbortController | null>(null);

  useEffect(() => () => controller.current?.abort(), []);

  const tooShort = password.length > 0 && password.length < minPasswordLength;
  const mismatch = confirmation.length > 0 && confirmation !== password;
  const ready = password.length >= minPasswordLength && confirmation === password;

  const finish = async (view: JobView) => {
    await queryClient.invalidateQueries({ queryKey: queryKeys.backups.all });
    const warnings = view.result?.type === "backup_exported" ? view.result.warnings.length : 0;
    if (warnings > 0) toast.warning(t("backup.create.warnings", { count: warnings }));
    else toast.success(t("backup.create.done"));
    onClose();
  };

  const create = async () => {
    if (running || !ready) return;
    setFailure(null);
    setJob(null);
    let target: FileTarget | null = null;
    if (!android) {
      try {
        const picked = await api.call("filesPickSave", { suggested_name: suggestedName(new Date()), kind: "backup" });
        if (!picked.target) return;
        target = picked.target;
      } catch (error) {
        setFailure(failureText.ofError(error));
        return;
      }
    }
    const abort = new AbortController();
    controller.current = abort;
    setRunning(true);
    let jobId: string | null = null;
    try {
      const accepted = await api.call("backupExport", { password, target, client_operation_id: crypto.randomUUID() });
      jobId = accepted.job_id;
      const settled = await watchJob(api, accepted.job_id, setJob, abort.signal);
      if (settled.state === "succeeded") await finish(settled);
      else if (settled.state === "failed") setFailure(failureText.ofJob(settled));
    } catch (error) {
      if (abort.signal.aborted) return;
      const settled = jobId ? await api.call("jobGet", { job_id: jobId }).catch(() => null) : null;
      if (settled?.state === "succeeded") await finish(settled);
      else if (settled?.state === "failed") setFailure(failureText.ofJob(settled));
      else setFailure(failureText.ofError(error));
    } finally {
      if (controller.current === abort) controller.current = null;
      setRunning(false);
    }
  };

  const stop = () => {
    if (job) void api.call("jobCancel", { job_id: job.id }).catch(() => undefined);
  };

  const progress = job !== null && job.progress.total ? Math.min(100, (job.progress.current / job.progress.total) * 100) : undefined;

  return (
    <Sheet
      open={open}
      dismissible={!running}
      onClose={() => { if (!running) onClose(); }}
      size="sm"
      title={t("backup.create.sheetTitle")}
      footer={<div className="flex gap-2">
        {running
          ? <Button variant="secondary" className="flex-1" disabled={job === null} onClick={stop}>{t("backup.create.stop")}</Button>
          : <Button variant="secondary" className="flex-1" onClick={onClose}>{common("actions.cancel")}</Button>}
        <Button variant="primary" className="flex-1" loading={running} disabled={!ready} onClick={() => void create()}>{t("backup.create.action")}</Button>
      </div>}
    >
      <form className="space-y-4 pb-1" onSubmit={(event) => { event.preventDefault(); void create(); }}>
        <Field label={t("backup.create.password")} required {...(tooShort ? { error: t("backup.create.tooShort", { count: minPasswordLength }) } : {})}>
          <SecretInput value={password} paste={false} disabled={running} onChange={setPassword} />
        </Field>
        <Field label={t("backup.create.confirm")} required {...(mismatch ? { error: t("backup.create.mismatch") } : {})}>
          <SecretInput value={confirmation} paste={false} disabled={running} onChange={setConfirmation} />
        </Field>
        <Text variant="hint">{t("backup.create.hint")}</Text>
        <Text variant="hint">{t(android ? "backup.create.whereAndroid" : "backup.create.whereDesktop")}</Text>
        {running && <div role="status"><Progress label={t("backup.create.running")} {...(progress !== undefined ? { value: progress } : {})} /></div>}
        {failure && <Alert tone="danger" title={failure} />}
      </form>
    </Sheet>
  );
}
