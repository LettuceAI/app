import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { toast } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import type {
  FileTarget,
  JobPage,
  JobView,
  UsageFilters_Deserialize,
} from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { terminalJobStates, watchJob } from "@/entities/job";
import { useSettingsApi } from "../data";
import { dayKey } from "./data";

export function useUsageActions(filters: UsageFilters_Deserialize) {
  const { t } = useTranslation("usage");
  const api = useSettingsApi();
  const client = useQueryClient();
  const status = useQuery(appStatusQuery(api));
  const [exporting, setExporting] = useState(false);
  const [starting, setStarting] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const [observed, setJob] = useState<JobView | null>(null);
  const [watchAttempt, setWatchAttempt] = useState(0);
  const [watchFailure, setWatchError] = useState<{ id: string; error: unknown } | null>(null);
  const operation = useRef<string | null>(null);
  const retry = useRef<() => void>(() => {});
  const alive = useRef(true);
  const busyExport = useRef(false);
  const busyStart = useRef(false);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);
  const recovered = useQuery({
    queryKey: [...queryKeys.jobs.lists(), "usage-prices"],
    queryFn: async () => {
      const jobs: JobView[] = [];
      let cursor: string | null = null;
      do {
        const page: JobPage = await api.call("jobsList", {
          kinds: ["maintenance"],
          states: ["queued", "claimed", "running", "cancellation_requested", "cleaning_up"],
          subject: { kind: "maintenance", id: "usage-recalculate-costs" },
          cursor,
          limit: 100,
        });
        jobs.push(...page.items);
        cursor = page.next_cursor;
      } while (cursor);
      return jobs;
    },
  });
  const recoveredJob = recovered.data?.[0];
  const job =
    observed && (!terminalJobStates.has(observed.state) || recoveredJob?.id === observed.id)
      ? observed
      : (recoveredJob ?? observed);
  const jobId = job?.id;
  useEffect(() => {
    if (!jobId) return;
    const abort = new AbortController();
    void watchJob(
      api,
      jobId,
      (value) => {
        if (!abort.signal.aborted) setJob(value);
      },
      abort.signal,
    )
      .then((settled) => {
        if (abort.signal.aborted) return;
        void client.invalidateQueries({ queryKey: queryKeys.usage.all });
        void client.invalidateQueries({ queryKey: queryKeys.jobs.lists() });
        if (settled.state === "succeeded" && settled.result?.type === "usage_costs_updated")
          toast.success(
            t("prices.done", { count: settled.result.priced, cleared: settled.result.cleared }),
          );
      })
      .catch((failure: unknown) => {
        if (!abort.signal.aborted) setWatchError({ id: jobId, error: failure });
      });
    return () => abort.abort();
  }, [api, client, jobId, watchAttempt, t]);
  const active = Boolean(job && !terminalJobStates.has(job.state));
  const exportCsv = async (saved?: {
    target: FileTarget | null;
    filters: UsageFilters_Deserialize;
  }) => {
    if (busyExport.current || !status.data) return;
    busyExport.current = true;
    setExporting(true);
    setError(null);
    let selection = saved;
    try {
      const snapshot = structuredClone(filters);
      if (!selection) {
        let target: FileTarget | null = null;
        if (status.data.platform !== "android") {
          const picked = await api.call("filesPickSave", {
            kind: "any",
            suggested_name: `usage-${dayKey(new Date())}.csv`,
          });
          if (!alive.current || !picked.target) return;
          target = picked.target;
        }
        selection = { target, filters: snapshot };
      }
      retry.current = () => void exportCsv(selection);
      await api.call("usageExportCsv", selection);
      if (alive.current) toast.success(t("exportDone"));
    } catch (failure) {
      if (alive.current) {
        setError(failure);
        retry.current = () => void exportCsv(selection);
      }
    } finally {
      busyExport.current = false;
      if (alive.current) setExporting(false);
    }
  };
  const start = async () => {
    if (busyStart.current || active) return;
    busyStart.current = true;
    setStarting(true);
    setError(null);
    operation.current ??= crypto.randomUUID();
    retry.current = () => void start();
    try {
      const accepted = await api.call("usageRecalculateCosts", {
        client_operation_id: operation.current,
      });
      if (!alive.current) return;
      const view = await api.call("jobGet", { job_id: accepted.job_id });
      if (!alive.current) return;
      setJob(view);
      setWatchAttempt((value) => value + 1);
      operation.current = null;
      void recovered.refetch();
    } catch (failure) {
      if (alive.current) setError(failure);
    } finally {
      busyStart.current = false;
      if (alive.current) setStarting(false);
    }
  };
  const cancel = async () => {
    if (!job || cancelling) return;
    setCancelling(true);
    setError(null);
    retry.current = () => void cancel();
    try {
      await api.call("jobCancel", { job_id: job.id });
    } catch (failure) {
      if (alive.current) setError(failure);
    } finally {
      if (alive.current) setCancelling(false);
    }
  };
  const refresh = () => void client.invalidateQueries({ queryKey: queryKeys.usage.all });
  return {
    exportCsv: () => void exportCsv(),
    exporting,
    exportReady: Boolean(status.data),
    start: () => void start(),
    starting,
    active,
    job,
    cancel: () => void cancel(),
    cancelling,
    error,
    retry: () => retry.current(),
    watchError: watchFailure && watchFailure.id === jobId ? watchFailure.error : null,
    retryWatch: () => {
      setWatchError(null);
      setWatchAttempt((value) => value + 1);
    },
    recovered,
    refresh,
  };
}
