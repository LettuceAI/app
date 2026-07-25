import { useCallback, useEffect, useRef, useState } from "react";
import { motion } from "framer-motion";
import { Dices, ImagePlus, Loader, Scan, Square } from "lucide-react";

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

function pendingRatio(entry: PlaygroundGenerationEntry): number {
  const size = entry.params.size ?? entry.params.advancedModelSettings?.sdSize ?? "";
  const match = /^\s*(\d+)\s*x\s*(\d+)\s*$/i.exec(size);
  if (!match) return 1;
  const width = Number(match[1]);
  const height = Number(match[2]);
  if (!width || !height) return 1;
  return width / height;
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
      {box && (
        <div
          style={{ width: box.width, height: box.height }}
          className="rounded-xl border border-fg/8 bg-fg/5"
        />
      )}
    </div>
  );
}

const DEMO_PROMPT =
  "A lone lighthouse keeper's daughter stands on the rusted spiral staircase inside a colossal abandoned lighthouse, its shattered lens crown open to a violet dusk sky. Hundreds of paper lanterns drift upward through the tower like slow embers, casting warm amber light across peeling teal paint and tangled ivy. Far below, waves crash silver against black rocks in the mist. Cinematic wide shot from below, painterly anime illustration, soft volumetric light, intricate detail, melancholic and serene";
const DEMO_SEED = "1891643658";
const DEMO_SIZE = "1152x896";
const DEMO_MODEL = "FLUX.2 Klein 4B (Q4 0)";

