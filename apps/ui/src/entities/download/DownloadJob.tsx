import type { ApiClient } from "@/api/client";
import type { JobView } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { DownloadCard } from "./DownloadCard";
import { useDownloadJob } from "./use-download-job";

export function DownloadJob({ api, id, name, onUpdate, onDone }: { api: ApiClient; id: string; name?: string | undefined; onUpdate?: ((job: JobView) => void) | undefined; onDone?: ((job: JobView) => void | Promise<void>) | undefined }) {
  const state = useDownloadJob(api, id, { onUpdate, onDone });
  return <div className="space-y-2">
    <DownloadCard job={state.job} id={id} {...(name === undefined ? {} : { name })} />
    {state.error !== null && <FailureState error={state.error} size="sm" onRetry={state.retry} />}
  </div>;
}
