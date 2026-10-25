import type { TFunction } from "i18next";
import type { UsageRow } from "@/api/generated/bindings";
export const operations = [
  "chat",
  "regenerate",
  "continue",
  "group_chat_message",
  "group_chat_regenerate",
  "group_chat_continue",
  "summary",
  "memory_manager",
  "reply_helper",
  "image_generation",
  "ai_creator",
  "companion_soul_writer",
  "companion_growth",
  "companion_consolidation",
  "lorebook_plan",
  "lorebook_entry_write",
  "lorebook_entry_refine",
  "lorebook_coherence",
  "lorebook_keywords",
  "lorebook_generator",
  "lorebook_entry_generator",
] as const;
export function operationLabel(value: string | null, t: TFunction<"usage">) {
  return value && operations.includes(value as (typeof operations)[number])
    ? t(`operations.${value as (typeof operations)[number]}`)
    : (value ?? t("unknownFeature"));
}
export function number(value: number, locale: string, compact = false) {
  return new Intl.NumberFormat(locale, {
    ...(compact ? { notation: "compact" as const } : {}),
    maximumFractionDigits: compact ? 1 : 0,
  }).format(value);
}
export function money(value: number, locale: string) {
  return new Intl.NumberFormat(locale, {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: Math.abs(value) > 0 && Math.abs(value) < 0.01 ? 4 : 2,
    maximumFractionDigits: Math.abs(value) > 0 && Math.abs(value) < 0.01 ? 4 : 2,
  }).format(value);
}
export function duration(ms: number, locale: string, t: TFunction<"usage">) {
  const minutes = Math.floor(ms / 60000);
  return minutes >= 60
    ? t("durationHours", {
        hours: number(Math.floor(minutes / 60), locale),
        minutes: number(minutes % 60, locale),
      })
    : minutes > 0
      ? t("durationMinutes", { minutes: number(minutes, locale) })
      : t("durationSeconds", { seconds: number(Math.floor(ms / 1000), locale) });
}
export function chartDuration(ms: number, locale: string) {
  const divisor = ms >= 3600000 ? 3600000 : ms >= 60000 ? 60000 : 1000;
  return new Intl.NumberFormat(locale, {
    style: "unit",
    unit: divisor === 3600000 ? "hour" : divisor === 60000 ? "minute" : "second",
    unitDisplay: "narrow",
    maximumFractionDigits: 1,
  }).format(ms / divisor);
}
export function dateLabel(day: string, locale: string, year = false) {
  const date = new Date(`${day}T12:00:00`);
  return Number.isNaN(date.getTime())
    ? day
    : new Intl.DateTimeFormat(locale, {
        month: "short",
        day: "numeric",
        ...(year ? { year: "numeric" as const } : {}),
      }).format(date);
}
export function timestamp(value: number, locale: string) {
  return new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(value);
}
export function rowTitle(row: UsageRow, t: TFunction<"usage">) {
  return [operationLabel(row.operation_type, t), row.character_name].filter(Boolean).join(" · ");
}
export function rowLine(row: UsageRow, locale: string, t: TFunction<"usage">) {
  return [
    timestamp(row.timestamp, locale),
    row.model_name ?? t("unknownModel"),
    row.prompt_tokens === null
      ? t("noTokenCounts")
      : t("rowTokens", {
          input: number(row.prompt_tokens, locale, true),
          output:
            row.completion_tokens === null
              ? t("notReported")
              : number(row.completion_tokens, locale, true),
          cache:
            row.cached_prompt_tokens !== null
              ? t("rowCache", { value: number(row.cached_prompt_tokens, locale, true) })
              : "",
        }),
  ].join(" · ");
}

export function quotaDate(value: string | null, locale: string) {
  if (value === null) return null;
  const numeric = Number(value);
  const date =
    Number.isFinite(numeric) && value.trim()
      ? new Date(numeric < 1e12 ? numeric * 1000 : numeric)
      : new Date(value);
  return Number.isNaN(date.getTime()) ? null : timestamp(date.getTime(), locale);
}
