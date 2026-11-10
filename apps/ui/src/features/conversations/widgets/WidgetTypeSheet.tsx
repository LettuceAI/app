import { MousePointerClick } from "lucide-react";
import { useTranslation } from "react-i18next";
import { icon, ListGroup, ListRowButton, Sheet } from "@lettuceai/crisp";
import { WIDGET_ICON, WIDGET_ORDER } from "./catalogue";
import { typeAvailable, type WidgetPlace, type WidgetType } from "./nodes";

export interface WidgetTypeSheetProps {
  open: boolean;
  where: WidgetPlace;
  onPick: (type: WidgetType | "actions") => void;
  onClose: () => void;
}

export function WidgetTypeSheet({ open, where, onPick, onClose }: WidgetTypeSheetProps) {
  const { t } = useTranslation("conversations");
  return (
    <Sheet open={open} onClose={onClose} size="sm" title={t("widgets.picker.title")}>
      <ListGroup className="pb-1">
        {WIDGET_ORDER.filter((type) => typeAvailable(type, where)).map((type) => {
          const Icon = WIDGET_ICON[type];
          return (
            <ListRowButton
              key={type}
              icon={<Icon size={icon.lg} />}
              title={t(`widgets.types.${type}.label`)}
              description={t(`widgets.types.${type}.desc`)}
              chevron={false}
              onClick={() => onPick(type)}
            />
          );
        })}
        <ListRowButton
          icon={<MousePointerClick size={icon.lg} />}
          title={t("widgets.presets.actions.label")}
          description={t("widgets.presets.actions.desc")}
          chevron={false}
          onClick={() => onPick("actions")}
        />
      </ListGroup>
    </Sheet>
  );
}
