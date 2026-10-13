import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { appStatusQuery } from "@/api/app-status-query";
import type { ApiClient } from "@/api/client";
import { queryKeys } from "@/api/query-keys";

const key = "sidebarExpanded";

/** Whether the desktop sidebar is unfolded, kept in this device's UI state, shown at once and saved behind it. */
export function useSidebarExpanded(api: ApiClient) {
  const queryClient = useQueryClient();
  const status = useQuery(appStatusQuery(api));
  const [chosen, setChosen] = useState<boolean | null>(null);
  const stored = status.data?.ui_state[key];
  const expanded = chosen ?? (typeof stored === "boolean" ? stored : true);
  const set = (next: boolean) => {
    setChosen(next);
    void api
      .call("appUiStateUpdate", { patch: { [key]: next } })
      .then(() => queryClient.invalidateQueries({ queryKey: queryKeys.app.status() }));
  };
  return [expanded, set] as const;
}
