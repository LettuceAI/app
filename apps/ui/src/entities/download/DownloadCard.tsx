import type { ComponentProps } from "react";
import { useMutation } from "@tanstack/react-query";
import { FailureState } from "@/shared/ui/FailureState";
import { useTranslation } from "react-i18next";
import { DownloadItem } from "@lettuceai/crisp";
import type { JobView } from "@/api/generated/bindings";
import { downloadPhaseOf, downloadProgressOf } from "@/entities/job";
import { downloadName } from "./download-data";
import { useDownloads, useDownloadView } from "./downloads-context";

type Props = Pick<ComponentProps<typeof DownloadItem>, "name" | "detail" | "error" | "className" | "status">;
export function DownloadCard({ job, id, name, status, error, ...rest }: { job: JobView | null; id?: string } & Partial<Props>) {
  const { t } = useTranslation();
  const view = useDownloadView(job, id);
  const { fraction, size, speed, eta } = downloadProgressOf(view);
  const { cancel: sharedCancel, cancelling } = useDownloads();
  const cancel = useMutation({ mutationFn: (request: () => Promise<void>) => request() });
  const phase = !view || view.state === "queued" || view.state === "claimed" ? "queued" : downloadPhaseOf(view);
  const cancellable = view?.state === "queued" || view?.state === "claimed" || view?.state === "running";
  const stop = sharedCancel && view ? () => cancel.mutate(() => sharedCancel(view.id)) : undefined;
  const busy = cancel.isPending || !!view && cancelling.has(view.id);
  return <div className="space-y-2"><DownloadItem {...rest} name={name ?? downloadName(view) ?? t("downloads.generic")} status={status ?? phase} error={error ?? t(view?.state === "cancelled" ? "errors.code.cancelled" : "errors.code.unavailable")} {...(fraction === undefined ? {} : { progress: fraction })} {...(size === undefined ? {} : { size })} {...(speed === undefined ? {} : { speed })} {...(eta === undefined ? {} : { eta })} {...(cancellable && stop && !busy ? { onCancel: stop } : {})} />{cancel.isError && <FailureState error={cancel.error} size="sm" {...(stop ? { onRetry: stop } : {})} />}</div>;
}
