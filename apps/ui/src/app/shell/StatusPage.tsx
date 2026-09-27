import { Card, LoadingState, Page } from "@lettuceai/crisp";
import { queryOptions, useQuery } from "@tanstack/react-query";
import { useRouteContext } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import type { ApiClient } from "@/api/client";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";

function appStatusQuery(api: ApiClient) {
  return queryOptions({
    queryKey: queryKeys.app.status(),
    queryFn: () => api.call("appStatus"),
  });
}

export function StatusPage() {
  const { t } = useTranslation();
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const status = useQuery(appStatusQuery(api));

  return (
    <Page title={t("app.name")}>
      {status.isPending ? (
        <LoadingState label={t("status.loading")} />
      ) : status.isError ? (
        <FailureState error={status.error} onRetry={() => void status.refetch()} />
      ) : (
        <Card className="p-4">
          <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-2 text-base">
            <dt className="text-fg-3">{t("status.version")}</dt>
            <dd>{status.data.version}</dd>
            <dt className="text-fg-3">{t("status.platform")}</dt>
            <dd>{t(`platform.${status.data.platform}`)}</dd>
          </dl>
        </Card>
      )}
    </Page>
  );
}
