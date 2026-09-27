import type { i18n } from "i18next";
import { createI18n } from "@/shared/i18n";

/** English, or `cimode`, where every `t()` call renders its key so a test can prove copy is localized. */
export function createTestI18n(language: "en" | "cimode" = "en"): Promise<i18n> {
  return createI18n(language);
}
