import type { ReactNode } from "react";
import { toast as sonnerToast } from "sonner";
import { AlertTriangle, CheckCircle2, Cpu, Info, LoaderCircle, XCircle } from "lucide-react";
import { cn } from "../design-tokens";

type ToastVariant = "info" | "warning" | "success" | "error";

// Base styling that matches your UI design language
const baseClassName =
  "pointer-events-auto flex w-full max-w-md items-start gap-3 rounded-xl border border-fg/20 bg-surface px-4 py-3 shadow-lg backdrop-blur-md";

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
  actionTone?: "neutral" | "accent";
  secondaryLabel?: string;
  onSecondary?: () => void;
  secondaryTone?: "neutral" | "danger";
  id?: string | number;
  duration?: number | typeof Infinity;
};

type ModelLoadToastOptions = {
  id?: string | number;
  title: string;
  subtitle: string;
  modelName: string;
  progress: number;
  duration?: number | typeof Infinity;
};

function showToast(
  variant: ToastVariant,
  title: string,
  description?: string,
  options?: ToastActionOptions,
) {
  const handleAction = (callback?: () => void) => (event: React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();
    callback?.();
    if (options?.id) sonnerToast.dismiss(options.id);
  };

  return sonnerToast(
    <div className="flex items-start gap-3 w-full">
      {variantIcons[variant]}
      <div className="flex-1 min-w-0">
        <div className={titleClassName}>{title}</div>
        {description && <div className={cn(descriptionClassName, "mt-0.5")}>{description}</div>}
      </div>
      {(options?.actionLabel || options?.secondaryLabel) && (
        <div className="flex shrink-0 gap-1.5">
          {options?.actionLabel && (
            <button
              type="button"
              onPointerDown={(event) => event.stopPropagation()}
              onClick={handleAction(options.onAction)}
              className={cn(
                "shrink-0 touch-manipulation rounded-lg px-3 py-1.5 text-xs font-medium transition-colors",
                options?.actionTone === "accent"
                  ? "border border-accent/35 bg-accent/18 text-accent hover:bg-accent/24"
                  : "border border-fg/30 bg-fg/5 text-fg/70 hover:bg-fg/10 hover:text-fg",
              )}
            >
              {options.actionLabel}
            </button>
          )}
          {options?.secondaryLabel && (
            <button
              type="button"
              onPointerDown={(event) => event.stopPropagation()}
              onClick={handleAction(options.onSecondary)}
              className={cn(
                "shrink-0 touch-manipulation rounded-lg px-3 py-1.5 text-xs font-medium transition-colors",
                options?.secondaryTone === "neutral"
                  ? "border border-fg/30 bg-fg/5 text-fg/70 hover:bg-fg/10 hover:text-fg"
                  : "border border-danger/30 bg-danger/10 text-danger hover:bg-danger/20",
              )}
            >
              {options.secondaryLabel}
            </button>
          )}
        </div>
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

function showModelLoadToast(options: ModelLoadToastOptions) {
  const progress = Math.min(1, Math.max(0, options.progress));
  const percent = Math.round(progress * 100);

  return sonnerToast(
    <div
      className="flex items-start gap-3 overflow-hidden"
      style={{ maxWidth: "calc(var(--width) - 32px)" }}
    >
      <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-fg/12 bg-fg/5">
        <Cpu className="h-4 w-4 text-fg/80" />
      </div>
      <div className="min-w-0 flex-1">
        <div className={titleClassName}>{options.title}</div>
        <div className={cn(descriptionClassName, "mt-0.5 truncate")}>{options.subtitle}</div>
        <div className="mt-2 flex min-w-0 items-center gap-1.5 text-[11px] text-fg/55">
          <LoaderCircle className="h-3.5 w-3.5 shrink-0 animate-spin" />
          <span className="min-w-0 truncate">{options.modelName}</span>
        </div>
        <div className="mt-2 flex items-center justify-between gap-3 text-[11px] text-fg/60">
          <span>Startup progress</span>
          <span className="shrink-0 font-medium tabular-nums text-fg/72">{percent}%</span>
        </div>
        <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-fg/10">
          <div
            className="h-full rounded-full bg-accent transition-[width] duration-150 ease-out"
            style={{ width: `${percent}%` }}
          />
        </div>
      </div>
    </div>,
    {
      className: cn(baseClassName, "border-fg/15 bg-surface/96 overflow-hidden"),
      unstyled: true,
      duration: options.duration ?? Infinity,
      id: options.id,
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
  modelLoad: (options: ModelLoadToastOptions) => showModelLoadToast(options),

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
