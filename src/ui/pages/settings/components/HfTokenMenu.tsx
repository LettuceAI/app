import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ExternalLink, Loader2 } from "lucide-react";

import { BottomMenu } from "../../../components/BottomMenu";
import { useI18n } from "../../../../core/i18n/context";
import { openExternalUrl } from "../../../../core/utils/openExternal";
import { cn } from "../../../design-tokens";
import type { AuthStatus } from "./imageBundle/types";

export function isHfAuthError(message: string): boolean {
  return /requires an access token|token is invalid or expired/i.test(message);
}

export function isHfGatedError(message: string): boolean {
  return isHfAuthError(message) || /accept access to|gated model license/i.test(message);
}

export function HfTokenMenu({
  isOpen,
  onClose,
  gated = false,
  modelId = null,
  onSaved,
}: {
  isOpen: boolean;
  onClose: () => void;
  gated?: boolean;
  modelId?: string | null;
  onSaved?: (status: AuthStatus) => void;
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
      const status = await invoke<AuthStatus>("hf_auth_save", { token });
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
    <BottomMenu isOpen={isOpen} onClose={close} title={t("hfBrowser.bundle.hfAccess")}>
      <p className="mb-4 text-[12.5px] leading-relaxed text-fg/55">
        {gated ? t("hfAuth.gatedBody") : t("hfAuth.manageBody")}
      </p>
      {gated && modelId && (
        <button
          type="button"
          onClick={() => void openExternalUrl(`https://huggingface.co/${modelId}`)}
          className="mb-4 flex w-full items-center justify-center gap-2 rounded-xl border border-fg/10 bg-fg/4 px-4 py-3 text-sm font-medium text-fg/85 transition hover:border-fg/20 hover:text-fg active:scale-[0.99]"
        >
          <ExternalLink className="h-4 w-4 text-fg/45" />
          {t("hfBrowser.browseOnHuggingFace")}
        </button>
      )}
      <input
        type="password"
        value={token}
        onChange={(event) => setToken(event.target.value)}
        placeholder={t("hfBrowser.bundle.tokenPlaceholder")}
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
        {t("hfBrowser.bundle.saveToken")}
      </button>
      {gated && (
        <p className="mt-3 text-[11px] leading-relaxed text-fg/45">
          {t("hfAuth.gatedApprovalNote")}
        </p>
      )}
      <p className="mt-3 text-[10.5px] leading-relaxed text-fg/40">
        {t("hfBrowser.bundle.tokenPrivacy")}
      </p>
    </BottomMenu>
  );
}
