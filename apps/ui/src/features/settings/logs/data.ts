import { queryOptions } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { LogLevel, LogLineView, LogViewFilter } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export const pageSize = 500;
export const levels: LogLevel[] = ["error", "warn", "info", "debug"];
export const defaultLevels: LogLevel[] = ["error", "warn", "info"];
export type Size = "sm" | "md" | "lg";
export type SearchOption = "case" | "word" | "regex";
export type Panel =
  | "line"
  | "filters"
  | "parts"
  | "time"
  | "more"
  | "report"
  | "delete-day"
  | "delete-all";
export interface Line extends LogLineView {
  n: number;
  at: string;
  time: string;
  timestamp: string;
  level: LogLevel | null;
  component: string;
  location: string;
  message: string;
}
export function parseLine(row: LogLineView): Line {
  const match = /^\[([^\]]+)\]\s+(DEBUG|INFO|WARN|ERROR)\s+(\S+)(?:\s+at=(\S+))?\s*\|\s?(.*)$/.exec(
    row.raw,
  );
  return {
    ...row,
    n: row.index + 1,
    at: match?.[4] ?? "",
    timestamp: match?.[1] ?? "",
    time: match?.[1]?.split("T")[1]?.replace(/(?:Z|[+-]\d\d:\d\d)$/, "") ?? "",
    level: match ? (match[2]!.toLowerCase() as LogLevel) : null,
    component: match?.[3] ?? "",
    location: match?.[4] ?? "",
    message: match?.[5] ?? row.raw,
  };
}
export const filesQuery = (api: ApiClient) =>
  queryOptions({
    queryKey: [...queryKeys.logs.all, "files"],
    queryFn: () => api.call("logsList"),
  });
export const viewQuery = (api: ApiClient, name: string, filter: LogViewFilter, offset = 0) =>
  queryOptions({
    queryKey: [...queryKeys.logs.all, "view", name, filter, offset],
    queryFn: () => api.call("logQuery", { name, filter, offset, limit: pageSize }),
  });
export function matcherFor(query: string, options: SearchOption[]): RegExp | null {
  if (!query) return null;
  try {
    const source = options.includes("regex") ? query : query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    return new RegExp(
      options.includes("word") && !options.includes("regex") ? `\\b${source}\\b` : source,
      options.includes("case") ? "g" : "gi",
    );
  } catch {
    return null;
  }
}
export function rawText(rows: Iterable<LogLineView>): string {
  return [...rows]
    .sort((a, b) => a.index - b.index)
    .map((row) => row.raw)
    .join("\n");
}
export function dayLabel(
  name: string,
  language: string,
  today: Date,
  labels: { today: string; yesterday: string },
): string {
  const match = /^app-(\d{4})-(\d{2})-(\d{2})\.log$/.exec(name);
  if (!match) return name;
  const day = new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
  if (Number.isNaN(day.getTime())) return name;
  const label = new Intl.DateTimeFormat(language, {
    weekday: "short",
    day: "numeric",
    month: "short",
  }).format(day);
  const start = new Date(today.getFullYear(), today.getMonth(), today.getDate());
  const yesterday = new Date(start);
  yesterday.setDate(yesterday.getDate() - 1);
  return day.getTime() === start.getTime()
    ? `${labels.today} · ${label}`
    : day.getTime() === yesterday.getTime()
      ? `${labels.yesterday} · ${label}`
      : label;
}
