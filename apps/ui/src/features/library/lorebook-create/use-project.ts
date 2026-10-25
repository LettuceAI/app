import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { ApiFailure } from "@/api/client";
import type { JobView, LorebookProjectView_Serialize } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { watchJob } from "@/entities/job";
import { useApi } from "@/api/use-api";
import { projectKey, projectOperation, projectQuery } from "../data/lorebook-projects";

type Task<T> = (
  project: LorebookProjectView_Serialize,
  operation: <P extends object>(command: string, payload: P) => P & { client_operation_id: string },
) => Promise<T>;
export function useProject(id: string) {
  const api = useApi();
  const navigate = useNavigate();
  const mounted = useRef(true);
  const client = useQueryClient();
  const query = useQuery(projectQuery(api, id));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const [watchError, setWatchError] = useState<unknown>(null);
  const [watchAttempt, setWatchAttempt] = useState(0);
  const [jobs, setJobs] = useState<Record<string, JobView>>({});
  const tail = useRef<Promise<unknown>>(Promise.resolve());
  const epoch = useRef(0);
  const retry = useRef<() => void>(() => {});
  useEffect(() => {
    mounted.current = true;
    let disposed = false;
    let stop: (() => void) | undefined;
    void api
      .subscribe((event) => {
        if (!disposed && event.type === "database_reloaded") {
          epoch.current += 1;
          mounted.current = false;
          void client.cancelQueries({ queryKey: queryKeys.lorebookProjects.all });
          client.removeQueries({ queryKey: queryKeys.lorebookProjects.all });
          void navigate({ to: "/library", search: {}, replace: true });
        }
      })
      .then((unsubscribe) => {
        if (disposed) unsubscribe();
        else stop = unsubscribe;
      })
      .catch((failure: unknown) => {
        if (!disposed) setError(failure);
      });
    return () => {
      disposed = true;
      mounted.current = false;
      epoch.current += 1;
      stop?.();
    };
  }, [api, client, navigate]);
  const ids = query.data?.active_job_ids.join("|") ?? "";
  useEffect(() => {
    if (!ids) return;
    const abort = new AbortController();
    for (const jobId of ids.split("|")) {
      void watchJob(
        api,
        jobId,
        (job) => {
          if (!abort.signal.aborted) setJobs((previous) => ({ ...previous, [job.id]: job }));
        },
        abort.signal,
      )
        .then(() => {
          if (!abort.signal.aborted) {
            void client.invalidateQueries({ queryKey: projectKey(id) });
            void client.invalidateQueries({ queryKey: queryKeys.lorebookProjects.all });
          }
        })
        .catch((failure: unknown) => {
          if (!abort.signal.aborted) setWatchError(failure);
        });
    }
    return () => abort.abort();
  }, [api, client, id, ids, watchAttempt]);
  const refresh = async () => {
    await client.invalidateQueries({ queryKey: queryKeys.lorebookProjects.all });
  };
  const perform = <T>(task: Task<T>, records = new Map<string, object>()): Promise<T | null> => {
    if (!mounted.current) return Promise.resolve(null);
    const generation = epoch.current;
    const run = tail.current.then(async () => {
      if (generation !== epoch.current) return null;
      setBusy(true);
      setError(null);
      retry.current = () => void perform(task, records);
      const operation = <P extends object>(
        command: string,
        payload: P,
      ): P & { client_operation_id: string } => {
        return projectOperation(records, command, payload);
      };
      try {
        for (let attempt = 0; attempt < 2; attempt++) {
          const project = await api.call("lorebookProjectGet", { project_id: id });
          if (generation !== epoch.current) return null;
          client.setQueryData(projectKey(id), project);
          try {
            const result = await task(project, operation);
            if (generation !== epoch.current) return null;
            setJobs({});
            await refresh();
            return generation === epoch.current ? result : null;
          } catch (failure) {
            if (ApiFailure.fromRejection(failure).code === "conflict" && attempt === 0) {
              records.clear();
              continue;
            }
            throw failure;
          }
        }
        return null;
      } catch (failure) {
        if (generation === epoch.current) setError(failure);
        return null;
      } finally {
        if (generation === epoch.current) setBusy(false);
      }
    });
    tail.current = run;
    return run;
  };
  return {
    api,
    query,
    project: query.data,
    busy,
    error,
    perform,
    retry: () => retry.current(),
    jobs,
    watchError,
    retryWatch: () => {
      setWatchError(null);
      setWatchAttempt((value) => value + 1);
    },
    refresh,
    isActive: () => mounted.current,
    clearError: () => setError(null),
  };
}
