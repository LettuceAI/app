import { useCallback, useEffect, useRef, useState } from "react";
import { ImagePlus, Loader, Square } from "lucide-react";

import { cn } from "../../design-tokens";
import { toast } from "../../components/toast";
import { useI18n, type TranslationKey } from "../../../core/i18n/context";
import {
  deletePlaygroundHistoryEntry,
  listPlaygroundHistory,
  type PlaygroundGenerationEntry,
} from "../../../core/image-generation/playground";
import type { SdcppGenerationProgress } from "../../../core/image-generation";
import { PlaygroundGenerationCard } from "./PlaygroundGenerationCard";
import type { PlaygroundGenerationController } from "./usePlaygroundGeneration";

const PAGE_SIZE = 30;

function progressLabelKey(progress: SdcppGenerationProgress | null): TranslationKey {
  switch (progress?.phase) {
    case "loading":
      return "playground.progress.loading";
    case "queued":
      return "playground.progress.queued";
    case "sampling":
      return "playground.progress.sampling";
    case "retrying":
      return "playground.progress.retrying";
    case "generating":
      return "playground.progress.generating";
    default:
      return "playground.progress.starting";
  }
}

export function PlaygroundFeed({
  generation,
}: {
  generation: PlaygroundGenerationController;
}) {
  const { t } = useI18n();
  const [entries, setEntries] = useState<PlaygroundGenerationEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const [hasOlder, setHasOlder] = useState(false);
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const bottomRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    let cancelled = false;
    listPlaygroundHistory(PAGE_SIZE)
      .then((page) => {
        if (cancelled) return;
        setEntries([...page].reverse());
        setHasOlder(page.length === PAGE_SIZE);
      })
      .catch(() => {
        if (!cancelled) setEntries([]);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    generation.onEntryFinalized((entry) => {
      setEntries((current) => {
        const existing = current.findIndex((item) => item.id === entry.id);
        if (existing >= 0) {
          const next = [...current];
          next[existing] = entry;
          return next;
        }
        return [...current, entry];
      });
    });
  }, [generation]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [entries.length, generation.activeEntry?.id]);

  const loadOlder = useCallback(async () => {
    if (loadingOlder || entries.length === 0) return;
    setLoadingOlder(true);
    try {
      const container = scrollRef.current;
      const previousHeight = container?.scrollHeight ?? 0;
      const page = await listPlaygroundHistory(PAGE_SIZE, entries[0].createdAt);
      setEntries((current) => {
        const seen = new Set(current.map((entry) => entry.id));
        return [...[...page].reverse().filter((entry) => !seen.has(entry.id)), ...current];
      });
      setHasOlder(page.length === PAGE_SIZE);
      requestAnimationFrame(() => {
        if (container) {
          container.scrollTop += container.scrollHeight - previousHeight;
        }
      });
    } finally {
      setLoadingOlder(false);
    }
  }, [loadingOlder, entries]);

  const handleDelete = useCallback(
    async (entry: PlaygroundGenerationEntry, deleteImages: boolean) => {
      try {
        await deletePlaygroundHistoryEntry(entry.id, deleteImages);
        setEntries((current) => current.filter((item) => item.id !== entry.id));
      } catch (error) {
        toast.error(
          t("playground.feed.deleteFailed"),
          error instanceof Error ? error.message : String(error),
        );
      }
    },
    [t],
  );

  const active = generation.activeEntry;
  const progress = generation.progress;
  const samplingPercent =
    progress?.phase === "sampling" && progress.step != null && progress.steps
      ? Math.min(100, Math.round((progress.step / progress.steps) * 100))
      : null;

  if (loading) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <Loader size={18} className="animate-spin text-fg/30" />
      </div>
    );
  }

  if (entries.length === 0 && !active) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-2 text-center">
        <ImagePlus size={22} className="text-fg/20" />
        <p className="text-[13px] font-medium text-fg/60">{t("playground.emptyFeed")}</p>
        <p className="text-[12px] text-fg/40">{t("playground.emptyFeedHint")}</p>
      </div>
    );
  }

  return (
    <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto">
      <div className="mx-auto flex w-full max-w-3xl flex-col gap-3 px-4 py-4">
        {hasOlder && (
          <button
            type="button"
            onClick={() => void loadOlder()}
            disabled={loadingOlder}
            className="mx-auto flex items-center gap-2 rounded-xl border border-fg/10 bg-fg/4 px-3.5 py-2 text-[12px] font-medium text-fg/60 transition hover:border-fg/20 hover:text-fg disabled:opacity-50"
          >
            {loadingOlder && <Loader size={12} className="animate-spin" />}
            {t("playground.feed.loadOlder")}
          </button>
        )}
        {entries.map((entry) => (
          <PlaygroundGenerationCard
            key={entry.id}
            entry={entry}
            actions={{ onDelete: (item, deleteImages) => void handleDelete(item, deleteImages), disabled: generation.generating }}
          />
        ))}
        {active && (
          <div className="rounded-xl border border-accent/20 bg-accent/[0.04] p-3">
            <div className="flex items-center gap-3">
              <Loader size={14} className="shrink-0 animate-spin text-accent/70" />
              <div className="min-w-0 flex-1">
                <p className="truncate text-[12.5px] font-medium text-fg/80">{active.prompt}</p>
                <p className="mt-0.5 text-[11px] text-fg/45">
                  {t(progressLabelKey(progress))}
                  {progress?.phase === "queued" && progress.queuePosition != null
                    ? ` (${progress.queuePosition})`
                    : ""}
                  {progress?.phase === "sampling" && progress.step != null && progress.steps
                    ? ` ${progress.step}/${progress.steps}`
                    : ""}
                </p>
              </div>
              <button
                type="button"
                onClick={() => void generation.cancel()}
                title={t("playground.feed.cancel")}
                className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-fg/10 bg-fg/4 text-fg/50 transition hover:border-danger/40 hover:text-danger"
              >
                <Square size={12} />
              </button>
            </div>
            {samplingPercent != null && (
              <div className="mt-2.5 h-1 overflow-hidden rounded-full bg-fg/8">
                <div
                  className={cn("h-full rounded-full bg-accent/70 transition-[width]")}
                  style={{ width: `${samplingPercent}%` }}
                />
              </div>
            )}
          </div>
        )}
        <div ref={bottomRef} />
      </div>
    </div>
  );
}
