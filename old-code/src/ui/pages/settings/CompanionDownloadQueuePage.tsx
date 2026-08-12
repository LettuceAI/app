import { useEffect, useMemo, useRef, useState } from "react";
import { motion } from "framer-motion";
import { Check, Download, X } from "lucide-react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { storageBridge } from "../../../core/storage/files";
import { useI18n, type TranslationKey } from "../../../core/i18n/context";
import {
  ModelDownloadProgress,
  type ModelDownloadPhase,
} from "./components/ModelDownloadProgress";
import { useModelDownload } from "./hooks/useModelDownload";
import {
  describeRequirement,
  type ModelRequirement,
  type ModelRequirementKind,
} from "../../modelRequirements";
import { cn, interactive, radius, spacing, typography } from "../../design-tokens";

const VALID_KINDS: ModelRequirementKind[] = ["embedding", "emotion", "ner", "router"];

const REQUIREMENT_KEYS = {
  embedding: {
    titleKey: "companion.models.embeddingTitle",
    subtitleKey: "companion.models.embeddingSubtitle",
    sizeKey: "companion.models.embeddingSize",
  },
  emotion: {
    titleKey: "companion.models.emotionTitle",
    subtitleKey: "companion.models.emotionSubtitle",
    sizeKey: "companion.models.emotionSize",
  },
  ner: {
    titleKey: "companion.models.nerTitle",
    subtitleKey: "companion.models.nerSubtitle",
    sizeKey: "companion.models.nerSize",
  },
  router: {
    titleKey: "companion.models.routerTitle",
    subtitleKey: "companion.models.routerSubtitle",
    sizeKey: "companion.models.routerSize",
  },
} satisfies Record<
  ModelRequirementKind,
  { titleKey: TranslationKey; subtitleKey: TranslationKey; sizeKey: TranslationKey }
>;

function parseQueue(raw: string | null): ModelRequirementKind[] {
  if (!raw) return [];
  const seen = new Set<ModelRequirementKind>();
  const result: ModelRequirementKind[] = [];
  for (const piece of raw.split(",")) {
    const trimmed = piece.trim() as ModelRequirementKind;
    if (VALID_KINDS.includes(trimmed) && !seen.has(trimmed)) {
      seen.add(trimmed);
      result.push(trimmed);
    }
  }
  return result;
}

function bridgeFor(kind: ModelRequirementKind): () => Promise<void> {
  if (kind === "embedding") return () => storageBridge.startEmbeddingDownload();
  return () => storageBridge.startCompanionDownload(kind);
}

