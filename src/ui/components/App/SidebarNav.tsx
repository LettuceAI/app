import { Plus } from "lucide-react";
import { useLayoutEffect, useRef } from "react";
import { useLocation } from "react-router-dom";

import { TabItem } from "./NavItem";
import { NAV_DESTINATIONS, resolveCreateAction, resolveNavEntries } from "./navDestinations";
import { useI18n } from "../../../core/i18n/context";
import type { NavAlign, NavigationSide, NavItemId } from "../../../core/storage/schemas";

function useAppNavSideVar(
  ref: React.RefObject<HTMLElement | null>,
  side: NavigationSide,
  floating: boolean,
) {
  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    const root = document.documentElement;
    const update = () => {
      const rect = element.getBoundingClientRect();
      const width =
        side === "right" ? window.innerWidth - rect.left : rect.right;
      const value = `${Math.max(0, Math.ceil(width))}px`;
      if (floating) {
        root.style.setProperty("--appnav-w", value);
        root.style.setProperty("--appnav-wr", value);
      } else {
        const contentVar = side === "right" ? "--appnav-wr" : "--appnav-w";
        const topVar = side === "right" ? "--appnav-top-wr" : "--appnav-top-w";
        root.style.setProperty(contentVar, value);
        root.style.setProperty(topVar, value);
      }
    };
    update();
    const observer = new ResizeObserver(update);
    observer.observe(element);
    window.addEventListener("resize", update);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", update);
      root.style.setProperty("--appnav-w", "0px");
      root.style.setProperty("--appnav-wr", "0px");
      root.style.setProperty("--appnav-top-w", "0px");
      root.style.setProperty("--appnav-top-wr", "0px");
    };
  }, [ref, side, floating]);
}

export function SidebarNav({
  onCreateClick,
  side = "left",
  floating = false,
  align = "start",
  items,
}: {
  onCreateClick: () => void;
  side?: NavigationSide;
  floating?: boolean;
  align?: NavAlign;
  items?: readonly NavItemId[] | null;
}) {
  const entries = resolveNavEntries(items);
  const hasExplicitSettings = entries.some(
    (entry) => entry.kind === "destination" && entry.destination.id === "settings",
  );
  const settingsDestination = hasExplicitSettings
    ? undefined
    : NAV_DESTINATIONS.find((entry) => entry.id === "settings");
  const { pathname } = useLocation();
  const { t } = useI18n();
  const containerRef = useRef<HTMLDivElement | null>(null);
  useAppNavSideVar(containerRef, side, floating);

  const layoutId = floating ? "activeTabSidebarFloating" : "activeTabSidebar";
  const itemRounded = floating ? "rounded-full" : "rounded-2xl";
  const dividerClassName = "my-1 h-px w-7 shrink-0 bg-fg/10";
  const floatingAnchor =
    align === "center"
      ? "top-1/2 -translate-y-1/2"
      : align === "end"
        ? "bottom-8"
        : "top-[calc(var(--titlebar-h,0px)+var(--topnav-h,72px)+16px)]";
  const containerClassName = floating
    ? `fixed ${floatingAnchor} z-30 flex flex-col items-center gap-1 rounded-full border border-fg/10 bg-nav/95 px-1.5 py-2 text-fg shadow-[0_12px_32px_rgba(0,0,0,0.35)] backdrop-blur-md ${
        side === "right" ? "right-8" : "left-8"
      }`
    : `fixed bottom-0 top-[var(--titlebar-h,0px)] z-30 flex w-16 flex-col items-center gap-1 pt-3 ${
        settingsDestination ? "pb-20" : "pb-3"
      } text-fg ${
        align === "center" ? "justify-center" : align === "end" ? "justify-end" : "justify-start"
      } ${side === "right" ? "right-0" : "left-0"}`;

  const createButton = (key: string) => (
    <button
      key={key}
      onClick={() => resolveCreateAction(pathname, onCreateClick)}
      data-tour-id="nav-create"
      title={t("common.bottomNav.create")}
      className={`flex h-12 w-12 shrink-0 items-center justify-center ${
        floating ? "rounded-full" : "rounded-2xl"
      } border border-fg/15 bg-fg/10 text-fg shadow-[0_8px_20px_rgba(0,0,0,0.25)] transition hover:border-fg/25 hover:bg-fg/20`}
      aria-label={t("common.bottomNav.create")}
    >
      <Plus size={20} />
    </button>
  );

  const settingsButton = settingsDestination ? (
    <TabItem
      to={settingsDestination.to}
      icon={settingsDestination.icon}
      label={t(settingsDestination.labelKey)}
      active={settingsDestination.isActive(pathname)}
      className="h-12 w-12"
      layoutId={layoutId}
      rounded={itemRounded}
    />
  ) : null;

  return (
    <div ref={containerRef} className={containerClassName}>
      {entries.map((entry, index) =>
        entry.kind === "create" ? (
          createButton(`create-${index}`)
        ) : (
          <TabItem
            key={entry.destination.id}
            to={entry.destination.to}
            icon={entry.destination.icon}
            label={t(entry.destination.labelKey)}
            active={entry.destination.isActive(pathname)}
            className="h-12 w-12"
            dataTourId={entry.destination.dataTourId}
            layoutId={layoutId}
            rounded={itemRounded}
          />
        ),
      )}
      {settingsButton &&
        (floating ? (
          <>
            <div className={dividerClassName} />
            {settingsButton}
          </>
        ) : (
          <div className="absolute inset-x-0 bottom-3 flex flex-col items-center gap-1">
            <div className={dividerClassName} />
            {settingsButton}
          </div>
        ))}
    </div>
  );
}
