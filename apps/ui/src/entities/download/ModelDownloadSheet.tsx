import { Download } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, Sheet } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import type { InstallState } from "@/entities/job";
import { ActionButton } from "@/shared/ui/ActionButton";
import { ActionError } from "@/shared/ui/ActionError";
import { DownloadCard } from "./DownloadCard";

export interface ModelDownloadCopy {
  name: string;
  title: string;
  titleDone: string;
  body: string;
  bodyDone: string;
  download: string;
}

export interface ModelDownloadSheetProps {
  open: boolean;
  onClose: () => void;
  install: InstallState;
  onStart: () => void;
  copy: ModelDownloadCopy;
  secondary: { label: string; onClick: () => void };
  done?: { label: string; onClick: () => void; loading: boolean; error: unknown };
}

export function ModelDownloadSheet({ open, onClose, install, onStart, copy, secondary, done: finish }: ModelDownloadSheetProps) {
  const { t, i18n } = useTranslation("common");
  const busy = install.phase === "downloading" || install.phase === "verifying";
  const done = install.phase === "done";
  const failed = install.phase === "failed";
  const failure = install.job?.failure ?? null;
  const failureText = isApiFailure(install.error)
    ? t(`errors.code.${install.error.code}`)
    : failure?.hugging_face?.type === "offline"
      ? t("memoryDownload.failedOffline")
      : failure && i18n.exists(`memoryDownload.failureCode.${failure.code}`, { ns: "common" })
        ? t(`memoryDownload.failureCode.${failure.code}` as never)
        : t("memoryDownload.failedOther");
  const secondaryButton = (
    <Button variant="ghost" size="lg" block onClick={secondary.onClick}>
      {secondary.label}
    </Button>
  );

  return (
    <Sheet
      open={open}
      onClose={onClose}
      dismissible={!busy}
      title={done ? copy.titleDone : copy.title}
      footer={
        done ? (
          finish && <ActionButton label={finish.label} loading={finish.loading} disabled={finish.loading} block onClick={finish.onClick} />
        ) : failed ? (
          <div className="flex flex-col gap-2">
            <ActionButton label={t("actions.retry")} block onClick={onStart} />
            {secondaryButton}
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            <ActionButton
              label={busy ? (install.phase === "verifying" ? t("memoryDownload.checking") : t("memoryDownload.downloading")) : copy.download}
              loading={busy}
              disabled={busy}
              block
              onClick={onStart}
            />
            {!busy && secondaryButton}
          </div>
        )
      }
    >
      <div className="space-y-4">
        <p className="text-base leading-relaxed text-fg-2">{done ? copy.bodyDone : copy.body}</p>
        {install.phase === "idle" ? (
          <div className="flex items-center gap-3 rounded-xl border border-line bg-surface-1 p-3">
            <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-line bg-fill-2 text-fg-3">
              <Download size={18} />
            </span>
            <span className="min-w-0 flex-1">
              <span className="block truncate text-base font-medium text-fg">{copy.name}</span>
            </span>
          </div>
        ) : (
          <DownloadCard
            job={install.job}
            name={copy.name}
            {...(install.job === null ? { status: failed ? "failed" as const : "queued" as const } : {})}
            error={failureText}
            {...(done ? { detail: t("memoryDownload.doneDetail"), className: "border-accent/30 bg-accent/5" } : {})}
          />
        )}
        {busy && <p className="text-sm text-fg-3">{t("memoryDownload.keepOpen")}</p>}
        {done && finish && <ActionError error={finish.error} busy={finish.loading} onRetry={finish.onClick} />}
      </div>
    </Sheet>
  );
}
