import { Plus } from "lucide-react";
import { useRef } from "react";
import { useLocation } from "react-router-dom";

import { TabItem } from "./NavItem";
import { useAppNavHeightVar } from "./BottomNav";
import {
  NAV_LEADING_DESTINATIONS,
  NAV_TRAILING_DESTINATIONS,
  resolveCreateAction,
} from "./navDestinations";
import { useI18n } from "../../../core/i18n/context";

export function DockNav({ onCreateClick }: { onCreateClick: () => void }) {
  const { pathname } = useLocation();
  const { t } = useI18n();
  const containerRef = useRef<HTMLDivElement | null>(null);
  useAppNavHeightVar(containerRef);

  return (
    <div
      ref={containerRef}
      className="fixed bottom-[calc(env(safe-area-inset-bottom)+12px)] left-1/2 z-30 flex -translate-x-1/2 items-center gap-1 rounded-full border border-fg/10 bg-nav/95 px-2 py-1.5 text-fg shadow-[0_12px_32px_rgba(0,0,0,0.35)] backdrop-blur-md"
    >
      {NAV_LEADING_DESTINATIONS.map((destination) => (
        <TabItem
          key={destination.id}
          to={destination.to}
          icon={destination.icon}
          label={t(destination.labelKey)}
          active={destination.isActive(pathname)}
          className="h-12 w-12"
          dataTourId={destination.dataTourId}
          layoutId="activeTabDock"
          rounded="rounded-full"
        />
      ))}

      <button
        onClick={() => resolveCreateAction(pathname, onCreateClick)}
        data-tour-id="nav-create"
        className="flex h-12 w-12 items-center justify-center rounded-full border border-fg/15 bg-fg/10 text-fg transition hover:border-fg/25 hover:bg-fg/20"
        aria-label={t("common.bottomNav.create")}
      >
        <Plus size={20} />
      </button>

      {NAV_TRAILING_DESTINATIONS.map((destination) => (
        <TabItem
          key={destination.id}
          to={destination.to}
          icon={destination.icon}
          label={t(destination.labelKey)}
          active={destination.isActive(pathname)}
          className="h-12 w-12"
          dataTourId={destination.dataTourId}
          layoutId="activeTabDock"
          rounded="rounded-full"
        />
      ))}
    </div>
  );
}
