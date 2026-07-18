import { Loader, X } from "lucide-react";

import { useI18n } from "../../../../../core/i18n/context";
import type { ImageBundleController } from "./useImageBundle";

export function BundleAuthPanel({ bundle }: { bundle: ImageBundleController }) {
  const { t } = useI18n();

  return (
    <div className="rounded-xl border border-fg/10 bg-fg/3 p-3">
      <div className="flex items-center gap-2">
        <input
          type="password"
          value={bundle.token}
          onChange={(event) => bundle.setToken(event.target.value)}
          placeholder={t("hfBrowser.bundle.tokenPlaceholder")}
          className="h-9 min-w-0 flex-1 rounded-lg border border-fg/10 bg-surface px-2.5 text-[12px] text-fg outline-none transition focus:border-accent/40"
        />
        <button
          onClick={() => void bundle.saveToken()}
          disabled={bundle.authBusy || !bundle.token.trim()}
          className="flex h-9 items-center rounded-lg bg-accent px-3 text-[11.5px] font-semibold text-white disabled:opacity-40"
        >
          {bundle.authBusy ? (
            <Loader size={13} className="animate-spin" />
          ) : (
            t("hfBrowser.bundle.saveToken")
          )}
        </button>
        {bundle.auth?.saved && (
          <button
            onClick={() => void bundle.clearToken()}
            className="flex h-9 items-center rounded-lg border border-fg/10 px-2.5 text-fg/55 hover:text-fg"
          >
            <X size={13} />
          </button>
        )}
      </div>
      <p className="mt-2 text-[10.5px] leading-relaxed text-fg/40">
        {t("hfBrowser.bundle.tokenPrivacy")}
      </p>
    </div>
  );
}
