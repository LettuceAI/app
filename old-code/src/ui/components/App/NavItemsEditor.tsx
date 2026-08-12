import { Reorder } from "framer-motion";
import { Plus, RotateCcw, X } from "lucide-react";

import { useI18n } from "../../../core/i18n/context";
import { cn } from "../../design-tokens";
import {
  DEFAULT_NAV_ITEMS,
  type NavigationStyle,
  type NavItemId,
} from "../../../core/storage/schemas";
import { NAV_DESTINATIONS } from "./navDestinations";

function itemsEqual(left: readonly NavItemId[], right: readonly NavItemId[]) {
  return left.length === right.length && left.every((id, index) => id === right[index]);
}

function iconFor(id: NavItemId) {
  if (id === "create") return Plus;
  return NAV_DESTINATIONS.find((destination) => destination.id === id)?.icon ?? Plus;
}

export function NavItemsEditor({
  value,
  onChange,
  navStyle,
}: {
  value: NavItemId[] | null;
  onChange: (next: NavItemId[] | null) => void;
  navStyle: NavigationStyle;
}) {
  const { t } = useI18n();
  const items = value && value.length > 0 ? value : [...DEFAULT_NAV_ITEMS];
  const isDefault = itemsEqual(items, DEFAULT_NAV_ITEMS);
  const available: NavItemId[] = NAV_DESTINATIONS.map(
    (destination) => destination.id,
  ).filter((id) => !items.includes(id));

  const vertical = navStyle === "sidebar" || navStyle === "floatingSidebar";
  const rounded =
    navStyle === "dock" || navStyle === "floatingSidebar"
      ? "rounded-full"
      : navStyle === "header"
        ? "rounded-xl"
        : "rounded-2xl";

  const labelFor = (id: NavItemId) => {
    if (id === "create") return t("common.bottomNav.create");
    const destination = NAV_DESTINATIONS.find((entry) => entry.id === id);
    return destination ? t(destination.labelKey) : id;
  };

  const commit = (next: NavItemId[]) => {
    onChange(itemsEqual(next, DEFAULT_NAV_ITEMS) ? null : next);
  };

  const isBottomBar = navStyle === "bottom" || navStyle === "bottomLabels";
  const containerClassName = cn(
    "flex items-center border border-fg/10 bg-nav/95 text-fg shadow-[0_12px_32px_rgba(0,0,0,0.35)]",
    isBottomBar
      ? "w-full gap-1 rounded-xl px-2 py-2"
      : vertical
        ? "mx-auto w-fit flex-col gap-1 px-1.5 py-2"
        : "mx-auto w-fit gap-1 px-2 py-1.5",
    !isBottomBar &&
      (navStyle === "dock" || navStyle === "floatingSidebar" || navStyle === "header"
        ? "rounded-full"
        : "rounded-2xl"),
  );

  return (
    <div className="space-y-2.5">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 space-y-0.5">
          <span className="block text-sm font-medium text-fg">
            {t("accessibility.navigation.itemsTitle")}
          </span>
          <span className="block text-[11px] text-fg/45">
            {t("accessibility.navigation.itemsDesc")}
          </span>
        </div>
        <button
          type="button"
          onClick={() => onChange(null)}
          disabled={isDefault}
          className={cn(
            "inline-flex shrink-0 items-center gap-1.5 rounded-md border px-2.5 py-1.5 text-[11px] font-medium transition-colors",
            isDefault
              ? "cursor-default border-fg/5 bg-transparent text-fg/25"
              : "border-fg/10 bg-fg/5 text-fg/65 hover:border-fg/20 hover:bg-fg/8 hover:text-fg",
          )}
        >
          <RotateCcw size={12} />
          {t("accessibility.navigation.itemsReset")}
        </button>
      </div>

      <div className="rounded-xl border border-fg/10 bg-fg/[0.03] px-4 py-5">
        <Reorder.Group
          axis={vertical ? "y" : "x"}
          values={items}
          onReorder={(next) => commit(next as NavItemId[])}
          className={containerClassName}
        >
          {items.map((id) => {
            const Icon = iconFor(id);
            const removable = id !== "create" && items.length > 2;
            return (
              <Reorder.Item
                key={id}
                value={id}
                dragMomentum={false}
                dragElastic={0}
                layout="position"
                whileDrag={{
                  zIndex: 20,
                  scale: 1.06,
                  boxShadow: "0 8px 20px rgba(0,0,0,0.3), 0 0 0 1px rgba(255,255,255,0.08)",
                }}
                transition={{ layout: { duration: 0.16, ease: "easeOut" } }}
                title={labelFor(id)}
                className={cn(
                  "group relative flex cursor-grab items-center justify-center active:cursor-grabbing",
                  navStyle === "bottomLabels" ? "h-14 flex-col gap-1" : "h-12",
                  isBottomBar ? "flex-1" : "w-12 shrink-0",
                  rounded,
                  id === "create"
                    ? "border border-fg/15 bg-fg/10 text-fg"
                    : "text-fg/60 hover:bg-fg/8 hover:text-fg",
                )}
                style={{ position: "relative" }}
              >
                <Icon
                  size={navStyle === "bottomLabels" ? 18 : 20}
                  className="pointer-events-none"
                />
                {navStyle === "bottomLabels" && (
                  <span className="pointer-events-none text-[10px] leading-none">
                    {labelFor(id)}
                  </span>
                )}
                {removable && (
                  <button
                    type="button"
                    onPointerDown={(event) => event.stopPropagation()}
                    onClick={(event) => {
                      event.stopPropagation();
                      commit(items.filter((entry) => entry !== id));
                    }}
                    className="absolute -right-1 -top-1 z-10 flex h-4.5 w-4.5 items-center justify-center rounded-full border border-fg/15 bg-surface text-fg/45 opacity-0 transition-opacity hover:text-danger group-hover:opacity-100"
                    aria-label={t("accessibility.navigation.itemsRemove", {
                      label: labelFor(id),
                    })}
                  >
                    <X size={10} />
                  </button>
                )}
              </Reorder.Item>
            );
          })}
        </Reorder.Group>
      </div>

      {available.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="text-[11px] text-fg/45">
            {t("accessibility.navigation.itemsAdd")}
          </span>
          {available.map((id) => {
            const Icon = iconFor(id);
            return (
              <button
                key={id}
                type="button"
                onClick={() => commit([...items, id])}
                className="inline-flex items-center gap-1 rounded-full border border-dashed border-fg/15 px-2.5 py-1 text-[11px] font-medium text-fg/60 transition-colors hover:border-fg/30 hover:bg-fg/5 hover:text-fg"
              >
                <Plus size={11} />
                <Icon size={12} />
                {labelFor(id)}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
