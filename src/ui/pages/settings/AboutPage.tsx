import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ExternalLink, Globe, RefreshCw } from "lucide-react";

import githubSvg from "../../../assets/github.svg";
import logoSvg from "../../../assets/logo.svg";
import { checkForAppUpdate, type AppUpdateInfo } from "../../../core/app-updates/checkForAppUpdate";
import { presentAppUpdateToast } from "../../../core/app-updates/presentAppUpdateToast";
import { useI18n } from "../../../core/i18n/context";
import { readSettings, saveAdvancedSettings } from "../../../core/storage/repo";
import { type Settings } from "../../../core/storage/schemas";
import {
  DISCORD_SERVER_LINK,
  DOWNLOADS_PAGE_LINK,
  GITHUB_REPO_LINK,
  SUBREDDIT_LINK,
} from "../../../core/utils/links";
import { getPlatform } from "../../../core/utils/platform";
import { isDevelopmentMode, setDeveloperModeOverride } from "../../../core/utils/env";
import { toast } from "../../components/toast";
import { cn, interactive } from "../../design-tokens";
import { Switch } from "../../components/Switch";

function ensureAdvancedSettings(settings: Settings): NonNullable<Settings["advancedSettings"]> {
  return {
    ...(settings.advancedSettings ?? {}),
    avatarGenerationEnabled: settings.advancedSettings?.avatarGenerationEnabled ?? true,
    creationHelperEnabled: settings.advancedSettings?.creationHelperEnabled ?? false,
    helpMeReplyEnabled: settings.advancedSettings?.helpMeReplyEnabled ?? true,
    sceneGenerationEnabled: settings.advancedSettings?.sceneGenerationEnabled ?? true,
    sceneGenerationMode: settings.advancedSettings?.sceneGenerationMode ?? "auto",
    accessibility: settings.advancedSettings?.accessibility,
    appUpdateChecksEnabled: settings.advancedSettings?.appUpdateChecksEnabled ?? true,
    developerModeEnabled: settings.advancedSettings?.developerModeEnabled ?? false,
  };
}

function SectionTitle({ title }: { title: string }) {
  return <h2 className="px-1 text-[11px] font-medium text-fg/42">{title}</h2>;
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-4 border-b border-fg/8 py-3 last:border-b-0 last:pb-0 first:pt-0">
      <span className="text-sm text-fg/52">{label}</span>
      <span className="text-sm font-medium text-fg">{value}</span>
    </div>
  );
}

