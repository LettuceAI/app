import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { toast } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { JobView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { terminalJobStates } from "@/entities/job";
import { downloadKinds, downloadName, listActiveDownloads } from "./download-data";
import { DownloadsContext } from "./downloads-context";

export function DownloadsProvider({ api, children }: { api: ApiClient; children: ReactNode }) {
  const { t } = useTranslation();
  const { data: seeded, error: seedError, refetch } = useQuery({ queryKey: queryKeys.downloads.list(), queryFn: () => listActiveDownloads(api), staleTime: Infinity, refetchOnWindowFocus: false });
  const [live, setLive] = useState<ReadonlyMap<string, JobView>>(new Map());
  const known = useRef(new Set<string>());
  const settled = useRef<(job: JobView) => void>(() => undefined);

  useEffect(() => {
    settled.current = (job) => {
      const name = downloadName(job) ?? t("downloads.generic");
      if (job.state === "succeeded") toast.success(t("downloads.done", { name }));
      else if (job.state === "failed" || job.state === "interrupted") toast.error(t("downloads.failed", { name }));
    };
  }, [t]);

  useEffect(() => {
    for (const job of seeded ?? []) known.current.add(job.id);
  }, [seeded]);

  useEffect(() => {
    let active = true;
    let stop: (() => void) | undefined;
    void api.subscribe((event) => {
      if (event.type !== "job_updated" || !downloadKinds.includes(event.job.kind)) return;
      const job = event.job;
      setLive((current) => new Map(current).set(job.id, job));
      if (!terminalJobStates.has(job.state)) known.current.add(job.id);
      else if (known.current.delete(job.id)) settled.current(job);
    }).then((unsubscribe) => {
      if (active) stop = unsubscribe;
      else unsubscribe();
    }).catch(() => undefined);
    return () => { active = false; stop?.(); };
  }, [api]);

  const downloads = useMemo(() => {
    const merged = new Map<string, JobView>((seeded ?? []).map((job) => [job.id, job]));
    for (const job of live.values()) {
      const current = merged.get(job.id);
      if (!current || job.updated_at >= current.updated_at) merged.set(job.id, job);
    }
    return [...merged.values()].filter((job) => !terminalJobStates.has(job.state)).sort((a, b) => a.created_at - b.created_at);
  }, [seeded, live]);

  const value = useMemo(() => ({ downloads, error: seedError, retry: () => void refetch() }), [downloads, seedError, refetch]);
  return <DownloadsContext.Provider value={value}>{children}</DownloadsContext.Provider>;
}
