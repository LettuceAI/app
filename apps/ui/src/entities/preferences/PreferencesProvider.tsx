import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTheme } from "@lettuceai/crisp";
import { ApiFailure, type ApiClient } from "@/api/client";
import type { SettingsView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { PreferencesContext, apply, themeFromPreferences, type PreferenceEdit } from "./context";

export function PreferencesProvider({ api, children }: { api: ApiClient; children: ReactNode }) {
  const client = useQueryClient();
  const query = useQuery({
    queryKey: queryKeys.settings.detail(),
    queryFn: () => api.call("settingsGet"),
  });
  const [pending, setPending] = useState<{ id: number; edit: PreferenceEdit }[]>([]);
  const [failure, setFailure] = useState<unknown>(null);
  const counter = useRef(0);
  const epoch = useRef(0);
  const latestByKey = useRef(new Map<string, number>());
  const tail = useRef<Promise<unknown>>(Promise.resolve());
  const failedRequest = useRef<{ edit: PreferenceEdit; coalesceKey: string | undefined } | null>(
    null,
  );
  const update = useCallback(
    (edit: PreferenceEdit, coalesceKey?: string) => {
      const id = ++counter.current;
      const generation = epoch.current;
      if (coalesceKey) latestByKey.current.set(coalesceKey, id);
      failedRequest.current = null;
      setFailure(null);
      setPending((current) => [...current, { id, edit }]);
      const run = tail.current.then(async () => {
        try {
          if (generation !== epoch.current) return false;
          if (coalesceKey && latestByKey.current.get(coalesceKey) !== id) return true;
          const current = client.getQueryData<SettingsView>(queryKeys.settings.detail());
          if (!current?.global)
            throw new ApiFailure("unavailable", "settings have not loaded", null);
          const changes = edit(current.global.ui_preferences);
          const view = await api.call("settingsUpdate", {
            patch: { section: "ui_preferences", changes },
            expected_revision: current.revision,
          });
          if (generation !== epoch.current) return false;
          client.setQueryData(queryKeys.settings.detail(), view);
          if (coalesceKey && failedRequest.current?.coalesceKey === coalesceKey) {
            failedRequest.current = null;
            setFailure(null);
          }
          if (changes.some((change) => change.key === "chatAppearance"))
            void client.invalidateQueries({ queryKey: queryKeys.conversations.all });
          return true;
        } catch (error) {
          if (generation !== epoch.current) return false;
          const failure = ApiFailure.fromRejection(error);
          failedRequest.current = { edit, coalesceKey };
          setFailure(failure);
          if (failure.code === "conflict")
            await client.invalidateQueries({ queryKey: queryKeys.settings.all });
          return false;
        } finally {
          if (coalesceKey && latestByKey.current.get(coalesceKey) === id)
            latestByKey.current.delete(coalesceKey);
          setPending((current) => current.filter((entry) => entry.id !== id));
        }
      });
      tail.current = run;
      return run;
    },
    [api, client],
  );
  const reset = useCallback(() => {
    epoch.current += 1;
    latestByKey.current.clear();
    setPending([]);
    setFailure(null);
    failedRequest.current = null;
  }, []);
  useEffect(
    () =>
      client.getQueryCache().subscribe((event) => {
        if (
          event.type === "removed" &&
          JSON.stringify(event.query.queryKey) === JSON.stringify(queryKeys.settings.detail())
        ) {
          reset();
        }
      }),
    [client, reset],
  );
  const value = useMemo(
    () =>
      pending.reduce(
        (current, entry) => apply(current, entry.edit(current)),
        query.data?.global?.ui_preferences ?? {},
      ),
    [pending, query.data],
  );
  const { setTheme } = useTheme();
  const {
    themePreset,
    theme: legacyTheme,
    customColors,
    customColorPresets,
    themeRadius,
    themeDensity,
  } = value;
  const theme = useMemo(
    () =>
      themeFromPreferences({
        themePreset,
        theme: legacyTheme,
        customColors,
        customColorPresets,
        themeRadius,
        themeDensity,
      }),
    [themePreset, legacyTheme, customColors, customColorPresets, themeRadius, themeDensity],
  );
  useEffect(() => {
    if (query.data?.global) setTheme(theme);
  }, [query.data, setTheme, theme]);
  const retry = () => {
    if (query.isError) void query.refetch();
    else if (failedRequest.current)
      void update(failedRequest.current.edit, failedRequest.current.coalesceKey);
  };
  return (
    <PreferencesContext.Provider
      value={{
        value,
        ready: query.data !== undefined,
        error: query.error ?? failure,
        retry,
        reset,
        update,
      }}
    >
      {children}
    </PreferencesContext.Provider>
  );
}
