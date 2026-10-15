import { useQuery } from "@tanstack/react-query";
import { useNavigate, useRouteContext } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Settings } from "lucide-react";
import { icon, IconButton, useCompactLayout } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";

export function MobileSettingsButton() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const { data } = useQuery(appStatusQuery(api));
  const compact = useCompactLayout();
  if (!compact && data?.platform !== "android" && data?.platform !== "ios") return null;
  return <IconButton label={t("navigation.settings")} onClick={() => void navigate({ to: "/settings" })}><Settings size={icon.lg} /></IconButton>;
}
