import { useTranslation } from "react-i18next";
import { Button } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";

export interface ActionErrorProps {
  error: unknown;
  busy: boolean;
  onRetry: () => void;
}

export function ActionError({ error, busy, onRetry }: ActionErrorProps) {
  const { t: common } = useTranslation("common");
  if (error === null || error === undefined) return null;
  const code = isApiFailure(error) ? error.code : "internal";
  return (
    <div role="alert" className="space-y-1">
      <p className="text-base text-danger">{common(`errors.code.${code}`)}</p>
      <Button variant="ghost" size="sm" disabled={busy} onClick={onRetry}>
        {common("actions.retry")}
      </Button>
    </div>
  );
}
