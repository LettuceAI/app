import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useRouteContext } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { RotateCcw } from "lucide-react";
import { icon, ListGroup, ListRow, ListRowButton, LoadingState, Page, useCompactLayout } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { lettuceLogo } from "@/shared/assets";

export function AboutPage() {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation();
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const status = useQuery(appStatusQuery(api));
  const compact = useCompactLayout();
  const tabs = compact || status.data?.platform === "android" || status.data?.platform === "ios";
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
    <Page title={t("sections.about.title")} {...(tabs ? { onBack: () => void navigate({ to: "/settings" }), backLabel: t("back") } : {})} contentClassName="mx-auto max-w-2xl space-y-6">
      {status.isPending ? (
        <LoadingState label={common("status.loading")} />
      ) : status.isError ? (
        <FailureState error={status.error} onRetry={() => void status.refetch()} />
      ) : (
        <>
          <div className="flex flex-col items-center pb-2 pt-6 text-center">
            <img src={lettuceLogo} alt="" className="h-20 w-20" />
            <h2 className="mt-4 text-2xl font-bold tracking-tight text-fg">{common("app.name")}</h2>
            <p className="mt-1 text-sm tabular-nums text-fg-3">
              {t("about.versionLine", { version: status.data.version, build: t(`about.variant.${status.data.build_variant}`) })}
            </p>
            <p className="mt-3 max-w-sm text-base text-fg-2">{t("about.tagline")}</p>
          </div>
          <ListGroup label={t("about.information")}>
            <ListRow title={t("about.version")} trailing={<span className="tabular-nums text-fg-3">{status.data.version}</span>} />
            <ListRow title={t("about.build")} trailing={<span className="text-fg-3">{t(`about.variant.${status.data.build_variant}`)}</span>} />
            <ListRow title={t("about.channel")} trailing={<span className="text-fg-3">{import.meta.env.DEV ? t("about.dev") : t("about.release")}</span>} />
            <ListRow title={t("about.platform")} trailing={<span className="text-fg-3">{common(`platform.${status.data.platform}`)}</span>} />
          </ListGroup>
          <ListGroup>
            <ListRowButton
              icon={<RotateCcw size={icon.lg} />}
              title={t("about.resetOnboarding")}
              description={t("about.resetOnboardingBody")}
              chevron={false}
              disabled={resetting}
              onClick={() => void resetOnboarding()}
            />
          </ListGroup>
        </>
      )}
    </Page>
  );
}
