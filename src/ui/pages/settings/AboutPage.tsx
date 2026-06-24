import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ExternalLink, Globe, RefreshCw, ScrollText, Sparkles } from "lucide-react";

import githubSvg from "../../../assets/github.svg";
import logoSvg from "../../../assets/logo.svg";
import {
  checkForAppUpdate,
  detectUpdateChannel,
  type AppUpdateInfo,
} from "../../../core/app-updates/checkForAppUpdate";
import { presentAppUpdateToast } from "../../../core/app-updates/presentAppUpdateToast";
import { useI18n } from "../../../core/i18n/context";
import { readSettings, saveAdvancedSettings } from "../../../core/storage/repo";
import { type Settings } from "../../../core/storage/schemas";
import {
  DISCORD_SERVER_LINK,
  DOWNLOADS_PAGE_LINK,
  GITHUB_REPO_LINK,
} from "../../../core/utils/links";
import { getPlatform } from "../../../core/utils/platform";
import { openExternalUrl } from "../../../core/utils/openExternal";
import { isDevelopmentMode, setDeveloperModeOverride } from "../../../core/utils/env";
import { toast } from "../../components/toast";
import { cn, interactive, typography } from "../../design-tokens";
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

function GroupLabel({ children }: { children: React.ReactNode }) {
  return (
    <h2
      className={cn(
        "px-1",
        typography.overline.size,
        typography.overline.weight,
        typography.overline.tracking,
        typography.overline.transform,
        "text-fg/40",
      )}
    >
      {children}
    </h2>
  );
}

function Group({ children }: { children: React.ReactNode }) {
  return (
    <div
      className={cn(
        "overflow-hidden rounded-xl",
        "border border-fg/10 bg-fg/[0.025]",
        "divide-y divide-fg/[0.06]",
      )}
    >
      {children}
    </div>
  );
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-4 px-4 py-3">
      <span className={cn(typography.body.size, "text-fg/55")}>{label}</span>
      <span className={cn(typography.body.size, "font-medium text-fg tabular-nums")}>{value}</span>
    </div>
  );
}

interface LinkRowProps {
  icon: React.ReactNode;
  title: string;
  subtitle: string;
  onClick: () => void;
}

