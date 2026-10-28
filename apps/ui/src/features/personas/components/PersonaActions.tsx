import { Download, MoreHorizontal, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { icon, IconButton, Menu, MenuContent, MenuItem, MenuSeparator, MenuTrigger } from "@lettuceai/crisp";

export function PersonaActions({ busy, onExport, onDelete }: { busy: boolean; onExport: () => void; onDelete: () => void }) {
  const { t } = useTranslation("personas");
  return <Menu>
    <MenuTrigger><IconButton label={t("edit.more")} disabled={busy}><MoreHorizontal size={icon.lg} /></IconButton></MenuTrigger>
    <MenuContent align="end">
      <MenuItem icon={<Download size={icon.md} />} onSelect={onExport}>{t("edit.export")}</MenuItem>
      <MenuSeparator />
      <MenuItem icon={<Trash2 size={icon.md} />} danger onSelect={onDelete}>{t("edit.delete")}</MenuItem>
    </MenuContent>
  </Menu>;
}