export function CompanionDownloadQueuePage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const returnTo = searchParams.get("returnTo") ?? "/settings/advanced/companions";
  const queue = useMemo(() => parseQueue(searchParams.get("queue")), [searchParams]);

  const [currentIndex, setCurrentIndex] = useState(0);
  const [completed, setCompleted] = useState<ModelRequirementKind[]>([]);
  const [allDone, setAllDone] = useState(false);
  const [countdown, setCountdown] = useState(3);
  const [autoStarted, setAutoStarted] = useState(false);

  const currentKindRef = useRef<ModelRequirementKind | null>(null);
  const currentRequirement: ModelRequirement | null =
    currentIndex < queue.length ? describeRequirement(queue[currentIndex]) : null;
  currentKindRef.current = currentRequirement?.kind ?? null;
  const currentTitle = currentRequirement
    ? t(REQUIREMENT_KEYS[currentRequirement.kind].titleKey)
    : null;

  const handleComplete = () => {
    const kind = currentKindRef.current;
    if (!kind) return;
    setCompleted((prev) => (prev.includes(kind) ? prev : [...prev, kind]));
    setCurrentIndex((prev) => prev + 1);
  };

  const download = useModelDownload({ onComplete: handleComplete });

  // Auto-start the first download once on mount.
  useEffect(() => {
    if (autoStarted || queue.length === 0 || !currentRequirement) return;
    setAutoStarted(true);
    void download.start(bridgeFor(currentRequirement.kind)).catch(() => {});
    // currentRequirement is stable on mount; we only want this to fire once.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [autoStarted, queue.length]);

  // After a step completes, kick off the next one.
  useEffect(() => {
    if (currentIndex === 0 || allDone) return;
    if (currentIndex >= queue.length) {
      setAllDone(true);
      return;
    }
    const next = queue[currentIndex];
    void download.start(bridgeFor(next)).catch(() => {});
    // We intentionally only react to currentIndex / queue changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentIndex, queue.length]);

  // Redirect after completion.
  useEffect(() => {
    if (!allDone) return;
    setCountdown(3);
    const timer = setInterval(() => {
      setCountdown((prev) => {
        if (prev <= 1) {
          clearInterval(timer);
          navigate(returnTo, { replace: true });
          return 0;
        }
        return prev - 1;
      });
    }, 1000);
    return () => clearInterval(timer);
  }, [allDone, navigate, returnTo]);

  const handleCancel = async () => {
    try {
      await download.cancel();
    } catch {
      // error captured by hook
    }
    navigate(returnTo);
  };

  const handleRetry = () => {
    if (!currentRequirement) return;
    download.setError(null);
    void download.start(bridgeFor(currentRequirement.kind)).catch(() => {});
  };

  if (queue.length === 0) {
    return (
      <div className="flex min-h-screen items-center justify-center px-4">
        <div
          className={cn(
            "border border-danger/20 bg-danger/10 p-4 text-sm text-danger/80",
            radius.lg,
          )}
        >
          {t("companion.queue.noModels")}
        </div>
      </div>
    );
  }

  const phase: ModelDownloadPhase = allDone
    ? "passed"
    : download.progress.status === "failed"
      ? "failed"
      : download.isDownloading || download.progress.status === "downloading"
        ? "downloading"
        : "idle";

  const stepLabel = t("companion.queue.stepLabel", {
    current: Math.min(currentIndex + 1, queue.length),
    total: queue.length,
  });
  const headerTitle = allDone
    ? t("companion.queue.allSet")
    : currentRequirement && currentTitle
      ? t("companion.queue.downloadingTitle", { name: currentTitle })
      : t("companion.queue.preparing");
  const headerDescription = allDone
    ? t("companion.queue.redirecting", { count: countdown })
    : currentRequirement
      ? t(REQUIREMENT_KEYS[currentRequirement.kind].subtitleKey)
      : t("companion.queue.currentSubtitleFallback");

  const statusText = allDone
    ? t("companion.queue.allInstalled")
    : download.progress.status === "downloading"
      ? currentTitle
        ? t("companion.queue.statusDownloading", { name: currentTitle.toLowerCase() })
        : t("companion.queue.statusDownloadingFallback")
      : download.progress.status === "failed"
        ? t("companion.queue.statusFailed")
        : download.progress.status === "cancelled"
          ? t("companion.queue.statusCancelled")
          : t("companion.queue.starting");

  const queueList = (
    <ol className="space-y-2">
      {queue.map((kind, index) => {
        const requirement = describeRequirement(kind);
        const Icon = requirement.icon;
        const isDone = completed.includes(kind);
        const isCurrent = !allDone && index === currentIndex;
        const isPending = index > currentIndex;
        return (
          <li
            key={kind}
            className={cn(
              "flex items-start gap-3 border px-3.5 py-3",
              radius.lg,
              isDone
                ? "border-accent/25 bg-accent/8"
                : isCurrent
                  ? "border-info/30 bg-info/8"
                  : "border-fg/10 bg-fg/5",
            )}
          >
            <div
              className={cn(
                "flex h-9 w-9 shrink-0 items-center justify-center border",
                radius.md,
                isDone
                  ? "border-accent/35 bg-accent/15 text-accent"
                  : isCurrent
                    ? "border-info/35 bg-info/15 text-info"
                    : "border-fg/10 bg-fg/5 text-fg/45",
              )}
            >
              {isDone ? <Check className="h-4 w-4" /> : <Icon className="h-4 w-4" />}
            </div>
            <div className="min-w-0 flex-1">
              <div className="flex items-center justify-between gap-2">
                <p
                  className={cn(
                    "text-sm font-medium",
                    isPending ? "text-fg/70" : "text-fg",
                  )}
                >
                  {t(REQUIREMENT_KEYS[kind].titleKey)}
                </p>
                <span
                  className={cn(
                    "shrink-0 border px-1.5 py-0.5 font-mono text-[10px]",
                    radius.md,
                    isDone
                      ? "border-accent/35 bg-accent/10 text-accent/85"
                      : isCurrent
                        ? "border-info/35 bg-info/10 text-info/85"
                        : "border-fg/10 bg-fg/5 text-fg/45",
                  )}
                >
                  {isDone
                    ? t("companion.queue.badgeDone")
                    : isCurrent
                      ? t("companion.queue.badgeNow")
                      : requirement.approxSize}
                </span>
              </div>
              <p className="mt-0.5 text-[11px] leading-relaxed text-fg/45">
                {requirement.subtitle}
              </p>
            </div>
          </li>
        );
      })}
    </ol>
  );

  const progressPanel = (
    <div className={spacing.item}>
      {!allDone && (
        <ModelDownloadProgress
          progress={download.progress}
          phase={phase}
          statusText={statusText}
        />
      )}

      {download.error && (
        <div className={cn("border border-danger/20 bg-danger/10 p-3", radius.lg)}>
          <p className="text-sm text-danger/80">{download.error}</p>
        </div>
      )}

      {!allDone && download.isDownloading && (
        <button
          onClick={handleCancel}
          className={cn(
            "flex w-full items-center justify-center gap-2 border border-danger/20 bg-danger/10 px-6 py-3 text-sm font-medium text-danger/80",
            radius.lg,
            interactive.transition.fast,
            "hover:border-danger/30 hover:bg-danger/15",
          )}
        >
          <X className="h-4 w-4" />
          {t("common.buttons.cancel")}
        </button>
      )}

      {!allDone && !download.isDownloading && download.progress.status === "failed" && (
        <div className="space-y-3">
          <button
            onClick={handleRetry}
            className={cn(
              "flex w-full items-center justify-center gap-2 bg-info px-6 py-3 text-sm font-medium text-fg",
              radius.lg,
              interactive.transition.fast,
              "hover:bg-info/80",
            )}
          >
            <Download className="h-4 w-4" />
            {currentTitle
              ? t("companion.queue.retry", { name: currentTitle })
              : t("companion.queue.retryFallback")}
          </button>
          <button
            onClick={() => navigate(returnTo)}
            className={cn(
              "flex w-full items-center justify-center gap-2 border border-fg/10 bg-fg/5 px-6 py-3 text-sm font-medium text-fg/60",
              radius.lg,
              interactive.transition.fast,
              "hover:bg-fg/10",
            )}
          >
            {t("common.buttons.goBack")}
          </button>
        </div>
      )}
    </div>
  );

  return (
    <div className="flex min-h-screen flex-col">
      <main className="flex-1 px-4 pb-24 pt-6 lg:px-8">
        <motion.div
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          className={cn("mx-auto w-full max-w-5xl", spacing.section)}
        >
          <div className="text-center">
            {!allDone && (
              <p className={cn(typography.caption.size, "text-fg/45")}>{stepLabel}</p>
            )}
            <h1 className={cn(typography.display.size, typography.display.weight, "mt-1 text-fg")}>
              {headerTitle}
            </h1>
            <p className={cn(typography.body.size, "mt-2 text-fg/55")}>{headerDescription}</p>
          </div>

          <div className="grid grid-cols-1 gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(360px,420px)] lg:items-start lg:gap-8">
            <div>{queueList}</div>
            <div className="lg:sticky lg:top-6">{progressPanel}</div>
          </div>
        </motion.div>
      </main>
    </div>
  );
}
