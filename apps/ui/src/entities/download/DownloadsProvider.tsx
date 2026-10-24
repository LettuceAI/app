import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { toast } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { JobView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { terminalJobStates } from "@/entities/job";
import { downloadKinds, downloadName, listActiveDownloads } from "./download-data";
import { DownloadRateTracker } from "./download-rate";
import { DownloadsContext } from "./downloads-context";

export function DownloadsProvider({ api, children }: { api: ApiClient; children: ReactNode }) {
  const { t } = useTranslation();
  const client = useQueryClient();
  const rates = useRef(new DownloadRateTracker());
  const { data: seeded, error: seedError, refetch } = useQuery({ queryKey: queryKeys.downloads.list(), queryFn: () => listActiveDownloads(api), staleTime: Infinity, refetchOnWindowFocus: false });
  const [live, setLive] = useState<ReadonlyMap<string, JobView>>(new Map());
  const known = useRef(new Set<string>());
  const cancellations = useRef(new Map<string, Promise<void>>());
  const [cancelling, setCancelling] = useState<ReadonlySet<string>>(new Set());
  const [subscriptionError, setSubscriptionError] = useState<unknown>(null);
  const [subscriptionAttempt, setSubscriptionAttempt] = useState(0);
  const cancel = useCallback((id: string): Promise<void> => {
    const existing = cancellations.current.get(id);
    if (existing) return existing;
    setCancelling((current) => new Set(current).add(id));
    const pending = api.call("jobCancel", { job_id: id }).then(() => undefined).finally(() => {
      cancellations.current.delete(id);
      setCancelling((current) => { const next = new Set(current); next.delete(id); return next; });
    });
    cancellations.current.set(id, pending);
    return pending;
  }, [api]);
  const retry = useCallback(() => { setSubscriptionError(null); setSubscriptionAttempt((value) => value + 1); void refetch(); }, [refetch]);
  const settled = useRef<(job: JobView) => void>(() => undefined);

  useEffect(() => {
    settled.current = (job) => {
      const name = downloadName(job) ?? t("downloads.generic");
      if (job.state === "succeeded") toast.success(t("downloads.done", { name }));
      else if (job.state === "failed" || job.state === "interrupted") toast.error(t("downloads.failed", { name }));
    };
  }, [t]);

  useEffect(() => {
    for (const job of seeded ?? []) {
      known.current.add(job.id);
      if (!rates.current.has(job.id)) rates.current.observe(job, performance.now());
    }
  }, [seeded]);

  useEffect(() => {
    let active = true;
    let stop: (() => void) | undefined;
    void api.subscribe((event) => {
      if (!active) return;
      if (event.type === "database_reloaded") {
        known.current.clear(); rates.current.clear(); setLive(new Map());
        void client.resetQueries({ queryKey: queryKeys.downloads.list(), exact: true });
        return;
      }
      if (event.type !== "job_updated" || !downloadKinds.includes(event.job.kind)) return;
      const job = rates.current.observe(event.job, performance.now());
      setLive((current) => new Map(current).set(job.id, job));
      if (!terminalJobStates.has(job.state)) known.current.add(job.id);
      else if (known.current.delete(job.id)) settled.current(job);
    }).then((unsubscribe) => {
      if (active) stop = unsubscribe;
      else unsubscribe();
    }).catch((error: unknown) => { if (active) setSubscriptionError(error); });
    return () => { active = false; stop?.(); };
  }, [api, client, subscriptionAttempt]);

  const jobs = useMemo(() => {
    const merged = new Map<string, JobView>((seeded ?? []).map((job) => [job.id, job]));
    for (const job of live.values()) {
      const current = merged.get(job.id);
      if (!current || job.updated_at >= current.updated_at) merged.set(job.id, job);
    }
    return merged;
  }, [seeded, live]);
  const downloads = useMemo(() => [...jobs.values()].filter((job) => !terminalJobStates.has(job.state)).sort((a, b) => a.created_at - b.created_at), [jobs]);

  const value = useMemo(() => ({ downloads, jobs, ready: seeded !== undefined && seedError === null && subscriptionError === null, cancel, cancelling, error: subscriptionError ?? seedError, retry }), [downloads, jobs, seeded, seedError, subscriptionError, cancel, cancelling, retry]);
  return <DownloadsContext.Provider value={value}>{children}</DownloadsContext.Provider>;
}
