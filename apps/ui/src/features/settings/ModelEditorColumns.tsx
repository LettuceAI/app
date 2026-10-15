import type { ReactNode } from "react";

export function ModelEditorColumns({ primary, secondary, secondaryLabel }: { primary: ReactNode; secondary?: ReactNode; secondaryLabel?: string | undefined }) {
  const column = "@container w-full min-w-0 max-w-2xl space-y-6";
  return <div className={secondary ? "grid items-start gap-6 @[85.5rem]:grid-cols-2" : "space-y-6"}>
    <div className={column}>{primary}</div>
    {secondary && <div role={secondaryLabel ? "region" : undefined} aria-label={secondaryLabel} className={column}>{secondary}</div>}
  </div>;
}
