import { Button, Card, LoadingState, Page } from "@lettuceai/crisp";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useRouteContext } from "@tanstack/react-router";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { appStatusQuery } from "@/api/app-status-query";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";

export function StatusPage() {
  const { t } = useTranslation();
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const status = useQuery(appStatusQuery(api));
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [resetting, setResetting] = useState(false);

  const resetOnboarding = async () => {
    setResetting(true);
    try {
      await api.call("appUiStateUpdate", { patch: { onboarding: null, legacyMigration: null } });
      await queryClient.invalidateQueries({ queryKey: queryKeys.app.status() });
      await navigate({ to: "/", replace: true });
    } finally {
      setResetting(false);
    }
  };

  return (
    <Page title={t("app.name")}>
      {status.isPending ? (
        <LoadingState label={t("status.loading")} />
      ) : status.isError ? (
        <FailureState error={status.error} onRetry={() => void status.refetch()} />
      ) : (
        <Card className="space-y-4 p-4">
          <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-2 text-base">
            <dt className="text-fg-3">{t("status.version")}</dt>
            <dd>{status.data.version}</dd>
            <dt className="text-fg-3">{t("status.platform")}</dt>
            <dd>{t(`platform.${status.data.platform}`)}</dd>
          </dl>
          <Button variant="secondary" size="sm" disabled={resetting} onClick={() => void resetOnboarding()}>
            {t("status.resetOnboarding")}
          </Button>
        </Card>
      )}
    </Page>
  );
}
