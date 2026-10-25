import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ApiFailure, type ApiClient } from "@/api/client";
import type { SettingsCompanionSoulWriterSettings, SettingsView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

type Edit =
  | { type: "writer"; patch: Partial<SettingsCompanionSoulWriterSettings> }
  | { type: "retention"; keep: boolean };
function apply(view: SettingsView, edit: Edit): SettingsView {
  return edit.type === "writer"
    ? {
        ...view,
        global: {
          ...view.global,
          companion_soul_writer: { ...view.global.companion_soul_writer, ...edit.patch },
        },
      }
    : { ...view, device: { ...view.device, thymos_keep_model_loaded: edit.keep } };
}

export function useCompanionSettings(api: ApiClient) {
  const client = useQueryClient();
  const query = useQuery({
    queryKey: queryKeys.settings.detail(),
    queryFn: () => api.call("settingsGet"),
  });
  const [pending, setPending] = useState<{ id: number; edit: Edit }[]>([]);
  const [failure, setFailure] = useState<{ error: unknown; edit: Edit } | null>(null);
  const [subscriptionError, setSubscriptionError] = useState<unknown>(null);
  const [attempt, setAttempt] = useState(0);
  const [databaseVersion, setDatabaseVersion] = useState(0);
  const sequence = useRef(0);
  const epoch = useRef(0);
  const tail = useRef<Promise<unknown>>(Promise.resolve());
  useEffect(() => {
    let active = true;
    let stop: (() => void) | undefined;
    const clear = () => {
      epoch.current += 1;
      setDatabaseVersion((value) => value + 1);
      setPending([]);
      setFailure(null);
    };
    const cacheStop = client.getQueryCache().subscribe((event) => {
      if (
        event.type === "removed" &&
        event.query.queryKey.join("/") === queryKeys.settings.detail().join("/")
      )
        clear();
    });
    void api
      .subscribe((event) => {
        if (active && event.type === "database_reloaded") clear();
      })
      .then((unsubscribe) => {
        if (active) stop = unsubscribe;
        else unsubscribe();
      })
      .catch((error: unknown) => {
        if (active) setSubscriptionError(error);
      });
    return () => {
      active = false;
      epoch.current += 1;
      cacheStop();
      stop?.();
    };
  }, [api, client, attempt]);
  const update = useCallback(
    (edit: Edit) => {
      const id = ++sequence.current;
      const generation = epoch.current;
      setPending((current) => [...current, { id, edit }]);
      setFailure(null);
      const run = tail.current.then(async () => {
        try {
          if (generation !== epoch.current) return false;
          const current = client.getQueryData<SettingsView>(queryKeys.settings.detail());
          if (!current?.global.companion_soul_writer || !current.device)
            throw new ApiFailure("unavailable", "companion settings are not loaded", null);
          const next = apply(current, edit);
          const view = await api.call("settingsUpdate", {
            expected_revision: edit.type === "writer" ? current.revision : current.device.revision,
            patch:
              edit.type === "writer"
                ? { section: "companion_soul_writer", value: next.global.companion_soul_writer }
                : { section: "device_onnx", model: "thymos", keep_model_loaded: edit.keep },
          });
          if (generation !== epoch.current) return false;
          client.setQueryData(queryKeys.settings.detail(), view);
          return true;
        } catch (error) {
          if (generation !== epoch.current) return false;
          const failure = ApiFailure.fromRejection(error);
          setFailure({ error: failure, edit });
          if (failure.code === "conflict")
            await client.invalidateQueries({ queryKey: queryKeys.settings.all });
          return false;
        } finally {
          setPending((current) => current.filter((value) => value.id !== id));
        }
      });
      tail.current = run;
      return run;
    },
    [api, client],
  );
  const view = useMemo(
    () =>
      query.data ? pending.reduce((view, { edit }) => apply(view, edit), query.data) : undefined,
    [query.data, pending],
  );
  const retry = () => {
    if (subscriptionError) {
      setSubscriptionError(null);
      setAttempt((value) => value + 1);
    } else if (query.isError) void query.refetch();
    else if (failure) void update(failure.edit);
  };
  return {
    databaseVersion,
    view,
    error: subscriptionError ?? query.error ?? failure?.error ?? null,
    retry,
    changeWriter: (patch: Partial<SettingsCompanionSoulWriterSettings>) =>
      update({ type: "writer", patch }),
    setKeepLoaded: (keep: boolean) => update({ type: "retention", keep }),
  };
}
