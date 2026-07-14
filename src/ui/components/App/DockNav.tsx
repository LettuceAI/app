import { Plus } from "lucide-react";
import { useRef } from "react";
import { useLocation } from "react-router-dom";

import { TabItem } from "./NavItem";
import { useAppNavHeightVar } from "./BottomNav";
import { resolveCreateAction, resolveNavEntries } from "./navDestinations";
import { useI18n } from "../../../core/i18n/context";
import type { NavAlign, NavEdge, NavItemId } from "../../../core/storage/schemas";

export function DockNav({
  onCreateClick,
  align = "center",
  edge = "bottom",
  items,
}: {
  onCreateClick: () => void;
  align?: NavAlign;
  edge?: NavEdge;
  items?: readonly NavItemId[] | null;
}) {
  const entries = resolveNavEntries(items);
  const { pathname } = useLocation();
  const { t } = useI18n();
  const containerRef = useRef<HTMLDivElement | null>(null);
  useAppNavHeightVar(containerRef, edge === "top");

  return (
    <div
      ref={containerRef}
      className={`fixed z-30 flex items-center gap-1 rounded-full border border-fg/10 bg-nav/95 px-2 py-1.5 text-fg shadow-[0_12px_32px_rgba(0,0,0,0.35)] backdrop-blur-md ${
        edge === "top"
          ? "top-[calc(var(--titlebar-h,0px)+var(--topnav-h,72px)+12px)]"
          : "bottom-[calc(env(safe-area-inset-bottom)+12px)]"
      } ${
        align === "start" ? "left-8" : align === "end" ? "right-8" : "left-1/2 -translate-x-1/2"
      }`}
    >
      {entries.map((entry, index) =>
        entry.kind === "create" ? (
          <button
            key={`create-${index}`}
            onClick={() => resolveCreateAction(pathname, onCreateClick)}
            data-tour-id="nav-create"
            className="flex h-12 w-12 items-center justify-center rounded-full border border-fg/15 bg-fg/10 text-fg transition hover:border-fg/25 hover:bg-fg/20"
            aria-label={t("common.bottomNav.create")}
          >
            <Plus size={20} />
          </button>
        ) : (
          <TabItem
            key={entry.destination.id}
            to={entry.destination.to}
            icon={entry.destination.icon}
            label={t(entry.destination.labelKey)}
            active={entry.destination.isActive(pathname)}
            className="h-12 w-12"
            dataTourId={entry.destination.dataTourId}
            layoutId="activeTabDock"
            rounded="rounded-full"
          />
        ),
      )}
    </div>
  );
}
