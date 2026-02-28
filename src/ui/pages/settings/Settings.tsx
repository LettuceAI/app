import { useNavigate } from "react-router-dom";
import { useEffect, useMemo, useState } from "react";
import {
  ChevronRight,
  Cpu,
  EthernetPort,
  Shield,
  RotateCcw,
  BookOpen,
  Github,
  BarChart3,
  FileText,
  Wrench,
  ScrollText,
  Sliders,
  HardDrive,
  FileCode,
  RefreshCw,
  Volume2,
  Accessibility,
  HelpCircle,
  ArrowLeftRight,
} from "lucide-react";
import { typography, radius, spacing, interactive, cn } from "../../design-tokens";
import { useSettingsSummary } from "./hooks/useSettingsSummary";
import { isDevelopmentMode } from "../../../core/utils/env";
import { invoke } from "@tauri-apps/api/core";
import { DISCORD_SERVER_LINK, GITHUB_REPO_LINK } from "../../../core/utils/links";
import { useNavigationManager } from "../../navigation";

interface RowProps {
  icon: React.ReactNode;
  title: string;
  subtitle?: string;
  onClick: () => void;
  count?: number | null;
  tone?:
    | "default"
    | "danger"
    | "intelligence"
    | "experience"
    | "connectivity"
    | "security"
    | "support"
    | "developer";
}

function Row({ icon, title, subtitle, onClick, count, tone = "default" }: RowProps) {
  const toneStyles = {
    intelligence: "border-accent/30 bg-accent/15 text-accent group-hover:border-accent/50",
    experience: "border-warning/30 bg-warning/15 text-warning group-hover:border-warning/50",
    connectivity: "border-info/30 bg-info/15 text-info group-hover:border-info/50",
    security: "border-accent/30 bg-accent/15 text-accent group-hover:border-accent/50",
    support: "border-info/30 bg-info/15 text-info group-hover:border-info/50",
    danger: "border-danger/30 bg-danger/15 text-danger group-hover:border-danger/50",
    developer: "border-warning/30 bg-warning/15 text-warning group-hover:border-warning/50",
    default: "border-fg/10 bg-fg/10 text-fg/70 group-hover:border-fg/20",
  };

  return (
    <button
      onClick={onClick}
      className={cn(
        "group w-full px-4 py-3 text-left",
        radius.md,
        "border border-fg/10 bg-fg/5",
        interactive.transition.default,
        "hover:border-fg/20 hover:bg-fg/8",
        interactive.active.scale,
        interactive.focus.ring,
      )}
    >
      <div className="flex items-center gap-3">
        <div
          className={cn(
            "flex h-8 w-8 shrink-0 items-center justify-center",
            radius.full,
            "border text-fg/70",
            interactive.transition.default,
            toneStyles[tone],
          )}
        >
          <span className="[&_svg]:h-4 [&_svg]:w-4">{icon}</span>
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span
              className={cn("truncate", typography.body.size, typography.body.weight, "text-fg")}
            >
              {title}
            </span>
            {typeof count === "number" && (
              <span
                className={cn(
                  "px-1.5 py-0.5",
                  radius.sm,
                  "border border-fg/10 bg-fg/10",
                  typography.caption.size,
                  typography.caption.weight,
                  "leading-none text-fg/70",
                )}
              >
                {count}
              </span>
            )}
          </div>
          {subtitle && (
            <div className={cn("mt-0.5 line-clamp-1", typography.caption.size, "text-fg/45")}>
              {subtitle}
            </div>
          )}
        </div>
        <ChevronRight
          className={cn(
            "h-4 w-4 shrink-0 text-fg/30",
            "transition-colors group-hover:text-fg/60",
          )}
        />
      </div>
    </button>
  );
}

