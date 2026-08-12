import { useEffect, useState } from "react";
import { AlertTriangle } from "lucide-react";
import { BottomMenu } from "./BottomMenu";
import { cn, radius } from "../design-tokens";
import { useI18n } from "../../core/i18n/context";

type ConfirmOptions = {
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  destructive?: boolean;
  warning?: { title: string; body: string };
};

type ConfirmRequest = {
  options: ConfirmOptions;
  resolve: (value: boolean) => void;
};

let confirmHandler: ((request: ConfirmRequest) => void) | null = null;

export function confirmBottomMenu(options: ConfirmOptions): Promise<boolean> {
  return new Promise((resolve) => {
    if (!confirmHandler) {
      resolve(false);
      return;
    }
    confirmHandler({ options, resolve });
  });
}

export function ConfirmBottomMenuHost() {
  const { t } = useI18n();
  const [request, setRequest] = useState<ConfirmRequest | null>(null);

  useEffect(() => {
    confirmHandler = (next) => {
      setRequest(next);
    };
    return () => {
      confirmHandler = null;
    };
  }, []);

  const close = (value: boolean) => {
    request?.resolve(value);
    setRequest(null);
  };

  const options = request?.options;
  const confirmLabel = options?.confirmLabel ?? t("components.confirmDialog.defaultLabel");
  const cancelLabel = options?.cancelLabel ?? t("common.buttons.cancel");

  return (
    <BottomMenu
      isOpen={Boolean(request)}
      onClose={() => close(false)}
      title={options?.title ?? t("components.confirmDialog.defaultTitle")}
    >
      <div className="space-y-4">
        {options?.warning ? (
          <div className={cn(radius.lg, "border border-warning/30 bg-warning/10 p-3")}>
            <div className="flex items-center gap-2 text-warning">
              <AlertTriangle className="h-4 w-4" />
              <p className="text-sm font-semibold">{options.warning.title}</p>
            </div>
            <p className="mt-1.5 text-xs leading-relaxed text-fg/70">{options.warning.body}</p>
          </div>
        ) : (
          <p className="text-sm text-fg/70">{options?.message}</p>
        )}
        <div className="flex gap-3">
          <button
            onClick={() => close(false)}
            className={cn(
              "flex-1 py-3 text-sm font-medium",
              radius.lg,
              "border border-fg/10 bg-fg/5 text-fg",
              "transition hover:border-fg/20 hover:bg-fg/10",
            )}
          >
            {cancelLabel}
          </button>
          <button
            onClick={() => close(true)}
            className={cn(
              "flex-1 py-3 text-sm font-medium",
              radius.lg,
              "border",
              options?.destructive
                ? "border-red-500/30 bg-red-500/20 text-red-200 hover:bg-red-500/30"
                : "border-emerald-500/30 bg-emerald-500/20 text-emerald-100 hover:bg-emerald-500/30",
              "transition",
            )}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </BottomMenu>
  );
}
