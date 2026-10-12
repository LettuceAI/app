import { Download } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, DownloadItem, Sheet } from "@lettuceai/crisp";
import type { JobView } from "@/api/generated/bindings";
import { ActionError } from "../components/ActionError";
import { ActionButton } from "../components/ui";
import type { InstallState } from "../data/install-job";

export interface MemoryDownloadSheetProps {
  open: boolean;
  onClose: () => void;
  install: InstallState;
  onStart: () => void;
  onManual: () => void;
  onOpenApp: () => void;
  opening: boolean;
  openError: unknown;
}

function progressOf(job: JobView | null): { fraction: number | undefined; size: number | undefined; speed: number | undefined; eta: number | undefined } {
  const progress = job?.progress;
  if (!progress || progress.unit !== "bytes" || !progress.total) {
    return { fraction: undefined, size: undefined, speed: undefined, eta: undefined };
  }
  const speed = progress.bytes_per_second && progress.bytes_per_second > 0 ? progress.bytes_per_second : undefined;
  return {
    fraction: Math.min(1, progress.current / progress.total),
    size: progress.total,
    speed,
    eta: speed ? Math.max(0, (progress.total - progress.current) / speed) : undefined,
  };
}

export function MemoryDownloadSheet(props: MemoryDownloadSheetProps) {
  const { t } = useTranslation("onboarding");
  const { t: common } = useTranslation("common");
  const { install } = props;
  const busy = install.phase === "downloading" || install.phase === "verifying";
  const done = install.phase === "done";
  const failed = install.phase === "failed";
  const name = t("memory.sheet.modelName");
  const { fraction, size, speed, eta } = progressOf(install.job);
  const offline = install.job?.failure?.hugging_face?.type === "offline";

  return (
    <Sheet
      open={props.open}
      onClose={props.onClose}
      dismissible={!busy}
      title={done ? t("memory.sheet.titleDone") : t("memory.sheet.title")}
      footer={
        done ? (
          <ActionButton label={t("actions.openApp")} loading={props.opening} disabled={props.opening} block onClick={props.onOpenApp} />
        ) : failed ? (
          <div className="flex flex-col gap-2">
            <ActionButton label={common("actions.retry")} block onClick={props.onStart} />
            <Button variant="ghost" size="lg" block onClick={props.onManual}>
              {t("memory.sheet.useManual")}
            </Button>
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            <ActionButton
              label={busy ? (install.phase === "verifying" ? t("memory.sheet.checking") : t("memory.sheet.downloading")) : t("memory.sheet.download")}
              loading={busy}
              disabled={busy}
              block
              onClick={props.onStart}
            />
            {!busy && (
              <Button variant="ghost" size="lg" block onClick={props.onManual}>
                {t("memory.sheet.useManual")}
              </Button>
            )}
          </div>
        )
      }
    >
      <div className="space-y-4">
        <p className="text-base leading-relaxed text-fg-2">{done ? t("memory.sheet.bodyDone") : t("memory.sheet.body")}</p>
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
          <DownloadItem
            name={name}
            status={install.phase === "downloading" ? "downloading" : install.phase === "verifying" ? "verifying" : done ? "done" : "failed"}
            {...(fraction !== undefined ? { progress: fraction } : {})}
            {...(size !== undefined ? { size } : {})}
            {...(speed !== undefined ? { speed } : {})}
            {...(eta !== undefined ? { eta } : {})}
            error={offline ? t("memory.sheet.failedOffline") : t("memory.sheet.failedOther")}
            {...(done ? { detail: t("memory.sheet.doneDetail"), className: "border-accent/30 bg-accent/5" } : {})}
          />
        )}
        {busy && <p className="text-sm text-fg-3">{t("memory.sheet.keepOpen")}</p>}
        {done && <ActionError error={props.openError} busy={props.opening} onRetry={props.onOpenApp} />}
      </div>
    </Sheet>
  );
}
