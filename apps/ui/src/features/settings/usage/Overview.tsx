import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { ArrowRight, Zap } from "lucide-react";
import {
  Button,
  Chart,
  EmptyState,
  Field,
  icon,
  Panel,
  Progress,
  Segmented,
  SelectMenu,
  Skeleton,
  StatTile,
  Text,
} from "@lettuceai/crisp";
import type { UsageGroupBy, UsageGroupTotals, UsageRow } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { calendarDays, rangeDates, statsQuery, type Range } from "./data";
import { dateLabel, money, number, operationLabel } from "./format";
import { ReportFilters } from "./Filters";
import { UsageRows } from "./UsageRows";

function Ranked({
  items,
  kind,
}: {
  items: UsageGroupTotals[];
  kind: Exclude<UsageGroupBy, "day">;
}) {
  const { t, i18n } = useTranslation("usage");
  const [shown, setShown] = useState(5);
  const sorted = [...items].sort(
    (a, b) => (b.totals.total_tokens ?? 0) - (a.totals.total_tokens ?? 0),
  );
  const max = sorted.reduce((highest, item) => Math.max(highest, item.totals.total_tokens ?? 0), 1);
  return items.length === 0 ? (
    <Text variant="hint">{t("nothingYet")}</Text>
  ) : (
    <div className="space-y-3">
      <ol className="space-y-3">
        {sorted.slice(0, shown).map((item, index) => (
          <li key={item.key ?? "unknown"} className="space-y-1.5">
            <div className="flex items-baseline justify-between gap-3">
              <div className="flex min-w-0 items-baseline gap-2">
                <span className="w-4 shrink-0 text-xs font-semibold tabular-nums text-fg-3">
                  {number(index + 1, i18n.language)}
                </span>
                <span className="truncate text-sm font-medium text-fg">
                  {kind === "operation"
                    ? operationLabel(item.key, t)
                    : (item.label ??
                      t(
                        kind === "character"
                          ? "noCharacter"
                          : kind === "model"
                            ? "unknownModel"
                            : "unknownProvider",
                      ))}
                </span>
              </div>
              <span className="shrink-0 text-sm tabular-nums text-fg-2">
                {item.totals.total_tokens === null
                  ? t("notReported")
                  : t("tokenCount", {
                      value: number(item.totals.total_tokens, i18n.language, true),
                    })}
                {item.totals.total_cost !== null && (
                  <span className="ml-2 text-accent">
                    {money(item.totals.total_cost, i18n.language)}
                  </span>
                )}
              </span>
            </div>
            <div className="ml-6">
              <Progress
                size="sm"
                value={((item.totals.total_tokens ?? 0) / max) * 100}
                label={t("tokenShare")}
              />
              <p className="mt-1 text-xs tabular-nums text-fg-3">
                {t("rankedDetail", {
                  input:
                    item.totals.prompt_tokens === null
                      ? t("notReported")
                      : number(item.totals.prompt_tokens, i18n.language, true),
                  output:
                    item.totals.completion_tokens === null
                      ? t("notReported")
                      : number(item.totals.completion_tokens, i18n.language, true),
                  count: item.totals.requests,
                })}
              </p>
            </div>
          </li>
        ))}
      </ol>
      {shown < sorted.length && (
        <Button size="sm" variant="ghost" onClick={() => setShown((value) => value + 5)}>
          {t("showMore")}
        </Button>
      )}
    </div>
  );
}
function GroupPanel({
  kind,
  range,
  provider,
  now,
}: {
  kind: Exclude<UsageGroupBy, "day">;
  range: Range;
  provider: string;
  now: number;
}) {
  const { t } = useTranslation("usage");
  const api = useSettingsApi();
  const query = useQuery(statsQuery(api, range, provider, kind, now));
  return (
    <Panel
      title={t(`groupTitles.${kind}`)}
      description={query.data ? t("groupCount", { count: query.data.groups.length }) : undefined}
    >
      {query.isError ? (
        <FailureState error={query.error} onRetry={() => void query.refetch()} size="sm" />
      ) : query.data ? (
        <Ranked items={query.data.groups} kind={kind} />
      ) : (
        <div className="space-y-3">
          {[0, 1, 2].map((value) => (
            <Skeleton key={value} className="h-12 w-full" />
          ))}
        </div>
      )}
    </Panel>
  );
}
export function Overview({
  range,
  provider,
  now,
  phone,
  onRange,
  onProvider,
  onOpen,
  onRequests,
}: {
  range: Range;
  provider: string;
  now: number;
  phone: boolean;
  onRange: (range: Range) => void;
  onProvider: (provider: string) => void;
  onOpen: (row: UsageRow) => void;
  onRequests: () => void;
}) {
  const { t, i18n } = useTranslation("usage");
  const api = useSettingsApi();
  const [metric, setMetric] = useState<"tokens" | "cost" | "requests">("tokens");
  const summary = useQuery(statsQuery(api, range, provider, "day", now));
  const providers = useQuery(statsQuery(api, "all", "all", "provider", now));
  const recent = useQuery({
    queryKey: [...queryKeys.usage.all, "recent", rangeDates(range, now), provider],
    queryFn: () =>
      api.call("usageQuery", {
        filters: {
          range: rangeDates(range, now),
          provider_kind: provider === "all" ? null : provider,
          model_id: null,
          character_id: null,
          operation_kind: null,
          status: null,
        },
        sort: "newest_first",
        cursor: null,
        limit: 6,
      }),
    enabled: summary.isSuccess && summary.data.totals.requests > 0,
  });
  const totals = summary.data?.totals;
  const byDay = new Map(
    summary.data?.groups.flatMap((group) =>
      group.key ? [[group.key, group.totals] as const] : [],
    ) ?? [],
  );
  const days = calendarDays(
    range,
    now,
    summary.data?.groups.find((group) => group.key)?.key ?? undefined,
  );
  const series = days.map((day) => {
    const value = byDay.get(day);
    return {
      day: dateLabel(day, i18n.language, range === "all"),
      input: value ? value.prompt_tokens : 0,
      cached: value ? value.cached_prompt_tokens : 0,
      output: value ? value.completion_tokens : 0,
      cost: value ? value.total_cost : 0,
      requests: value?.requests ?? 0,
    };
  });
  const cache = totals?.cached_prompt_tokens;
  const input = totals?.prompt_tokens;
  const cachePercent =
    cache !== null &&
    cache !== undefined &&
    input !== null &&
    input !== undefined &&
    input > 0 &&
    cache <= input
      ? new Intl.NumberFormat(i18n.language, { style: "percent", maximumFractionDigits: 0 }).format(
          cache / input,
        )
      : null;
  return (
    <div className="space-y-5">
      <ReportFilters
        range={range}
        onRange={onRange}
        provider={provider}
        onProvider={onProvider}
        phone={phone}
        providers={(providers.data?.groups ?? []).flatMap((group) =>
          group.key ? [{ value: group.key, label: group.label ?? group.key }] : [],
        )}
      />
      {providers.isError && (
        <FailureState error={providers.error} onRetry={() => void providers.refetch()} size="sm" />
      )}
      {summary.isError ? (
        <FailureState error={summary.error} onRetry={() => void summary.refetch()} />
      ) : !totals ? (
        <div role="status" aria-label={t("loadingSummary")} className="space-y-4">
          <div className="grid grid-cols-2 gap-2.5 @xl:grid-cols-4">
            {[0, 1, 2, 3].map((value) => (
              <Skeleton key={value} shape="block" className="h-24 w-full" />
            ))}
          </div>
          <Skeleton shape="block" className="h-72 w-full" />
        </div>
      ) : totals.requests === 0 ? (
        <EmptyState
          icon={<Zap size={icon.xl} />}
          title={t("emptyRange")}
          description={t("emptyRangeBody")}
        />
      ) : (
        <>
          <div className={phone ? "grid grid-cols-2 gap-2.5" : "grid grid-cols-4 gap-2.5"}>
            <StatTile
              label={t("knownCost")}
              value={
                totals.total_cost === null ? t("none") : money(totals.total_cost, i18n.language)
              }
              sub={t("pricedCoverage", {
                known: totals.requests - totals.unknown_cost_requests,
                count: totals.requests,
              })}
              highlight
            />
            <StatTile
              label={t("tokens")}
              value={
                totals.total_tokens === null
                  ? t("notReported")
                  : number(totals.total_tokens, i18n.language, true)
              }
              sub={
                totals.unknown_token_requests
                  ? t("unknownTokens", { count: totals.unknown_token_requests })
                  : t("inputOutput", {
                      input:
                        totals.prompt_tokens === null
                          ? t("notReported")
                          : number(totals.prompt_tokens, i18n.language, true),
                      output:
                        totals.completion_tokens === null
                          ? t("notReported")
                          : number(totals.completion_tokens, i18n.language, true),
                    })
              }
            />
            <StatTile
              label={t("cached")}
              value={
                cachePercent ??
                (cache === null || cache === undefined
                  ? t("notReported")
                  : number(cache, i18n.language, true))
              }
              sub={t("cacheCoverage", {
                count: totals.cache_reporting_requests,
                requests: totals.requests,
              })}
            />
            <StatTile
              label={t("requests")}
              value={number(totals.requests, i18n.language)}
              sub={t("failedCount", { count: totals.failed_requests })}
            />
          </div>
          <Panel
            title={t("trend")}
            description={t(`trendBody.${metric}`)}
            action={
              phone ? (
                <Field
                  label={<span className="sr-only">{t("chartMeasure")}</span>}
                  className="space-y-0"
                >
                  <SelectMenu
                    className="w-32"
                    value={metric}
                    onChange={(value) => value && setMetric(value)}
                    options={(["tokens", "cost", "requests"] as const).map((value) => ({
                      value,
                      label: t(value),
                    }))}
                  />
                </Field>
              ) : (
                <Segmented
                  size="sm"
                  aria-label={t("chartMeasure")}
                  value={metric}
                  onChange={setMetric}
                  options={(["tokens", "cost", "requests"] as const).map((value) => ({
                    value,
                    label: t(value),
                  }))}
                />
              )
            }
          >
            {metric === "tokens" && (
              <div className="flex flex-wrap gap-3 text-xs text-fg-3">
                {(["input", "cached", "output"] as const).map((key, index) => (
                  <span key={key} className="flex items-center gap-1.5">
                    <span
                      className="h-2 w-2 rounded-full"
                      style={{
                        background: [
                          "var(--color-info)",
                          "var(--color-secondary)",
                          "var(--color-accent)",
                        ][index],
                      }}
                    />
                    {t(key)}
                  </span>
                ))}
              </div>
            )}
            {metric === "cost" && totals.total_cost === null ? (
              <Text variant="hint" className="py-10 text-center">
                {t("noPrices")}
              </Text>
            ) : metric === "tokens" && totals.total_tokens === null ? (
              <Text variant="hint" className="py-10 text-center">
                {t("noTokenCounts")}
              </Text>
            ) : range === "today" ? (
              <Text variant="hint" className="py-10 text-center">
                {t("longerRange")}
              </Text>
            ) : (
              <Chart
                data={series}
                x="day"
                kind={metric === "requests" ? "bar" : "area"}
                series={
                  metric === "tokens"
                    ? [
                        { key: "input", label: t("input"), color: "var(--color-info)" },
                        { key: "cached", label: t("cached"), color: "var(--color-secondary)" },
                        { key: "output", label: t("output"), color: "var(--color-accent)" },
                      ]
                    : [
                        {
                          key: metric,
                          label: t(metric === "cost" ? "knownCost" : "requests"),
                          color: "var(--color-accent)",
                        },
                      ]
                }
                format={(value) =>
                  metric === "cost"
                    ? money(value, i18n.language)
                    : number(value, i18n.language, true)
                }
                height={phone ? 190 : 230}
              />
            )}
          </Panel>
          <div className={phone ? "grid gap-4" : "grid grid-cols-2 gap-4"}>
            {(["model", "character", "operation", "provider"] as const).map((kind) => (
              <GroupPanel key={kind} kind={kind} range={range} provider={provider} now={now} />
            ))}
          </div>
          <Panel
            title={t("recent")}
            description={t("recentBody")}
            action={
              <Button
                size="sm"
                variant="ghost"
                trailing={<ArrowRight size={icon.sm} />}
                onClick={onRequests}
              >
                {t("viewAll")}
              </Button>
            }
          >
            {recent.isError ? (
              <FailureState error={recent.error} onRetry={() => void recent.refetch()} size="sm" />
            ) : recent.data ? (
              <div className="-mx-3.5 -mb-3 divide-y divide-line border-t border-line">
                <UsageRows rows={recent.data.items} onOpen={onOpen} />
              </div>
            ) : (
              <Skeleton className="h-20 w-full" />
            )}
          </Panel>
          <Text variant="hint" className="px-1">
            {t("pricingHint")}
          </Text>
        </>
      )}
    </div>
  );
}
