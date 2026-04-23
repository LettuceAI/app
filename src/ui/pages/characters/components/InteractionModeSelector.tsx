import { BookOpen, MessageCircleHeart } from "lucide-react";
import type { CharacterMode } from "../../../../core/storage/schemas";
import { cn, interactive, radius, typography } from "../../../design-tokens";

interface InteractionModeSelectorProps {
  mode: CharacterMode;
  onChange: (mode: CharacterMode) => void;
  disabled?: boolean;
}

const modes: Array<{
  id: CharacterMode;
  title: string;
  subtitle: string;
  icon: typeof BookOpen;
}> = [
  {
    id: "roleplay",
    title: "Roleplay",
    subtitle: "Scene-driven chats, narrative framing, and starting scenarios.",
    icon: BookOpen,
  },
  {
    id: "companion",
    title: "Companion",
    subtitle: "Relationship-driven chats with emotional state and companion memory.",
    icon: MessageCircleHeart,
  },
];

export function InteractionModeSelector({
  mode,
  onChange,
  disabled = false,
}: InteractionModeSelectorProps) {
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between gap-2">
        <div>
          <div
            className={cn(
              typography.label.size,
              typography.label.weight,
              typography.label.tracking,
              "uppercase text-fg/70",
            )}
          >
            Interaction Mode
          </div>
          <p className={cn(typography.bodySmall.size, "mt-1 text-fg/45")}>
            Choose whether this character behaves like an RP character or a persistent companion.
          </p>
        </div>
      </div>

      <div className="grid gap-2 sm:grid-cols-2">
        {modes.map((option) => {
          const Icon = option.icon;
          const selected = mode === option.id;
          return (
            <button
              key={option.id}
              type="button"
              disabled={disabled}
              onClick={() => onChange(option.id)}
              className={cn(
                "group rounded-xl border px-4 py-3 text-left",
                interactive.transition.default,
                interactive.active.scale,
                disabled && "cursor-not-allowed opacity-60",
                selected
                  ? "border-accent/40 bg-accent/15 shadow-[0_0_0_1px_rgba(16,185,129,0.22)]"
                  : "border-fg/10 bg-surface-el/20 hover:border-fg/20 hover:bg-surface-el/30",
              )}
            >
              <div className="flex items-start gap-3">
                <div
                  className={cn(
                    "flex h-9 w-9 shrink-0 items-center justify-center border",
                    radius.lg,
                    selected
                      ? "border-accent/35 bg-accent/15 text-accent"
                      : "border-fg/10 bg-fg/5 text-fg/50 group-hover:text-fg/70",
                  )}
                >
                  <Icon className="h-4 w-4" />
                </div>
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <p className="text-sm font-semibold text-fg">{option.title}</p>
                    {selected && (
                      <span className="rounded-full border border-accent/30 bg-accent/15 px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide text-accent/90">
                        Active
                      </span>
                    )}
                  </div>
                  <p className="mt-1 text-xs leading-relaxed text-fg/50">{option.subtitle}</p>
                </div>
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}
