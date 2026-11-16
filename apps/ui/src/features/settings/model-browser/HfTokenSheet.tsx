import { useState } from "react";
import { useQueryClient, type UseQueryResult } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Lock } from "lucide-react";
import { Alert, Button, Field, LoadingState, SecretInput, Sheet, Text, icon } from "@lettuceai/crisp";
import type { HfTokenStatus } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsAction, useSettingsApi } from "../data";
import { browserKey } from "./data";

export function HfTokenSheet({
  open,
  onClose,
  auth,
}: {
  open: boolean;
  onClose: () => void;
  auth: UseQueryResult<HfTokenStatus>;
}) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const client = useQueryClient();
  const action = useSettingsAction();
  const [token, setToken] = useState("");
  const refresh = () =>
    Promise.all([
      auth.refetch(),
      client.invalidateQueries({ queryKey: browserKey }),
      client.invalidateQueries({ queryKey: queryKeys.localImages.all }),
    ]);
  const valid = auth.data?.type === "valid";
  return (
    <Sheet open={open} onClose={onClose} title={t("hf.token")} description={t("hf.tokenSheetHint")}>
      <div className="space-y-4 pb-2">
        {auth.isPending ? (
          <LoadingState />
        ) : auth.isError ? (
          <FailureState error={auth.error} onRetry={() => void auth.refetch()} />
        ) : auth.data.type === "valid" ? (
          <Alert
            tone="success"
            title={t("hf.tokenValid", { name: auth.data.username })}
            description={t("hf.tokenValidBody")}
            action={
              <Button
                size="sm"
                variant="ghost"
                disabled={action.isPending}
                onClick={() =>
                  action.mutate(async () => {
                    await api.call("hfAuthClear");
                    await refresh();
                  })
                }
              >
                {t("hf.removeToken")}
              </Button>
            }
          />
        ) : auth.data.type === "invalid" ? (
          <Alert tone="danger" title={t("hf.tokenRefused")} description={t("hf.tokenRefusedBody")} />
        ) : auth.data.type === "unknown" ? (
          <Text variant="hint">{t("hf.tokenUnknown")}</Text>
        ) : null}
        {!valid && (
          <>
            <Field label={t("hf.tokenField")} hint={t("hf.tokenFieldHint")}>
              <SecretInput value={token} onChange={setToken} placeholder={tokenPlaceholder} />
            </Field>
            <Text variant="hint" className="flex items-start gap-1.5">
              <Lock size={icon.sm} className="mt-0.5 shrink-0" />
              {t("hf.tokenPrivacy")}
            </Text>
            <div className="flex justify-end gap-2">
              <Button variant="ghost" onClick={onClose}>
                {t("manage.cancel")}
              </Button>
              <Button
                variant="primary"
                disabled={!token.trim()}
                loading={action.isPending}
                onClick={() =>
                  action.mutate(async () => {
                    await api.call("hfAuthSave", { token: token.trim() });
                    setToken("");
                    await refresh();
                  })
                }
              >
                {t("hf.saveToken")}
              </Button>
            </div>
          </>
        )}
        {action.isError && <FailureState error={action.error} size="sm" />}
      </div>
    </Sheet>
  );
}

const tokenPlaceholder = "hf_…";
