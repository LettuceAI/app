import { memo } from "react";
import { cn } from "../../../design-tokens";

interface SkeletonCardProps {
  variant?: "default" | "compact" | "featured";
}

const SkeletonCard = memo(function SkeletonCard({ variant = "default" }: SkeletonCardProps) {
  const isFeatured = variant === "featured";
  const isCompact = variant === "compact";

  return (
    <div
      className={cn(
        "relative overflow-hidden rounded-xl border border-fg/10 bg-fg/5",
        isFeatured
          ? "aspect-16/10 w-full lg:aspect-21/9 lg:max-w-5xl lg:mx-auto xl:max-w-6xl"
          : isCompact
            ? "aspect-3/4 w-36 shrink-0"
            : "aspect-3/4 w-full",
      )}
    >
      {/* Shimmer animation overlay */}
      <div className="absolute inset-0 -translate-x-full animate-[shimmer_2s_infinite] bg-linear-to-r from-transparent via-white/10 to-transparent" />

      {/* Badge skeleton */}
      <div className="absolute left-2 right-2 top-2 flex items-start justify-between">
        <div className="h-5 w-12 rounded-full bg-fg/10" />
        <div className="h-5 w-14 rounded-full bg-fg/10" />
      </div>

      {/* Content skeleton */}
      <div className="absolute bottom-0 left-0 right-0 p-3">
        {!isCompact && <div className="mb-2 h-3 w-16 rounded bg-fg/10" />}
        <div className="mb-2 h-4 w-3/4 rounded bg-fg/15" />
        {!isCompact && (
          <>
            <div className="mb-1 h-3 w-full rounded bg-fg/10" />
            <div className="mb-3 h-3 w-2/3 rounded bg-fg/10" />
            <div className="flex gap-3">
              <div className="h-4 w-10 rounded bg-fg/10" />
              <div className="h-4 w-10 rounded bg-fg/10" />
              <div className="h-4 w-10 rounded bg-fg/10" />
            </div>
          </>
        )}
      </div>
    </div>
  );
});

interface DiscoverySectionSkeletonProps {
  cardCount?: number;
}

export const DiscoverySectionSkeleton = memo(function DiscoverySectionSkeleton({
  cardCount = 5,
}: DiscoverySectionSkeletonProps) {
  return (
    <section className="py-4">
      {/* Header skeleton */}
      <div className="mb-4 flex items-center justify-between px-4 lg:px-8">
        <div className="flex items-center gap-3">
          <div className="h-8 w-8 rounded-lg bg-fg/10" />
          <div>
            <div className="h-5 w-24 rounded bg-fg/15" />
            <div className="mt-1 h-3 w-32 rounded bg-fg/10" />
          </div>
        </div>
        <div className="h-7 w-20 rounded-full bg-fg/10" />
      </div>

      {/* Cards skeleton */}
      <div
        className="flex gap-3 overflow-hidden px-4 lg:hidden"
        style={{ scrollbarWidth: "none", msOverflowStyle: "none" }}
      >
        {Array.from({ length: cardCount }).map((_, i) => (
          <SkeletonCard key={i} variant="compact" />
        ))}
      </div>

      <div className="hidden px-4 lg:grid lg:grid-cols-4 lg:gap-4 lg:px-8 xl:grid-cols-5 2xl:grid-cols-6">
        {Array.from({ length: cardCount }).map((_, i) => (
          <SkeletonCard key={i} variant="default" />
        ))}
      </div>
    </section>
  );
});

interface DiscoveryGridSkeletonProps {
  cardCount?: number;
  columns?: 2 | 3;
}

export const DiscoveryGridSkeleton = memo(function DiscoveryGridSkeleton({
  cardCount = 6,
  columns = 2,
}: DiscoveryGridSkeletonProps) {
  return (
    <div
      className={cn(
        "grid gap-3 px-4",
        columns === 2 ? "grid-cols-2" : "grid-cols-2 lg:grid-cols-3",
      )}
    >
      {Array.from({ length: cardCount }).map((_, i) => (
        <SkeletonCard key={i} variant="default" />
      ))}
    </div>
  );
});

export const DiscoveryFeaturedSkeleton = memo(function DiscoveryFeaturedSkeleton() {
  return (
    <div className="px-4 py-3">
      <SkeletonCard variant="featured" />
    </div>
  );
});

export const DiscoverySearchSkeleton = memo(function DiscoverySearchSkeleton() {
  return (
    <div className="space-y-6">
      {/* Search bar skeleton */}
      <div className="px-4">
        <div className="h-12 w-full rounded-xl bg-fg/10" />
      </div>

      {/* Results count skeleton */}
      <div className="px-4">
        <div className="h-4 w-32 rounded bg-fg/10" />
      </div>

      {/* Results grid skeleton */}
      <DiscoveryGridSkeleton cardCount={8} />
    </div>
  );
});

export const DiscoveryDetailSkeleton = memo(function DiscoveryDetailSkeleton() {
  return (
    <div className="space-y-6 pb-24">
      {/* Hero image skeleton */}
      <div className="relative aspect-4/3 w-full bg-fg/5 lg:aspect-21/9 lg:max-w-6xl lg:mx-auto lg:rounded-2xl lg:border lg:border-fg/10">
        <div className="absolute inset-0 -translate-x-full animate-[shimmer_2s_infinite] bg-linear-to-r from-transparent via-white/10 to-transparent" />
      </div>

      {/* Content skeleton */}
      <div className="space-y-4 px-4">
        {/* Title */}
        <div className="h-7 w-3/4 rounded bg-fg/15" />

        {/* Author & stats */}
        <div className="flex items-center gap-4">
          <div className="h-5 w-24 rounded bg-fg/10" />
          <div className="h-5 w-16 rounded bg-fg/10" />
          <div className="h-5 w-16 rounded bg-fg/10" />
        </div>

        {/* Tags */}
        <div className="flex flex-wrap gap-2">
          {Array.from({ length: 4 }).map((_, i) => (
            <div key={i} className="h-6 w-16 rounded-full bg-fg/10" />
          ))}
        </div>

        {/* Divider */}
        <div className="h-px w-full bg-fg/10" />

        {/* Description */}
        <div className="space-y-2">
          <div className="h-4 w-full rounded bg-fg/10" />
          <div className="h-4 w-full rounded bg-fg/10" />
          <div className="h-4 w-5/6 rounded bg-fg/10" />
          <div className="h-4 w-4/6 rounded bg-fg/10" />
        </div>

        {/* Token info */}
        <div className="grid grid-cols-2 gap-3 pt-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <div key={i} className="h-16 rounded-xl bg-fg/5" />
          ))}
        </div>
      </div>
    </div>
  );
});

export default {
  Section: DiscoverySectionSkeleton,
  Grid: DiscoveryGridSkeleton,
  Featured: DiscoveryFeaturedSkeleton,
  Search: DiscoverySearchSkeleton,
  Detail: DiscoveryDetailSkeleton,
};
