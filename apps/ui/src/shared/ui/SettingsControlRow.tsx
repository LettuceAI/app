import type { ReactNode } from "react";
import { cn, ListRow } from "@lettuceai/crisp";

export function SettingsControlRow({
  phone,
  icon: rowIcon,
  title,
  description,
  control,
  tone,
  inline = false,
}: {
  phone: boolean;
  icon?: ReactNode;
  title: string;
  description?: ReactNode;
  control: ReactNode;
  tone?: "warning";
  inline?: boolean;
}) {
  if (!phone || inline)
    return (
      <ListRow
        icon={rowIcon}
        title={inline ? <span className="whitespace-normal">{title}</span> : title}
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
