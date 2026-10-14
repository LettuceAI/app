import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Button, cn, ListRowButton } from "@lettuceai/crisp";
import type { SettingSource } from "@/api/generated/bindings";

export interface SettingRowProps {
  icon: ReactNode;
  title: string;
  value: ReactNode;
  source?: SettingSource;
  onOpen: () => void;
  onReset?: () => void;
  disabled?: boolean;
}

export function SettingRow({ icon, title, value, source, onOpen, onReset, disabled }: SettingRowProps) {
  const { t } = useTranslation("conversations");
  const own = source === "conversation";
  return (
    <div className="flex items-center">
      <ListRowButton
        icon={icon}
        title={title}
        description={
          <span className="block">
            <span className="block truncate text-fg-2">{value}</span>
            {source && <span className={cn("block text-xs", own ? "text-accent" : "text-fg-3")}>{t(`settings.source.${source}`)}</span>}
          </span>
        }
        className="min-w-0 flex-1"
        disabled={disabled ?? false}
        onClick={onOpen}
      />
      {own && onReset && (
        <Button variant="ghost" size="sm" className="mr-2 shrink-0" disabled={disabled ?? false} onClick={onReset}>
          {t("settings.reset")}
        </Button>
      )}
    </div>
  );
}

export function ControlRow({ title, description, control }: { title: string; description?: ReactNode; control: ReactNode }) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-2 px-4 py-3">
      <div className="min-w-0 flex-1 basis-40">
        <span className="block text-base font-medium text-fg">{title}</span>
        {description && <span className="block text-sm text-fg-3">{description}</span>}
      </div>
      <div className="flex shrink-0 items-center gap-2">{control}</div>
    </div>
  );
}
