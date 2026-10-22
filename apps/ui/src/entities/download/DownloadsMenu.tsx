import { useMutation } from "@tanstack/react-query";
import { Download } from "lucide-react";
import { useTranslation } from "react-i18next";
import { DownloadItem, EmptyState, icon, IconButton, Popover, PopoverContent, PopoverTrigger } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { JobView } from "@/api/generated/bindings";
import { downloadPhaseOf, downloadProgressOf } from "@/entities/job";
import { FailureState } from "@/shared/ui/FailureState";
import { downloadName } from "./download-data";
import { useDownloads } from "./downloads-context";

function DownloadRow({ job, onCancel }: { job: JobView; onCancel: (id: string) => void }) {
  const { t } = useTranslation();
  const { fraction, size, speed, eta } = downloadProgressOf(job);
  const status = job.state === "queued" || job.state === "claimed" ? "queued" : downloadPhaseOf(job);
  const cancellable = job.state === "queued" || job.state === "claimed" || job.state === "running";
  return (
    <DownloadItem
      name={downloadName(job) ?? t("downloads.generic")}
      status={status}
      {...(fraction !== undefined ? { progress: fraction } : {})}
      {...(size !== undefined ? { size } : {})}
      {...(speed !== undefined ? { speed } : {})}
      {...(eta !== undefined ? { eta } : {})}
      {...(cancellable ? { onCancel: () => onCancel(job.id) } : {})}
      className="rounded-none border-0 bg-transparent"
    />
  );
}

export function DownloadsMenu({ api }: { api: ApiClient }) {
  const { t } = useTranslation();
  const { downloads, error, retry } = useDownloads();
  const cancel = useMutation({ mutationFn: (id: string) => api.call("jobCancel", { job_id: id }) });
  const label = downloads.length > 0 ? t("downloads.titleActive", { count: downloads.length }) : t("downloads.title");
  return (
    <div data-tauri-drag-region="false" className="pointer-events-auto ml-auto">
      <Popover>
        <PopoverTrigger>
          <IconButton label={label} variant="ghost" size="sm" className="relative">
            <Download size={icon.md} />
            {downloads.length > 0 && <span aria-hidden="true" className="absolute right-1 top-1 h-2 w-2 rounded-full bg-accent" />}
          </IconButton>
        </PopoverTrigger>
        <PopoverContent align="end" className="w-96 max-w-full p-0">
          <p className="border-b border-line px-4 py-3 text-sm font-medium text-fg">{t("downloads.title")}</p>
          <div className="max-h-96 overflow-y-auto">
            {error !== null && <FailureState error={error} size="sm" onRetry={retry} />}
            {error === null && downloads.length === 0 && <EmptyState size="sm" title={t("downloads.empty")} description={t("downloads.emptyBody")} />}
            {downloads.length > 0 && <div className="divide-y divide-line">{downloads.map((job) => <DownloadRow key={job.id} job={job} onCancel={(id) => cancel.mutate(id)} />)}</div>}
          </div>
        </PopoverContent>
      </Popover>
    </div>
  );
}
