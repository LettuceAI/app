import type { ComponentProps, ReactNode } from "react";
import { MoreHorizontal } from "lucide-react";
import { icon, IconButton, ListRowButton, Menu, MenuContent, MenuItem, MenuTrigger } from "@lettuceai/crisp";

export interface RowAction {
  label: string;
  icon: ReactNode;
  danger?: boolean;
  disabled?: boolean;
  onSelect: () => void;
}

export function RowMenu({ label, actions }: { label: string; actions: readonly RowAction[] }) {
  return (
    <Menu>
      <MenuTrigger><IconButton label={label}><MoreHorizontal size={icon.lg} /></IconButton></MenuTrigger>
      <MenuContent align="end">
        {actions.map((action) => <MenuItem key={action.label} icon={action.icon} {...(action.danger ? { danger: true } : {})} {...(action.disabled ? { disabled: true } : {})} onSelect={action.onSelect}>{action.label}</MenuItem>)}
      </MenuContent>
    </Menu>
  );
}

/** A row that opens its item, with the item's menu beside it; a button cannot hold the menu's button, so the two sit side by side. */
export function ActionRow({ menuLabel, actions, ...row }: ComponentProps<typeof ListRowButton> & { menuLabel: string; actions: readonly RowAction[] }) {
  return (
    <div className="flex items-center pr-2">
      <ListRowButton {...row} chevron={false} className="min-w-0 flex-1" />
      <RowMenu label={menuLabel} actions={actions} />
    </div>
  );
}
