import { useNavigate } from "react-router-dom";
import { useMemo } from "react";
import {
  ChevronRight,
  Cpu,
  EthernetPort,
  Shield,
  RotateCcw,
  BookOpen,
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
  Image,
  Info,
} from "lucide-react";
import { typography, radius, spacing, interactive, cn } from "../../design-tokens";
import { useSettingsSummary } from "./hooks/useSettingsSummary";
import { isDevelopmentMode } from "../../../core/utils/env";
import { useNavigationManager } from "../../navigation";
import { useI18n } from "../../../core/i18n/context";

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
          className={cn("h-4 w-4 shrink-0 text-fg/30", "transition-colors group-hover:text-fg/60")}
        />
      </div>
    </button>
  );
}

export function SettingsPage() {
  const navigate = useNavigate();
  const { t } = useI18n();
  const { toModelsList } = useNavigationManager();
  const {
    state: { providers, models, characterCount, isLoading },
  } = useSettingsSummary();

  const providerCount = providers.length;
  const modelCount = models.length;
  const items = useMemo(
    () => [
      {
        key: "providers",
        icon: <EthernetPort />,
        title: t("settings.items.providers.title"),
        subtitle: t("settings.items.providers.subtitle"),
        count: providerCount,
        tone: "intelligence" as const,
        onClick: () => navigate("/settings/providers"),
      },
      {
        key: "models",
        icon: <Cpu />,
        title: t("settings.items.models.title"),
        subtitle: t("settings.items.models.subtitle"),
        count: modelCount,
        tone: "intelligence" as const,
        onClick: () => toModelsList(),
      },
      {
        key: "imageGeneration",
        icon: <Image />,
        title: t("settings.items.imageGeneration.title"),
        subtitle: t("settings.items.imageGeneration.subtitle"),
        tone: "intelligence" as const,
        onClick: () => navigate("/settings/image-generation"),
      },
      {
        key: "voices",
        icon: <Volume2 />,
        title: t("settings.items.voices.title"),
        subtitle: t("settings.items.voices.subtitle"),
        tone: "experience" as const,
        onClick: () => navigate("/settings/providers?tab=audio"),
      },
      {
        key: "accessibility",
        icon: <Accessibility />,
        title: t("settings.items.accessibility.title"),
        subtitle: t("settings.items.accessibility.subtitle"),
        tone: "experience" as const,
        onClick: () => navigate("/settings/accessibility"),
      },
      {
        key: "prompts",
        icon: <FileText />,
        title: t("settings.items.prompts.title"),
        subtitle: t("settings.items.prompts.subtitle"),
        tone: "intelligence" as const,
        onClick: () => navigate("/settings/prompts"),
      },
      {
        key: "security",
        icon: <Shield />,
        title: t("settings.items.security.title"),
        subtitle: t("settings.items.security.subtitle"),
        tone: "security" as const,
        onClick: () => navigate("/settings/security"),
      },
      {
        key: "backup",
        icon: <HardDrive />,
        title: t("settings.items.backup.title"),
        subtitle: t("settings.items.backup.subtitle"),
        tone: "connectivity" as const,
        onClick: () => navigate("/settings/backup"),
      },
      {
        key: "convert",
        icon: <ArrowLeftRight />,
        title: t("settings.items.convert.title"),
        subtitle: t("settings.items.convert.subtitle"),
        tone: "support" as const,
        onClick: () => navigate("/settings/convert"),
      },
      {
        key: "sync",
        icon: <RefreshCw />,
        title: t("settings.items.sync.title"),
        subtitle: t("settings.items.sync.subtitle"),
        tone: "connectivity" as const,
        onClick: () => navigate("/settings/sync"),
      },
      {
        key: "usage",
        icon: <BarChart3 />,
        title: t("settings.items.usage.title"),
        subtitle: t("settings.items.usage.subtitle"),
        tone: "security" as const,
        onClick: () => navigate("/settings/usage"),
      },
      {
        key: "advanced",
        icon: <Sliders />,
        title: t("settings.items.advanced.title"),
        subtitle: t("settings.items.advanced.subtitle"),
        tone: "intelligence" as const,
        onClick: () => navigate("/settings/advanced"),
      },
      {
        key: "about",
        icon: <Info />,
        title: t("settings.items.about.title"),
        subtitle: t("settings.items.about.subtitle"),
        tone: "support" as const,
        onClick: () => navigate("/settings/about"),
      },
      {
        key: "changelog",
        icon: <ScrollText />,
        title: t("settings.items.changelog.title"),
        subtitle: t("settings.items.changelog.subtitle"),
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
        key: "docs",
        icon: <HelpCircle />,
        title: t("settings.items.docs.title"),
        subtitle: t("settings.items.docs.subtitle"),
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
        key: "logs",
        icon: <FileCode />,
        title: t("settings.items.logs.title"),
        subtitle: t("settings.items.logs.subtitle"),
        tone: "support" as const,
        onClick: () => navigate("/settings/logs"),
      },
      {
        key: "guide",
        icon: <BookOpen />,
        title: t("settings.items.guide.title"),
        subtitle: t("settings.items.guide.subtitle"),
        tone: "support" as const,
        onClick: () => navigate("/welcome"),
      },
      {
        key: "reset",
        icon: <RotateCcw />,
        title: t("settings.items.reset.title"),
        subtitle: t("settings.items.reset.subtitle"),
        tone: "danger" as const,
        onClick: () => navigate("/settings/reset"),
      },
      ...(isDevelopmentMode()
        ? [
            {
              key: "developer",
              icon: <Wrench />,
              title: t("settings.items.developer.title"),
              subtitle: t("settings.items.developer.subtitle"),
              tone: "developer" as const,
              onClick: () => navigate("/settings/developer"),
            },
          ]
        : []),
    ],
    [providerCount, modelCount, characterCount, navigate, t, toModelsList],
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
            {t("settings.sections.intelligence")}
          </h2>
          <div className={spacing.field}>
            {items
              .filter((i) =>
                ["providers", "models", "imageGeneration", "prompts", "advanced"].includes(i.key),
              )
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
            {t("settings.sections.experience")}
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
            {t("settings.sections.connectivity")}
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
            {t("settings.sections.securityPrivacy")}
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
            {t("settings.sections.supportInfo")}
          </h2>
          <div className={spacing.field}>
            {items
              .filter((i) => ["about", "changelog", "docs", "logs", "guide"].includes(i.key))
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
            {t("settings.sections.dangerZone")}
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
              {t("settings.sections.developer")}
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
