import type { ReactNode } from "react";
import { cn, ListRow } from "@lettuceai/crisp";

export function SettingsControlRow({
  phone,
  icon: rowIcon,
  title,
  description,
  control,
  tone,
}: {
  phone: boolean;
  icon?: ReactNode;
  title: string;
  description?: ReactNode;
  control: ReactNode;
  tone?: "warning";
}) {
  if (!phone)
    return (
      <ListRow
        icon={rowIcon}
        title={title}
        description={description}
        {...(tone ? { tone } : {})}
        trailing={control}
      />
    );
  return (
    <div>
      <ListRow icon={rowIcon} title={title} description={description} {...(tone ? { tone } : {})} />
      <div className={cn("-mt-1 px-4 pb-3", rowIcon && "pl-16")}>{control}</div>
    </div>
  );
}
