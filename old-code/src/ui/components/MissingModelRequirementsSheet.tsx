import { Download } from "lucide-react";

import { BottomMenu } from "./BottomMenu";
import { cn, interactive, radius, typography } from "../design-tokens";
import type { ModelRequirement } from "../modelRequirements";
import { useI18n } from "../../core/i18n/context";

interface MissingModelRequirementsSheetProps {
  isOpen: boolean;
  title: string;
  description: string;
  missing: ModelRequirement[];
  onClose: () => void;
  onDownload: () => void;
  closeLabel: string;
  downloadLabel?: string;
}

export function MissingModelRequirementsSheet({
  isOpen,
  title,
  description,
  missing,
  onClose,
  onDownload,
  closeLabel,
  downloadLabel,
}: MissingModelRequirementsSheetProps) {
  const { t } = useI18n();
  const resolvedDownloadLabel = downloadLabel ?? t("components.extra.companionDownloadNow");
  return (
    <BottomMenu isOpen={isOpen} onClose={onClose} title={title}>
      <div className="flex flex-col gap-4 px-4 pb-2 pt-1">
        <div className="min-w-0">
          <p className={cn(typography.body.size, "text-fg")}>{description}</p>
        </div>

        <ul className="flex flex-col gap-2">
          {missing.map((requirement) => {
            const Icon = requirement.icon;
            return (
              <li
                key={requirement.kind}
                className={cn(
                  "flex items-start gap-3 border border-warning/20 bg-warning/5 px-3 py-2.5",
                  radius.lg,
                )}
              >
                <div
                  className={cn(
                    "flex h-9 w-9 shrink-0 items-center justify-center border border-warning/30 bg-warning/10 text-warning/80",
                    radius.md,
                  )}
                >
                  <Icon className="h-4 w-4" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center justify-between gap-2">
                    <p className="text-sm font-medium text-fg">{requirement.title}</p>
                    <span className="shrink-0 font-mono text-[10px] text-fg/45">
                      {requirement.approxSize}
                    </span>
                  </div>
                  <p className="mt-0.5 text-[11px] leading-relaxed text-fg/50">
                    {requirement.subtitle}
                  </p>
                </div>
              </li>
            );
          })}
        </ul>

        <div className="flex gap-2 pt-1">
          <button
            type="button"
            onClick={onClose}
            className={cn(
              "flex flex-1 items-center justify-center border border-fg/10 bg-fg/5 px-4 py-3 text-sm font-medium text-fg/70",
              radius.md,
              interactive.transition.fast,
              interactive.active.scale,
              "hover:border-fg/20 hover:bg-fg/10",
            )}
          >
            {closeLabel}
          </button>
          <button
            type="button"
            onClick={onDownload}
            className={cn(
              "flex flex-1 items-center justify-center gap-2 border border-accent/30 bg-accent/15 px-4 py-3 text-sm font-semibold text-accent",
              radius.md,
              interactive.transition.fast,
              interactive.active.scale,
              "hover:border-accent/45 hover:bg-accent/25",
            )}
          >
            <Download className="h-3.5 w-3.5" />
            {resolvedDownloadLabel}
          </button>
        </div>
      </div>
    </BottomMenu>
  );
}
