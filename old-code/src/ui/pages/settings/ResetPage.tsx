import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { AlertTriangle, Trash2 } from "lucide-react";
import { exit } from "@tauri-apps/plugin-process";

import { ResetManager } from "../../../core/storage/reset";
import { typography, interactive, cn } from "../../design-tokens";
import { useI18n, type TranslationKey } from "../../../core/i18n/context";
import { confirmBottomMenu } from "../../components/ConfirmBottomMenu";

export function ResetPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [isResetting, setIsResetting] = useState(false);

  const handleResetClick = async () => {
    const confirmed = await confirmBottomMenu({
      title: t("reset.confirmTitle"),
      message: t("reset.confirmDescription"),
      confirmLabel: t("reset.confirmButton"),
      cancelLabel: t("common.buttons.cancel"),
      destructive: true,
    });
    if (!confirmed) return;

    try {
      setIsResetting(true);
      await ResetManager.resetAllData();
      await exit(0);
    } catch (error: any) {
      console.error("Reset failed:", error);
      const message =
        error instanceof Error
          ? error.message
          : typeof error === "string"
            ? error
            : t("reset.unknownError");
      alert(t("reset.failed", { message }));
      setIsResetting(false);
    }
  };

  const removed = [
    "reset.items.providers",
    "reset.items.characters",
    "reset.items.preferences",
  ] satisfies TranslationKey[];

  return (
    <div className="flex h-full flex-col">
      <section className="flex-1 overflow-y-auto">
        <div className="mx-auto w-full max-w-2xl px-3 pt-4 pb-10 sm:px-4 lg:pt-8">
          <div className="flex flex-col gap-7">
            <div className="flex flex-col gap-2">
              <h1
                className={cn(
                  typography.h1.size,
                  typography.h1.weight,
                  typography.h1.tracking,
                  "text-fg",
                )}
              >
                {t("reset.title")}
              </h1>
              <p className={cn(typography.body.size, "leading-relaxed text-fg/55")}>
                {t("reset.description")}
              </p>
            </div>

            <div className="flex flex-col gap-2">
              <h2
                className={cn(
                  "px-1",
                  typography.overline.size,
                  typography.overline.weight,
                  typography.overline.tracking,
                  typography.overline.transform,
                  "text-fg/40",
                )}
              >
                {t("reset.willBeRemoved")}
              </h2>
              <div
                className={cn(
                  "overflow-hidden rounded-xl",
                  "border border-fg/10 bg-fg/[0.025]",
                  "divide-y divide-fg/[0.06]",
                )}
              >
                {removed.map((key) => (
                  <div key={key} className="flex items-center gap-3 px-4 py-3">
                    <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-danger/60" />
                    <span className={cn(typography.body.size, "text-fg/75")}>{t(key)}</span>
                  </div>
                ))}
              </div>
              <div
                className={cn(
                  "mt-1 inline-flex items-center gap-1.5 self-start px-1",
                  typography.caption.size,
                  "font-medium text-danger/85",
                )}
              >
                <AlertTriangle className="h-3.5 w-3.5" strokeWidth={2.5} />
                {t("reset.warning")}
              </div>
            </div>

            <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
              <button
                type="button"
                onClick={() => navigate("/settings/about")}
                disabled={isResetting}
                className={cn(
                  "inline-flex h-10 items-center justify-center rounded-lg border border-fg/10 px-5",
                  typography.body.size,
                  "font-medium text-fg/75",
                  interactive.transition.default,
                  "hover:bg-fg/[0.04] hover:text-fg",
                  "disabled:cursor-not-allowed disabled:opacity-50",
                )}
              >
                {t("common.buttons.cancel")}
              </button>
              <button
                type="button"
                onClick={() => void handleResetClick()}
                disabled={isResetting}
                className={cn(
                  "inline-flex h-10 items-center justify-center gap-2 rounded-lg border border-danger/30 bg-danger/10 px-5",
                  typography.body.size,
                  "font-medium text-danger",
                  interactive.transition.default,
                  "hover:bg-danger/15",
                  "disabled:cursor-not-allowed disabled:opacity-50",
                )}
              >
                {isResetting ? (
                  <>
                    <span className="h-4 w-4 animate-spin rounded-full border-2 border-danger/30 border-t-danger" />
                    <span>{t("reset.resetting")}</span>
                  </>
                ) : (
                  <>
                    <Trash2 className="h-4 w-4" />
                    {t("reset.resetButton")}
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}
