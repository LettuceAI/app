import { useTranslation } from "react-i18next";
import { Input } from "@lettuceai/crisp";
import { ConnectNote } from "../components/ConnectNote";
import { ScreenTitle } from "../components/ui";
import type { GuidedRoute } from "../content/slides";
import { useOnboarding } from "../flow/context";

export function KeyScreen({ route }: { route: GuidedRoute }) {
  const { t } = useTranslation("onboarding");
  const { state, dispatch } = useOnboarding();
  const routeName = t(`guide.routes.${route}`);
  return (
    <>
      <ScreenTitle title={t("guide.key.title")} lead={t(`guide.key.${route}Body`)} />
      <div className="max-w-md space-y-3">
        <Input
          type="password"
          value={state.apiKey}
          onChange={(event) => dispatch({ type: "patch", patch: { apiKey: event.target.value } })}
          placeholder={t(`guide.key.${route}Placeholder`)}
          className="control-lg text-lg"
        />
        <p className="text-sm text-fg-3">{t("guide.key.hint", { name: routeName })}</p>
        <ConnectNote />
      </div>
    </>
  );
}
