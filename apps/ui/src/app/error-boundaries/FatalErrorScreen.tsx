import { Button, EmptyState } from "@lettuceai/crisp";
import { AlertTriangle } from "lucide-react";
import { useTranslation } from "react-i18next";
import { isApiFailure } from "@/api/client";

export interface FatalErrorScreenProps {
  error: unknown;
}

export function FatalErrorScreen({ error }: FatalErrorScreenProps) {
  const { t } = useTranslation();
  const description = isApiFailure(error) ? t(`errors.code.${error.code}`) : t("errors.fatalDescription");
  return (
    <div className="flex h-full min-h-screen items-center justify-center bg-surface">
      <EmptyState
        icon={<AlertTriangle />}
        title={t("errors.fatalTitle")}
        description={description}
        action={
          <Button variant="primary" onClick={() => window.location.reload()}>
            {t("actions.reload")}
          </Button>
        }
      />
    </div>
  );
}
