import { Download } from "lucide-react";
import { useTranslation } from "react-i18next";
import { EmptyState, icon, IconButton, Popover, PopoverContent, PopoverTrigger, Skeleton } from "@lettuceai/crisp";
import { FailureState } from "@/shared/ui/FailureState";
import { DownloadCard } from "./DownloadCard";
import { useDownloads } from "./downloads-context";

export function DownloadsMenu() {
  const { t } = useTranslation();
  const { downloads, error, retry, ready } = useDownloads();
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
            {!ready && error === null && <div className="p-3" role="status" aria-label={t("downloads.loading")}><Skeleton className="h-16 w-full" /></div>}
            {ready && error === null && downloads.length === 0 && <EmptyState size="sm" title={t("downloads.empty")} description={t("downloads.emptyBody")} />}
            {downloads.length > 0 && <div className="divide-y divide-line">{downloads.map((job) => <DownloadCard key={job.id} job={job} className="rounded-none border-0 bg-transparent" />)}</div>}
          </div>
        </PopoverContent>
      </Popover>
    </div>
  );
}