function LinkRow({ icon, title, subtitle, onClick }: LinkRowProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "group flex w-full items-center gap-3 px-4 py-3 text-left",
        "transition-colors duration-150",
        "hover:bg-fg/[0.04] focus:bg-fg/[0.04]",
        "focus:outline-none",
      )}
    >
      <span className="flex h-7 w-7 shrink-0 items-center justify-center text-fg/55 transition-colors group-hover:text-fg/80">
        {icon}
      </span>
      <span className="min-w-0 flex-1">
        <span className={cn("block truncate", typography.body.size, "font-medium text-fg")}>
          {title}
        </span>
        <span
          className={cn(
            "mt-0.5 block line-clamp-1",
            typography.caption.size,
            "font-normal text-fg/45",
          )}
        >
          {subtitle}
        </span>
      </span>
      <ExternalLink className="h-4 w-4 shrink-0 text-fg/25 transition-colors group-hover:text-fg/45" />
    </button>
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

  const isDevBuild = import.meta.env.DEV;
  const isDevChannel = isDevBuild || detectUpdateChannel(appVersion) === "dev";
  const buildChannel = isDevChannel
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

  const openExternal = (url: string) => openExternalUrl(url);

  const openChangelog = () => {
    void openExternal("https://www.lettuceai.app/changelog");
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
    <div className="flex h-full flex-col">
      <section className="flex-1 overflow-y-auto">
        <div className="mx-auto w-full max-w-5xl px-3 pt-4 pb-10 sm:px-4 lg:px-8 lg:pt-8">
          <div className="flex flex-col gap-7">
            {/* Hero card */}
            <div
              className={cn(
                "relative overflow-hidden rounded-2xl border border-fg/10 bg-fg/[0.03]",
                "px-5 py-6 sm:px-7 sm:py-7",
              )}
            >
              <div
                aria-hidden
                className="pointer-events-none absolute -top-16 -right-12 h-48 w-48 rounded-full bg-accent/10 blur-3xl"
              />
              <div className="relative flex flex-col gap-5 sm:flex-row sm:items-start sm:gap-5">
                <div className="flex h-16 w-16 shrink-0 items-center justify-center rounded-2xl border border-fg/10 bg-surface/60 shadow-sm">
                  <img src={logoSvg} alt="LettuceAI" className="h-10 w-10" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <h1
                      className={cn(
                        typography.h1.size,
                        typography.h1.weight,
                        typography.h1.tracking,
                        "text-fg",
                      )}
                    >
                      {t("about.appName")}
                    </h1>
                    <span
                      className={cn(
                        "shrink-0 rounded-md border border-fg/10 bg-surface/55 px-2 py-0.5",
                        typography.caption.size,
                        "font-medium tabular-nums text-fg/65",
                      )}
                    >
                      {isDevBuild ? t("about.devVersion") : appVersion}
                    </span>
                    {isDevChannel && (
                      <span
                        className={cn(
                          "shrink-0 rounded-md border border-warning/25 bg-warning/12 px-2 py-0.5",
                          typography.caption.size,
                          "font-medium uppercase tracking-wide text-warning",
                        )}
                      >
                        {buildChannel}
                      </span>
                    )}
                  </div>
                  <p className={cn("mt-2", typography.body.size, "leading-relaxed text-fg/60")}>
                    {t("about.description")}
                  </p>

                  <div className="mt-5 flex flex-wrap gap-2">
                    <button
                      type="button"
                      onClick={handleCheckNow}
                      disabled={isCheckingUpdates}
                      className={cn(
                        "inline-flex h-9 items-center gap-2 rounded-lg border border-fg/12 bg-surface/60 px-3.5",
                        typography.body.size,
                        "font-medium text-fg",
                        interactive.transition.default,
                        "hover:bg-surface-el/65 disabled:cursor-wait disabled:opacity-60",
                      )}
                    >
                      <RefreshCw
                        className={cn("h-4 w-4 text-fg/70", isCheckingUpdates && "animate-spin")}
                      />
                      {isCheckingUpdates
                        ? t("about.update.checking")
                        : t("about.update.checkNow")}
                    </button>
                    <button
                      type="button"
                      onClick={openChangelog}
                      className={cn(
                        "inline-flex h-9 items-center gap-2 rounded-lg border border-fg/10 bg-transparent px-3.5",
                        typography.body.size,
                        "font-medium text-fg/75",
                        interactive.transition.default,
                        "hover:bg-fg/[0.04] hover:text-fg",
                      )}
                    >
                      <ScrollText className="h-4 w-4 text-fg/55" />
                      {t("settings.items.changelog.title")}
                    </button>
                  </div>
                </div>
              </div>
            </div>

            {/* Update status banner (only when an update is available or up to date) */}
            {updateState === "available" && availableUpdate && (
              <div className="rounded-xl border border-accent/20 bg-accent/8 px-4 py-3">
                <div className={cn(typography.body.size, "font-medium text-fg")}>
                  {t("updates.available.title")}
                </div>
                <div className={cn("mt-1", typography.caption.size, "leading-5 text-fg/55")}>
                  {t("updates.available.description", {
                    currentVersion: availableUpdate.currentVersion,
                    latestVersion: availableUpdate.latestVersion,
                  })}
                </div>
                <button
                  type="button"
                  onClick={() => void openExternal(availableUpdate.downloadUrl)}
                  className={cn(
                    "mt-3 inline-flex h-8 items-center gap-2 rounded-md border border-accent/25 bg-surface/55 px-3",
                    typography.body.size,
                    "font-medium text-fg",
                    interactive.transition.default,
                    "hover:bg-surface-el/60",
                  )}
                >
                  {t("updates.available.actions.view")}
                  <ExternalLink className="h-3.5 w-3.5 text-fg/55" />
                </button>
              </div>
            )}

            {updateState === "upToDate" && (
              <div className="rounded-xl border border-fg/10 bg-fg/[0.025] px-4 py-3">
                <div className={cn(typography.body.size, "font-medium text-fg")}>
                  {t("about.update.upToDateTitle")}
                </div>
                <div className={cn("mt-1", typography.caption.size, "leading-5 text-fg/45")}>
                  {t("about.update.upToDateDescription")}
                </div>
              </div>
            )}

            {/* Two-column grid at lg+ */}
            <div className="grid grid-cols-1 gap-7 lg:grid-cols-2">
              {/* Left column: Information + Updates */}
              <div className="flex flex-col gap-7">
                <div className="flex flex-col gap-2">
                  <GroupLabel>{t("about.sections.information")}</GroupLabel>
                  <Group>
                    <InfoRow
                      label={t("about.info.version")}
                      value={isDevBuild ? t("about.devVersion") : appVersion}
                    />
                    <InfoRow label={t("about.info.channel")} value={buildChannel} />
                    <InfoRow label={t("about.info.platform")} value={platform.os} />
                  </Group>
                </div>

                <div className="flex flex-col gap-2">
                  <GroupLabel>{t("about.update.sectionTitle")}</GroupLabel>
                  <Group>
                    <div className="flex items-center justify-between gap-4 px-4 py-3">
                      <div className="min-w-0">
                        <div className={cn(typography.body.size, "font-medium text-fg")}>
                          {t("about.update.autoChecks")}
                        </div>
                        <div className={cn("mt-0.5", typography.caption.size, "text-fg/45")}>
                          {t("about.update.autoChecksDescription")}
                        </div>
                      </div>
                      <Switch
                        checked={autoChecksEnabled}
                        onChange={() => void toggleAutoChecks()}
                        aria-label={t("about.update.autoChecks")}
                      />
                    </div>
                  </Group>
                </div>
              </div>

              {/* Right column: Links + Advanced */}
              <div className="flex flex-col gap-7">
                <div className="flex flex-col gap-2">
                  <GroupLabel>{t("about.links.sectionTitle")}</GroupLabel>
                  <Group>
                <LinkRow
                  icon={<Globe className="h-[18px] w-[18px]" />}
                  title={t("about.links.website")}
                  subtitle={t("about.links.websiteDescription")}
                  onClick={() => void openExternal(DOWNLOADS_PAGE_LINK)}
                />
                <LinkRow
                  icon={<img src={githubSvg} alt="" className="h-[18px] w-[18px]" />}
                  title={t("about.links.github")}
                  subtitle={t("about.links.githubDescription")}
                  onClick={() => void openExternal(GITHUB_REPO_LINK)}
                />
                <LinkRow
                  icon={
                    <svg viewBox="0 0 24 24" fill="currentColor" className="h-[18px] w-[18px]">
                      <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515a.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0a12.64 12.64 0 0 0-.617-1.25a.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057a19.9 19.9 0 0 0 5.993 3.03a.078.078 0 0 0 .084-.028a14.09 14.09 0 0 0 1.226-1.994a.076.076 0 0 0-.041-.106a13.107 13.107 0 0 1-1.872-.892a.077.077 0 0 1-.008-.128a10.2 10.2 0 0 0 .372-.292a.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127a12.299 12.299 0 0 1-1.873.892a.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028a19.839 19.839 0 0 0 6.002-3.03a.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419c0-1.333.956-2.419 2.157-2.419c1.21 0 2.176 1.096 2.157 2.42c0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419c0-1.333.955-2.419 2.157-2.419c1.21 0 2.176 1.096 2.157 2.42c0 1.333-.946 2.418-2.157 2.418z" />
                    </svg>
                  }
                  title={t("about.links.discord")}
                  subtitle={t("about.links.discordDescription")}
                  onClick={() => void openExternal(DISCORD_SERVER_LINK)}
                />
                  </Group>
                </div>

                <div className="flex flex-col gap-2">
                  <GroupLabel>{t("about.sections.advanced")}</GroupLabel>
                  <Group>
                    <button
                      type="button"
                      onClick={handleEnableDeveloperMode}
                      disabled={developerModeEnabled}
                      className={cn(
                        "group flex w-full items-center gap-3 px-4 py-3 text-left",
                        "transition-colors duration-150",
                        !developerModeEnabled && "hover:bg-fg/[0.04]",
                        "disabled:cursor-default",
                        "focus:outline-none focus:bg-fg/[0.04]",
                      )}
                    >
                      <span
                        className={cn(
                          "flex h-7 w-7 shrink-0 items-center justify-center",
                          developerModeEnabled ? "text-warning/60" : "text-warning",
                        )}
                      >
                        <Sparkles className="h-[18px] w-[18px]" />
                      </span>
                      <span className="min-w-0 flex-1">
                        <span
                          className={cn(
                            "block truncate",
                            typography.body.size,
                            "font-medium text-fg",
                          )}
                        >
                          {developerModeEnabled
                            ? t("about.developerMode.enabled")
                            : t("about.developerMode.enable")}
                        </span>
                        <span
                          className={cn(
                            "mt-0.5 block line-clamp-1",
                            typography.caption.size,
                            "font-normal text-fg/45",
                          )}
                        >
                          {developerModeEnabled
                            ? t("about.developerMode.enabledDescription")
                            : t("about.developerMode.enableDescription")}
                        </span>
                      </span>
                    </button>
                  </Group>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}
