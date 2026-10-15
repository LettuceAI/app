import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { toast } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { AppStatus } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export function useSidebarOnDetailPages(api: ApiClient) {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  const status = useQuery(appStatusQuery(api));
  const save = useMutation({
    mutationFn: (enabled: boolean) => api.call("appUiStateUpdate", { patch: { sidebarOnDetailPages: enabled } }),
    onSuccess: (_result, enabled) => {
      queryClient.setQueryData<AppStatus>(queryKeys.app.status(), (current) => current && {
        ...current,
        ui_state: { ...current.ui_state, sidebarOnDetailPages: enabled },
      });
      return queryClient.invalidateQueries({ queryKey: queryKeys.app.status() });
    },
    onError: (error) => toast.error(t(`errors.code.${isApiFailure(error) ? error.code : "internal"}`)),
  });
  return { enabled: status.data?.ui_state.sidebarOnDetailPages !== false, set: save.mutate, pending: save.isPending };
}
