import { useTranslation } from "react-i18next";
import { Check } from "lucide-react";
import { icon, ListGroup, ListRowButton, Text } from "@lettuceai/crisp";

type Format = "json" | "xml";

export function StructuredFallbackSelector({
  value,
  onChange,
}: {
  value: Format;
  onChange: (value: Format) => void;
}) {
  const { t } = useTranslation("common");
  return (
    <section className="space-y-2">
      <ListGroup label={t("structuredFallback.title")}>
        {(["json", "xml"] as const).map((format) => (
          <ListRowButton
            key={format}
            title={t(`structuredFallback.formats.${format}.title`)}
            description={t(`structuredFallback.formats.${format}.body`)}
            tone={format === value ? "accent" : "neutral"}
            chevron={false}
            aria-pressed={format === value}
            trailing={
              format === value ? <Check size={icon.md} className="text-accent" /> : undefined
            }
            onClick={() => onChange(format)}
          />
        ))}
      </ListGroup>
      <Text variant="hint" className="px-1">
        {t("structuredFallback.hint")}
      </Text>
    </section>
  );
}
