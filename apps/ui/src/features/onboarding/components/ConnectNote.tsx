import { useTranslation } from "react-i18next";
import { Button } from "@lettuceai/crisp";
import { useDescriptor, useOnboarding } from "../flow/context";

export function ConnectNote() {
  const { t } = useTranslation("onboarding");
  const { t: common } = useTranslation("common");
  const { state, connect } = useOnboarding();
  const descriptor = useDescriptor(state.providerKind);
  const error = connect.error;
  if (!error) return null;
  return (
    <div role="alert" className="space-y-1">
      {error.kind === "rejected" && (
        <>
          <p className="text-base text-danger">{t("connect.error.rejected")}</p>
          {error.providerMessage && <p className="text-sm text-fg-3">{error.providerMessage}</p>}
        </>
      )}
      {error.kind === "unreachable" && (
        <p className="text-base text-danger">
          {t("connect.error.unreachable", { name: descriptor?.display_name ?? "" })}
        </p>
      )}
      {error.kind === "code" && <p className="text-base text-danger">{common(`errors.code.${error.code}`)}</p>}
      {error.kind !== "rejected" && (
        <Button variant="ghost" size="sm" disabled={connect.busy} onClick={connect.run}>
          {common("actions.retry")}
        </Button>
      )}
    </div>
  );
}
