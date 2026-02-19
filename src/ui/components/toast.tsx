import type { ReactNode } from "react";
import { toast as sonnerToast } from "sonner";
import { AlertTriangle, CheckCircle2, Info, XCircle } from "lucide-react";
import { cn } from "../design-tokens";

type ToastVariant = "info" | "warning" | "success" | "error";

// Base styling that matches your UI design language
const baseClassName =
  "pointer-events-auto flex w-full max-w-md items-start gap-3 rounded-xl border border-fg/20 bg-surface/80 px-4 py-3 shadow-lg backdrop-blur-md";

const titleClassName = "text-sm font-semibold text-fg leading-tight";
const descriptionClassName = "text-xs text-fg/80 leading-relaxed";

// More subtle, professional variant styling that fits your dark UI
const variantClasses: Record<ToastVariant, string> = {
  info: "border-info/30 bg-info/20",
  warning: "border-warning/30 bg-warning/20",
  success: "border-accent/30 bg-accent/20",
  error: "border-danger/30 bg-danger/20",
};

// Icon styling to match your UI's color scheme
const variantIcons: Record<ToastVariant, ReactNode> = {
  info: <Info className="h-4 w-4 text-info shrink-0 mt-0.5" />,
  warning: <AlertTriangle className="h-4 w-4 text-warning shrink-0 mt-0.5" />,
  success: <CheckCircle2 className="h-4 w-4 text-accent shrink-0 mt-0.5" />,
  error: <XCircle className="h-4 w-4 text-danger shrink-0 mt-0.5" />,
};

type ToastActionOptions = {
  actionLabel?: string;
  onAction?: () => void;
  id?: string | number;
  duration?: number | typeof Infinity;
};

function showToast(
  variant: ToastVariant,
  title: string,
  description?: string,
  options?: ToastActionOptions,
) {
  return sonnerToast(
    <div className="flex items-start gap-3 w-full">
      {variantIcons[variant]}
      <div className="flex-1 min-w-0">
        <div className={titleClassName}>{title}</div>
        {description && <div className={cn(descriptionClassName, "mt-0.5")}>{description}</div>}
      </div>
      {options?.actionLabel && (
        <button
          onClick={() => {
            options.onAction?.();
            if (options.id) sonnerToast.dismiss(options.id);
          }}
          className={cn(
            "shrink-0 rounded-lg border border-fg/30 bg-fg/5 px-3 py-1.5",
            "text-xs font-medium text-fg/70",
            "hover:bg-fg/10 hover:text-fg transition-colors",
          )}
        >
          {options.actionLabel}
        </button>
      )}
    </div>,
    {
      className: cn(baseClassName, variantClasses[variant]),
      unstyled: true,
      duration: options?.duration ?? 5000,
      id: options?.id,
    },
  );
}

export const toast = {
  info: (title: string, description?: string, options?: ToastActionOptions) =>
    showToast("info", title, description, options),

  warning: (title: string, description?: string, options?: ToastActionOptions) =>
    showToast("warning", title, description, options),

  success: (title: string, description?: string, options?: ToastActionOptions) =>
    showToast("success", title, description, options),

  error: (title: string, description?: string, options?: ToastActionOptions) =>
    showToast("error", title, description, options),

  // Legacy support for warningAction
  warningAction: (
    title: string,
    description: string | undefined,
    actionLabel: string,
    onAction: () => void,
    id?: string | number,
  ) => showToast("warning", title, description, { actionLabel, onAction, id }),
  warningSticky: (
    title: string,
    description: string | undefined,
    actionLabel: string,
    onAction: () => void,
    id?: string | number,
  ) =>
    showToast("warning", title, description, {
      actionLabel,
      onAction,
      id,
      duration: Infinity,
    }),
  dismiss: (id: string | number) => sonnerToast.dismiss(id),
  isVisible: (id: string | number) =>
    sonnerToast.getToasts().some((entry: any) => entry?.id === id && !entry?.dismiss),
};
