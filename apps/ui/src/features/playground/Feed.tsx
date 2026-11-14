import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { motion } from "framer-motion";
import { useTranslation } from "react-i18next";
import { ImagePlus, Square } from "lucide-react";
import { Button, cn, Dialog, EmptyState, icon, IconButton, ImageViewer, LoadingState, Panel, Progress, Skeleton, Spinner, motion as m, toast } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { ImageProgress } from "@/api/generated/bindings";
import { watchJob } from "@/entities/job";
import { useOverlayBack } from "@/shared/hooks/useOverlayBack";
import { FailureState } from "@/shared/ui/FailureState";
import { GenerationCard, type CardActions } from "./GenerationCard";
import { entryShape, playgroundKeys, useFailureText, type PlaygroundEntry } from "./playground-data";

const wheelThreshold = 40;
const wheelLockMs = 350;

type FeedActions = Omit<CardActions, "onOpenImage" | "onDelete">;

function ratioOf(size: string | null): number {
  const match = /^\s*(\d+)\s*x\s*(\d+)\s*$/i.exec(size ?? "");
  const width = Number(match?.[1]);
  const height = Number(match?.[2]);
  return width > 0 && height > 0 ? width / height : 1;
}

function FittedSkeleton({ ratio }: { ratio: number }) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [box, setBox] = useState<{ width: number; height: number } | null>(null);

  useEffect(() => {
    const element = containerRef.current;
    if (!element) return;
    const compute = () => {
      const containerWidth = element.clientWidth;
      const containerHeight = element.clientHeight;
      if (!containerWidth || !containerHeight) return;
      const height = Math.min(containerHeight, containerWidth / ratio);
      setBox({ width: height * ratio, height });
    };
    compute();
    const observer = new ResizeObserver(compute);
    observer.observe(element);
    return () => observer.disconnect();
  }, [ratio]);

  return (
    <div ref={containerRef} className="absolute inset-0 flex items-center justify-center">
      {box && <Skeleton shape="block" width={box.width} height={box.height} />}
    </div>
  );
}

function LiveCard({ api, entry }: { api: ApiClient; entry: PlaygroundEntry }) {
  const { t } = useTranslation("playground");
  const client = useQueryClient();
  const failureText = useFailureText();
  const [progress, setProgress] = useState<ImageProgress | null>(null);
  const jobId = entry.job_id;

  useEffect(() => {
    if (jobId === null) return;
    const stop = new AbortController();
    const settled = () => {
      if (!stop.signal.aborted) void client.invalidateQueries({ queryKey: playgroundKeys.history });
    };
    void watchJob(api, jobId, () => undefined, stop.signal, undefined, setProgress).then(settled, settled);
    return () => stop.abort();
  }, [api, client, jobId]);

  const cancel = async () => {
    if (jobId === null) return;
    try {
      await api.call("jobCancel", { job_id: jobId });
    } catch (error) {
      toast.error(failureText(error));
    }
  };

  const shape = entryShape(entry);
  const ratio = ratioOf(shape.size);
  const phase = (() => {
    switch (progress?.phase) {
      case "loading":
        return t("progress.loading");
      case "queued":
        return t("progress.queued");
      case "sampling":
        return t("progress.sampling");
      case "retrying":
        return t("progress.retrying");
      case "generating":
        return t("progress.generating");
      default:
        return t("progress.starting");
    }
  })();
  const step = progress?.phase === "sampling" && progress.step !== null && progress.total_steps ? { step: progress.step, total: progress.total_steps } : null;
  const queued = progress?.phase === "queued" && progress.queue_position !== null ? ` (${progress.queue_position})` : "";
  const percent = step ? Math.min(100, Math.round((step.step / step.total) * 100)) : null;
  const status = `${phase}${queued}${step ? ` ${step.step}/${step.total}` : ""}`;

  return (
    <div className="flex h-full w-full flex-col items-center gap-3 px-4 pb-4 pt-3 @xl:px-8">
      <div className="relative min-h-0 w-full flex-1">
        {shape.count > 1 ? (
          <div className={cn("absolute inset-0 grid auto-rows-fr gap-3", shape.count === 2 ? "grid-cols-2" : "grid-cols-2 @xl:grid-cols-3")}>
            {Array.from({ length: shape.count }, (_, index) => (
              <div key={index} className="relative min-h-0 min-w-0">
                <FittedSkeleton ratio={ratio} />
              </div>
            ))}
          </div>
        ) : (
          <FittedSkeleton ratio={ratio} />
        )}
        <div className="absolute inset-0 flex flex-col items-center justify-center gap-3 p-4">
          <Spinner size="md" />
          <p role="status" className="text-center text-sm font-medium text-fg-2">
            {status}
          </p>
          {percent !== null && <Progress value={percent} size="sm" label={phase} className="w-44" />}
          <IconButton variant="secondary" label={t("feed.cancel")} onClick={() => void cancel()}>
            <Square size={icon.sm} />
          </IconButton>
        </div>
      </div>
      <Panel className="w-full max-w-2xl shrink-0">
        <p className="line-clamp-2 select-text text-sm text-fg-2">{entry.prompt}</p>
      </Panel>
    </div>
  );
}

