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
  Palette,
  Mic,
  HelpCircle,
  ArrowLeftRight,
  Image,
  Info,
  Sparkles,
} from "lucide-react";
import { typography, cn } from "../../design-tokens";
import { useSettingsSummary } from "./hooks/useSettingsSummary";
import { isDevelopmentMode } from "../../../core/utils/env";
import { openExternalUrl } from "../../../core/utils/openExternal";
import { useNavigationManager } from "../../navigation";
import { useI18n } from "../../../core/i18n/context";

interface Item {
  key: string;
  icon: React.ReactNode;
  title: string;
  subtitle?: string;
  count?: number | null;
  danger?: boolean;
  onClick: () => void;
}

interface RowProps extends Omit<Item, "key"> {}

function Row({ icon, title, subtitle, count, danger, onClick }: RowProps) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "group relative flex w-full items-center gap-3 px-4 py-3 text-left",
        "transition-colors duration-150",
        "hover:bg-fg/[0.04] focus:bg-fg/[0.04]",
        "focus:outline-none",
      )}
    >
      <span
        className={cn(
          "flex h-7 w-7 shrink-0 items-center justify-center",
          danger ? "text-danger" : "text-fg/55",
          "transition-colors group-hover:text-fg/80",
          danger && "group-hover:text-danger",
        )}
      >
        <span className="[&_svg]:h-[18px] [&_svg]:w-[18px]">{icon}</span>
      </span>
      <span className="min-w-0 flex-1">
        <span
          className={cn(
            "truncate block",
            typography.body.size,
            "font-medium",
            danger ? "text-danger" : "text-fg",
          )}
        >
          {title}
        </span>
        {subtitle && (
          <span
            className={cn(
              "mt-0.5 line-clamp-1 block",
              typography.caption.size,
              "font-normal text-fg/45",
            )}
          >
            {subtitle}
          </span>
        )}
      </span>
      {typeof count === "number" && (
        <span
          className={cn(
            "shrink-0 tabular-nums",
            typography.caption.size,
            "font-medium text-fg/45",
          )}
        >
          {count}
        </span>
      )}
      <ChevronRight className="h-4 w-4 shrink-0 text-fg/25 transition-colors group-hover:text-fg/45" />
    </button>
  );
}

interface SectionProps {
  label: string;
  items: Item[];
}

function Section({ label, items }: SectionProps) {
  if (items.length === 0) return null;
  return (
    <section className="flex flex-col gap-2">
      <h2 className={cn("px-1", typography.bodySmall.size, "font-medium text-fg/50")}>
        {label}
      </h2>
      <div
        className={cn(
          "overflow-hidden rounded-xl",
          "border border-fg/10 bg-fg/[0.025]",
          "divide-y divide-fg/[0.06]",
        )}
      >
        {items.map((item) => (
          <Row
            key={item.key}
            icon={item.icon}
            title={item.title}
            subtitle={item.subtitle}
            count={item.count ?? undefined}
            danger={item.danger}
            onClick={item.onClick}
          />
        ))}
      </div>
    </section>
  );
}

