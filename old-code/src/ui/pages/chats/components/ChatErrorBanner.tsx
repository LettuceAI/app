import { useState } from "react";
import { AlertCircle, ChevronDown, Copy, Info, X } from "lucide-react";
import { cn, radius, typography } from "../../../design-tokens";
import { explainChatError } from "../../../../core/chat/errorExplainer";
import { useI18n } from "../../../../core/i18n/context";

interface ChatErrorBannerProps {
  error: string;
  onDismiss?: () => void;
}

export function ChatErrorBanner({ error, onDismiss }: ChatErrorBannerProps) {
  const { t } = useI18n();
  const explained = explainChatError(error);
  const [expanded, setExpanded] = useState(false);
  const [copied, setCopied] = useState(false);

  if (!explained) return null;

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(explained.raw);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // ignore
    }
  };

  const isInfo = explained.category === "aborted";
  const Icon = isInfo ? Info : AlertCircle;

  return (
    <div
      className={cn(
        "mb-3 overflow-hidden border border-fg/10 bg-surface-el/30 backdrop-blur-xl",
        radius.md,
      )}
    >
      <div className="flex items-start gap-3 px-3.5 py-3">
        <div
          className={cn(
            "mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-full",
            isInfo ? "bg-fg/[0.06] text-fg/65" : "bg-danger/[0.12] text-danger/85",
          )}
        >
          <Icon size={16} strokeWidth={2} />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <p className={cn(typography.bodySmall.size, "font-medium leading-tight text-fg")}>
              {explained.title}
            </p>
            {explained.status && (
              <span className="rounded-md border border-fg/10 px-1 py-0 text-[10px] font-medium tabular-nums text-fg/45">
                {explained.status}
              </span>
            )}
          </div>
          <p className={cn("mt-1 leading-relaxed", typography.bodySmall.size, "text-fg/60")}>
            {explained.explanation}
          </p>
          {explained.suggestion && (
            <p className={cn("mt-1 leading-relaxed", typography.bodySmall.size, "text-fg/45")}>
              {explained.suggestion}
            </p>
          )}
          <button
            type="button"
            onClick={() => setExpanded((v) => !v)}
            className="mt-1.5 inline-flex items-center gap-1 text-[11px] font-medium text-fg/40 transition hover:text-fg/65"
          >
            <ChevronDown
              size={11}
              strokeWidth={2}
              className={cn("transition-transform", expanded && "rotate-180")}
            />
            {expanded
              ? t("chats.errorBanner.hideDetails")
              : t("chats.errorBanner.showDetails")}
          </button>
          {expanded && (
            <div className="mt-2 space-y-1.5">
              <pre className="max-h-32 overflow-auto whitespace-pre-wrap break-words rounded-md border border-fg/10 bg-surface/60 px-2 py-1.5 text-[11px] leading-snug text-fg/55">
                {explained.raw}
              </pre>
              <button
                type="button"
                onClick={handleCopy}
                className="inline-flex items-center gap-1 text-[11px] font-medium text-fg/40 transition hover:text-fg/65"
              >
                <Copy size={10} strokeWidth={2} />
                {copied
                  ? t("chats.errorBanner.copied")
                  : t("chats.errorBanner.copyRawError")}
              </button>
            </div>
          )}
        </div>
        {onDismiss && (
          <button
            type="button"
            onClick={onDismiss}
            className="shrink-0 rounded-md p-1 text-fg/35 transition hover:bg-fg/[0.06] hover:text-fg/70"
            aria-label={t("chats.errorBanner.dismiss")}
          >
            <X size={13} strokeWidth={2} />
          </button>
        )}
      </div>
    </div>
  );
}
