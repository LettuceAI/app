import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { toast } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { LogSelection } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export function useLogActions(api: ApiClient) {
  const { t } = useTranslation("logs");
  const client = useQueryClient();
  const [error, setError] = useState<unknown>(null);
  const [busy, setBusy] = useState(false);
  const [subscriptionError, setSubscriptionError] = useState<unknown>(null);
  const [subscriptionAttempt, setSubscriptionAttempt] = useState(0);
  const [follow, setFollow] = useState(false);
  const active = useRef(true);
  const running = useRef(false);
  const epoch = useRef({});
  const retry = useRef<() => void>(() => {});
  const following = useRef(follow);
  useLayoutEffect(() => {
    following.current = follow;
  }, [follow]);
  const refresh = useCallback(async () => {
    await client.invalidateQueries({
      queryKey: queryKeys.logs.all,
      predicate: (query) => query.queryKey[1] !== "diagnostics",
    });
  }, [client]);
  useEffect(() => {
    active.current = true;
    let disposed = false;
    let stop: (() => void) | undefined;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const schedule = () => {
      if (timer !== undefined) return;
      timer = setTimeout(() => {
        timer = undefined;
        if (!disposed && following.current) void refresh();
      }, 120);
    };
    void api
      .subscribe((event) => {
        if (disposed) return;
        if (event.type === "logs_changed" && following.current) schedule();
        if (event.type === "database_reloaded") {
          epoch.current = {};
          void refresh();
        }
      })
      .then((unsubscribe) => {
        if (disposed) unsubscribe();
        else {
          stop = unsubscribe;
          if (following.current) void refresh();
        }
      })
      .catch((failure: unknown) => {
        if (!disposed) setSubscriptionError(failure);
      });
    return () => {
      disposed = true;
      active.current = false;
      epoch.current = {};
      stop?.();
      if (timer !== undefined) clearTimeout(timer);
    };
  }, [api, refresh, subscriptionAttempt]);
  const run = async (work: () => Promise<void>) => {
    if (running.current) return false;
    running.current = true;
    setBusy(true);
    setError(null);
    const generation = epoch.current;
    retry.current = () => void run(work);
    try {
      await work();
      return generation === epoch.current;
    } catch (failure) {
      if (active.current && generation === epoch.current) setError(failure);
      return false;
    } finally {
      running.current = false;
      if (active.current) setBusy(false);
    }
  };
  const copy = (text: string) =>
    run(async () => {
      await navigator.clipboard.writeText(text);
      if (active.current) toast.success(t("copied"));
    });
  const save = (name: string, selection?: LogSelection) =>
    run(async () => {
      const picked = await api.call("filesPickSave", {
        suggested_name: selection ? name.replace(/\.log$/, "-selection.log") : name,
        kind: "any",
      });
      if (!picked.target || !active.current) return;
      if (selection)
        await api.call("logExportSelection", { name, target: picked.target, selection });
      else await api.call("logExport", { name, target: picked.target });
      if (active.current) toast.success(t("saved"));
    });
  return {
    error,
    busy,
    run,
    copy,
    save,
    refresh,
    follow,
    setFollow,
    subscriptionError,
    retry: () => retry.current(),
    retrySubscription: () => {
      setSubscriptionError(null);
      setSubscriptionAttempt((value) => value + 1);
    },
  };
}
