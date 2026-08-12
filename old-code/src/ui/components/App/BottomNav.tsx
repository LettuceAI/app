import { Plus } from "lucide-react";
import { useLayoutEffect, useRef } from "react";
import { useLocation } from "react-router-dom";

import { TabItem } from "./NavItem";
import { resolveCreateAction, resolveNavEntries } from "./navDestinations";
import { useI18n } from "../../../core/i18n/context";
import type { NavItemId } from "../../../core/storage/schemas";

export function useAppNavHeightVar(
  ref: React.RefObject<HTMLElement | null>,
  topEdge = false,
) {
  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    const root = document.documentElement;
    const update = () => {
      const rect = element.getBoundingClientRect();
      if (topEdge) {
        const titlebar =
          parseFloat(getComputedStyle(root).getPropertyValue("--titlebar-h")) || 0;
        root.style.setProperty(
          "--appnav-ht",
          `${Math.max(0, Math.ceil(rect.bottom - titlebar))}px`,
        );
      } else {
        root.style.setProperty(
          "--appnav-h",
          `${Math.ceil(window.innerHeight - rect.top)}px`,
        );
      }
    };
    update();
    const observer = new ResizeObserver(update);
    observer.observe(element);
    window.addEventListener("resize", update);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", update);
      root.style.setProperty("--appnav-h", "0px");
      root.style.setProperty("--appnav-ht", "0px");
    };
  }, [ref, topEdge]);
}

export function BottomNav({
  onCreateClick,
  showLabels = false,
  items,
}: {
  onCreateClick: () => void;
  showLabels?: boolean;
  items?: readonly NavItemId[] | null;
}) {
  const entries = resolveNavEntries(items);
  const { pathname } = useLocation();
  const { t } = useI18n();
  const containerRef = useRef<HTMLDivElement | null>(null);
  useAppNavHeightVar(containerRef);

  const itemHeight = showLabels ? "h-14" : "h-12";
  const layoutId = showLabels ? "activeTabLabels" : "activeTab";

  return (
    <div
      ref={containerRef}
      className="fixed bottom-0 left-0 right-0 z-30 border-t border-fg/8 bg-nav/95 px-2 pb-[calc(env(safe-area-inset-bottom)+8px)] pt-2 text-fg shadow-[0_-12px_32px_rgba(0,0,0,0.35)]"
    >
      <div className="mx-auto flex w-full max-w-md lg:max-w-none items-stretch gap-1 lg:gap-2 lg:px-6">
        {entries.map((entry, index) =>
          entry.kind === "create" ? (
            <button
              key={`create-${index}`}
              onClick={() => resolveCreateAction(pathname, onCreateClick)}
              data-tour-id="nav-create"
              className={`flex flex-1 ${itemHeight} items-center justify-center rounded-xl border border-fg/15 bg-fg/10 text-fg shadow-[0_8px_20px_rgba(0,0,0,0.25)] transition hover:border-fg/25 hover:bg-fg/20 ${
                showLabels ? "flex-col gap-1" : ""
              }`}
              aria-label={t("common.bottomNav.create")}
            >
              <Plus size={showLabels ? 18 : 20} />
              {showLabels && (
                <span className="text-[10px] leading-none">{t("common.bottomNav.create")}</span>
              )}
            </button>
          ) : (
            <TabItem
              key={entry.destination.id}
              to={entry.destination.to}
              icon={entry.destination.icon}
              label={t(entry.destination.labelKey)}
              active={entry.destination.isActive(pathname)}
              className={`flex-1 ${itemHeight} text-sm`}
              dataTourId={entry.destination.dataTourId}
              layoutId={layoutId}
              showLabel={showLabels}
            />
          ),
        )}
      </div>
    </div>
  );
}