export function SettingsPage() {
  const navigate = useNavigate();
  const { toModelsList } = useNavigationManager();
  const {
    state: { providers, models, characterCount, isLoading },
  } = useSettingsSummary();
  const [version, setVersion] = useState<string>("loading...");

  useEffect(() => {
    let cancelled = false;

    const loadVersion = async () => {
      try {
        const appversion = await invoke<string>("get_app_version");
        if (!cancelled) {
          setVersion(appversion);
        }
      } catch (error) {
        console.error("Failed to get app version:", error);
        if (!cancelled) {
          setVersion("unknown");
        }
      }
    };

    loadVersion();

    return () => {
      cancelled = true;
    };
  }, []);

  const providerCount = providers.length;
  const modelCount = models.length;
  const items = useMemo(
    () => [
      {
        key: "providers",
        icon: <EthernetPort />,
        title: "Providers",
        subtitle: "Connect to AI services",
        count: providerCount,
        tone: "intelligence" as const,
        onClick: () => navigate("/settings/providers"),
      },
      {
        key: "models",
        icon: <Cpu />,
        title: "Models",
        subtitle: "Configure AI models",
        count: modelCount,
        tone: "intelligence" as const,
        onClick: () => toModelsList(),
      },
      {
        key: "voices",
        icon: <Volume2 />,
        title: "Voices",
        subtitle: "Text-to-speech voices",
        tone: "experience" as const,
        onClick: () => navigate("/settings/providers?tab=audio"),
      },
      {
        key: "accessibility",
        icon: <Accessibility />,
        title: "Accessibility",
        subtitle: "Sound cues & haptics",
        tone: "experience" as const,
        onClick: () => navigate("/settings/accessibility"),
      },
      {
        key: "prompts",
        icon: <FileText />,
        title: "System Prompts",
        subtitle: "Shape AI personality",
        tone: "intelligence" as const,
        onClick: () => navigate("/settings/prompts"),
      },
      {
        key: "security",
        icon: <Shield />,
        title: "Security",
        subtitle: "Encryption & privacy",
        tone: "security" as const,
        onClick: () => navigate("/settings/security"),
      },
      {
        key: "backup",
        icon: <HardDrive />,
        title: "Backup & Restore",
        subtitle: "Export or import data",
        tone: "connectivity" as const,
        onClick: () => navigate("/settings/backup"),
      },
      {
        key: "convert",
        icon: <ArrowLeftRight />,
        title: "Convert Files",
        subtitle: "Migrate legacy .json exports to .uec",
        tone: "support" as const,
        onClick: () => navigate("/settings/convert"),
      },
      {
        key: "sync",
        icon: <RefreshCw />,
        title: "Local Sync",
        subtitle: "Sync across devices",
        tone: "connectivity" as const,
        onClick: () => navigate("/settings/sync"),
      },
      {
        key: "usage",
        icon: <BarChart3 />,
        title: "Usage Analytics",
        subtitle: "Costs & token stats",
        tone: "security" as const,
        onClick: () => navigate("/settings/usage"),
      },
      {
        key: "advanced",
        icon: <Sliders />,
        title: "Advanced",
        subtitle: "Memory & features",
        tone: "intelligence" as const,
        onClick: () => navigate("/settings/advanced"),
      },
      {
        key: "logs",
        icon: <FileCode />,
        title: "Logs",
        subtitle: "Debug & diagnostics",
        tone: "support" as const,
        onClick: () => navigate("/settings/logs"),
      },
      {
        key: "guide",
        icon: <BookOpen />,
        title: "Setup Guide",
        subtitle: "Rerun onboarding",
        tone: "support" as const,
        onClick: () => navigate("/welcome"),
      },
      {
        key: "docs",
        icon: <HelpCircle />,
        title: "Documentation",
        subtitle: "Guides & reference",
        tone: "support" as const,
        onClick: async () => {
          try {
            const { openUrl } = await import("@tauri-apps/plugin-opener");
            await openUrl("https://www.lettuceai.app/docs");
          } catch (error) {
            console.error("Failed to open URL:", error);
            window.open("https://www.lettuceai.app/docs", "_blank");
          }
        },
      },
      {
        key: "github",
        icon: <Github />,
        title: "Report Issues",
        subtitle: `Bugs & feedback • v${version}`,
        tone: "support" as const,
        onClick: async () => {
          try {
            const { openUrl } = await import("@tauri-apps/plugin-opener");
            await openUrl(`${GITHUB_REPO_LINK}/issues`);
          } catch (error) {
            console.error("Failed to open URL:", error);
            window.open(`${GITHUB_REPO_LINK}/issues`, "_blank");
          }
        },
      },
      {
        key: "discord",
        icon: (
          <svg viewBox="0 0 24 24" fill="currentColor" className="w-5 h-5">
            <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515a.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0a12.64 12.64 0 0 0-.617-1.25a.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057a19.9 19.9 0 0 0 5.993 3.03a.078.078 0 0 0 .084-.028a14.09 14.09 0 0 0 1.226-1.994a.076.076 0 0 0-.041-.106a13.107 13.107 0 0 1-1.872-.892a.077.077 0 0 1-.008-.128a10.2 10.2 0 0 0 .372-.292a.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127a12.299 12.299 0 0 1-1.873.892a.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028a19.839 19.839 0 0 0 6.002-3.03a.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419c0-1.333.956-2.419 2.157-2.419c1.21 0 2.176 1.096 2.157 2.42c0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419c0-1.333.955-2.419 2.157-2.419c1.21 0 2.176 1.096 2.157 2.42c0 1.333-.946 2.418-2.157 2.418z" />
          </svg>
        ),
        title: "Join Discord",
        subtitle: "Community & help",
        tone: "support" as const,
        onClick: async () => {
          try {
            const { openUrl } = await import("@tauri-apps/plugin-opener");
            await openUrl(DISCORD_SERVER_LINK);
          } catch (error) {
            console.error("Failed to open URL:", error);
            window.open(DISCORD_SERVER_LINK, "_blank");
          }
        },
      },
      {
        key: "changelog",
        icon: <ScrollText />,
        title: "Changelog",
        subtitle: `What's new`,
        tone: "support" as const,
        onClick: async () => {
          try {
            const { openUrl } = await import("@tauri-apps/plugin-opener");
            await openUrl("https://www.lettuceai.app/changelog");
          } catch (error) {
            console.error("Failed to open URL:", error);
            window.open("https://www.lettuceai.app/changelog", "_blank");
          }
        },
      },
      {
        key: "reset",
        icon: <RotateCcw />,
        title: "Reset",
        subtitle: "Erase everything",
        tone: "danger" as const,
        onClick: () => navigate("/settings/reset"),
      },
      ...(isDevelopmentMode()
        ? [
            {
              key: "developer",
              icon: <Wrench />,
              title: "Developer Tools",
              subtitle: "Debug & testing",
              tone: "developer" as const,
              onClick: () => navigate("/settings/developer"),
            },
          ]
        : []),
    ],
    [providerCount, modelCount, characterCount, navigate, version],
  );

  return (
    <div className="flex h-full flex-col pb-16 text-fg/90">
      <section className={cn("flex-1 overflow-y-auto px-1 pt-4", spacing.section)}>
        {/* Section: Intelligence */}
        <div>
          <h2
            className={cn(
              "mb-2 px-1",
              typography.overline.size,
              typography.overline.weight,
              typography.overline.tracking,
              typography.overline.transform,
              "text-fg/35",
            )}
          >
            Intelligence
          </h2>
          <div className={spacing.field}>
            {items
              .filter((i) => ["providers", "models", "prompts", "advanced"].includes(i.key))
              .map((item) => (
                <Row
                  key={item.key}
                  icon={item.icon}
                  title={item.title}
                  subtitle={item.subtitle}
                  count={item.count as number | undefined}
                  onClick={item.onClick}
                  tone={item.tone}
                />
              ))}
          </div>
        </div>

        {/* Section: Experience */}
        <div>
          <h2
            className={cn(
              "mb-2 px-1",
              typography.overline.size,
              typography.overline.weight,
              typography.overline.tracking,
              typography.overline.transform,
              "text-fg/35",
            )}
          >
            Experience
          </h2>
          <div className={spacing.field}>
            {items
              .filter((i) => ["voices", "accessibility"].includes(i.key))
              .map((item) => (
                <Row
                  key={item.key}
                  icon={item.icon}
                  title={item.title}
                  subtitle={item.subtitle}
                  count={item.count as number | undefined}
                  onClick={item.onClick}
                  tone={item.tone}
                />
              ))}
          </div>
        </div>

        {/* Section: Connectivity */}
        <div>
          <h2
            className={cn(
              "mb-2 px-1",
              typography.overline.size,
              typography.overline.weight,
              typography.overline.tracking,
              typography.overline.transform,
              "text-fg/35",
            )}
          >
            Connectivity
          </h2>
          <div className={spacing.field}>
            {items
              .filter((i) => ["sync", "backup", "convert"].includes(i.key))
              .map((item) => (
                <Row
                  key={item.key}
                  icon={item.icon}
                  title={item.title}
                  subtitle={item.subtitle}
                  count={item.count as number | undefined}
                  onClick={item.onClick}
                  tone={item.tone}
                />
              ))}
          </div>
        </div>

        {/* Section: Security & Privacy */}
        <div>
          <h2
            className={cn(
              "mb-2 px-1",
              typography.overline.size,
              typography.overline.weight,
              typography.overline.tracking,
              typography.overline.transform,
              "text-fg/35",
            )}
          >
            Security & Privacy
          </h2>
          <div className={spacing.field}>
            {items
              .filter((i) => ["security", "usage"].includes(i.key))
              .map((item) => (
                <Row
                  key={item.key}
                  icon={item.icon}
                  title={item.title}
                  subtitle={item.subtitle}
                  count={item.count as number | undefined}
                  onClick={item.onClick}
                  tone={item.tone}
                />
              ))}
          </div>
        </div>

        {/* Section: Support & Info */}
        <div>
          <h2
            className={cn(
              "mb-2 px-1",
              typography.overline.size,
              typography.overline.weight,
              typography.overline.tracking,
              typography.overline.transform,
              "text-fg/35",
            )}
          >
            Support & Info
          </h2>
          <div className={spacing.field}>
            {items
              .filter((i) =>
                ["guide", "docs", "changelog", "logs", "github", "discord"].includes(i.key),
              )
              .map((item) => (
                <Row
                  key={item.key}
                  icon={item.icon}
                  title={item.title}
                  subtitle={item.subtitle}
                  onClick={item.onClick}
                  tone={item.tone}
                />
              ))}
          </div>
        </div>

        {/* Section: Danger Zone */}
        <div>
          <h2
            className={cn(
              "mb-2 px-1",
              typography.overline.size,
              typography.overline.weight,
              typography.overline.tracking,
              typography.overline.transform,
              "text-fg/35",
            )}
          >
            Danger Zone
          </h2>
          <div className={spacing.field}>
            {items
              .filter((i) => ["reset"].includes(i.key))
              .map((item) => (
                <Row
                  key={item.key}
                  icon={item.icon}
                  title={item.title}
                  subtitle={item.subtitle}
                  onClick={item.onClick}
                  tone={item.tone}
                />
              ))}
          </div>
        </div>

        {/* Section: Developer (only in dev mode) */}
        {isDevelopmentMode() && (
          <div>
            <h2
              className={cn(
                "mb-2 px-1",
                typography.overline.size,
                typography.overline.weight,
                typography.overline.tracking,
                typography.overline.transform,
                "text-fg/35",
              )}
            >
              Developer
            </h2>
            <div className={spacing.field}>
              {items
                .filter((i) => ["developer"].includes(i.key))
                .map((item) => (
                  <Row
                    key={item.key}
                    icon={item.icon}
                    title={item.title}
                    subtitle={item.subtitle}
                    onClick={item.onClick}
                    tone={item.tone}
                  />
                ))}
            </div>
          </div>
        )}

        {/* Loading overlay */}
        {isLoading && (
          <div className="pointer-events-none absolute inset-x-0 top-0 px-4 pt-4">
            <div className={spacing.field}>
              {Array.from({ length: 5 }).map((_, i) => (
                <div key={i} className={cn("h-13 w-full animate-pulse", radius.md, "bg-fg/5")} />
              ))}
            </div>
          </div>
        )}
      </section>
    </div>
  );
}
