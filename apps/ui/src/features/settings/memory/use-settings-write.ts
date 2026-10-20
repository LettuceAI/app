import { useCallback, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { SettingsPatch_Serialize, SettingsView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export type SettingsWrite = (build: (view: SettingsView) => SettingsPatch_Serialize | null) => Promise<SettingsView | null>;

/**
 * Writes settings one patch at a time. Each patch is built from the latest view when its turn comes, so quick edits
 * never overwrite each other; a failure shows the typed error and a conflict refetches.
 */
export function useSettingsWrite(api: ApiClient): SettingsWrite {
  const queryClient = useQueryClient();
  const { t } = useTranslation("common");
  const tail = useRef<Promise<unknown>>(Promise.resolve());
  return useCallback((build) => {
    const run = tail.current.then(async (): Promise<SettingsView | null> => {
      const current = queryClient.getQueryData<SettingsView>(queryKeys.settings.detail());
      if (!current) return null;
      const patch = build(current);
      if (!patch) return current;
      try {
        const view = await api.call("settingsUpdate", { patch, expected_revision: current.revision });
        queryClient.setQueryData(queryKeys.settings.detail(), view);
        void queryClient.invalidateQueries({ queryKey: queryKeys.settings.all });
        return view;
      } catch (error) {
        const code = isApiFailure(error) ? error.code : "internal";
        toast.error(t(`errors.code.${code}`));
        if (code === "conflict") await queryClient.invalidateQueries({ queryKey: queryKeys.settings.all });
        return null;
      }
    });
    tail.current = run;
    return run;
  }, [api, queryClient, t]);
}
