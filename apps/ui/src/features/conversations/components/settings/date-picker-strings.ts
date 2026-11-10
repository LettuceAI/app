import type { TFunction } from "i18next";

export function datePickerStrings(t: TFunction<"conversations">) {
  return { datePickerPreviousMonth: t("settings.time.previousMonth"), datePickerNextMonth: t("settings.time.nextMonth"), datePickerChooseMonth: t("settings.time.chooseMonth"), datePickerPreviousYear: t("settings.time.previousYear"), datePickerNextYear: t("settings.time.nextYear"), datePickerYear: t("settings.time.year"), datePickerTime: t("settings.time.time") };
}
