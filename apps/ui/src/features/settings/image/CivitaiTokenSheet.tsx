import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Alert, Button, Field, SecretInput, Sheet, toast } from "@lettuceai/crisp";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { useCivitaiAuth } from "./local-data";

export function CivitaiTokenSheet({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const client = useQueryClient();
  const auth = useCivitaiAuth();
  const [token, setToken] = useState("");
  const save = useMutation({
    mutationFn: (run: () => Promise<unknown>) => run(),
    onSettled: () => client.invalidateQueries({ queryKey: queryKeys.civitai.auth() }),
  });
  const close = () => { setToken(""); save.reset(); onClose(); };
  return (
    <Sheet open={open} onClose={close} title={t("image.local.lora.token")} dismissible={!save.isPending}>
      <div className="space-y-3">
        <Field label={t("image.local.lora.token")} hint={t("image.local.lora.tokenPrivacy")}><SecretInput value={token} onChange={setToken} aria-label={t("image.local.lora.token")} /></Field>
        <div className="flex gap-2">
          <Button disabled={!token.trim() || save.isPending} onClick={() => save.mutate(async () => {
            const status = await api.call("civitaiAuthSave", { token: token.trim() });
            if (status.valid) toast.success(t("image.local.lora.tokenSaved"));
            else toast.warning(t("image.local.lora.tokenUnverified"));
          }, { onSuccess: close })}>{t("manage.save")}</Button>
          {auth.data?.saved && <Button variant="ghost" disabled={save.isPending} onClick={() => save.mutate(() => api.call("civitaiAuthClear"), { onSuccess: close })}>{t("image.local.remove")}</Button>}
        </div>
        {save.isError && <FailureState error={save.error} size="sm" />}
      </div>
    </Sheet>
  );
}

export function TokenAlert({ saved, onOpen }: { saved: boolean; onOpen: () => void }) {
  const { t } = useTranslation("settings");
  return <Alert tone="warning" title={t(saved ? "image.local.lora.tokenInvalid" : "image.local.lora.tokenMissing")} description={t("image.local.lora.tokenBody")} action={<Button variant="secondary" size="sm" onClick={onOpen}>{t(saved ? "image.local.lora.tokenReplace" : "image.local.lora.tokenAdd")}</Button>} />;
}