export function AboutPage() {
  const { t } = useI18n();
  const platform = useMemo(() => getPlatform(), []);
  const [appVersion, setAppVersion] = useState("...");
  const [autoChecksEnabled, setAutoChecksEnabled] = useState(true);
  const [isCheckingUpdates, setIsCheckingUpdates] = useState(false);
  const [updateState, setUpdateState] = useState<"idle" | "available" | "upToDate">("idle");
  const [availableUpdate, setAvailableUpdate] = useState<AppUpdateInfo | null>(null);
  const [developerModeEnabled, setDeveloperModeEnabled] = useState(isDevelopmentMode());

  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      try {
        const [version, settings] = await Promise.all([
          invoke<string>("get_app_version"),
          readSettings(),
        ]);
        if (cancelled) return;
        setAppVersion(version);
        setAutoChecksEnabled(settings.advancedSettings?.appUpdateChecksEnabled ?? true);
        setDeveloperModeEnabled(
          isDevelopmentMode() || settings.advancedSettings?.developerModeEnabled === true,
        );
      } catch (error) {
        console.error("Failed to load about page state:", error);
      }
    };

    void load();

    return () => {
      cancelled = true;
    };
  }, []);

  const buildChannel = appVersion.includes("-dev.")
    ? t("about.buildChannel.dev")
    : t("about.buildChannel.release");

  const persistAutoChecks = async (enabled: boolean) => {
    const settings = await readSettings();
    const advanced = ensureAdvancedSettings(settings);
    advanced.appUpdateChecksEnabled = enabled;
    await saveAdvancedSettings(advanced);
  };

  const toggleAutoChecks = async () => {
    const next = !autoChecksEnabled;
    setAutoChecksEnabled(next);
    try {
      await persistAutoChecks(next);
    } catch (error) {
      console.error("Failed to save app update preference:", error);
      setAutoChecksEnabled(!next);
      toast.error(t("about.errors.saveTitle"), t("about.errors.saveDescription"));
    }
  };

  const handleCheckNow = async () => {
    setIsCheckingUpdates(true);
    try {
      const update = await checkForAppUpdate(platform);
      if (!update) {
        setAvailableUpdate(null);
        setUpdateState("upToDate");
        toast.info(t("about.update.upToDateTitle"), t("about.update.upToDateDescription"));
        return;
      }
      setAvailableUpdate(update);
      setUpdateState("available");
      presentAppUpdateToast(update, platform.os, {
        title: t("updates.available.title"),
        description: t("updates.available.description", {
          currentVersion: update.currentVersion,
          latestVersion: update.latestVersion,
        }),
        viewLabel: t("updates.available.actions.view"),
        laterLabel: t("common.buttons.later"),
      });
    } catch (error) {
      console.error("Manual update check failed:", error);
      toast.error(t("about.update.failedTitle"), t("about.update.failedDescription"));
    } finally {
      setIsCheckingUpdates(false);
    }
  };

  const openExternal = async (url: string) => {
    try {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(url);
    } catch {
      window.open(url, "_blank");
    }
  };

  const handleEnableDeveloperMode = async () => {
    try {
      const settings = await readSettings();
      const advanced = ensureAdvancedSettings(settings);
      advanced.developerModeEnabled = true;
      setDeveloperModeOverride(true);
      await saveAdvancedSettings(advanced);
      setDeveloperModeEnabled(true);
      window.location.reload();
    } catch (error) {
      console.error("Failed to enable developer mode:", error);
      toast.error(t("about.errors.saveTitle"), t("about.errors.saveDescription"));
    }
  };

  return (
    <div className="flex h-full flex-col pb-16">
      <section className="flex-1 space-y-5 overflow-y-auto px-3 pt-3">
        <div className="rounded-xl border border-fg/10 bg-fg/5">
          <div className="border-b border-fg/8 px-4 py-4">
            <div className="flex items-start gap-4">
              <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-xl border border-fg/10 bg-surface/60">
                <img src={logoSvg} alt="LettuceAI" className="h-9 w-9" />
              </div>
              <div className="min-w-0 flex-1">
                <div className="text-lg font-semibold text-fg">{t("about.appName")}</div>
                <p className="mt-1 text-sm leading-6 text-fg/55">{t("about.description")}</p>
                <div className="mt-3 flex flex-wrap gap-2">
                  <div className="rounded-md border border-fg/10 bg-surface/55 px-2.5 py-1 text-xs font-medium text-fg/72">
                    {appVersion}
                  </div>
                  <div className="rounded-md border border-fg/10 bg-surface/55 px-2.5 py-1 text-xs font-medium text-fg/60">
                    {buildChannel}
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div className="px-4 py-4">
            <InfoRow label={t("about.info.version")} value={appVersion} />
            <InfoRow label={t("about.info.channel")} value={buildChannel} />
            <InfoRow label={t("about.info.platform")} value={platform.os} />
          </div>
        </div>

        <div>
          <SectionTitle title={t("about.update.sectionTitle")} />
          <div className="mt-2 rounded-xl border border-fg/10 bg-fg/5">
            <div className="border-b border-fg/8 px-4 py-4">
              <div className="flex items-start gap-3">
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-fg/10 bg-surface/60">
                  <RefreshCw className="h-4 w-4 text-fg/70" />
                </div>
                <div>
                  <div className="text-sm font-medium text-fg">{t("about.update.title")}</div>
                  <div className="mt-1 text-sm leading-6 text-fg/50">
                    {t("about.update.description")}
                  </div>
                </div>
              </div>
            </div>

            <div className="space-y-4 px-4 py-4">
              <div className="flex items-center justify-between gap-4 rounded-lg border border-fg/8 bg-surface/35 px-3 py-3">
                <div>
                  <div className="text-sm font-medium text-fg">{t("about.update.autoChecks")}</div>
                  <div className="mt-1 text-xs leading-5 text-fg/45">
                    {t("about.update.autoChecksDescription")}
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <span className="text-[11px] font-medium text-fg/50">
                    {autoChecksEnabled ? t("common.labels.on") : t("common.labels.off")}
                  </span>
                  <Switch
                    checked={autoChecksEnabled}
                    onChange={() => void toggleAutoChecks()}
                    aria-label={t("about.update.autoChecks")}
                  />
                </div>
              </div>

              {updateState === "available" && availableUpdate ? (
                <div className="rounded-lg border border-accent/18 bg-accent/7 px-3 py-3">
                  <div className="text-sm font-medium text-fg">{t("updates.available.title")}</div>
                  <div className="mt-1 text-xs leading-5 text-fg/50">
                    {t("updates.available.description", {
                      currentVersion: availableUpdate.currentVersion,
                      latestVersion: availableUpdate.latestVersion,
                    })}
                  </div>
                  <button
                    type="button"
                    onClick={() => void openExternal(availableUpdate.downloadUrl)}
                    className={cn(
                      "mt-3 inline-flex h-8 items-center gap-2 rounded-md border border-accent/18 bg-surface/45 px-3 text-sm font-medium text-fg",
                      interactive.transition.default,
                      "hover:bg-surface-el/55",
                    )}
                  >
                    {t("updates.available.actions.view")}
                    <ExternalLink className="h-3.5 w-3.5 text-fg/45" />
                  </button>
                </div>
              ) : null}

              {updateState === "upToDate" ? (
                <div className="rounded-lg border border-fg/8 bg-surface/30 px-3 py-3">
                  <div className="text-sm font-medium text-fg">
                    {t("about.update.upToDateTitle")}
                  </div>
                  <div className="mt-1 text-xs leading-5 text-fg/45">
                    {t("about.update.upToDateDescription")}
                  </div>
                </div>
              ) : null}

              <button
                type="button"
                onClick={handleCheckNow}
                disabled={isCheckingUpdates}
                className={cn(
                  "flex h-10 w-full items-center justify-center gap-2 rounded-lg border border-fg/12 bg-surface/55 text-sm font-medium text-fg",
                  interactive.transition.default,
                  "hover:bg-surface-el/60 disabled:cursor-wait disabled:opacity-60",
                )}
              >
                <RefreshCw className={cn("h-4 w-4", isCheckingUpdates && "animate-spin")} />
                {isCheckingUpdates ? t("about.update.checking") : t("about.update.checkNow")}
              </button>
            </div>
          </div>
        </div>

        <div>
          <SectionTitle title={t("about.links.sectionTitle")} />
          <div className="mt-2 rounded-xl border border-fg/10 bg-fg/5">
            {[
              {
                key: "website",
                icon: <Globe className="h-4 w-4" />,
                title: t("about.links.website"),
                subtitle: t("about.links.websiteDescription"),
                url: DOWNLOADS_PAGE_LINK,
              },
              {
                key: "github",
                icon: <img src={githubSvg} alt="" className="h-4 w-4" />,
                title: t("about.links.github"),
                subtitle: t("about.links.githubDescription"),
                url: GITHUB_REPO_LINK,
              },
              {
                key: "discord",
                icon: (
                  <svg viewBox="0 0 24 24" fill="currentColor" className="h-4 w-4">
                    <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515a.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0a12.64 12.64 0 0 0-.617-1.25a.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057a19.9 19.9 0 0 0 5.993 3.03a.078.078 0 0 0 .084-.028a14.09 14.09 0 0 0 1.226-1.994a.076.076 0 0 0-.041-.106a13.107 13.107 0 0 1-1.872-.892a.077.077 0 0 1-.008-.128a10.2 10.2 0 0 0 .372-.292a.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127a12.299 12.299 0 0 1-1.873.892a.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028a19.839 19.839 0 0 0 6.002-3.03a.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419c0-1.333.956-2.419 2.157-2.419c1.21 0 2.176 1.096 2.157 2.42c0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419c0-1.333.955-2.419 2.157-2.419c1.21 0 2.176 1.096 2.157 2.42c0 1.333-.946 2.418-2.157 2.418z" />
                  </svg>
                ),
                title: t("about.links.discord"),
                subtitle: t("about.links.discordDescription"),
                url: DISCORD_SERVER_LINK,
              },
              {
                key: "reddit",
                icon: (
                  <svg viewBox="0 0 24 24" fill="currentColor" className="h-4 w-4">
                    <path d="M14.19 14.79c.11.11.11.29 0 .4c-.54.54-1.42.8-2.69.8s-2.15-.26-2.69-.8a.283.283 0 1 1 .4-.4c.43.43 1.18.63 2.29.63s1.86-.2 2.29-.63a.283.283 0 0 1 .4 0M9.5 13.01c0-.55-.45-.99-1-.99s-1 .44-1 .99s.45 1 1 1s1-.45 1-1m5 0c0-.55-.45-.99-1-.99s-1 .44-1 .99s.45 1 1 1s1-.45 1-1m6.5-.99c0-1.54-1.25-2.79-2.79-2.79c-.76 0-1.45.3-1.95.79c-1.36-.93-3.17-1.52-5.16-1.58l.88-2.78l2.39.56a1.99 1.99 0 1 0 .33-1.11l-2.68-.63a.56.56 0 0 0-.67.37l-1.01 3.18c-2.08.03-3.98.62-5.39 1.58a2.79 2.79 0 0 0-4.74 2c0 .81.35 1.55.9 2.06a4.91 4.91 0 0 0-.1.97c0 2.95 3.39 5.35 7.56 5.35s7.56-2.4 7.56-5.35c0-.33-.04-.65-.1-.97c.56-.51.9-1.25.9-2.06" />
                  </svg>
                ),
                title: t("about.links.reddit"),
                subtitle: t("about.links.redditDescription"),
                url: SUBREDDIT_LINK,
              },
            ].map((item) => (
              <button
                key={item.key}
                type="button"
                onClick={() => void openExternal(item.url)}
                className={cn(
                  "group flex w-full items-center gap-3 border-b border-fg/8 px-4 py-3 text-left last:border-b-0",
                  interactive.transition.default,
                  "hover:bg-fg/4",
                  interactive.focus.ring,
                )}
              >
                <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-fg/10 bg-surface/55 text-fg/70">
                  {item.icon}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="text-sm font-medium text-fg">{item.title}</div>
                  <div className="mt-1 text-xs leading-5 text-fg/45">{item.subtitle}</div>
                </div>
                <ExternalLink className="h-4 w-4 shrink-0 text-fg/30" />
              </button>
            ))}
          </div>
        </div>

        <div className="pt-1">
          <button
            type="button"
            onClick={handleEnableDeveloperMode}
            disabled={developerModeEnabled}
            className={cn(
              "flex h-10 w-full items-center justify-center rounded-lg border text-sm font-medium",
              developerModeEnabled
                ? "border-warning/12 bg-warning/8 text-warning/70"
                : "border-warning/18 bg-surface/45 text-warning",
              interactive.transition.default,
              "hover:bg-surface-el/55 disabled:cursor-default",
            )}
          >
            {developerModeEnabled
              ? t("about.developerMode.enabled")
              : t("about.developerMode.enable")}
          </button>
        </div>
      </section>
    </div>
  );
}
