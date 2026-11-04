export function formatMessageTime(
  at: number | Date,
  format: "time" | "time12" | "datetime",
  locale: string,
): string {
  return new Intl.DateTimeFormat(
    locale,
    format === "datetime"
      ? { dateStyle: "medium", timeStyle: "short" }
      : {
          hour: format === "time12" ? "numeric" : "2-digit",
          minute: "2-digit",
          hourCycle: format === "time12" ? "h12" : "h23",
        },
  ).format(at);
}
