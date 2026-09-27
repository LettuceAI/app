import { EmptyState } from "@lettuceai/crisp";
import { useTranslation } from "react-i18next";

export function RouteNotFound() {
  const { t } = useTranslation();
  return <EmptyState title={t("errors.notFoundTitle")} description={t("errors.notFoundDescription")} />;
}
