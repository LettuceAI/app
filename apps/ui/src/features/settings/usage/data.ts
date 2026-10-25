import { infiniteQueryOptions, queryOptions } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type {
  AppUsageDayView,
  UsageFilters_Deserialize,
  UsageGroupBy,
  UsageRow,
} from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export type Range = "today" | "7d" | "30d" | "all";
export type UsageTab = "overview" | "requests" | "time" | "nanogpt";
export const currentInstant = () => Date.now();
export const timeZone = () => Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
export function dayKey(value: Date) {
  return `${value.getFullYear()}-${String(value.getMonth() + 1).padStart(2, "0")}-${String(value.getDate()).padStart(2, "0")}`;
}
export function rangeDates(range: Range, now: number) {
  if (range === "all") return { start: null, end: null };
  const start = new Date(now);
  start.setHours(0, 0, 0, 0);
  start.setDate(start.getDate() - (range === "7d" ? 6 : range === "30d" ? 29 : 0));
  const end = new Date(now);
  end.setHours(23, 59, 59, 999);
  return { start: start.getTime(), end: end.getTime() };
}
export function calendarDays(range: Range, now: number, earliest?: string) {
  const end = new Date(now);
  end.setHours(12, 0, 0, 0);
  const start =
    range === "all" && earliest
      ? new Date(`${earliest}T12:00:00`)
      : new Date(rangeDates(range === "all" ? "today" : range, now).start!);
  start.setHours(12, 0, 0, 0);
  const days: string[] = [];
  for (const date = new Date(start); date <= end; date.setDate(date.getDate() + 1))
    days.push(dayKey(date));
  return days;
}
export function statsQuery(
  api: ApiClient,
  range: Range,
  provider: string,
  group: UsageGroupBy,
  now: number,
) {
  const request = {
    range: rangeDates(range, now),
    provider_kind: provider === "all" ? null : provider,
    group_by: group,
    time_zone: timeZone(),
  };
  return queryOptions({
    queryKey: [...queryKeys.usage.all, "stats", request],
    queryFn: () => api.call("usageStats", request),
  });
}
export function requestsQuery(api: ApiClient, filters: UsageFilters_Deserialize) {
  return infiniteQueryOptions({
    queryKey: [...queryKeys.usage.all, "requests", filters],
    initialPageParam: null as string | null,
    queryFn: ({ pageParam }) =>
      api.call("usageQuery", { filters, sort: "newest_first", cursor: pageParam, limit: 25 }),
    getNextPageParam: (page) => page.next_cursor,
  });
}
export function detailQuery(api: ApiClient, id: string) {
  return queryOptions({
    queryKey: [...queryKeys.usage.all, "detail", id],
    queryFn: () => api.call("usageGet", { id }),
    enabled: Boolean(id),
  });
}
export function timeQuery(api: ApiClient) {
  return queryOptions({
    queryKey: queryKeys.usage.time(),
    queryFn: () => api.call("appUsageDays"),
  });
}
export function nanoQuery(api: ApiClient, accountId: string) {
  return queryOptions({
    queryKey: queryKeys.usage.nano(accountId),
    queryFn: () => api.call("providerNanogptUsage", { account_id: accountId, refresh: false }),
    enabled: Boolean(accountId),
  });
}
export function clockQuery() {
  return queryOptions({ queryKey: queryKeys.usage.clock(), queryFn: currentInstant });
}
export function requestFilters(
  status: string,
  operation: string,
  model: string,
): UsageFilters_Deserialize {
  return {
    range: { start: null, end: null },
    provider_kind: null,
    character_id: null,
    model_id: model === "all" ? null : model,
    operation_kind: operation === "all" ? null : operation,
    status: status === "all" ? null : (status as UsageFilters_Deserialize["status"]),
  };
}
export function rowTokens(row: UsageRow) {
  return (
    row.total_tokens ??
    (row.prompt_tokens !== null && row.completion_tokens !== null
      ? row.prompt_tokens + row.completion_tokens
      : null)
  );
}
export function timeSummary(days: AppUsageDayView[], range: Range, now: number) {
  const sorted = [...days].sort((a, b) => a.day.localeCompare(b.day));
  const keys = calendarDays(range, now, sorted[0]?.day);
  const byDay = new Map(days.map((day) => [day.day, day.active_ms]));
  const total = keys.reduce((sum, key) => sum + (byDay.get(key) ?? 0), 0);
  const previousEnd = new Date(`${keys[0] ?? dayKey(new Date(now))}T12:00:00`);
  previousEnd.setDate(previousEnd.getDate() - 1);
  const previous =
    range === "all"
      ? null
      : calendarDays(range, previousEnd.getTime()).reduce(
          (sum, key) => sum + (byDay.get(key) ?? 0),
          0,
        );
  const today = dayKey(new Date(now));
  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  const month = calendarDays("30d", now).reduce((sum, key) => sum + (byDay.get(key) ?? 0), 0);
  return {
    keys,
    byDay,
    total,
    previous,
    average: keys.length ? total / keys.length : 0,
    today: byDay.get(today) ?? 0,
    yesterday: byDay.get(dayKey(yesterday)) ?? 0,
    monthAverage: month / 30,
  };
}
