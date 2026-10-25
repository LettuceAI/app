import { useTranslation } from "react-i18next";
import { useNavigate } from "@tanstack/react-router";
import { Alert, Button } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import { ActionError } from "@/shared/ui/ActionError";

export function CreationError({
  error,
  busy,
  retry,
}: {
  error: unknown;
  busy: boolean;
  retry: () => void;
}) {
  const { t } = useTranslation("lorebook-create");
  const navigate = useNavigate();
  if (!error) return null;
  const details = isApiFailure(error) ? error.details : null;
  if (details?.type === "lorebook_model_unavailable")
    return (
      <Alert
        tone="warning"
        title={t("modelUnavailable")}
        description={t(`modelErrors.${details.reason}`)}
        action={
          <Button
            size="sm"
            variant="secondary"
            onClick={() =>
              void navigate({
                to: "/settings/$section",
                params: { section: "lorebooks" },
                search: { loreTab: "lorebook" },
              })
            }
          >
            {t("openSettings")}
          </Button>
        }
      />
    );
  return <ActionError error={error} busy={busy} onRetry={retry} />;
}
