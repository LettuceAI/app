import { Download } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, Sheet } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import { DownloadCard } from "@/entities/download";
import { ActionButton } from "@/shared/ui/ActionButton";
import { ActionError } from "@/shared/ui/ActionError";
import type { InstallState } from "./use-embedding-install";

export interface MemoryDownloadSheetProps {
  open: boolean;
  onClose: () => void;
  install: InstallState;
  onStart: () => void;
  onManual: () => void;
  mode?: "enable" | "install";
  done?: { label: string; onClick: () => void; loading: boolean; error: unknown };
}

export function MemoryDownloadSheet(props: MemoryDownloadSheetProps) {
  const { t, i18n } = useTranslation("common");
  const { install } = props;
  const installOnly = props.mode === "install";
  const busy = install.phase === "downloading" || install.phase === "verifying";
  const done = install.phase === "done";
  const failed = install.phase === "failed";
  const name = t("memoryDownload.modelName");
  const failure = install.job?.failure ?? null;
  const failureText = isApiFailure(install.error)
    ? t(`errors.code.${install.error.code}`)
    : failure?.hugging_face?.type === "offline"
      ? t("memoryDownload.failedOffline")
      : failure && i18n.exists(`memoryDownload.failureCode.${failure.code}`, { ns: "common" })
        ? t(`memoryDownload.failureCode.${failure.code}` as never)
        : t("memoryDownload.failedOther");
  const secondary = (
    <Button variant="ghost" size="lg" block onClick={installOnly ? props.onClose : props.onManual}>
      {installOnly ? t("actions.cancel") : t("memoryDownload.useManual")}
    </Button>
  );

  return (
    <Sheet
      open={props.open}
      onClose={props.onClose}
      dismissible={!busy}
      title={done ? t("memoryDownload.titleDone") : t("memoryDownload.title")}
      footer={
        done ? (
          props.done && <ActionButton label={props.done.label} loading={props.done.loading} disabled={props.done.loading} block onClick={props.done.onClick} />
        ) : failed ? (
          <div className="flex flex-col gap-2">
            <ActionButton label={t("actions.retry")} block onClick={props.onStart} />
            {secondary}
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            <ActionButton
              label={busy ? (install.phase === "verifying" ? t("memoryDownload.checking") : t("memoryDownload.downloading")) : t(installOnly ? "memoryDownload.downloadOnly" : "memoryDownload.download")}
              loading={busy}
              disabled={busy}
              block
              onClick={props.onStart}
            />
            {!busy && secondary}
          </div>
        )
      }
    >
      <div className="space-y-4">
        <p className="text-base leading-relaxed text-fg-2">{done ? t("memoryDownload.bodyDone") : t(installOnly ? "memoryDownload.bodyInstall" : "memoryDownload.body")}</p>
        {install.phase === "idle" ? (
          <div className="flex items-center gap-3 rounded-xl border border-line bg-surface-1 p-3">
            <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-line bg-fill-2 text-fg-3">
              <Download size={18} />
            </span>
            <span className="min-w-0 flex-1">
              <span className="block truncate text-base font-medium text-fg">{name}</span>
            </span>
          </div>
        ) : (
          <DownloadCard
            job={install.job}
            name={name}
            {...(install.job === null ? { status: failed ? "failed" as const : "queued" as const } : {})}
            error={failureText}
            {...(done ? { detail: t("memoryDownload.doneDetail"), className: "border-accent/30 bg-accent/5" } : {})}
          />
        )}
        {busy && <p className="text-sm text-fg-3">{t("memoryDownload.keepOpen")}</p>}
        {done && props.done && <ActionError error={props.done.error} busy={props.done.loading} onRetry={props.done.onClick} />}
      </div>
    </Sheet>
  );
}
