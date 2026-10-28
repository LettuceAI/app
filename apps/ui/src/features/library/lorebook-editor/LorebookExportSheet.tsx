import { FileCode, Package } from "lucide-react";
import { useTranslation } from "react-i18next";
import { icon, ListGroup, ListRowButton, Sheet } from "@lettuceai/crisp";
import type { LorebookExportFormat } from "@/api/generated/bindings";

const formats: readonly { format: LorebookExportFormat; glyph: React.ReactNode }[] = [
  { format: "uec", glyph: <Package size={icon.lg} /> },
  { format: "world_info", glyph: <FileCode size={icon.lg} /> },
];

export function LorebookExportSheet({
  open,
  busy,
  onClose,
  onChoose,
}: {
  open: boolean;
  busy: boolean;
  onClose: () => void;
  onChoose: (format: LorebookExportFormat) => void;
}) {
  const { t } = useTranslation("lorebook-editor");
  return (
    <Sheet
      open={open}
      onClose={onClose}
      dismissible={!busy}
      size="sm"
      title={t("exportTitle")}
      description={t("exportBody")}
    >
      <ListGroup className="mb-1">
        {formats.map(({ format, glyph }) => (
          <ListRowButton
            key={format}
            icon={glyph}
            title={t(`exportFormats.${format}.title`)}
            description={t(`exportFormats.${format}.body`)}
            chevron={false}
            disabled={busy}
            onClick={() => onChoose(format)}
          />
        ))}
      </ListGroup>
    </Sheet>
  );
}
