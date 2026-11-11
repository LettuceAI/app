import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ListGroup, Skeleton, SkeletonRow } from "@lettuceai/crisp";

/**
 * Loading state shaped like the page it stands in for: an optional full-width group, then the two-column grid,
 * each group a label bar over a card with that many rows. `aside` replaces the last column when it holds something else.
 */
export function VoicesSkeleton({ top, columns, aside }: { top?: number; columns: readonly (readonly number[])[]; aside?: ReactNode }) {
  const { t } = useTranslation("settings");
  const group = (rows: number, key: string) => (
    <div key={key} className="space-y-2">
      <Skeleton className="ml-1 h-3 w-24" />
      <ListGroup>{Array.from({ length: rows }, (_, index) => <SkeletonRow key={index} className="px-4" />)}</ListGroup>
    </div>
  );
  return (
    <div className="@container space-y-7" role="status" aria-label={t("voices.loading")}>
      {top !== undefined && group(top, "top")}
      <div className="grid grid-cols-1 gap-7 @4xl:grid-cols-2 @4xl:items-start @4xl:gap-x-10">
        {columns.map((groups, column) => <div key={column} className="min-w-0 space-y-7">{groups.map((rows, index) => group(rows, `${column}-${index}`))}</div>)}
        {aside !== undefined && <div className="min-w-0">{aside}</div>}
      </div>
    </div>
  );
}

/** The voice library while its catalog loads: the search row, then language blocks of chip-shaped bars. */
export function LibrarySkeleton({ label }: { label?: ReactNode }) {
  const chips = (count: number, offset: number) => Array.from({ length: count }, (_, index) => <Skeleton key={index} className="h-8 rounded-full" width={56 + ((index + offset) % 3) * 16} />);
  return (
    <div className="space-y-2">
      {label ?? <Skeleton className="ml-1 h-3 w-24" />}
      <ListGroup>
        <div className="px-4 py-3"><Skeleton className="h-10 w-full rounded-lg" /></div>
        {[5, 3, 2].map((count, block) => (
          <div key={block} className="space-y-2 px-4 py-3">
            <div className="flex items-baseline justify-between gap-3"><Skeleton className="h-4 w-32" /><Skeleton className="h-3 w-20" /></div>
            {[count, Math.max(1, count - 1)].map((line, row) => (
              <div key={row} className="flex gap-3">
                <Skeleton className="mt-2.5 h-3 w-14 shrink-0" />
                <div className="flex min-w-0 flex-1 flex-wrap gap-1.5">{chips(line, block + row)}</div>
              </div>
            ))}
          </div>
        ))}
      </ListGroup>
    </div>
  );
}
