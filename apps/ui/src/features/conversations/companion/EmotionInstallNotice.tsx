import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Alert, Button, toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { JobView } from "@/api/generated/bindings";
import { DownloadJob, installEmotionModel, refreshEmotionModel, useDownloads } from "@/entities/download";
import { terminalJobStates } from "@/entities/job";
import { FailureState } from "@/shared/ui/FailureState";

export function EmotionInstallNotice({ api }: { api: ApiClient }) {
  const { t } = useTranslation("conversations");
  const { t: common } = useTranslation("common");
  const { t: modelText } = useTranslation("companions");
  const client = useQueryClient();
  const downloads = useDownloads();
  const [accepted, setAccepted] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [observed, setObserved] = useState<JobView | null>(null);
  const active = downloads.downloads.find((job) => job.kind === "artifact_install" && job.subject.kind === "artifact_install" && job.subject.display_name === "Lettuce Thymos");
  const id = active?.id ?? accepted;
  const terminal = observed?.id === id && terminalJobStates.has(observed.state);
  const install = async () => {
    if (busy || (id !== null && !terminal)) return;
    setBusy(true);
    try {
      setAccepted((await installEmotionModel(api, client)).job_id);
      setObserved(null);
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      setBusy(false);
    }
  };
  return <div className="space-y-2">
    <Alert tone="info" title={t("companion.missingTitle")} description={t("companion.missingBody")} action={<Button size="sm" variant="secondary" loading={busy} disabled={busy || (id !== null && !terminal) || !downloads.ready} onClick={() => void install()}>{t("companion.install")}</Button>} />
    {downloads.error !== null && <FailureState error={downloads.error} onRetry={downloads.retry} size="sm" />}
    {id && <DownloadJob key={id} api={api} id={id} name={modelText("modelName")} onUpdate={setObserved} onDone={() => refreshEmotionModel(client)} />}
  </div>;
}
