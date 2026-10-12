import { LoadingState } from "@lettuceai/crisp";
import { useTranslation } from "react-i18next";

export function PendingScreen() {
  const { t } = useTranslation();
  return <LoadingState label={t("status.loading")} />;
}
