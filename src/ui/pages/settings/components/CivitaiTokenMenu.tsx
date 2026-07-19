import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ExternalLink, Loader2 } from "lucide-react";

import { BottomMenu } from "../../../components/BottomMenu";
import { useI18n } from "../../../../core/i18n/context";
import { openExternalUrl } from "../../../../core/utils/openExternal";
import { cn } from "../../../design-tokens";

export type CivitaiAuthStatus = {
  saved: boolean;
  valid: boolean;
  errorKind: "missingToken" | "unverified" | "invalidOrExpired" | null;
};

export function isCivitaiAuthError(message: string): boolean {
  return /CivitAI (file requires an API token|token is invalid or expired)/i.test(message);
}

export function CivitaiTokenMenu({
  isOpen,
  onClose,
  onSaved,
}: {
  isOpen: boolean;
  onClose: () => void;
  onSaved?: (status: CivitaiAuthStatus) => void;
}) {
  const { t } = useI18n();
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const close = () => {
    if (busy) return;
    setError(null);
    onClose();
  };

  const save = async () => {
    if (!token.trim() || busy) return;
    setBusy(true);
    setError(null);
    try {
      const status = await invoke<CivitaiAuthStatus>("civitai_auth_save", { token });
      setToken("");
      onSaved?.(status);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <BottomMenu isOpen={isOpen} onClose={close} title={t("loraLibrary.tokenTitle")}>
      <p className="mb-4 text-[12.5px] leading-relaxed text-fg/55">{t("loraLibrary.tokenBody")}</p>
      <button
        type="button"
        onClick={() => void openExternalUrl("https://civitai.com/user/account")}
        className="mb-4 flex w-full items-center justify-center gap-2 rounded-xl border border-fg/10 bg-fg/4 px-4 py-3 text-sm font-medium text-fg/85 transition hover:border-fg/20 hover:text-fg active:scale-[0.99]"
      >
        <ExternalLink className="h-4 w-4 text-fg/45" />
        {t("loraLibrary.tokenGetOne")}
      </button>
      <input
        type="password"
        value={token}
        onChange={(event) => setToken(event.target.value)}
        placeholder={t("loraLibrary.tokenPlaceholder")}
        className="h-11 w-full rounded-xl border border-fg/10 bg-surface px-3 text-sm text-fg outline-none transition focus:border-accent/40"
      />
      {error && <p className="mt-2 text-[12px] leading-relaxed text-danger">{error}</p>}
      <button
        type="button"
        onClick={() => void save()}
        disabled={busy || !token.trim()}
        className={cn(
          "mt-4 flex w-full items-center justify-center gap-2 rounded-xl border border-accent/40 bg-accent/15 px-4 py-3 text-sm font-medium text-accent transition",
          busy || !token.trim()
            ? "cursor-not-allowed opacity-60"
            : "hover:bg-accent/25 active:scale-[0.99]",
        )}
      >
        {busy && <Loader2 className="h-4 w-4 animate-spin" />}
        {t("loraLibrary.tokenSave")}
      </button>
      <p className="mt-3 text-[10.5px] leading-relaxed text-fg/40">
        {t("loraLibrary.tokenPrivacy")}
      </p>
    </BottomMenu>
  );
}
