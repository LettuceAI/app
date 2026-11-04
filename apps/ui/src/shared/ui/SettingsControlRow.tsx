import type { ReactNode } from "react";
import { cn, ListRow } from "@lettuceai/crisp";

/**
 * A setting with a title and one control. On a phone the title takes the first line, with
 * `marker` after it and `reset` at the end, and the control takes the full width of the
 * second line. Elsewhere the control sits at the end of the title line, after `reset`.
 */
export function SettingsControlRow({
  phone,
  icon: rowIcon,
  title,
  description,
  control,
  tone,
  marker,
  reset,
}: {
  phone: boolean;
  icon?: ReactNode;
  title: ReactNode;
  description?: ReactNode;
  control: ReactNode;
  tone?: "warning";
  marker?: ReactNode;
  reset?: ReactNode;
}) {
  const heading = (
    <span className="inline-flex max-w-full items-center gap-2 whitespace-normal">
      {title}
      {marker}
    </span>
  );
  if (!phone)
    return (
      <ListRow
        icon={rowIcon}
        title={heading}
        description={description}
        {...(tone ? { tone } : {})}
        trailing={reset ? <span className="flex items-center gap-2">{reset}{control}</span> : control}
      />
    );
  return (
    <div>
      <ListRow icon={rowIcon} title={heading} description={description} {...(tone ? { tone } : {})} {...(reset ? { trailing: reset } : {})} />
      <div className={cn("-mt-1 px-4 pb-3", rowIcon && "pl-16")}>{control}</div>
    </div>
  );
}
