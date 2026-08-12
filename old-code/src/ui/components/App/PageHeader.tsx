import { useEffect, useLayoutEffect, useRef, useState, type ComponentType } from "react";
import { createPortal } from "react-dom";
import { ArrowLeft, Search, X } from "lucide-react";

import { useI18n } from "../../../core/i18n/context";
import { interactive, cn } from "../../design-tokens";
import { CONTENT_COLUMN_MAX_W } from "./navPrefs";

interface PageHeaderProps {
  title: string;
  meta?: string;
  onBack?: () => void;
  backLabel?: string;
  searchValue?: string;
  onSearchChange?: (value: string) => void;
  searchPlaceholder?: string;
  actions?: React.ReactNode;
  filters?: React.ReactNode;
}

export function PageHeader({
  title,
  meta,
  onBack,
  backLabel,
  searchValue,
  onSearchChange,
  searchPlaceholder,
  actions,
  filters,
}: PageHeaderProps) {
  const { t } = useI18n();
  const sentinelRef = useRef<HTMLDivElement | null>(null);
  const barRef = useRef<HTMLDivElement | null>(null);
  const [condensed, setCondensed] = useState(false);
  const [reserved, setReserved] = useState(0);

  useEffect(() => {
    const sentinel = sentinelRef.current;
    if (!sentinel || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver(
      ([entry]) => setCondensed(!entry.isIntersecting),
      { threshold: 1 },
    );
    observer.observe(sentinel);
    return () => observer.disconnect();
  }, []);

  useLayoutEffect(() => {
    if (condensed) return;
    const bar = barRef.current;
    if (!bar) return;
    const publish = () => setReserved(bar.offsetHeight);
    publish();
    const observer = new ResizeObserver(publish);
    observer.observe(bar);
    return () => observer.disconnect();
  }, [condensed]);

  const bar = (
    <div
      ref={barRef}
      className="fixed left-[var(--appnav-w,0px)] right-[var(--appnav-wr,0px)] top-[calc(var(--titlebar-h,0px)+var(--topnav-h,0px))] z-40 bg-surface"
    >
      <div className="px-4">
        <div className={cn("mx-auto w-full px-8", CONTENT_COLUMN_MAX_W)}>
          <div
            className={cn(
              "border-b border-fg/10",
              condensed ? "pb-2 pt-2" : "pb-4 pt-4",
              interactive.transition.default,
            )}
          >
          {onBack && (
            <button
              type="button"
              onClick={onBack}
              className={cn(
                "mb-1.5 flex items-center gap-1 text-xs font-medium text-fg/45 hover:text-fg",
                interactive.transition.fast,
              )}
            >
              <ArrowLeft size={13} strokeWidth={2.5} />
              {backLabel ?? t("common.buttons.back")}
            </button>
          )}
          <div className="flex items-end justify-between gap-6">
            <div className="flex min-w-0 items-baseline gap-3">
              <h1
                className={cn(
                  "truncate font-bold leading-tight tracking-tight text-fg",
                  condensed ? "text-[18px]" : "text-[28px]",
                  interactive.transition.default,
                )}
              >
                {title}
              </h1>
              {meta && <span className="shrink-0 text-sm text-fg/40">{meta}</span>}
            </div>

            <div className="flex shrink-0 items-center gap-2">
              {onSearchChange && (
                <div className="relative">
                  <Search
                    size={16}
                    className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-fg/40"
                  />
                  <input
                    type="text"
                    value={searchValue ?? ""}
                    onChange={(event) => onSearchChange(event.target.value)}
                    placeholder={searchPlaceholder}
                    className={cn(
                      "h-9 w-64 rounded-xl border border-fg/10 bg-fg/5 pl-9 pr-8 text-sm text-fg outline-none",
                      "placeholder:text-fg/35 focus:border-accent/40 focus:bg-fg/10 xl:w-80",
                      interactive.transition.fast,
                    )}
                  />
                  {searchValue ? (
                    <button
                      type="button"
                      onClick={() => onSearchChange("")}
                      aria-label={t("pageHeader.clearSearch")}
                      className={cn(
                        "absolute right-2 top-1/2 flex -translate-y-1/2 items-center justify-center rounded-full p-1",
                        "text-fg/40 hover:bg-fg/10 hover:text-fg",
                        interactive.transition.fast,
                      )}
                    >
                      <X size={14} />
                    </button>
                  ) : null}
                </div>
              )}
              {actions}
            </div>
          </div>
          {filters && (
            <div className="scrollbar-hide mt-3 flex gap-2 overflow-x-auto">{filters}</div>
          )}
          </div>
        </div>
      </div>
    </div>
  );

  return (
    <>
      <div ref={sentinelRef} aria-hidden className="h-px" />
      <div aria-hidden style={{ height: reserved }} />
      {typeof document === "undefined" ? bar : createPortal(bar, document.body)}
    </>
  );
}

interface PageHeaderActionProps {
  icon: ComponentType<{ size?: number; strokeWidth?: number; className?: string }>;
  label: string;
  onClick: () => void;
  dataTourId?: string;
}

export function PageHeaderAction({ icon: Icon, label, onClick, dataTourId }: PageHeaderActionProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      title={label}
      data-tour-id={dataTourId}
      className={cn(
        "flex h-9 w-9 items-center justify-center rounded-xl border border-fg/10 bg-fg/5",
        "text-fg/70 hover:border-fg/20 hover:bg-fg/10 hover:text-fg",
        interactive.transition.fast,
        interactive.active.scale,
      )}
    >
      <Icon size={18} strokeWidth={2.2} />
    </button>
  );
}
