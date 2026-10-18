import { Archive, ArchiveRestore, Copy, Download, MessageSquare, MoreHorizontal } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, icon, IconButton, Menu, MenuContent, MenuItem, MenuSeparator, MenuTrigger } from "@lettuceai/crisp";

export function CharacterActions({ phone, archived, busy, onStartChat, onExport, onDuplicate, onToggleArchive }: {
  phone: boolean; archived: boolean; busy: boolean; onStartChat: () => void; onExport: () => void; onDuplicate: () => void; onToggleArchive: () => void;
}) {
  const { t } = useTranslation("characters");
  return <div className="flex items-center gap-1.5">
    {!phone && <Button variant="secondary" size="sm" disabled={busy} leading={<MessageSquare size={icon.sm} />} onClick={onStartChat}>{t("edit.startChat")}</Button>}
    <Menu>
      <MenuTrigger><IconButton label={t("edit.more")} disabled={busy}><MoreHorizontal size={icon.lg} /></IconButton></MenuTrigger>
      <MenuContent align="end">
        {phone && <MenuItem icon={<MessageSquare size={icon.md} />} onSelect={onStartChat}>{t("edit.startChat")}</MenuItem>}
        <MenuItem icon={<Download size={icon.md} />} onSelect={onExport}>{t("edit.export")}</MenuItem>
        <MenuItem icon={<Copy size={icon.md} />} onSelect={onDuplicate}>{t("edit.duplicate")}</MenuItem>
        <MenuSeparator />
        {archived
          ? <MenuItem icon={<ArchiveRestore size={icon.md} />} onSelect={onToggleArchive}>{t("edit.restore")}</MenuItem>
          : <MenuItem icon={<Archive size={icon.md} />} danger onSelect={onToggleArchive}>{t("edit.archive")}</MenuItem>}
      </MenuContent>
    </Menu>
  </div>;
}
