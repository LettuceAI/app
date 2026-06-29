import { useNavigate, useLocation, Outlet } from "react-router-dom";
import { useEffect, useMemo, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";

const SIDEBAR_COLLAPSED_KEY = "settings.sidebar.collapsed";
import {
  Cpu,
  EthernetPort,
  Shield,
  RotateCcw,
  BookOpen,
  Activity,
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
  Image as ImageIcon,
  Info,
  Sparkles,
  PenLine,
  Heart,
  Network,
  PanelLeftClose,
  PanelLeftOpen,
} from "lucide-react";
import { typography, cn } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import { useSettingsSummary } from "./hooks/useSettingsSummary";
import { hasLlmMetrics } from "../../../core/storage/metrics";
import { useNavigationManager } from "../../navigation";
import { isDevelopmentMode } from "../../../core/utils/env";
import { openExternalUrl } from "../../../core/utils/openExternal";

interface NavItem {
  key: string;
  icon: React.ReactNode;
  label: string;
  /** Used to determine active state via pathname match. Omit for items that don't route. */
  matchPath?: string;
  /** Additional pathname prefixes that should also activate this item. */
  extraMatchPaths?: string[];
  /** Optional override: when present, this fully decides active state. */
  isActive?: (pathname: string, search: string) => boolean;
  /** When another item's `isActive` returns true for the same path, this item should yield. */
  yieldsTo?: (pathname: string, search: string) => boolean;
  /** Action when clicked — navigation, modelsList helper, or external link. */
  onSelect: () => void;
  count?: number;
  danger?: boolean;
  /** Sub-items shown indented under this item when it (or one of them) is active. */
  children?: NavItem[];
}

interface NavGroup {
  key: string;
  label?: string;
  items: NavItem[];
}

function NavButton({
  item,
  active,
  nested = false,
  collapsed = false,
}: {
  item: NavItem;
  active: boolean;
  nested?: boolean;
  collapsed?: boolean;
}) {
  return (
    <button
      onClick={item.onSelect}
      title={collapsed ? item.label : undefined}
      aria-label={item.label}
      className={cn(
        "group relative flex w-full items-center gap-2.5 rounded-md text-left",
        nested ? "px-2.5 py-1.5 pl-9" : "px-2.5 py-2.25",
        "transition-colors duration-150",
        "focus:outline-none focus-visible:ring-1 focus-visible:ring-fg/20",
        active
          ? "text-fg"
          : item.danger
            ? "text-danger/75 hover:bg-danger/10 hover:text-danger"
            : "text-fg/60 hover:bg-fg/4 hover:text-fg",
      )}
    >
      {active && (
        <motion.span
          aria-hidden
          layoutId={nested ? "settings-nav-pill-nested" : "settings-nav-pill"}
          className={cn(
            "absolute inset-0 rounded-md",
            nested
              ? "bg-fg/6"
              : "bg-fg/8 shadow-[inset_0_0_0_1px_rgba(255,255,255,0.04)]",
          )}
          transition={{ type: "spring", stiffness: 420, damping: 34, mass: 0.6 }}
        />
      )}
      {active && !nested && (
        <motion.span
          aria-hidden
          layoutId="settings-nav-accent"
          className="absolute left-0 top-1/2 -translate-y-1/2 h-4 w-0.5 rounded-full bg-accent"
          transition={{ type: "spring", stiffness: 420, damping: 34, mass: 0.6 }}
        />
      )}
      <span
        className={cn(
          "relative flex shrink-0 items-center justify-center transition-colors",
          nested
            ? "h-4.5 w-4.5 [&_svg]:h-3.75 `[&_svg]:w-3.75"
            : "h-6 w-6 [&_svg]:h-4.5 [&_svg]:w-4.5",
          active
            ? nested
              ? "text-fg/85"
              : "text-accent"
            : item.danger
              ? ""
              : "text-fg/40 group-hover:text-fg/70",
        )}
      >
        {item.icon}
      </span>
      <span
        className={cn(
          "relative flex-1 truncate whitespace-nowrap transition-opacity duration-150",
          nested ? typography.caption.size : typography.body.size,
          active ? "font-semibold" : "font-medium",
          "tracking-[-0.005em]",
          collapsed && "opacity-0",
        )}
      >
        {item.label}
      </span>
      {typeof item.count === "number" && (
        <span
          className={cn(
            "relative shrink-0 tabular-nums rounded-full px-1.5 py-px",
            "text-[10px] font-semibold leading-tight transition-opacity duration-150",
            active ? "bg-fg/10 text-fg/80" : "bg-fg/4 text-fg/40",
            collapsed && "opacity-0",
          )}
        >
          {item.count}
        </span>
      )}
    </button>
  );
}

export function SettingsLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const { t } = useI18n();
  const { toModelsList } = useNavigationManager();
  const [collapsed, setCollapsed] = useState<boolean>(() => {
    if (typeof window === "undefined") return false;
    try {
      return window.localStorage.getItem(SIDEBAR_COLLAPSED_KEY) === "1";
    } catch {
      return false;
    }
  });

  const toggleCollapsed = () => {
    setCollapsed((prev) => {
      const next = !prev;
      try {
        window.localStorage.setItem(SIDEBAR_COLLAPSED_KEY, next ? "1" : "0");
      } catch {
        /* ignore */
      }
      return next;
    });
  };
  const {
    state: { providers, models },
  } = useSettingsSummary();

  const providerCount = providers.length;
  const modelCount = models.length;

  const [hasMetrics, setHasMetrics] = useState(false);
  useEffect(() => {
    let cancelled = false;
    void hasLlmMetrics().then((value) => {
      if (!cancelled) setHasMetrics(value);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const groups = useMemo<NavGroup[]>(() => {
    const main: NavItem[] = [
      {
        key: "providers",
        icon: <EthernetPort />,
        label: t("settings.items.providers.title"),
        matchPath: "/settings/providers",
        count: providerCount,
        onSelect: () => navigate("/settings/providers"),
        yieldsTo: (_pathname, search) => new URLSearchParams(search).get("tab") === "audio",
      },
      {
        key: "models",
        icon: <Cpu />,
        label: t("settings.items.models.title"),
        matchPath: "/settings/models",
        count: modelCount,
        onSelect: () => toModelsList(),
      },
      {
        key: "imageGeneration",
        icon: <ImageIcon />,
        label: t("settings.items.imageGeneration.title"),
        matchPath: "/settings/image-generation",
        onSelect: () => navigate("/settings/image-generation"),
      },
      ...(hasMetrics
        ? [
            {
              key: "performance",
              icon: <Activity />,
              label: t("settings.items.performance.title"),
              matchPath: "/settings/performance",
              onSelect: () => navigate("/settings/performance"),
            },
          ]
        : []),
      {
        key: "prompts",
        icon: <FileText />,
        label: t("settings.items.prompts.title"),
        matchPath: "/settings/prompts",
        onSelect: () => navigate("/settings/prompts"),
      },
      {
        key: "voices",
        icon: <Volume2 />,
        label: t("settings.items.voices.title"),
        matchPath: "/settings/voices",
        isActive: (pathname, search) =>
          pathname.startsWith("/settings/providers") &&
          new URLSearchParams(search).get("tab") === "audio",
        onSelect: () => navigate("/settings/providers?tab=audio"),
      },
      {
        key: "customization",
        icon: <Palette />,
        label: t("settings.items.accessibility.title"),
        matchPath: "/settings/customization",
        onSelect: () => navigate("/settings/customization"),
      },
      {
        key: "speechRecognition",
        icon: <Mic />,
        label: t("settings.items.speechRecognition.title"),
        matchPath: "/settings/speech-recognition",
        onSelect: () => navigate("/settings/speech-recognition"),
      },
      {
        key: "sync",
        icon: <RefreshCw />,
        label: t("settings.items.sync.title"),
        matchPath: "/settings/sync",
        onSelect: () => navigate("/settings/sync"),
      },
      {
        key: "backup",
        icon: <HardDrive />,
        label: t("settings.items.backup.title"),
        matchPath: "/settings/backup",
        onSelect: () => navigate("/settings/backup"),
      },
      {
        key: "convert",
        icon: <ArrowLeftRight />,
        label: t("settings.items.convert.title"),
        onSelect: () => void openExternalUrl("https://www.lettuceai.app/convert"),
      },
      {
        key: "security",
        icon: <Shield />,
        label: t("settings.items.security.title"),
        matchPath: "/settings/security",
        onSelect: () => navigate("/settings/security"),
      },
      {
        key: "usage",
        icon: <BarChart3 />,
        label: t("settings.items.usage.title"),
        matchPath: "/settings/usage",
        onSelect: () => navigate("/settings/usage"),
      },
      {
        key: "advanced",
        icon: <Sliders />,
        label: t("settings.items.advanced.title"),
        matchPath: "/settings/advanced",
        onSelect: () => navigate("/settings/advanced"),
        children: [
          {
            key: "advanced.creationHelper",
            icon: <Sparkles />,
            label: t("advanced.creationHelper.title"),
            matchPath: "/settings/advanced/creation-helper",
            onSelect: () => navigate("/settings/advanced/creation-helper"),
          },
          {
            key: "advanced.helpMeReply",
            icon: <PenLine />,
            label: t("advanced.helpMeReply.title"),
            matchPath: "/settings/advanced/help-me-reply",
            onSelect: () => navigate("/settings/advanced/help-me-reply"),
          },
          {
            key: "advanced.lorebooks",
            icon: <BookOpen />,
            label: t("settings.items.lorebooks.title"),
            matchPath: "/settings/advanced/lorebooks",
            onSelect: () => navigate("/settings/advanced/lorebooks"),
          },
          {
            key: "advanced.memory",
            icon: <Cpu />,
            label: t("advanced.dynamicMemory.title"),
            matchPath: "/settings/advanced/memory",
            onSelect: () => navigate("/settings/advanced/memory"),
          },
          {
            key: "advanced.companions",
            icon: <Heart />,
            label: t("settings.items.companions.title"),
            matchPath: "/settings/advanced/companions",
            extraMatchPaths: ["/settings/advanced/companion-soul-writer"],
            onSelect: () => navigate("/settings/advanced/companions"),
          },
          {
            key: "advanced.hostApi",
            icon: <Network />,
            label: t("settings.items.hostApi.title"),
            matchPath: "/settings/advanced/host-api",
            onSelect: () => navigate("/settings/advanced/host-api"),
          },
        ],
      },
    ];

    const support: NavItem[] = [
      {
        key: "help",
        icon: <HelpCircle />,
        label: t("settings.items.help.title"),
        matchPath: "/settings/help",
        onSelect: () => navigate("/settings/help"),
      },
      {
        key: "about",
        icon: <Info />,
        label: t("settings.items.about.title"),
        matchPath: "/settings/about",
        onSelect: () => navigate("/settings/about"),
      },
      {
        key: "whatsNew",
        icon: <Sparkles />,
        label: t("settings.items.whatsNew.title"),
        matchPath: "__never__",
        onSelect: () => window.dispatchEvent(new Event("whatsnew:open")),
      },
      {
        key: "changelog",
        icon: <ScrollText />,
        label: t("settings.items.changelog.title"),
        matchPath: "/settings/changelog",
        onSelect: () => void openExternalUrl("https://www.lettuceai.app/changelog"),
      },
      {
        key: "docs",
        icon: <HelpCircle />,
        label: t("settings.items.docs.title"),
        matchPath: "__never__",
        onSelect: () => void openExternalUrl("https://www.lettuceai.app/docs"),
      },
      {
        key: "logs",
        icon: <FileCode />,
        label: t("settings.items.logs.title"),
        matchPath: "/settings/logs",
        onSelect: () => navigate("/settings/logs"),
      },
      {
        key: "guide",
        icon: <BookOpen />,
        label: t("settings.items.guide.title"),
        matchPath: "/welcome",
        onSelect: () => navigate("/welcome"),
      },
    ];

    const danger: NavItem[] = [
      {
        key: "reset",
        icon: <RotateCcw />,
        label: t("settings.items.reset.title"),
        matchPath: "/settings/reset",
        danger: true,
        onSelect: () => navigate("/settings/reset"),
      },
      ...(isDevelopmentMode()
        ? [
            {
              key: "developer",
              icon: <Wrench />,
              label: t("settings.items.developer.title"),
              matchPath: "/settings/developer",
              onSelect: () => navigate("/settings/developer"),
            },
          ]
        : []),
    ];

    return [
      { key: "main", label: t("settings.groups.configuration"), items: main },
      { key: "support", label: t("settings.groups.help"), items: support },
      { key: "danger", items: danger },
    ];
  }, [providerCount, modelCount, navigate, toModelsList, t]);

  const allItems = groups.flatMap((g) =>
    g.items.flatMap((item) => [item, ...(item.children ?? [])]),
  );

  // Auto-redirect from bare /settings to About on desktop only.
  useEffect(() => {
    if (location.pathname !== "/settings") return;
    const lg = window.matchMedia("(min-width: 1024px)").matches;
    if (lg) {
      navigate("/settings/about", { replace: true });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [location.pathname]);

  const activeKey = useMemo(() => {
    const path = location.pathname;
    const search = location.search;

    // Custom isActive overrides take priority.
    for (const item of allItems) {
      if (item.isActive?.(path, search)) {
        return item.key;
      }
    }

    // Find the longest matchPath prefix, skipping items whose `yieldsTo` says no.
    let best: NavItem | undefined;
    let bestLen = 0;
    for (const item of allItems) {
      if (!item.matchPath || item.matchPath === "__never__") continue;
      if (item.yieldsTo?.(path, search)) continue;
      const candidates = [item.matchPath, ...(item.extraMatchPaths ?? [])].filter(
        (mp): mp is string => typeof mp === "string",
      );
      const matched = candidates.find(
        (mp) => path === mp || path.startsWith(mp + "/"),
      );
      if (matched && matched.length > bestLen) {
        best = item;
        bestLen = matched.length;
      }
    }
    return best?.key;
  }, [location.pathname, location.search, allItems]);

  return (
    <div
      className="flex h-full flex-col text-fg/90 lg:flex-row"
      style={{
        ["--settings-sidebar-w" as string]: collapsed ? "3.5rem" : "15.5rem",
        ["--settings-sidebar-inner-w" as string]: "15.5rem",
      }}
    >
      {/* Desktop sidebar */}
      <aside
        className={cn(
          "hidden lg:flex lg:w-(--settings-sidebar-w) lg:shrink-0 lg:flex-col",
          "lg:border-r lg:border-fg/10",
          "lg:bg-linear-to-b lg:from-fg/1.5 lg:to-transparent",
          "lg:overflow-y-auto lg:overflow-x-hidden",
          "transition-[width] duration-200 ease-out",
        )}
      >
        <div className="flex w-(--settings-sidebar-inner-w) shrink-0 flex-col">
          <div
            className={cn(
              "sticky top-0 z-10 flex items-start gap-2 px-3 pt-5 pb-3",
              "bg-surface",
              "border-b border-fg/6",
            )}
          >
            <button
              type="button"
              onClick={toggleCollapsed}
              title={collapsed ? t("settings.sidebar.expand") : t("settings.sidebar.collapse")}
              aria-label={collapsed ? t("settings.sidebar.expand") : t("settings.sidebar.collapse")}
              className={cn(
                "flex h-8 w-8 shrink-0 items-center justify-center rounded-md",
                "text-fg/50 hover:bg-fg/6 hover:text-fg",
                "transition-colors duration-150",
                "focus:outline-none focus-visible:ring-1 focus-visible:ring-fg/20",
              )}
            >
              {collapsed ? (
                <PanelLeftOpen className="h-4.25 w-4.25" />
              ) : (
                <PanelLeftClose className="h-4.25 w-4.25" />
              )}
            </button>
            <div
              className={cn(
                "min-w-0 flex-1 transition-opacity duration-150",
                collapsed && "pointer-events-none opacity-0",
              )}
            >
              <h2 className="truncate text-[15px] font-semibold tracking-tight text-fg">
                {t("common.nav.settings")}
              </h2>
              <p className="mt-0.5 text-[11px] leading-snug text-fg/40">
                {t("settings.sidebar.subtitle")}
              </p>
            </div>
          </div>
          <nav className="flex flex-col gap-4 px-2 pb-4">
            {groups.map((group) => (
              <div key={group.key} className="flex flex-col gap-0.5">
                {group.label && (
                  <p
                    className={cn(
                      "px-2.5 pb-1 pt-1 transition-opacity duration-150",
                      typography.overline.size,
                      typography.overline.weight,
                      typography.overline.tracking,
                      typography.overline.transform,
                      "text-fg/30",
                      collapsed && "opacity-0",
                    )}
                  >
                    {group.label}
                  </p>
                )}
                {group.items.map((item) => {
                const childKeys = item.children?.map((c) => c.key) ?? [];
                const childActive = activeKey ? childKeys.includes(activeKey) : false;
                const showChildren =
                  !collapsed && item.children && (item.key === activeKey || childActive);
                return (
                  <div key={item.key} className="flex flex-col gap-0.5">
                    <NavButton
                      item={item}
                      active={item.key === activeKey || childActive}
                      collapsed={collapsed}
                    />
                    <AnimatePresence>
                      {showChildren && (
                        <motion.div
                          key="submenu"
                          initial={{ height: 0, opacity: 0 }}
                          animate={{ height: "auto", opacity: 1 }}
                          exit={{ height: 0, opacity: 0 }}
                          transition={{
                            height: { duration: 0.22, ease: [0.16, 1, 0.3, 1] },
                            opacity: { duration: 0.18, ease: "easeOut" },
                          }}
                          style={{ overflow: "hidden" }}
                        >
                          <div className="ml-2.5 mb-1 mt-0.5 flex flex-col gap-0.5 border-l border-fg/8 pl-1">
                            {item.children!.map((child) => (
                              <NavButton
                                key={child.key}
                                item={child}
                                active={child.key === activeKey}
                                nested
                              />
                            ))}
                          </div>
                        </motion.div>
                      )}
                    </AnimatePresence>
                  </div>
                );
              })}
            </div>
          ))}
          </nav>
        </div>
      </aside>

      {/* Main content */}
      <div className="min-w-0 flex-1 overflow-y-auto">
        <Outlet />
      </div>
    </div>
  );
}
