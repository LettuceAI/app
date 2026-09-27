import { ErrorState } from "@lettuceai/crisp";
import { useTranslation } from "react-i18next";
import { isApiFailure } from "@/api/client";

export interface FailureStateProps {
  error: unknown;
  onRetry?: () => void;
  size?: "sm" | "md";
}

/** A failed load, described by its typed error code. */
export function FailureState({ error, onRetry, size = "md" }: FailureStateProps) {
  const { t } = useTranslation();
  const code = isApiFailure(error) ? error.code : "internal";
  return (
    <ErrorState
      title={t("errors.loadTitle")}
      description={t(`errors.code.${code}`)}
      retryLabel={t("actions.retry")}
      size={size}
      {...(onRetry ? { onRetry } : {})}
    />
  );
}
