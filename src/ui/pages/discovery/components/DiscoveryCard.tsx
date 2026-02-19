import { memo, useState, useEffect } from "react";
import { Heart, Download, Eye, MessageCircle, Shield, Sparkles, BookOpen } from "lucide-react";
import { cn, typography } from "../../../design-tokens";
import { getCardImageUrl, formatCount } from "../../../../core/discovery";
import type { DiscoveryCard as DiscoveryCardType } from "../../../../core/discovery";

interface DiscoveryCardProps {
  card: DiscoveryCardType;
  onClick: (card: DiscoveryCardType) => void;
  variant?: "default" | "compact" | "featured";
  index?: number;
}

function StatBadge({ icon: Icon, value }: { icon: typeof Heart; value: string }) {
  return (
    <span className="flex items-center gap-1 text-[10px] text-fg/60">
      <Icon className="h-3 w-3" />
      {value}
    </span>
  );
}

export const DiscoveryCard = memo(function DiscoveryCard({
  card,
  onClick,
  variant = "default",
}: DiscoveryCardProps) {
  const [imageUrl, setImageUrl] = useState<string | null>(null);
  const [imageLoaded, setImageLoaded] = useState(false);
  const [imageError, setImageError] = useState(false);

  useEffect(() => {
    if (!card.path) return;

    let cancelled = false;
    const width = variant === "featured" ? 600 : variant === "compact" ? 300 : 400;

    getCardImageUrl(card.path, "webp", width, 85)
      .then((url) => {
        if (!cancelled) setImageUrl(url);
      })
      .catch((err) => {
        console.error("Failed to get card image URL:", err);
        if (!cancelled) setImageError(true);
      });

    return () => {
      cancelled = true;
    };
  }, [card.path, variant]);

  const handleClick = () => onClick(card);

  const isFeatured = variant === "featured";
  const isCompact = variant === "compact";

  // Generate gradient fallback based on card name
  const gradientHue = card.name.split("").reduce((acc, char) => acc + char.charCodeAt(0), 0) % 360;
  const fallbackGradient = `linear-gradient(135deg, hsl(${gradientHue}, 60%, 20%) 0%, hsl(${(gradientHue + 60) % 360}, 50%, 15%) 100%)`;

  return (
    <button
      onClick={handleClick}
      className={cn(
        "group relative flex flex-col justify-end overflow-hidden text-left",
        "border border-fg/10 hover:border-fg/20",
        "transition-all duration-200 active:scale-[0.98]",
        isFeatured
          ? "aspect-16/10 w-full rounded-xl lg:aspect-21/9 lg:max-w-5xl lg:mx-auto xl:max-w-6xl"
          : isCompact
            ? "aspect-3/4 w-36 shrink-0 rounded-xl"
            : "aspect-3/4 w-full rounded-xl",
      )}
    >
      {/* Background Image */}
      <div className="absolute inset-0 z-0" style={{ background: fallbackGradient }}>
        {imageUrl && !imageError && (
          <img
            src={imageUrl}
            alt={card.name}
            className={cn(
              "h-full w-full object-cover transition-all duration-500",
              "group-hover:scale-105",
              imageLoaded ? "opacity-100" : "opacity-0",
              card.isNsfw && "blur-xl",
            )}
            onLoad={() => setImageLoaded(true)}
            onError={() => setImageError(true)}
          />
        )}

        {/* Skeleton loader */}
        {!imageLoaded && !imageError && (
          <div className="absolute inset-0 animate-pulse bg-linear-to-br from-white/5 to-white/10" />
        )}
      </div>

      {/* NSFW Overlay */}
      {card.isNsfw && (
        <div className="absolute inset-0 z-5 flex items-center justify-center bg-surface-el/40">
          <div className="flex flex-col items-center gap-1">
            <Shield className="h-8 w-8 text-danger" />
            <span className="text-xs font-bold uppercase tracking-wider text-danger">NSFW</span>
          </div>
        </div>
      )}

      {/* Gradient overlays for depth */}
      <div className="absolute inset-0 z-10 bg-linear-to-t from-black/95 via-black/40 to-transparent" />
      <div className="absolute inset-0 z-10 bg-linear-to-br from-transparent via-transparent to-black/30" />

      {/* Top badges row */}
      <div className="absolute left-2 right-2 top-2 z-20 flex items-start justify-between">
        <div className="flex flex-wrap gap-1.5">
          {/* NSFW Badge */}
          {card.isNsfw && (
            <span className="flex items-center gap-1 rounded-full bg-danger px-2 py-0.5 text-[9px] font-bold uppercase tracking-wider text-fg shadow-lg shadow-red-600/40">
              <Shield className="h-2.5 w-2.5" />
              NSFW
            </span>
          )}

          {/* Original Character Badge */}
          {card.isOc && (
            <span className="flex items-center gap-1 rounded-full bg-secondary/80 px-2 py-0.5 text-[9px] font-bold uppercase tracking-wider text-fg shadow-lg shadow-violet-500/30 backdrop-blur-sm">
              <Sparkles className="h-2.5 w-2.5" />
              OC
            </span>
          )}

          {/* Has Lorebook Badge */}
          {card.hasLorebook && (
            <span className="flex items-center gap-1 rounded-full bg-warning/80 px-2 py-0.5 text-[9px] font-bold uppercase tracking-wider text-fg shadow-lg shadow-amber-500/30 backdrop-blur-sm">
              <BookOpen className="h-2.5 w-2.5" />
            </span>
          )}
        </div>

        {/* Likes count prominent */}
        {card.likes !== undefined && card.likes > 0 && (
          <span className="flex items-center gap-1 rounded-full bg-surface-el/50 px-2 py-1 text-[10px] font-semibold text-fg backdrop-blur-md">
            <Heart className="h-3 w-3 fill-danger text-danger" />
            {formatCount(card.likes)}
          </span>
        )}
      </div>

      {/* Content area with glassmorphism */}
      <div className="relative z-20 flex flex-col gap-1.5 p-3">
        {/* Author */}
        {card.author && !isCompact && (
          <span className="text-[10px] font-medium text-fg/50 tracking-wide">
            by {card.author}
          </span>
        )}

        {/* Name */}
        <h3
          className={cn(
            "font-bold text-fg leading-tight",
            isFeatured ? "text-xl" : isCompact ? "text-xs" : typography.body.size,
            "line-clamp-2",
          )}
        >
          {card.name}
        </h3>

        {/* Tagline */}
        {card.tagline && !isCompact && (
          <p
            className={cn(
              "text-fg/60 line-clamp-2",
              isFeatured ? "text-sm" : "text-[11px]",
              "leading-relaxed",
            )}
          >
            {card.tagline}
          </p>
        )}

        {/* Stats row */}
        {!isCompact && (
          <div className="mt-1 flex items-center gap-3">
            {card.downloads !== undefined && (
              <StatBadge icon={Download} value={formatCount(card.downloads)} />
            )}
            {card.views !== undefined && <StatBadge icon={Eye} value={formatCount(card.views)} />}
            {card.messages !== undefined && (
              <StatBadge icon={MessageCircle} value={formatCount(card.messages)} />
            )}
          </div>
        )}

        {/* Tags */}
        {isFeatured && card.tags.length > 0 && (
          <div className="mt-2 flex flex-wrap gap-1.5">
            {card.tags.slice(0, 4).map((tag) => (
              <span
                key={tag}
                className="rounded-full bg-fg/10 px-2 py-0.5 text-[9px] font-medium text-fg/70 backdrop-blur-sm"
              >
                {tag}
              </span>
            ))}
            {card.tags.length > 4 && (
              <span className="rounded-full bg-fg/5 px-2 py-0.5 text-[9px] font-medium text-fg/40">
                +{card.tags.length - 4}
              </span>
            )}
          </div>
        )}
      </div>

      {/* Hover shine effect */}
      <div className="absolute inset-0 z-30 bg-linear-to-r from-transparent via-white/5 to-transparent opacity-0 transition-opacity group-hover:opacity-100" />
    </button>
  );
});

export default DiscoveryCard;
