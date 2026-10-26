import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Archive, BookOpen, KeyRound, MessagesSquare, RotateCcw, Settings2 } from "lucide-react";
import { Alert, Button, confirm, icon, ListGroup, ListRow, Page, Progress, Text } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import { isApiFailure } from "@/api/client";
import type { JobView } from "@/api/generated/bindings";
import { watchJob } from "@/entities/job";
import { useSettingsApi, useSettingsLayout } from "../data";

const stages = ["preflight", "workers", "database", "webview_storage", "restart"] as const;

export function ResetPage() {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation();
  const api = useSettingsApi();
  const { back } = useSettingsLayout();
  const status = useQuery(appStatusQuery(api));
  const [running, setRunning] = useState(false);
  const [job, setJob] = useState<JobView | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const controller = useRef<AbortController | null>(null);
  const mobile = status.data?.platform === "android" || status.data?.platform === "ios";

  useEffect(() => () => controller.current?.abort(), []);

  const reset = async () => {
    if (running) return;
    const sure = await confirm({
      title: t("dataReset.confirmTitle"),
      message: t(mobile ? "dataReset.confirmBodyMobile" : "dataReset.confirmBody"),
      confirmLabel: t("dataReset.confirm"),
      cancelLabel: common("actions.cancel"),
      destructive: true,
    });
    if (!sure) return;
    const abort = new AbortController();
    controller.current = abort;
    setRunning(true);
    setFailure(null);
    setJob(null);
    try {
      const accepted = await api.call("appDataReset", { client_operation_id: crypto.randomUUID() });
      const settled = await watchJob(api, accepted.job_id, setJob, abort.signal);
      if (settled.state === "failed" || settled.state === "cancelled") setFailure(t("dataReset.failed"));
    } catch (error) {
      if (abort.signal.aborted) return;
      setFailure(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      if (controller.current === abort) controller.current = null;
      setRunning(false);
    }
  };

  const stage = stages.find((value) => value === job?.progress.label_code) ?? null;

  return (
    <Page title={t("sections.reset.title")} {...back}>
      <div className="space-y-7">
        <Text variant="body">{t("dataReset.intro")}</Text>
        <ListGroup label={t("dataReset.cleared")}>
          <ListRow icon={<MessagesSquare size={icon.lg} />} title={t("dataReset.items.chats")} />
          <ListRow icon={<BookOpen size={icon.lg} />} title={t("dataReset.items.library")} />
          <ListRow icon={<Settings2 size={icon.lg} />} title={t("dataReset.items.settings")} />
        </ListGroup>
        <ListGroup label={t("dataReset.kept")}>
          <ListRow icon={<KeyRound size={icon.lg} />} title={t("dataReset.items.providers")} />
          <ListRow icon={<Archive size={icon.lg} />} title={t("dataReset.items.previous")} description={t("dataReset.items.previousBody")} />
        </ListGroup>
        <Alert tone="warning" title={t(mobile ? "dataReset.afterMobile" : "dataReset.after")} />
        {running && (
          <div role="status" className="space-y-2">
            <p className="text-sm font-medium text-fg">{stage ? t(`dataReset.stages.${stage}`) : t("dataReset.starting")}</p>
            <Progress label={t("dataReset.running")} />
          </div>
        )}
        {failure && <Alert title={failure} />}
        <div className="flex justify-end">
          <Button variant="danger" block={mobile} leading={<RotateCcw size={icon.md} />} loading={running} disabled={status.isPending} onClick={() => void reset()}>
            {t("dataReset.action")}
          </Button>
        </div>
      </div>
    </Page>
  );
}
