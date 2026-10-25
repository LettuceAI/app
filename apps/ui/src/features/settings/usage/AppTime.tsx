import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Chart, EmptyState, Panel, Skeleton, StatTile, Text } from "@lettuceai/crisp";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { timeQuery, timeSummary, type Range } from "./data";
import { chartDuration, dateLabel, duration } from "./format";
import { RangeControl } from "./Filters";
export function AppTime({
  range,
  now,
  phone,
  onRange,
}: {
  range: Range;
  now: number;
  phone: boolean;
  onRange: (value: Range) => void;
}) {
  const { t, i18n } = useTranslation("usage");
  const api = useSettingsApi();
  const query = useQuery(timeQuery(api));
  const result = timeSummary(query.data?.days ?? [], range, now);
  const format = (value: number) => duration(value, i18n.language, t);
  const previous = result.previous;
  const trend =
    previous !== null && previous > 0
      ? {
          value: Math.abs(Math.round(((result.total - previous) / previous) * 100)),
          direction: result.total >= previous ? ("up" as const) : ("down" as const),
        }
      : undefined;
  return (
    <div className="space-y-5">
      <RangeControl value={range} onChange={onRange} phone={phone} />
      {query.isError ? (
        <FailureState error={query.error} onRetry={() => void query.refetch()} />
      ) : query.isPending ? (
        <div role="status" aria-label={t("loadingTime")} className="space-y-3">
          <Skeleton shape="block" className="h-24 w-full" />
          <Skeleton shape="block" className="h-64 w-full" />
        </div>
      ) : query.data.days.length === 0 ? (
        <EmptyState title={t("noTime")} description={t("noTimeBody")} />
      ) : (
        <>
          <div className={phone ? "grid grid-cols-2 gap-2.5" : "grid grid-cols-4 gap-2.5"}>
            <StatTile
              label={t("inRange")}
              value={format(result.total)}
              sub={t("dayCount", { count: result.keys.length })}
              {...(trend ? { trend } : {})}
              highlight
            />
            <StatTile label={t("dailyAverage")} value={format(result.average)} sub={t("inRange")} />
            <StatTile
              label={t("ranges.today")}
              value={format(result.today)}
              sub={t("yesterdayValue", { value: format(result.yesterday) })}
            />
            <StatTile label={t("monthAverage")} value={format(result.monthAverage)} />
          </div>
          <Panel
            title={t("timePerDay")}
            description={t("foregroundOnly")}
            action={<Text variant="hint">{t("totalValue", { value: format(result.total) })}</Text>}
          >
            {range === "today" ? (
              <Text variant="hint" className="py-10 text-center">
                {t("longerRange")}
              </Text>
            ) : (
              <Chart
                data={result.keys.map((day) => ({
                  day: dateLabel(day, i18n.language, range === "all"),
                  active: result.byDay.get(day) ?? 0,
                }))}
                x="day"
                kind={"area"}
                series={[{ key: "active", label: t("timeInApp"), color: "var(--color-accent)" }]}
                format={(value) => chartDuration(value, i18n.language)}
                height={phone ? 190 : 230}
              />
            )}
          </Panel>
        </>
      )}
      <Text variant="hint" className="px-1">
        {t("deviceTimeHint")}
      </Text>
    </div>
  );
}
