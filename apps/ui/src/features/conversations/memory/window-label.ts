import { useTranslation } from "react-i18next";
import { windowOf } from "../data/memory";

export function useWindowLabel(): (label: string | null) => string | null {
  const { t } = useTranslation("conversations");
  return (label) => {
    if (label === null) return null;
    const span = windowOf(label);
    return span ? t("memory.window", span) : label;
  };
}
