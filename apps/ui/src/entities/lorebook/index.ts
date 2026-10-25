import { queryOptions } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import { queryKeys } from "@/api/query-keys";
export const generatorDefaultsQuery = (api: ApiClient) =>
  queryOptions({
    queryKey: [...queryKeys.lorebookProjects.all, "defaults"],
    queryFn: () => api.call("lorebookGeneratorDefaults"),
  });
