import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useApi } from "@/api/use-api";
import type { ApiClient } from "@/api/client";
import type { CreatorView } from "@/api/generated/bindings";

export const creatorKeys = {
  all: ["creator"] as const,
  lists: ["creator", "list"] as const,
  detail: (id: string) => ["creator", "detail", id] as const,
};

const creatorQuery = (api: ApiClient, id: string) => ({
  queryKey: creatorKeys.detail(id),
  queryFn: () => api.call("creatorGet", { workflow_id: id }),
});

export function useCreator(id: string) {
  const api = useApi();
  const client = useQueryClient();
  const navigate = useNavigate();
  const query = useQuery(creatorQuery(api, id));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const running = useRef(false);
  const alive = useRef(true);
  const epoch = useRef(0);
  const operations = useRef(new Map<string, string>());
  useEffect(() => {
    alive.current = true;
    let stop: (() => void) | undefined;
    const subscription = new AbortController();
    void api.subscribe((event) => {
      if (subscription.signal.aborted) return;
      if (event.type === "job_updated" && event.job.kind === "creation_run" && event.job.subject.kind === "creation_project" && event.job.subject.id === id) {
        void client.invalidateQueries({ queryKey: creatorKeys.detail(id) });
      }
      if (event.type === "database_reloaded") {
        epoch.current += 1;
        alive.current = false;
        client.removeQueries({ queryKey: creatorKeys.all });
        void navigate({ to: "/library", replace: true });
      }
    }).then((unsubscribe) => {
      if (subscription.signal.aborted) unsubscribe();
      else stop = unsubscribe;
    }).catch((failure: unknown) => { if (!subscription.signal.aborted) setError(failure); });
    return () => { subscription.abort(); alive.current = false; epoch.current += 1; stop?.(); };
  }, [api, client, navigate, id]);
  const operation = (name: string, payload: unknown) => {
    const signature = JSON.stringify([name, payload]);
    let key = operations.current.get(signature);
    if (!key) { key = crypto.randomUUID(); operations.current.set(signature, key); }
    return key;
  };
  const refresh = async () => {
    const generation = epoch.current;
    const next = await api.call("creatorGet", { workflow_id: id });
    if (alive.current && generation === epoch.current) {
      client.setQueryData(creatorKeys.detail(id), next);
      void client.invalidateQueries({ queryKey: creatorKeys.lists });
    }
    return next;
  };
  const perform = async <T,>(action: (work: CreatorView) => Promise<T>): Promise<T | null> => {
    const current = client.getQueryData<CreatorView>(creatorKeys.detail(id)) ?? query.data;
    if (running.current || !current || !alive.current) return null;
    running.current = true;
    setBusy(true);
    setError(null);
    const generation = epoch.current;
    try {
      const result = await action(current);
      if (!alive.current || generation !== epoch.current) return null;
      await refresh();
      operations.current.clear();
      return result;
    } catch (failure) {
      if (alive.current && generation === epoch.current) setError(failure);
      return null;
    } finally {
      running.current = false;
      if (alive.current && generation === epoch.current) setBusy(false);
    }
  };
  return { api, query, busy, error, perform, operation, refresh };
}
