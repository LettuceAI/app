import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ApiClient } from "@/api/client";
import type { JobView } from "@/api/generated/bindings";
import { watchJob } from "@/entities/job";
import { useDownloadView } from "./downloads-context";

export function useDownloadJob(api: ApiClient, id: string, callbacks: { onUpdate?: ((job: JobView) => void) | undefined; onDone?: ((job: JobView) => void | Promise<void>) | undefined } = {}) {
  const [observed, setObserved] = useState<JobView | null>(null);
  const [failure, setFailure] = useState<{ id: string; error: unknown } | null>(null);
  const [attempt, setAttempt] = useState(0);
  const handlers = useRef(callbacks);
  useLayoutEffect(() => { handlers.current = callbacks; });
  useEffect(() => {
    const controller = new AbortController();
    void watchJob(api, id, (job) => { setObserved(job); handlers.current.onUpdate?.(job); }, controller.signal).then((job) => { if (!controller.signal.aborted) return handlers.current.onDone?.(job); }).catch((error: unknown) => { if (!controller.signal.aborted) setFailure({ id, error }); });
    return () => controller.abort();
  }, [api, id, attempt]);
  const job = useDownloadView(observed?.id === id ? observed : null, id);
  return { job, error: failure?.id === id ? failure.error : null, retry: () => { setFailure(null); setAttempt((value) => value + 1); } };
}
