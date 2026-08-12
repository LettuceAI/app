import { FileCode, Package } from "lucide-react";
import { BottomMenu, MenuButton, MenuButtonGroup, MenuLabel } from "./BottomMenu";
import { useI18n, type TranslationKey } from "../../core/i18n/context";

export type PromptTemplateExportFormat = "external_json" | "usc";

interface PromptTemplateExportMenuProps {
  isOpen: boolean;
  onClose: () => void;
  onSelect: (format: PromptTemplateExportFormat) => void;
  exporting?: boolean;
}

const FORMATS = [
  {
    id: "usc",
    titleKey: "components.promptTemplateExportMenu.formats.usc.title",
    descriptionKey: "components.promptTemplateExportMenu.formats.usc.description",
    icon: Package,
    color: "from-emerald-500 to-emerald-600",
  },
  {
    id: "external_json",
    titleKey: "components.promptTemplateExportMenu.formats.externalJson.title",
    descriptionKey: "components.promptTemplateExportMenu.formats.externalJson.description",
    icon: FileCode,
    color: "from-amber-500 to-orange-600",
  },
] satisfies Array<{
  id: PromptTemplateExportFormat;
  titleKey: TranslationKey;
  descriptionKey: TranslationKey;
  icon: typeof FileCode;
  color: string;
}>;

export function PromptTemplateExportMenu({
  isOpen,
  onClose,
  onSelect,
  exporting = false,
}: PromptTemplateExportMenuProps) {
  const { t } = useI18n();
  return (
    <BottomMenu isOpen={isOpen} onClose={onClose} title={t("components.promptTemplateExportMenu.title")}>
      <div className="space-y-4">
        <MenuLabel>{t("components.promptTemplateExportMenu.selectFormat")}</MenuLabel>
        <MenuButtonGroup>
          {FORMATS.map((format) => (
            <MenuButton
              key={format.id}
              icon={<format.icon className="h-4 w-4" />}
              title={t(format.titleKey)}
              description={t(format.descriptionKey)}
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
