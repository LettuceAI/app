import { useTranslation } from "react-i18next";
import { Badge, ListRowButton } from "@lettuceai/crisp";
import type { UsageRow } from "@/api/generated/bindings";
import { money, rowLine, rowTitle } from "./format";
export function UsageRows({ rows, onOpen }: { rows: UsageRow[]; onOpen: (row: UsageRow) => void }) {
  const { t, i18n } = useTranslation("usage");
  return rows.map((row) => (
    <ListRowButton
      key={row.id}
      title={rowTitle(row, t)}
      description={rowLine(row, i18n.language, t)}
      trailing={
        row.status !== "succeeded" ? (
          <Badge
            tone={
              row.status === "failed"
                ? "danger"
                : row.status === "interrupted"
                  ? "warning"
                  : "neutral"
            }
          >
            {t(`statuses.${row.status}`)}
          </Badge>
        ) : row.total_cost !== null ? (
          <span className="tabular-nums text-accent">{money(row.total_cost, i18n.language)}</span>
        ) : undefined
      }
      onClick={() => onOpen(row)}
    />
  ));
}
