import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ApiFailure } from "@/api/client";
import type {
  SettingsLorebookEntryGeneratorSettings,
  SettingsLorebookGeneratorSelection,
  SettingsLorebookGeneratorSettings,
  SettingsPatch_Serialize,
  SettingsView,
} from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { useSettingsApi } from "../data";
type Edit =
  | { type: "scan"; value: number }
  | { type: "generator"; patch: Partial<SettingsLorebookGeneratorSettings> }
  | { type: "selection"; patch: Partial<SettingsLorebookGeneratorSelection> }
  | { type: "entry"; patch: Partial<SettingsLorebookEntryGeneratorSettings> };
function apply(view: SettingsView, edit: Edit): SettingsView {
  const global = { ...view.global };
  if (edit.type === "scan") global.lorebook_scan_depth = edit.value;
  else if (edit.type === "entry")
    global.lorebook_entry_generator = { ...global.lorebook_entry_generator, ...edit.patch };
  else if (edit.type === "generator")
    global.lorebook_generator = { ...global.lorebook_generator, ...edit.patch };
  else
    global.lorebook_generator = {
      ...global.lorebook_generator,
      selection: { ...global.lorebook_generator.selection, ...edit.patch },
    };
  return { ...view, global };
}
export function useLorebookSettings() {
  const api = useSettingsApi();
  const client = useQueryClient();
  const query = useQuery({
    queryKey: queryKeys.settings.detail(),
    queryFn: () => api.call("settingsGet"),
  });
  const [pending, setPending] = useState<{ id: number; edit: Edit }[]>([]);
  const [failure, setFailure] = useState<{ error: unknown; edit: Edit } | null>(null);
  const [subscriptionError, setSubscriptionError] = useState<unknown>(null);
  const [attempt, setAttempt] = useState(0);
  const tail = useRef<Promise<unknown>>(Promise.resolve());
  const sequence = useRef(0);
  const epoch = useRef(0);
  useEffect(() => {
    let active = true;
    let stop: (() => void) | undefined;
    void api
      .subscribe((event) => {
        if (active && event.type === "database_reloaded") {
          epoch.current += 1;
          setPending([]);
          setFailure(null);
        }
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
      stop?.();
    };
  }, [api, attempt]);
  const update = useCallback(
    (edit: Edit) => {
      const id = ++sequence.current;
      const generation = epoch.current;
      setPending((value) => [...value, { id, edit }]);
      setFailure(null);
      const run = tail.current.then(async () => {
        if (generation !== epoch.current) return false;
        try {
          const current = client.getQueryData<SettingsView>(queryKeys.settings.detail());
          if (!current) return false;
          const next = apply(current, edit);
          const patch: SettingsPatch_Serialize =
            edit.type === "scan"
              ? {
                  section: "general",
                  pure_mode: null,
                  analytics_enabled: null,
                  update_checks_enabled: null,
                  developer_mode_enabled: null,
                  auto_download_character_card_avatars: null,
                  manual_mode_context_window: null,
                  lorebook_scan_depth: edit.value,
                }
              : edit.type === "entry"
                ? {
                    section: "lorebook_entry_generator",
                    value: next.global.lorebook_entry_generator,
                  }
                : { section: "lorebook_generator", value: next.global.lorebook_generator };
          const saved = await api.call("settingsUpdate", {
            expected_revision: current.revision,
            patch,
          });
          if (generation !== epoch.current) return false;
          client.setQueryData(queryKeys.settings.detail(), saved);
          void client.invalidateQueries({ queryKey: queryKeys.lorebookProjects.all });
          return true;
        } catch (error) {
          if (generation === epoch.current) {
            setFailure({ error, edit });
            if (ApiFailure.fromRejection(error).code === "conflict") void query.refetch();
          }
          return false;
        } finally {
          if (generation === epoch.current)
            setPending((values) => values.filter((value) => value.id !== id));
        }
      });
      tail.current = run;
      return run;
    },
    [api, client, query],
  );
  const view = useMemo(
    () =>
      query.data ? pending.reduce((view, { edit }) => apply(view, edit), query.data) : undefined,
    [query.data, pending],
  );
  return {
    view,
    error: subscriptionError ?? query.error ?? failure?.error ?? null,
    update,
    retry: () => {
      if (subscriptionError) {
        setSubscriptionError(null);
        setAttempt((value) => value + 1);
      } else if (failure) void update(failure.edit);
      else void query.refetch();
    },
  };
}
