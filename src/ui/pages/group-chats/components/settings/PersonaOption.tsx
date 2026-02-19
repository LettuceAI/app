import { useState } from "react";
import { Check } from "lucide-react";

import { radius, interactive, typography, cn } from "../../../../design-tokens";

export function PersonaOption({
  title,
  description,
  isDefault,
  isSelected,
  onClick,
  onLongPress,
}: {
  title: string;
  description: string;
  isDefault?: boolean;
  isSelected: boolean;
  onClick: () => void;
  onLongPress?: () => void;
}) {
  const [longPressTimer, setLongPressTimer] = useState<number | null>(null);
  const [isLongPressTriggered, setIsLongPressTriggered] = useState(false);

  const handleTouchStart = () => {
    if (!onLongPress) return;
    setIsLongPressTriggered(false);
    const timer = window.setTimeout(() => {
      setIsLongPressTriggered(true);
      onLongPress();
    }, 500);
    setLongPressTimer(timer);
  };

  const handleTouchEnd = () => {
    if (longPressTimer) {
      clearTimeout(longPressTimer);
      setLongPressTimer(null);
    }
    if (!isLongPressTriggered) {
      onClick();
    }
  };

  return (
    <button
      onTouchStart={handleTouchStart}
      onTouchEnd={handleTouchEnd}
      onClick={() => {
        if (!isLongPressTriggered) {
          onClick();
        }
      }}
      className={cn(
        "group relative flex w-full items-center gap-3 justify-between",
        radius.lg,
        "p-4 text-left",
        interactive.transition.default,
        interactive.active.scale,
        isSelected
          ? "border border-accent/40 bg-accent/15 ring-2 ring-accent/30 text-accent"
          : "border border-fg/10 bg-fg/5 text-fg hover:border-fg/20 hover:bg-fg/10",
      )}
      aria-pressed={isSelected}
    >
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <div className={cn(typography.body.size, typography.h3.weight, "truncate", "py-0.5")}>{title}</div>
          {isDefault && (
            <span
              className={cn(
                "shrink-0 rounded-full border border-info/30 bg-info/10 px-2 text-[10px] font-medium text-info",
              )}
            >
              App default
            </span>
          )}
        </div>
        <div className={cn(typography.caption.size, "mt-1 truncate text-fg/50")}>{description}</div>
      </div>

      <div
        className={cn(
          "flex h-8 w-8 items-center justify-center rounded-full border",
          isSelected
            ? "bg-accent/20 border-accent/50 text-accent/80"
            : "bg-fg/5 border-fg/10 text-fg/70 group-hover:border-fg/20",
        )}
        aria-hidden="true"
      >
        {isSelected ? <Check className="h-4 w-4" /> : <span className="h-4 w-4" />}
      </div>
    </button>
  );
}
