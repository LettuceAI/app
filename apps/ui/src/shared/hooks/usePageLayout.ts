import { useQuery } from "@tanstack/react-query";
import { useCompactLayout } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import { useApi } from "@/api/use-api";
export function usePageLayout() {
  const compact = useCompactLayout();
  const status = useQuery(appStatusQuery(useApi()));
  return {
    phone: compact || status.data?.platform === "android" || status.data?.platform === "ios",
  };
}
