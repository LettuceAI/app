import { Compass } from "lucide-react";
import { BottomMenu } from "../../../components/BottomMenu";
import { cn, interactive, radius, typography } from "../../../design-tokens";
import { useI18n } from "../../../../core/i18n/context";

interface SoulDirectionBottomMenuProps {
  isOpen: boolean;
  onClose: () => void;
  direction: string;
  onDirectionChange: (next: string) => void;
}

export function SoulDirectionBottomMenu({
  isOpen,
  onClose,
  direction,
  onDirectionChange,
}: SoulDirectionBottomMenuProps) {
  const { t } = useI18n();

  return (
    <BottomMenu isOpen={isOpen} onClose={onClose} title={t("characters.soulEditor.directionLabel")}>
      <div className="space-y-4">
        <div className="flex items-start gap-3">
          <div
            className={cn(
              "flex h-9 w-9 shrink-0 items-center justify-center border border-info/30 bg-info/10 text-info",
              radius.md,
            )}
          >
            <Compass className="h-4 w-4" />
          </div>
          <p className={cn(typography.bodySmall.size, "text-fg/55")}>
            {t("characters.soulEditor.directionMenuDesc")}
          </p>
        </div>

        <textarea
          value={direction}
          onChange={(event) => onDirectionChange(event.target.value)}
          placeholder={t("characters.soulEditor.directionPlaceholder")}
          rows={6}
          autoFocus
          className={cn(
            "w-full resize-none border bg-fg/[0.04] px-3 py-3 text-sm leading-relaxed text-fg outline-none",
            "placeholder:text-fg/35 focus:border-fg/25 focus:bg-fg/[0.06]",
            radius.md,
          )}
        />

        <div className="grid grid-cols-2 gap-2">
          <button
            type="button"
            onClick={() => onDirectionChange("")}
            disabled={!direction.trim()}
            className={cn(
              "h-11 border px-3 text-sm font-semibold",
              radius.md,
              interactive.transition.fast,
              "border-fg/10 bg-fg/[0.04] text-fg/75 hover:border-fg/20 hover:bg-fg/[0.07]",
              "disabled:cursor-not-allowed disabled:opacity-45",
            )}
          >
            {t("characters.soulEditor.directionClear")}
          </button>
          <button
            type="button"
            onClick={onClose}
            className={cn(
              "h-11 border px-3 text-sm font-semibold",
              radius.md,
              interactive.transition.fast,
              "border-accent/40 bg-accent/20 text-accent hover:border-accent/55 hover:bg-accent/30",
            )}
          >
            {t("common.buttons.done")}
          </button>
        </div>
      </div>
    </BottomMenu>
  );
}
