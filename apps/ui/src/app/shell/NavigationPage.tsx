import { Page } from "@lettuceai/crisp";
import { useLocation } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { destinationOf } from "./navigation";

export function NavigationPage() {
  const pathname = useLocation({ select: (location) => location.pathname });
  const { t } = useTranslation();
  return <Page title={t(`navigation.${destinationOf(pathname)}`)}>{null}</Page>;
}
