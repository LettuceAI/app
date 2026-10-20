import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import { isApiFailure } from "@/api/client";

/** The localized text for a failed memory call: the refused reason when the backend names one, else the error code. */
export function useMemoryError(): (error: unknown) => string {
  const { t } = useTranslation("conversations");
  const { t: common } = useTranslation("common");
  return useCallback(
    (error) => {
      if (!isApiFailure(error)) return common("errors.code.internal");
      const details = error.details;
      if (details?.type === "memory_gate") return t(`memory.gate.${details.gate}`);
      if (details?.type === "memory_cycle_dependent") return t("memory.activity.dependent");
      if (details?.type === "memory_cycle_user_edited") return t("memory.activity.userEdited");
      return common(`errors.code.${error.code}`);
    },
    [t, common],
  );
}
