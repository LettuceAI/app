import { FileCode, Package } from "lucide-react";
import { BottomMenu, MenuButton, MenuButtonGroup, MenuLabel } from "./BottomMenu";
import { useI18n } from "../../core/i18n/context";

export type ChatTemplateExportFormat = "json" | "usc";

interface ChatTemplateExportMenuProps {
  isOpen: boolean;
  onClose: () => void;
  onSelect: (format: ChatTemplateExportFormat) => void;
  exporting?: boolean;
}

export function ChatTemplateExportMenu({
  isOpen,
  onClose,
  onSelect,
  exporting = false,
}: ChatTemplateExportMenuProps) {
  const { t } = useI18n();

  const FORMATS: Array<{
    id: ChatTemplateExportFormat;
    title: string;
    description: string;
    icon: typeof FileCode;
    color: string;
  }> = [
    {
      id: "usc",
      title: t("exportMenu.extra.uscTemplate"),
      description: t("exportMenu.extra.portableTemplate"),
      icon: Package,
      color: "from-emerald-500 to-emerald-600",
    },
    {
      id: "json",
      title: t("exportMenu.extra.templateJson"),
      description: t("exportMenu.extra.nativeTemplate"),
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