export function SettingsPage() {
  const navigate = useNavigate();
  const { t } = useI18n();
  const { toModelsList } = useNavigationManager();
  const {
    state: { providers, models, characterCount },
  } = useSettingsSummary();

  const providerCount = providers.length;
  const modelCount = models.length;

  const itemsByKey = useMemo<Record<string, Item>>(
    () => ({
      providers: {
        key: "providers",
        icon: <EthernetPort />,
        title: t("settings.items.providers.title"),
        subtitle: t("settings.items.providers.subtitle"),
        count: providerCount,
        onClick: () => navigate("/settings/providers"),
      },
      models: {
        key: "models",
        icon: <Cpu />,
        title: t("settings.items.models.title"),
        subtitle: t("settings.items.models.subtitle"),
        count: modelCount,
        onClick: () => toModelsList(),
      },
      imageGeneration: {
        key: "imageGeneration",
        icon: <Image />,
        title: t("settings.items.imageGeneration.title"),
        subtitle: t("settings.items.imageGeneration.subtitle"),
        onClick: () => navigate("/settings/image-generation"),
      },
      prompts: {
        key: "prompts",
        icon: <FileText />,
        title: t("settings.items.prompts.title"),
        subtitle: t("settings.items.prompts.subtitle"),
        onClick: () => navigate("/settings/prompts"),
      },
      advanced: {
        key: "advanced",
        icon: <Sliders />,
        title: t("settings.items.advanced.title"),
        subtitle: t("settings.items.advanced.subtitle"),
        onClick: () => navigate("/settings/advanced"),
      },
      voices: {
        key: "voices",
        icon: <Volume2 />,
        title: t("settings.items.voices.title"),
        subtitle: t("settings.items.voices.subtitle"),
        onClick: () => navigate("/settings/providers?tab=audio"),
      },
      customization: {
        key: "customization",
        icon: <Palette />,
        title: t("settings.items.accessibility.title"),
        subtitle: t("settings.items.accessibility.subtitle"),
        onClick: () => navigate("/settings/customization"),
      },
      speechRecognition: {
        key: "speechRecognition",
        icon: <Mic />,
        title: t("settings.items.speechRecognition.title"),
        subtitle: t("settings.items.speechRecognition.subtitle"),
        onClick: () => navigate("/settings/speech-recognition"),
      },
      sync: {
        key: "sync",
        icon: <RefreshCw />,
        title: t("settings.items.sync.title"),
        subtitle: t("settings.items.sync.subtitle"),
        onClick: () => navigate("/settings/sync"),
      },
      backup: {
        key: "backup",
        icon: <HardDrive />,
        title: t("settings.items.backup.title"),
        subtitle: t("settings.items.backup.subtitle"),
        onClick: () => navigate("/settings/backup"),
      },
      convert: {
        key: "convert",
        icon: <ArrowLeftRight />,
        title: t("settings.items.convert.title"),
        subtitle: t("settings.items.convert.subtitle"),
        onClick: () => void openExternalUrl("https://www.lettuceai.app/convert"),
      },
      security: {
        key: "security",
        icon: <Shield />,
        title: t("settings.items.security.title"),
        subtitle: t("settings.items.security.subtitle"),
        onClick: () => navigate("/settings/security"),
      },
      usage: {
        key: "usage",
        icon: <BarChart3 />,
        title: t("settings.items.usage.title"),
        subtitle: t("settings.items.usage.subtitle"),
        onClick: () => navigate("/settings/usage"),
      },
      help: {
        key: "help",
        icon: <HelpCircle />,
        title: t("settings.items.help.title"),
        subtitle: t("settings.items.help.subtitle"),
        onClick: () => navigate("/settings/help"),
      },
      whatsNew: {
        key: "whatsNew",
        icon: <Sparkles />,
        title: t("settings.items.whatsNew.title"),
        subtitle: t("settings.items.whatsNew.subtitle"),
        onClick: () => window.dispatchEvent(new Event("whatsnew:open")),
      },
      about: {
        key: "about",
        icon: <Info />,
        title: t("settings.items.about.title"),
        subtitle: t("settings.items.about.subtitle"),
        onClick: () => navigate("/settings/about"),
      },
      changelog: {
        key: "changelog",
        icon: <ScrollText />,
        title: t("settings.items.changelog.title"),
        subtitle: t("settings.items.changelog.subtitle"),
        onClick: () => void openExternalUrl("https://www.lettuceai.app/changelog"),
      },
      docs: {
        key: "docs",
        icon: <HelpCircle />,
        title: t("settings.items.docs.title"),
        subtitle: t("settings.items.docs.subtitle"),
        onClick: () => void openExternalUrl("https://www.lettuceai.app/docs"),
      },
      logs: {
        key: "logs",
        icon: <FileCode />,
        title: t("settings.items.logs.title"),
        subtitle: t("settings.items.logs.subtitle"),
        onClick: () => navigate("/settings/logs"),
      },
      guide: {
        key: "guide",
        icon: <BookOpen />,
        title: t("settings.items.guide.title"),
        subtitle: t("settings.items.guide.subtitle"),
        onClick: () => navigate("/welcome"),
      },
      reset: {
        key: "reset",
        icon: <RotateCcw />,
        title: t("settings.items.reset.title"),
        subtitle: t("settings.items.reset.subtitle"),
        danger: true,
        onClick: () => navigate("/settings/reset"),
      },
      developer: {
        key: "developer",
        icon: <Wrench />,
        title: t("settings.items.developer.title"),
        subtitle: t("settings.items.developer.subtitle"),
        onClick: () => navigate("/settings/developer"),
      },
    }),
    [providerCount, modelCount, characterCount, navigate, t, toModelsList],
  );

  const sections = useMemo(
    () => [
      {
        key: "intelligence",
        label: t("settings.sections.intelligence"),
        keys: ["providers", "models", "imageGeneration", "prompts", "advanced"],
      },
      {
        key: "experience",
        label: t("settings.sections.experience"),
        keys: ["voices", "customization", "speechRecognition"],
      },
      {
        key: "connectivity",
        label: t("settings.sections.connectivity"),
        keys: ["sync", "backup", "convert"],
      },
      {
        key: "security",
        label: t("settings.sections.securityPrivacy"),
        keys: ["security", "usage"],
      },
      {
        key: "support",
        label: t("settings.sections.supportInfo"),
        keys: ["help", "whatsNew", "about", "changelog", "docs", "logs", "guide"],
      },
      {
        key: "danger",
        label: t("settings.sections.dangerZone"),
        keys: ["reset"],
      },
      ...(isDevelopmentMode()
        ? [
            {
              key: "developer",
              label: t("settings.sections.developer"),
              keys: ["developer"],
            },
          ]
        : []),
    ],
    [t],
  );

  return (
    <div className="flex h-full flex-col text-fg/90 lg:hidden">
      <div className="flex-1 overflow-y-auto">
        <div className="mx-auto w-full max-w-2xl px-3 pt-4 pb-16 sm:px-4">
          <div className="flex flex-col gap-7">
            {sections.map((section) => {
              const items = section.keys
                .map((k) => itemsByKey[k])
                .filter((x): x is Item => Boolean(x));
              return <Section key={section.key} label={section.label} items={items} />;
            })}
          </div>
        </div>
      </div>
    </div>
  );
}
