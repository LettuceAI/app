import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useRouteContext } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { useCompactLayout } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import { queryKeys } from "@/api/query-keys";

export function useSettingsApi() {
  return useRouteContext({ from: "__root__", select: (context) => context.api });
}
export function useSettingsLayout() {
  const api = useSettingsApi();
  const { t } = useTranslation("settings");
  const navigate = useNavigate();
  const compact = useCompactLayout();
  const { data } = useQuery(appStatusQuery(api));
  const phone = compact || data?.platform === "android" || data?.platform === "ios";
  const standalone = phone || data?.ui_state.sidebarOnDetailPages === false;
  return { phone, standalone, back: standalone ? { onBack: () => void navigate({ to: "/settings" }), backLabel: t("title") } : {} };
}
export function useProviderData() {
  const api = useSettingsApi();
  const accounts = useQuery({ queryKey: queryKeys.providers.accounts(), queryFn: () => api.call("providerAccountsList") });
  const catalog = useQuery({ queryKey: queryKeys.providers.catalog(), queryFn: async () => (await api.call("providerCatalog")).providers });
  return { accounts, catalog };
}
export function useSettingsAction() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (run: () => Promise<unknown>) => run(),
    onSuccess: async () => {
      await Promise.all([queryKeys.providers.all, queryKeys.audioProviders.all, queryKeys.models.all, queryKeys.settings.all, queryKeys.loras.all, queryKeys.localModels.all, queryKeys.jobs.lists()].map((queryKey) => client.invalidateQueries({ queryKey })));
    },
  });
}
