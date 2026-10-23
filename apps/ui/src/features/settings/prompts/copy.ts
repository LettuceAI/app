import { useCallback } from "react";
import { useTranslation } from "react-i18next";
export function usePromptText() {
  const { t, i18n } = useTranslation("prompts");
  const translate = useCallback((key: string, values?: Record<string, unknown>): string => t(key as never, { ...values, keySeparator: false, nsSeparator: false }) as string, [t]);
  return { t: translate, i18n };
}
