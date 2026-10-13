import { Check } from "lucide-react";
import { useTranslation } from "react-i18next";
import { icon, ListGroup, ListRowButton, Sheet } from "@lettuceai/crisp";
import { phoneLayouts, type PhoneLayout } from "../data/layout";
import { layoutIcons } from "./layout-icons";

export function LayoutSheet({ open, value, onClose, onChange }: { open: boolean; value: PhoneLayout; onClose: () => void; onChange: (layout: PhoneLayout) => void }) {
  const { t } = useTranslation("home");
  return (
    <Sheet open={open} onClose={onClose} size="sm" title={t("layout.title")} description={t("layout.body")}>
      <ListGroup className="mb-1">
        {phoneLayouts.map((option) => (
          <ListRowButton
            key={option}
            icon={layoutIcons[option]}
            tone={option === value ? "accent" : "neutral"}
            title={t(`layout.${option}.title`)}
            description={t(`layout.${option}.body`)}
            chevron={false}
            aria-pressed={option === value}
            trailing={option === value ? <Check size={icon.md} className="text-accent" /> : undefined}
            onClick={() => {
              onChange(option);
              onClose();
            }}
          />
        ))}
      </ListGroup>
    </Sheet>
  );
}
