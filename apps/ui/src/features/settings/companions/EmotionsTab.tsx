import { useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Download, HeartPulse, MemoryStick, ShieldCheck, Trash2 } from "lucide-react";
import {
  Alert,
  Badge,
  Button,
  Chip,
  Collapsible,
  confirm,
  icon,
  ListGroup,
  ListRow,
  ListRowButton,
  Skeleton,
  Switch,
  Text,
} from "@lettuceai/crisp";
import type { JobView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { DownloadCard, DownloadJob, useDownloads } from "@/entities/download";
import { terminalJobStates } from "@/entities/job";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";

const feelings = [
  "admiration",
  "amusement",
  "anger",
  "annoyance",
  "approval",
  "caring",
  "confusion",
  "curiosity",
  "desire",
  "disappointment",
  "disapproval",
  "disgust",
  "embarrassment",
  "excitement",
  "fear",
  "gratitude",
  "grief",
  "joy",
  "love",
  "nervousness",
  "optimism",
  "pride",
  "realization",
  "relief",
  "remorse",
  "sadness",
  "surprise",
  "neutral",
] as const;

export function EmotionsTab({
  keepLoaded,
  onKeepLoaded,
}: {
  keepLoaded: boolean;
  onKeepLoaded: (keep: boolean) => Promise<boolean>;
}) {
  const { t } = useTranslation("companions");
  const api = useSettingsApi();
  const client = useQueryClient();
  const status = useQuery({
    queryKey: queryKeys.companions.emotion(),
    queryFn: () => api.call("companionEmotionStatus"),
  });
  const downloads = useDownloads();
  const [accepted, setAccepted] = useState<string | null>(null);
  const [observed, setObserved] = useState<JobView | null>(null);
  const [operation, setOperation] = useState<"install" | "remove" | null>(null);
  const [failure, setFailure] = useState<{ error: unknown; action: "install" | "remove" } | null>(
    null,
  );
  const working = useRef(false);
  const active = downloads.downloads.find(
    (job) =>
      job.kind === "artifact_install" &&
      job.subject.kind === "artifact_install" &&
      job.subject.display_name === "Lettuce Thymos" &&
      !(observed?.id === job.id && terminalJobStates.has(observed.state)),
  );
  const activeId = active?.id;
  if (activeId && activeId !== accepted) setAccepted(activeId);
  const id = active?.id ?? accepted;
  const local = observed?.id === id ? observed : null;
  const job =
    local && terminalJobStates.has(local.state)
      ? local
      : id
        ? (downloads.jobs.get(id) ?? local)
        : local;
  const installing =
    operation === "install" || (id !== null && (!job || !terminalJobStates.has(job.state)));
  const busy = operation !== null || installing;
  const installed = status.data?.type === "installed";
  const damaged = status.data?.type === "damaged";
  const refresh = async () => {
    await Promise.all(
      [queryKeys.companions.all, queryKeys.downloads.list(), queryKeys.conversations.all].map(
        (queryKey) => client.invalidateQueries({ queryKey }),
      ),
    );
  };
  const install = async () => {
    if (working.current || busy || !downloads.ready) return;
    working.current = true;
    setOperation("install");
    setFailure(null);
    try {
      const accepted = await api.call("companionEmotionInstall");
      setAccepted(accepted.job_id);
      void client.invalidateQueries({ queryKey: queryKeys.downloads.list() });
    } catch (error) {
      setFailure({ error, action: "install" });
    } finally {
      working.current = false;
      setOperation(null);
    }
  };
  const remove = async () => {
    if (working.current || busy || !downloads.ready) return;
    if (
      !(await confirm({
        title: t("deleteTitle"),
        message: t("deleteBody"),
        confirmLabel: t("delete"),
        cancelLabel: t("cancel"),
        destructive: true,
      }))
    )
      return;
    if (working.current) return;
    working.current = true;
    setOperation("remove");
    setFailure(null);
    try {
      await api.call("companionEmotionRemove");
      setAccepted(null);
      setObserved(null);
      await refresh();
    } catch (error) {
      setFailure({ error, action: "remove" });
    } finally {
      working.current = false;
      setOperation(null);
    }
  };
  const showJob = id !== null && (job?.state !== "succeeded" || !installed);
  return (
    <div className="space-y-7">
      <Text variant="secondary" className="px-1">
        {t("emotionsIntro")}
      </Text>
      <section className="space-y-2">
        {failure && (
          <FailureState
            error={failure.error}
            onRetry={() => void (failure.action === "install" ? install() : remove())}
            size="sm"
          />
        )}
        {downloads.error !== null && (
          <FailureState error={downloads.error} onRetry={downloads.retry} size="sm" />
        )}
        {(installing || showJob) && (
          <div className="space-y-2">
            {installing && (
              <Text variant="label" className="px-1">
                {t("emotionModel")}
              </Text>
            )}
            {id ? (
              <DownloadJob
                key={id}
                api={api}
                id={id}
                name={t("modelName")}
                onUpdate={setObserved}
                onDone={refresh}
              />
            ) : (
              <DownloadCard job={null} name={t("modelName")} status="queued" />
            )}
          </div>
        )}
        {!installing &&
          (status.isPending ? (
            <ListGroup label={t("emotionModel")}>
              <div role="status" aria-label={t("loadingEmotion")}>
                <ListRow
                  icon={<Skeleton className="h-5 w-5" />}
                  title={<Skeleton className="h-5 w-32" />}
                  description={<Skeleton className="h-4 w-48 max-w-full" />}
                  trailing={<Skeleton className="h-8 w-20" />}
                />
              </div>
            </ListGroup>
          ) : status.isError ? (
            <FailureState error={status.error} onRetry={() => void status.refetch()} />
          ) : (
            <>
              <ListGroup label={t("emotionModel")}>
                <ListRow
                  icon={<HeartPulse size={icon.lg} />}
                  tone={installed ? "accent" : damaged ? "danger" : "neutral"}
                  title={t("modelName")}
                  description={t(installed ? "installed" : damaged ? "damaged" : "notDownloaded")}
                  trailing={
                    installed ? (
                      <Badge tone="accent" dot>
                        {t("ready")}
                      </Badge>
                    ) : (
                      <Button
                        size="sm"
                        variant={damaged ? "tonal" : "primary"}
                        disabled={!downloads.ready}
                        leading={!damaged ? <Download size={icon.sm} /> : undefined}
                        onClick={() => void install()}
                      >
                        {t(
                          damaged
                            ? "repair"
                            : job && (job.state === "failed" || job.state === "interrupted")
                              ? "retry"
                              : "download",
                        )}
                      </Button>
                    )
                  }
                />
              </ListGroup>
              {!installed && !damaged && (
                <Text variant="hint" className="px-1">
                  {t("missingHint")}
                </Text>
              )}
              {damaged && (
                <Alert tone="warning" title={t("damagedTitle")} description={t("damagedBody")} />
              )}
            </>
          ))}
      </section>
      {installed && !installing && (
        <ListGroup label={t("onThisDevice")}>
          <ListRow
            icon={<MemoryStick size={icon.lg} />}
            title={t("keepInMemory")}
            description={t("keepInMemoryBody")}
            trailing={
              <Switch
                disabled={busy}
                checked={keepLoaded}
                onChange={(keep) => void onKeepLoaded(keep)}
                aria-label={t("keepInMemoryLabel")}
              />
            }
          />
          <ListRowButton
            icon={<Trash2 size={icon.lg} />}
            tone="danger"
            title={t("deleteThymos")}
            description={t(operation === "remove" ? "removing" : "deleteHint")}
            chevron={false}
            disabled={busy || !downloads.ready}
            onClick={() => void remove()}
          />
        </ListGroup>
      )}
      <Collapsible
        title={t("feelingsTitle")}
        description={t("feelingsBody", { count: feelings.length })}
        variant="panel"
      >
        <div className="flex flex-wrap gap-1.5">
          {feelings.map((feeling) => (
            <Chip key={feeling}>{t(`feelings.${feeling}`)}</Chip>
          ))}
        </div>
      </Collapsible>
      <div className="flex items-start gap-3 px-1">
        <ShieldCheck size={icon.md} className="mt-0.5 shrink-0 text-fg-3" />
        <Text variant="hint">{t("privacy")}</Text>
      </div>
    </div>
  );
}
