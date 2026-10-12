import { useTranslation } from "react-i18next";
import { useDescriptor, useOnboarding } from "./context";

export function useRouteName(): string {
  const { t } = useTranslation("onboarding");
  const { state } = useOnboarding();
  const descriptor = useDescriptor(state.providerKind);
  return state.route === "provider" ? (descriptor?.display_name ?? "") : t(`guide.routes.${state.route}`);
}
