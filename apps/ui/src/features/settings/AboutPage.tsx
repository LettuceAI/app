import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useRouteContext } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { ArrowLeft, ArrowUpRight, Bug, Check, ClipboardCopy, CodeXml, Globe, MessagesSquare, RefreshCw, RotateCcw } from "lucide-react";
import { Button, confirm, ErrorState, icon, IconButton, ListGroup, ListRow, ListRowButton, LoadingState, Switch } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import { openExternalUrl } from "@/api/external-links";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { lettuceLogo } from "@/shared/assets";
import { projectLinks } from "@/shared/links";
import { useSettingsLayout } from "./data";

export function AboutPage() {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation();
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const status = useQuery(appStatusQuery(api));
  const { standalone: tabs } = useSettingsLayout();
  const [resetting, setResetting] = useState(false);
  const [copied, setCopied] = useState<"version" | "details" | null>(null);
  const externalLink = useMutation({ mutationFn: (url: string) => openExternalUrl(url) });
  const openLink = (url: string) => externalLink.mutate(url);

  const reportProblem = () => {
    if (!status.data) return;
    const url = new URL(`${projectLinks.source}/issues/new`);
    url.searchParams.set("template", "bug_report.yml");
    url.searchParams.set("version", `${status.data.version} (${status.data.build_variant})`);
    const platform = { android: "Android", ios: "iOS", windows: "Windows", macos: "macOS", linux: "Linux", other: "" }[status.data.platform];
    if (platform) url.searchParams.set("platform", platform);
    openLink(url.href);
  };

  const external = <ArrowUpRight size={icon.md} className="text-fg-4" />;
  const facts = status.data
    ? [
        [t("about.version"), status.data.version],
        [t("about.build"), t(`about.variant.${status.data.build_variant}`)],
        [t("about.channel"), import.meta.env.DEV ? t("about.dev") : t("about.release")],
        [t("about.platform"), common(`platform.${status.data.platform}`)],
      ]
    : [];

  const copy = async (what: "version" | "details") => {
    if (!status.data) return;
    const text = what === "version" ? status.data.version : facts.map(([label, value]) => `${label}: ${value}`).join("\n");
    try {
      await navigator.clipboard.writeText(text);
      setCopied(what);
      window.setTimeout(() => setCopied((current) => (current === what ? null : current)), 1600);
    } catch {
      setCopied(null);
    }
  };

  const resetOnboarding = async () => {
    const sure = await confirm({
      title: t("about.resetOnboardingTitle"),
      message: t("about.resetOnboardingBody"),
      confirmLabel: t("about.resetOnboarding"),
      cancelLabel: common("actions.cancel"),
    });
    if (!sure) return;
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
      {tabs && (
        <div className="flex items-center gap-2 px-4 pb-1 pt-4">
          <IconButton label={t("back")} shape="round" onClick={() => void navigate({ to: "/settings" })}>
            <ArrowLeft size={icon.lg} />
          </IconButton>
          <h1 className="text-lg font-semibold text-fg">{t("sections.about.title")}</h1>
        </div>
      )}
      <div className={`@container mx-auto w-full max-w-4xl pb-12 ${tabs ? "px-4 pt-4" : "px-10 pt-14"}`}>
        {status.isPending ? (
          <LoadingState label={common("status.loading")} />
        ) : status.isError ? (
          <FailureState error={status.error} onRetry={() => void status.refetch()} />
        ) : (
          <>
            <header className="flex flex-col items-center gap-5 pb-10 text-center @[34rem]:flex-row @[34rem]:gap-7 @[34rem]:text-left">
              <div className="flex h-24 w-24 shrink-0 items-center justify-center rounded-[1.75rem] border border-line bg-surface-1 shadow-raised">
                <img src={lettuceLogo} alt="" className="h-14 w-14" />
              </div>
              <div className="min-w-0 flex-1">
                <h2 className="text-4xl font-semibold tracking-[-0.035em] text-fg">{common("app.name")}</h2>
                <p className="mt-2 max-w-md text-[15px] leading-relaxed text-fg-2">{t("about.tagline")}</p>
                <div className="mt-4 flex flex-wrap items-center justify-center gap-2 @[34rem]:justify-start">
                  <button
                    type="button"
                    onClick={() => void copy("version")}
                    title={t("about.copyVersion")}
                    className="flex h-8 items-center gap-2 rounded-full border border-line bg-surface-1 px-3 text-sm tabular-nums text-fg-2 transition-colors hover:bg-fill hover:text-fg"
                  >
                    {t("about.versionChip", { version: status.data.version })}
                    {copied === "version" ? <Check size={icon.sm} className="text-accent" /> : <ClipboardCopy size={icon.sm} className="text-fg-3" />}
                  </button>
                  <span className="text-sm text-fg-3">
                    {t("about.buildLine", { build: t(`about.variant.${status.data.build_variant}`), platform: common(`platform.${status.data.platform}`) })}
                  </span>
                </div>
              </div>
            </header>

            <div className="space-y-7">
              {externalLink.isError && <ErrorState size="sm" title={t("about.openLinkFailed")} description={common("errors.code.unavailable")} retryLabel={common("actions.retry")} onRetry={() => { if (externalLink.variables) openLink(externalLink.variables); }} />}
              <ListGroup>
                <div className="flex items-center gap-4 px-4 py-4">
                  <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-fill-2 text-fg-3">
                    <RefreshCw size={icon.md} />
                  </div>
                  <div className="min-w-0 flex-1">
                    <p className="text-base font-medium text-fg">{t("about.updates")}</p>
                    <p id="about-updates-unavailable" className="mt-0.5 text-sm text-fg-3">{t("about.updatesUnavailable")}</p>
                  </div>
                  <Button variant="secondary" size="sm" disabled aria-describedby="about-updates-unavailable">{t("about.checkNow")}</Button>
                </div>
                <ListRow
                  title={t("about.automaticUpdates")}
                  description={t("about.automaticUpdatesBody")}
                  trailing={<Switch checked={false} onChange={() => {}} disabled aria-label={t("about.automaticUpdates")} />}
                />
                <ListRowButton title={t("about.whatsNew")} description={t("about.whatsNewBody", { version: status.data.version })} chevron={false} trailing={external} disabled={externalLink.isPending} onClick={() => openLink(projectLinks.changelog)} />
              </ListGroup>

              <div className="grid gap-7 @[48rem]:grid-cols-[minmax(0,1fr)_17rem] @[48rem]:items-start">
                <ListGroup label={t("about.community")}>
                  <ListRowButton icon={<Globe size={icon.lg} />} title={t("about.website")} description={t("about.websiteBody")} chevron={false} trailing={external} disabled={externalLink.isPending} onClick={() => openLink(projectLinks.website)} />
                  <ListRowButton icon={<MessagesSquare size={icon.lg} />} title={t("about.discord")} description={t("about.discordBody")} chevron={false} trailing={external} disabled={externalLink.isPending} onClick={() => openLink(projectLinks.discord)} />
                  <ListRowButton icon={<CodeXml size={icon.lg} />} title={t("about.sourceCode")} description={t("about.sourceCodeBody")} chevron={false} trailing={external} disabled={externalLink.isPending} onClick={() => openLink(projectLinks.source)} />
                  <ListRowButton icon={<Bug size={icon.lg} />} title={t("about.reportProblem")} description={t("about.reportProblemBody")} chevron={false} trailing={external} disabled={externalLink.isPending} onClick={reportProblem} />
                </ListGroup>

                <div className="space-y-7">
                  <section aria-labelledby="about-build" className="space-y-2">
                    <h3 id="about-build" className="px-1 text-2xs font-medium uppercase tracking-[0.18em] text-fg-3">{t("about.information")}</h3>
                    <div className="overflow-hidden rounded-xl border border-line bg-surface-1">
                      <dl className="divide-y divide-line">
                        {facts.map(([label, value]) => (
                          <div key={label} className="flex items-baseline justify-between gap-4 px-4 py-2.5 text-sm">
                            <dt className="text-fg-3">{label}</dt>
                            <dd className="font-medium tabular-nums text-fg">{value}</dd>
                          </div>
                        ))}
                      </dl>
                      <button
                        type="button"
                        onClick={() => void copy("details")}
                        className="flex w-full items-center justify-center gap-2 border-t border-line px-4 py-2.5 text-sm text-fg-2 transition-colors hover:bg-fill hover:text-fg"
                      >
                        {copied === "details" ? <Check size={icon.sm} className="text-accent" /> : <ClipboardCopy size={icon.sm} />}
                        {copied === "details" ? t("about.copied") : t("about.copyDetails")}
                      </button>
                    </div>
                  </section>

                  <ListGroup label={t("about.troubleshooting")}>
                    <ListRowButton
                      icon={<RotateCcw size={icon.lg} />}
                      title={t("about.resetOnboarding")}
                      description={t("about.resetOnboardingHint")}
                      disabled={resetting}
                      onClick={() => void resetOnboarding()}
                    />
                  </ListGroup>
                </div>
              </div>

              <p className="pt-2 text-center text-xs leading-relaxed text-fg-3">{t("about.credits")}</p>
            </div>
          </>
        )}
      </div>
    </main>
  );
}
