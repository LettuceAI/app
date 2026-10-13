import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useRouteContext } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { ArrowLeft, CodeXml, Globe, MessagesSquare, RefreshCw, RotateCcw } from "lucide-react";
import { Button, icon, IconButton, LoadingState, Switch, useCompactLayout } from "@lettuceai/crisp";
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
  const links = [
    { label: t("about.website"), icon: <Globe size={icon.sm} /> },
    { label: t("about.github"), icon: <CodeXml size={icon.sm} /> },
    { label: t("about.discord"), icon: <MessagesSquare size={icon.sm} /> },
  ];
  const details = status.data ? [
    [t("about.version"), status.data.version],
    [t("about.build"), t(`about.variant.${status.data.build_variant}`)],
    [t("about.channel"), import.meta.env.DEV ? t("about.dev") : t("about.release")],
    [t("about.platform"), common(`platform.${status.data.platform}`)],
  ] : [];

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
    <main className="h-full min-h-0 overflow-y-auto">
      <div className="mx-auto w-full max-w-3xl px-6 pb-9">
        <div className={`relative flex items-center justify-center ${tabs ? "pb-2 pt-4" : "pb-2 pt-10"}`}>
          {tabs && (
            <IconButton label={t("back")} shape="round" className="absolute left-0" onClick={() => void navigate({ to: "/settings" })}>
              <ArrowLeft size={icon.lg} />
            </IconButton>
          )}
          <h1 className="text-base font-medium text-fg-3">{t("sections.about.title")}</h1>
        </div>
        {status.isPending ? (
          <LoadingState label={common("status.loading")} />
        ) : status.isError ? (
          <FailureState error={status.error} onRetry={() => void status.refetch()} />
        ) : (
          <>
            <header className={`flex flex-col items-center text-center ${tabs ? "pb-6 pt-4" : "pb-7 pt-5"}`}>
              <img src={lettuceLogo} alt="" className={tabs ? "h-16 w-16" : "h-20 w-20"} />
              <h2 className={`mt-4 font-semibold tracking-[-0.035em] text-fg ${tabs ? "text-3xl" : "text-4xl"}`}>{common("app.name")}</h2>
              <p className="mt-1.5 text-sm tabular-nums text-fg-3">
                {t("about.versionLine", { version: status.data.version, build: t(`about.variant.${status.data.build_variant}`) })}
              </p>
              <div className="mt-5 flex flex-wrap items-center justify-center gap-1" aria-describedby="about-links-unavailable">
                {links.map((link) => (
                  <Button key={link.label} variant="ghost" size="sm" leading={link.icon} className="px-2.5 text-fg-2" disabled title={t("notYet")}>
                    {link.label}
                  </Button>
                ))}
              </div>
              <p id="about-links-unavailable" className="mt-1 text-xs text-fg-3">{t("about.linksUnavailable")}</p>
            </header>

            <div className="mx-auto max-w-lg">
              <section aria-label={t("about.updates")} className="border-y border-line">
                <div className="flex items-center gap-3 py-6">
                  <div className="min-w-0 flex-1">
                    <p className="text-base font-medium text-fg">{t("about.updates")}</p>
                    <p id="about-updates-unavailable" className="mt-1 text-xs leading-relaxed text-fg-3">{t("about.updatesUnavailable")}</p>
                  </div>
                  <Button variant="secondary" size="sm" disabled leading={<RefreshCw size={icon.sm} />} aria-describedby="about-updates-unavailable">
                    {t("about.checkNow")}
                  </Button>
                </div>
                <div className="flex items-center justify-between gap-5 border-t border-line py-4">
                  <div className="min-w-0">
                    <p className="text-sm font-medium text-fg">{t("about.automaticUpdates")}</p>
                    <p className="mt-1 text-xs leading-relaxed text-fg-3">{t("about.automaticUpdatesBody")}</p>
                  </div>
                  <Switch checked={false} onChange={() => {}} disabled aria-label={t("about.automaticUpdates")} />
                </div>
              </section>

              <section aria-labelledby="about-details" className="pt-5">
                <h3 id="about-details" className="mb-2 text-xs font-medium text-fg-3">{t("about.information")}</h3>
                <dl>
                  {details.map(([label, value]) => (
                    <div key={label} className="flex items-baseline justify-between gap-5 py-2 text-sm">
                      <dt className="text-fg-3">{label}</dt>
                      <dd className="text-right tabular-nums text-fg-2">{value}</dd>
                    </div>
                  ))}
                </dl>
              </section>

              <div className="mt-5 flex items-center justify-between gap-4 border-t border-line py-5">
                <p className="text-xs leading-relaxed text-fg-3">{t("about.resetOnboardingBody")}</p>
                <Button variant="ghost" size="sm" leading={<RotateCcw size={icon.sm} />} loading={resetting} onClick={() => void resetOnboarding()}>
                  {t("about.resetOnboarding")}
                </Button>
              </div>
              <footer className="border-t border-line pt-5 text-center text-xs leading-relaxed text-fg-3">
                <p>{t("about.license")}</p>
                <p className="mt-1">{t("about.credits")}</p>
              </footer>
            </div>
          </>
        )}
      </div>
    </main>
  );
}
