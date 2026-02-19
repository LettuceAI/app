import { memo, useRef } from "react";
import { ChevronRight, Loader2 } from "lucide-react";
import { cn, typography } from "../../../design-tokens";
import { DiscoveryCard } from "./DiscoveryCard";
import type { DiscoveryCard as DiscoveryCardType } from "../../../../core/discovery";

interface DiscoverySectionProps {
  title: string;
  subtitle?: string;
  cards: DiscoveryCardType[];
  onCardClick: (card: DiscoveryCardType) => void;
  onViewAll?: () => void;
  loading?: boolean;
  icon?: React.ReactNode;
  accentColor?: string;
}

export const DiscoverySection = memo(function DiscoverySection({
  title,
  subtitle,
  cards,
  onCardClick,
  onViewAll,
  loading = false,
  icon,
  accentColor = "from-accent to-info/80",
}: DiscoverySectionProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  if (loading) {
    return (
      <section className="py-4">
        <div className="mb-4 flex items-center justify-between px-4">
          <div className="flex items-center gap-3">
            {icon && (
              <div
                className={cn(
                  "flex h-8 w-8 items-center justify-center rounded-lg bg-linear-to-br",
                  accentColor,
                )}
              >
                {icon}
              </div>
            )}
            <div>
              <h2 className={cn(typography.h2.size, "font-bold text-fg")}>{title}</h2>
              {subtitle && <p className="text-xs text-fg/50">{subtitle}</p>}
            </div>
          </div>
        </div>
        <div className="flex items-center justify-center py-12">
          <Loader2 className="h-8 w-8 animate-spin text-fg/30" />
        </div>
      </section>
    );
  }

  if (cards.length === 0) {
    return null;
  }

  return (
    <section className="py-4">
      {/* Header */}
      <div className="mb-4 flex items-center justify-between px-4 lg:px-8">
        <div className="flex items-center gap-3">
          {icon && (
            <div
              className={cn(
                "flex h-8 w-8 items-center justify-center rounded-lg bg-linear-to-br shadow-lg",
                accentColor,
              )}
            >
              {icon}
            </div>
          )}
          <div>
            <h2 className={cn(typography.h2.size, "font-bold text-fg")}>{title}</h2>
            {subtitle && <p className="text-xs text-fg/50">{subtitle}</p>}
          </div>
        </div>

        {onViewAll && (
          <button
            onClick={onViewAll}
            className="flex items-center gap-1 rounded-full border border-fg/10 bg-fg/5 px-3 py-1.5 text-xs font-medium text-fg/70 transition-all hover:border-fg/20 hover:bg-fg/10 hover:text-fg active:scale-95"
          >
            View All
            <ChevronRight className="h-3.5 w-3.5" />
          </button>
        )}
      </div>

      {/* Horizontal scroll container */}
      <div
        ref={scrollRef}
        className="scrollbar-hide flex gap-3 overflow-x-auto px-4 pb-2 lg:hidden"
        style={{
          scrollbarWidth: "none",
          msOverflowStyle: "none",
          WebkitOverflowScrolling: "touch",
        }}
      >
        {cards.map((card, index) => (
          <DiscoveryCard
            key={card.id}
            card={card}
            onClick={onCardClick}
            variant="compact"
            index={index}
          />
        ))}

        {/* View all card at end */}
        {onViewAll && cards.length >= 5 && (
          <button
            onClick={onViewAll}
            className="flex aspect-3/4 w-36 shrink-0 flex-col items-center justify-center gap-2 rounded-xl border border-fg/10 bg-fg/5 transition-all hover:border-fg/20 hover:bg-fg/10 active:scale-95"
          >
            <div
              className={cn(
                "flex h-12 w-12 items-center justify-center rounded-full bg-linear-to-br",
                accentColor,
              )}
            >
              <ChevronRight className="h-6 w-6 text-fg" />
            </div>
            <span className="text-xs font-medium text-fg/70">View All</span>
          </button>
        )}
      </div>

      <div className="hidden px-4 pb-2 lg:grid lg:grid-cols-4 lg:gap-4 lg:px-8 xl:grid-cols-5 2xl:grid-cols-6">
        {cards.map((card, index) => (
          <DiscoveryCard
            key={card.id}
            card={card}
            onClick={onCardClick}
            variant="default"
            index={index}
          />
        ))}

        {onViewAll && cards.length >= 5 && (
          <button
            onClick={onViewAll}
            className="flex aspect-3/4 w-full flex-col items-center justify-center gap-2 rounded-xl border border-fg/10 bg-fg/5 transition-all hover:border-fg/20 hover:bg-fg/10 active:scale-95"
          >
            <div
              className={cn(
                "flex h-12 w-12 items-center justify-center rounded-full bg-linear-to-br",
                accentColor,
              )}
            >
              <ChevronRight className="h-6 w-6 text-fg" />
            </div>
            <span className="text-xs font-medium text-fg/70">View All</span>
          </button>
        )}
      </div>

      {/* Scroll indicator gradient */}
      <div className="pointer-events-none relative">
        <div className="absolute -top-24 right-0 h-24 w-12 bg-linear-to-l from-[#050505] to-transparent lg:hidden" />
      </div>
    </section>
  );
});

export default DiscoverySection;