function PlaygroundDemoCard() {
  const { t } = useI18n();
  const [url, setUrl] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void import("../../../assets/demoimage.webp")
      .then((module) => {
        if (!cancelled) setUrl(module.default);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="flex h-full min-h-0 w-full flex-col items-center gap-3 px-4 pb-4 pt-3 sm:px-8">
      <div className="flex min-h-0 w-full flex-1 items-center justify-center">
        {url ? (
          <img
            src={url}
            alt=""
            decoding="async"
            className="max-h-full max-w-full rounded-xl border border-fg/8 object-contain shadow-[0_20px_60px_rgba(0,0,0,0.35)]"
          />
        ) : (
          <div className="h-40 w-40 rounded-xl border border-fg/8 bg-fg/5" />
        )}
      </div>
      <div className="w-full max-w-2xl shrink-0 rounded-2xl border border-fg/10 bg-fg/4 px-3.5 py-3">
        <p className="line-clamp-2 text-[12.5px] leading-relaxed text-fg/75">{DEMO_PROMPT}</p>
        <div className="mt-2 flex flex-wrap items-center gap-x-1.5 gap-y-1 text-[10.5px] text-fg/45">
          <span className="rounded-md bg-accent/12 px-1.5 py-0.5 font-medium text-accent/80">
            {t("playground.demoExample")}
          </span>
          <span className="max-w-[220px] truncate rounded-md bg-fg/6 px-1.5 py-0.5">
            {DEMO_MODEL}
          </span>
          <span className="flex items-center gap-1 rounded-md bg-fg/6 px-1.5 py-0.5 font-mono">
            <Scan size={10} />
            {DEMO_SIZE}
          </span>
          <span className="flex items-center gap-1 rounded-md bg-fg/6 px-1.5 py-0.5 font-mono">
            <Dices size={10} />
            {DEMO_SEED}
          </span>
        </div>
      </div>
    </div>
  );
}

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
  onSendToImg2img,
  onUpscale,
  onReuseSeed,
  onRegenerate,
  busy = false,
  showDemo = false,
}: {
  generation: PlaygroundGenerationController;
  onSendToImg2img?: (image: PlaygroundGenerationEntry["images"][number]) => void;
  onUpscale?: (
    entry: PlaygroundGenerationEntry,
    image: PlaygroundGenerationEntry["images"][number],
  ) => void;
  onReuseSeed?: (entry: PlaygroundGenerationEntry) => void;
  onRegenerate?: (entry: PlaygroundGenerationEntry) => void;
  busy?: boolean;
  showDemo?: boolean;
}) {
  const { t } = useI18n();
  const [entries, setEntries] = useState<PlaygroundGenerationEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const [hasOlder, setHasOlder] = useState(false);
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const wheelLockRef = useRef(false);
  const wheelAccumRef = useRef(0);

  useEffect(() => {
    const element = scrollRef.current;
    if (!element) return;
    const hasScrollableAncestor = (target: EventTarget | null): boolean => {
      let node = target instanceof Element ? target : null;
      while (node && node !== element) {
        if (node.scrollHeight > node.clientHeight + 1) {
          const overflowY = getComputedStyle(node).overflowY;
          if (overflowY === "auto" || overflowY === "scroll") return true;
        }
        node = node.parentElement;
      }
      return false;
    };
    const handler = (event: WheelEvent) => {
      if (hasScrollableAncestor(event.target)) return;
      event.preventDefault();
      if (wheelLockRef.current) return;
      wheelAccumRef.current += event.deltaY;
      if (Math.abs(wheelAccumRef.current) < 40) return;
      const direction = wheelAccumRef.current > 0 ? 1 : -1;
      wheelAccumRef.current = 0;
      const slideHeight = element.clientHeight;
      if (!slideHeight) return;
      const currentPage = Math.round(element.scrollTop / slideHeight);
      const lastPage = Math.max(0, Math.round(element.scrollHeight / slideHeight) - 1);
      const target = Math.min(lastPage, Math.max(0, currentPage + direction));
      if (target === currentPage && element.scrollTop === target * slideHeight) return;
      wheelLockRef.current = true;
      element.scrollTo({ top: target * slideHeight, behavior: "smooth" });
      window.setTimeout(() => {
        wheelLockRef.current = false;
        wheelAccumRef.current = 0;
      }, 350);
    };
    element.addEventListener("wheel", handler, { passive: false });
    return () => element.removeEventListener("wheel", handler);
  }, [loading]);

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

  const initializedRef = useRef(false);
  useEffect(() => {
    if (loading) return;
    const element = scrollRef.current;
    if (!element) return;
    const top = Math.max(0, element.scrollHeight - element.clientHeight);
    element.scrollTo({ top, behavior: initializedRef.current ? "smooth" : "auto" });
    initializedRef.current = true;
  }, [loading, entries.length, generation.activeEntry?.id]);

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
    if (showDemo) {
      return <PlaygroundDemoCard />;
    }
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-1.5 text-center">
        <div className="mb-2 flex h-16 w-16 items-center justify-center rounded-2xl border border-fg/10 bg-fg/5">
          <ImagePlus size={22} className="text-fg/30" />
        </div>
        <p className="text-[13px] font-medium text-fg/60">{t("playground.emptyFeed")}</p>
        <p className="text-[12px] text-fg/40">{t("playground.emptyFeedHint")}</p>
      </div>
    );
  }

  return (
    <div className="relative min-h-0 flex-1">
      {hasOlder && (
        <button
          type="button"
          onClick={() => void loadOlder()}
          disabled={loadingOlder}
          className="absolute left-1/2 top-3 z-10 flex -translate-x-1/2 items-center gap-2 rounded-full border border-fg/10 bg-surface/90 px-3.5 py-1.5 text-[11.5px] font-medium text-fg/60 backdrop-blur-md transition-all hover:border-fg/20 hover:text-fg disabled:opacity-50"
        >
          {loadingOlder && <Loader size={11} className="animate-spin" />}
          {t("playground.feed.loadOlder")}
        </button>
      )}
      <div ref={scrollRef} className="h-full snap-y snap-proximity overflow-y-auto">
        {entries.map((entry) => (
          <motion.div
            key={entry.id}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.3, ease: "easeOut" }}
            className="h-full w-full snap-start snap-always"
          >
            <PlaygroundGenerationCard
              entry={entry}
              actions={{
                onDelete: (item, deleteImages) => void handleDelete(item, deleteImages),
                onSendToImg2img,
                onUpscale,
                onReuseSeed,
                onRegenerate,
                disabled: generation.generating || busy,
              }}
            />
          </motion.div>
        ))}
        {active && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.3, ease: "easeOut" }}
            className="flex h-full w-full snap-start snap-always flex-col items-center gap-3 px-4 pb-4 pt-3 sm:px-8"
          >
            <div className="relative min-h-0 w-full flex-1">
              {(active.params.n ?? 1) > 1 ? (
                <div
                  className={cn(
                    "absolute inset-0 grid auto-rows-fr gap-3",
                    (active.params.n ?? 1) === 2 ? "grid-cols-2" : "grid-cols-2 sm:grid-cols-3",
                  )}
                >
                  {Array.from({ length: Math.min(active.params.n ?? 1, 8) }).map((_, index) => (
                    <div key={index} className="relative min-h-0 min-w-0">
                      <FittedSkeleton ratio={pendingRatio(active)} />
                    </div>
                  ))}
                </div>
              ) : (
                <FittedSkeleton ratio={pendingRatio(active)} />
              )}
              <div className="absolute inset-0 flex flex-col items-center justify-center gap-3 p-4">
                <Loader size={18} className="animate-spin text-accent/70" />
                <p className="text-center text-[12px] font-medium text-fg/60">
                  {t(progressLabelKey(progress))}
                  {progress?.phase === "queued" && progress.queuePosition != null
                    ? ` (${progress.queuePosition})`
                    : ""}
                  {progress?.phase === "sampling" && progress.step != null && progress.steps
                    ? ` ${progress.step}/${progress.steps}`
                    : ""}
                </p>
                {samplingPercent != null && (
                  <div className="h-1 w-44 overflow-hidden rounded-full bg-fg/10">
                    <div
                      className={cn("h-full rounded-full bg-accent/70 transition-[width]")}
                      style={{ width: `${samplingPercent}%` }}
                    />
                  </div>
                )}
                <button
                  type="button"
                  onClick={() => void generation.cancel()}
                  title={t("playground.feed.cancel")}
                  className="mt-1 flex h-8 w-8 items-center justify-center rounded-lg border border-fg/10 bg-fg/5 text-fg/50 transition-all hover:border-danger/40 hover:text-danger active:scale-95"
                >
                  <Square size={12} />
                </button>
              </div>
            </div>
            <div className="w-full max-w-2xl shrink-0 rounded-2xl border border-fg/10 bg-fg/4 px-3.5 py-3">
              <p className="text-[12.5px] leading-relaxed text-fg/60 line-clamp-2">
                {active.prompt}
              </p>
            </div>
          </motion.div>
        )}
      </div>
    </div>
  );
}
