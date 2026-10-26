import { useTranslation } from "react-i18next";
import { Alert, Button } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";

export function HfAccessNotice({ error, onToken }: { error: unknown; onToken: () => void }) {
  const { t } = useTranslation("settings");
  if (
    !isApiFailure(error) ||
    error.details?.type !== "hugging_face" ||
    !["token_missing", "token_invalid", "gated_access"].includes(error.details.failure.type)
  )
    return null;
  return (
    <Alert
      tone="warning"
      title={t("hf.gatedTitle")}
      description={t("hf.gatedBody")}
      action={
        <Button variant="secondary" onClick={onToken}>
          {t("hf.token")}
        </Button>
      }
    />
  );
}