export function Feed({ api, entries, loading, error, onRetry, hasOlder, loadingOlder, onLoadOlder, actions }: {
  api: ApiClient;
  entries: readonly PlaygroundEntry[];
  loading: boolean;
  error: unknown;
  onRetry: () => void;
  hasOlder: boolean;
  loadingOlder: boolean;
  onLoadOlder: () => void;
  actions: FeedActions;
}) {
  const { t } = useTranslation("playground");
  const client = useQueryClient();
  const failureText = useFailureText();
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const wheelLock = useRef(false);
  const wheelAccumulated = useRef(0);
  const anchorHeight = useRef<number | null>(null);
  const initialized = useRef(false);
  const [viewer, setViewer] = useState<{ entryId: string; index: number } | null>(null);
  const [deleting, setDeleting] = useState<PlaygroundEntry | null>(null);
  const hasEntries = entries.length > 0;
  const newestId = entries.at(-1)?.id;
  const oldestId = entries[0]?.id;
  useOverlayBack(viewer !== null || deleting !== null, () => {
    setViewer(null);
    setDeleting(null);
  });

  useEffect(() => {
    const element = scrollRef.current;
    if (!element) return;
    const insideScrollable = (target: EventTarget | null): boolean => {
      let node = target instanceof Element ? target : null;
      while (node && node !== element) {
        if (node.scrollHeight > node.clientHeight + 1) {
          const overflow = getComputedStyle(node).overflowY;
          if (overflow === "auto" || overflow === "scroll") return true;
        }
        node = node.parentElement;
      }
      return false;
    };
    const onWheel = (event: WheelEvent) => {
      if (insideScrollable(event.target)) return;
      event.preventDefault();
      if (wheelLock.current) return;
      wheelAccumulated.current += event.deltaY;
      if (Math.abs(wheelAccumulated.current) < wheelThreshold) return;
      const direction = wheelAccumulated.current > 0 ? 1 : -1;
      wheelAccumulated.current = 0;
      const slide = element.clientHeight;
      if (!slide) return;
      const current = Math.round(element.scrollTop / slide);
      const last = Math.max(0, Math.round(element.scrollHeight / slide) - 1);
      const target = Math.min(last, Math.max(0, current + direction));
      if (target === current && element.scrollTop === target * slide) return;
      wheelLock.current = true;
      element.scrollTo({ top: target * slide, behavior: "smooth" });
      window.setTimeout(() => {
        wheelLock.current = false;
        wheelAccumulated.current = 0;
      }, wheelLockMs);
    };
    element.addEventListener("wheel", onWheel, { passive: false });
    return () => element.removeEventListener("wheel", onWheel);
  }, [hasEntries]);

  useEffect(() => {
    const element = scrollRef.current;
    if (!element) return;
    element.scrollTo({ top: Math.max(0, element.scrollHeight - element.clientHeight), behavior: initialized.current ? "smooth" : "auto" });
    initialized.current = true;
  }, [newestId]);

  useLayoutEffect(() => {
    const element = scrollRef.current;
    if (loadingOlder || anchorHeight.current === null) return;
    if (element) element.scrollTop += element.scrollHeight - anchorHeight.current;
    anchorHeight.current = null;
  }, [oldestId, loadingOlder]);

  const loadOlder = useCallback(() => {
    anchorHeight.current = scrollRef.current?.scrollHeight ?? null;
    onLoadOlder();
  }, [onLoadOlder]);

  const remove = async (entry: PlaygroundEntry, deleteImages: boolean) => {
    setDeleting(null);
    try {
      await api.call("playgroundHistoryDelete", { id: entry.id, delete_images: deleteImages });
      await client.invalidateQueries({ queryKey: playgroundKeys.history });
    } catch (failure) {
      toast.error(t("feed.deleteFailed"), failureText(failure));
    }
  };

  if (loading || error || !hasEntries) {
    return (
      <div className="flex h-full items-center justify-center px-4">
        {loading ? (
          <LoadingState />
        ) : error ? (
          <FailureState error={error} onRetry={onRetry} />
        ) : (
          <EmptyState icon={<ImagePlus size={icon.xl} />} title={t("emptyFeed")} description={t("emptyFeedHint")} />
        )}
      </div>
    );
  }

  const viewed = viewer ? entries.find((entry) => entry.id === viewer.entryId) : undefined;
  const viewable = (viewed?.images ?? []).flatMap((image) => (image.asset ? [{ src: image.asset.url, alt: viewed?.prompt ?? "" }] : []));
  const cardActions: CardActions = {
    ...actions,
    onOpenImage: (entry, index) => setViewer({ entryId: entry.id, index }),
    onDelete: setDeleting,
  };

  return (
    <div className="relative h-full min-h-0">
      {hasOlder && (
        <Button variant="secondary" size="sm" loading={loadingOlder} onClick={loadOlder} className="absolute left-1/2 top-3 z-10 -translate-x-1/2">
          {t("feed.loadOlder")}
        </Button>
      )}
      <div ref={scrollRef} className="h-full snap-y snap-proximity overflow-y-auto">
        {entries.map((entry) => (
          <motion.div
            key={entry.id}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={m.slow}
            className="h-full w-full snap-start snap-always"
          >
            {entry.status === "pending" && entry.job_id !== null ? <LiveCard api={api} entry={entry} /> : <GenerationCard entry={entry} actions={cardActions} />}
          </motion.div>
        ))}
      </div>
      <ImageViewer
        images={viewable}
        index={viewer && viewable.length > 0 ? viewer.index : null}
        onIndexChange={(index) => setViewer(index === null || !viewer ? null : { ...viewer, index })}
      />
      <Dialog
        open={deleting !== null}
        onClose={() => setDeleting(null)}
        size="sm"
        title={t("feed.deleteTitle")}
        description={t("feed.deleteBody")}
        footer={
          <div className="flex w-full flex-col gap-2">
            <Button variant="secondary" block onClick={() => deleting && void remove(deleting, false)}>
              {t("feed.deleteKeepImages")}
            </Button>
            <Button variant="danger" block onClick={() => deleting && void remove(deleting, true)}>
              {t("feed.deleteWithImages")}
            </Button>
          </div>
        }
      />
    </div>
  );
}
