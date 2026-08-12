import { FileCode, Package } from "lucide-react";
import { BottomMenu, MenuButton, MenuButtonGroup, MenuLabel } from "./BottomMenu";
import { useI18n } from "../../core/i18n/context";

export type ModelExportFormat = "json" | "usc";

interface ModelExportMenuProps {
  isOpen: boolean;
  onClose: () => void;
  onSelect: (format: ModelExportFormat) => void;
  exporting?: boolean;
}

export function ModelExportMenu({
  isOpen,
  onClose,
  onSelect,
  exporting = false,
}: ModelExportMenuProps) {
  const { t } = useI18n();

  const FORMATS: Array<{
    id: ModelExportFormat;
    title: string;
    description: string;
    icon: typeof FileCode;
    color: string;
  }> = [
    {
      id: "usc",
      title: t("exportMenu.extra.uscModel"),
      description: t("exportMenu.extra.portableModel"),
      icon: Package,
      color: "from-emerald-500 to-emerald-600",
    },
    {
      id: "json",
      title: t("exportMenu.extra.modelJson"),
      description: t("exportMenu.extra.safeModel"),
      icon: FileCode,
      color: "from-amber-500 to-orange-600",
    },
  ];

  return (
    <BottomMenu isOpen={isOpen} onClose={onClose} title={t("exportMenu.extra.exportFormatTitle")}>
      <div className="space-y-4">
        <MenuLabel>{t("exportMenu.selectFormat")}</MenuLabel>
        <MenuButtonGroup>
          {FORMATS.map((format) => (
            <MenuButton
              key={format.id}
              icon={<format.icon className="h-4 w-4" />}
              title={format.title}
              description={format.description}
              color={format.color}
              onClick={() => onSelect(format.id)}
              disabled={exporting}
            />
          ))}
        </MenuButtonGroup>
      </div>
    </BottomMenu>
  );
}
