import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  AlertTriangle,
  ChevronLeft,
  ChevronRight,
  Clock,
  Copy,
  Dices,
  ImageOff,
  ImageUp,
  Maximize2,
  RefreshCw,
  Scan,
  Trash2,
  X,
} from "lucide-react";

import { cn } from "../../design-tokens";
import { BottomMenu } from "../../components/BottomMenu";
import { toast } from "../../components/toast";
import { useI18n } from "../../../core/i18n/context";
import { resolveGeneratedImageUrl } from "../../../core/image-generation";
import type {
  PlaygroundGenerationEntry,
  PlaygroundGenerationImage,
} from "../../../core/image-generation/playground";

function useResolvedImageUrl(image: PlaygroundGenerationImage): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    void resolveGeneratedImageUrl({
      assetId: image.assetId,
      filePath: image.filePath,
      mimeType: image.mimeType ?? "image/png",
      url: image.url ?? undefined,
    })
      .then((resolved) => {
        if (!cancelled) setUrl(resolved ?? null);
      })
      .catch(() => {
        if (!cancelled) setUrl(null);
      });
    return () => {
      cancelled = true;
    };
  }, [image.assetId, image.filePath, image.url]);
  return url;
}

function CardImage({
  image,
  onClick,
}: {
  image: PlaygroundGenerationImage;
  onClick: () => void;
}) {
  const url = useResolvedImageUrl(image);
  if (!url) {
    return (
      <div className="absolute inset-0 flex items-center justify-center">
        <div className="flex h-40 w-40 items-center justify-center rounded-xl border border-fg/8 bg-fg/4 text-fg/20">
          <ImageOff size={18} />
        </div>
      </div>
    );
  }
  return (
    <button
      type="button"
      onClick={onClick}
      className="group absolute inset-0 flex cursor-zoom-in items-center justify-center"
    >
      <img
        src={url}
        alt=""
        loading="lazy"
        decoding="async"
        className="max-h-full max-w-full rounded-xl border border-fg/8 object-contain shadow-[0_20px_60px_rgba(0,0,0,0.35)] transition group-hover:brightness-105"
      />
    </button>
  );
}

function LightboxImage({ image }: { image: PlaygroundGenerationImage }) {
  const url = useResolvedImageUrl(image);
  if (!url) return null;
  return (
    <motion.img
      key={image.assetId || image.filePath}
      initial={{ opacity: 0, scale: 0.96 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.96 }}
      transition={{ duration: 0.2 }}
      src={url}
      alt=""
      className="max-h-[92vh] max-w-[92vw] rounded-2xl object-contain shadow-[0_30px_80px_rgba(0,0,0,0.45)]"
      onClick={(event) => event.stopPropagation()}
    />
  );
}

export type PlaygroundCardActions = {
  onDelete: (entry: PlaygroundGenerationEntry, deleteImages: boolean) => void;
  onSendToImg2img?: (image: PlaygroundGenerationImage) => void;
  onUpscale?: (entry: PlaygroundGenerationEntry, image: PlaygroundGenerationImage) => void;
  onReuseSeed?: (entry: PlaygroundGenerationEntry) => void;
  onRegenerate?: (entry: PlaygroundGenerationEntry) => void;
  disabled: boolean;
};

export function PlaygroundGenerationCard({
  entry,
  actions,
}: {
  entry: PlaygroundGenerationEntry;
  actions: PlaygroundCardActions;
}) {
  const { t } = useI18n();
  const [lightboxIndex, setLightboxIndex] = useState<number | null>(null);
  const [deleteMenuOpen, setDeleteMenuOpen] = useState(false);

  useEffect(() => {
    if (lightboxIndex === null) return;
    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape") setLightboxIndex(null);
      else if (event.key === "ArrowRight") {
        setLightboxIndex((current) =>
          current === null ? null : Math.min(current + 1, entry.images.length - 1),
        );
      } else if (event.key === "ArrowLeft") {
        setLightboxIndex((current) => (current === null ? null : Math.max(current - 1, 0)));
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [lightboxIndex, entry.images.length]);

  const failed = entry.status === "failed";
  const cancelled = entry.status === "cancelled";
  const interrupted = entry.status === "pending";
  const firstImage = entry.images[0];
  const dimensions =
    firstImage?.width && firstImage?.height
      ? `${firstImage.width}x${firstImage.height}`
      : entry.params.size ?? entry.params.advancedModelSettings?.sdSize ?? null;
  const timestamp = new Date(entry.createdAt).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });

  return (
    <div className="flex h-full min-h-0 w-full flex-col items-center gap-3 px-4 pb-4 pt-3 sm:px-8">
      <div className="flex min-h-0 w-full flex-1 items-center justify-center">
        {entry.images.length > 1 ? (
          <div
            className={cn(
              "grid h-full w-full auto-rows-fr gap-3",
              entry.images.length === 2 ? "grid-cols-2" : "grid-cols-2 sm:grid-cols-3",
            )}
          >
            {entry.images.map((image, index) => (
              <div
                key={image.assetId || image.filePath || index}
                className="group/cell relative min-h-0 min-w-0"
              >
                <CardImage image={image} onClick={() => setLightboxIndex(index)} />
                {entry.status === "complete" &&
                  (actions.onSendToImg2img || actions.onUpscale) && (
                    <div className="absolute bottom-2 right-2 z-10 flex items-center gap-1 rounded-lg bg-black/60 p-1 opacity-0 backdrop-blur-md transition-opacity group-hover/cell:opacity-100">
                      {actions.onUpscale && (
                        <button
                          type="button"
                          onClick={() => actions.onUpscale?.(entry, image)}
                          disabled={actions.disabled}
                          title={t("playground.feed.upscale")}
                          className="rounded-md p-1.5 text-white/75 transition hover:bg-white/15 hover:text-white disabled:opacity-40"
                        >
                          <Maximize2 size={13} />
                        </button>
                      )}
                      {actions.onSendToImg2img && (
                        <button
                          type="button"
                          onClick={() => actions.onSendToImg2img?.(image)}
                          disabled={actions.disabled}
                          title={t("playground.feed.sendToImg2img")}
                          className="rounded-md p-1.5 text-white/75 transition hover:bg-white/15 hover:text-white disabled:opacity-40"
                        >
                          <ImageUp size={13} />
                        </button>
                      )}
                    </div>
                  )}
              </div>
            ))}
          </div>
        ) : entry.images.length === 1 ? (
          <div className="relative h-full w-full">
            <CardImage image={entry.images[0]} onClick={() => setLightboxIndex(0)} />
          </div>
        ) : (
          <div
            className={cn(
              "flex max-w-md items-start gap-2 rounded-xl border px-4 py-3 text-[12.5px] leading-relaxed",
              failed
                ? "border-danger/20 bg-danger/5 text-danger/85"
                : "border-fg/10 bg-fg/4 text-fg/55",
            )}
          >
            <AlertTriangle size={14} className="mt-0.5 shrink-0" />
            <span>
              {failed
                ? entry.error || t("playground.feed.failed")
                : cancelled
                  ? t("playground.feed.cancelled")
                  : t("playground.feed.interrupted")}
            </span>
          </div>
        )}
      </div>
      <div className="w-full max-w-2xl shrink-0 rounded-2xl border border-fg/10 bg-fg/4 px-3.5 py-3">
      {(failed || cancelled || interrupted) && entry.images.length > 0 && (
        <div
          className={cn(
            "mb-2 flex items-start gap-2 rounded-lg border px-3 py-2 text-[12px] leading-relaxed",
            failed
              ? "border-danger/20 bg-danger/5 text-danger/85"
              : "border-fg/10 bg-fg/4 text-fg/55",
          )}
        >
          <AlertTriangle size={13} className="mt-0.5 shrink-0" />
          <span>
            {failed
              ? entry.error || t("playground.feed.failed")
              : cancelled
                ? t("playground.feed.cancelled")
                : t("playground.feed.interrupted")}
          </span>
        </div>
      )}
      <p className="text-[12.5px] leading-relaxed text-fg/75 line-clamp-2">{entry.prompt}</p>
      <div className="mt-2 flex flex-wrap items-center gap-x-1.5 gap-y-1 text-[10.5px] text-fg/45">
        <span className="max-w-[220px] truncate rounded-md bg-fg/6 px-1.5 py-0.5">
          {entry.modelName}
        </span>
        {dimensions && (
          <span className="flex items-center gap-1 rounded-md bg-fg/6 px-1.5 py-0.5 font-mono">
            <Scan size={10} />
            {dimensions}
          </span>
        )}
        {entry.seed != null && (
          <span className="flex items-center gap-1 rounded-md bg-fg/6 px-1.5 py-0.5 font-mono">
            <Dices size={10} />
            {entry.seed}
          </span>
        )}
        <span className="flex items-center gap-1 px-1">
          <Clock size={10} />
          {timestamp}
        </span>
        <span className="ml-auto flex items-center gap-0.5">
          <button
            type="button"
            onClick={() => {
              void navigator.clipboard.writeText(entry.prompt).then(() => {
                toast.success(t("playground.feed.promptCopied"));
              });
            }}
            title={t("playground.feed.copyPrompt")}
            className="rounded-md p-1.5 text-fg/35 transition hover:bg-fg/8 hover:text-fg/80"
          >
            <Copy size={13} />
          </button>
          {actions.onReuseSeed && entry.seed != null && (
            <button
              type="button"
              onClick={() => actions.onReuseSeed?.(entry)}
              disabled={actions.disabled}
              title={t("playground.feed.reuseSeed")}
              className="rounded-md p-1.5 text-fg/35 transition hover:bg-fg/8 hover:text-fg/80 disabled:opacity-40"
            >
              <Dices size={13} />
            </button>
          )}
          {actions.onRegenerate && entry.status !== "pending" && (
            <button
              type="button"
              onClick={() => actions.onRegenerate?.(entry)}
              disabled={actions.disabled}
              title={t("playground.feed.regenerate")}
              className="rounded-md p-1.5 text-fg/35 transition hover:bg-fg/8 hover:text-fg/80 disabled:opacity-40"
            >
              <RefreshCw size={13} />
            </button>
          )}
          {actions.onUpscale && entry.status === "complete" && entry.images.length === 1 && (
            <button
              type="button"
              onClick={() => actions.onUpscale?.(entry, entry.images[0])}
              disabled={actions.disabled}
              title={t("playground.feed.upscale")}
              className="rounded-md p-1.5 text-fg/35 transition hover:bg-fg/8 hover:text-fg/80 disabled:opacity-40"
            >
              <Maximize2 size={13} />
            </button>
          )}
          {actions.onSendToImg2img && entry.status === "complete" && entry.images.length === 1 && (
            <button
              type="button"
              onClick={() => actions.onSendToImg2img?.(entry.images[0])}
              disabled={actions.disabled}
              title={t("playground.feed.sendToImg2img")}
              className="rounded-md p-1.5 text-fg/35 transition hover:bg-fg/8 hover:text-fg/80 disabled:opacity-40"
            >
              <ImageUp size={13} />
            </button>
          )}
          <button
            type="button"
            onClick={() => setDeleteMenuOpen(true)}
            disabled={actions.disabled}
            title={t("playground.feed.delete")}
            className="rounded-md p-1.5 text-fg/35 transition hover:bg-fg/8 hover:text-danger disabled:opacity-40"
          >
            <Trash2 size={13} />
          </button>
        </span>
      </div>
      </div>

      <BottomMenu
        isOpen={deleteMenuOpen}
        onClose={() => setDeleteMenuOpen(false)}
        title={t("playground.feed.deleteTitle")}
      >
        <p className="mb-4 text-[12.5px] leading-relaxed text-fg/55">
          {t("playground.feed.deleteBody")}
        </p>
        <div className="space-y-2">
          <button
            type="button"
            onClick={() => {
              setDeleteMenuOpen(false);
              actions.onDelete(entry, false);
            }}
            className="w-full rounded-xl border border-fg/10 bg-fg/4 px-4 py-3 text-sm font-medium text-fg/80 transition hover:border-fg/20"
          >
            {t("playground.feed.deleteKeepImages")}
          </button>
          <button
            type="button"
            onClick={() => {
              setDeleteMenuOpen(false);
              actions.onDelete(entry, true);
            }}
            className="w-full rounded-xl border border-danger/30 bg-danger/10 px-4 py-3 text-sm font-medium text-danger transition hover:bg-danger/15"
          >
            {t("playground.feed.deleteWithImages")}
          </button>
        </div>
      </BottomMenu>

      <AnimatePresence>
        {lightboxIndex !== null && entry.images[lightboxIndex] && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
            className="fixed inset-0 z-100 flex items-center justify-center bg-black/95 p-4"
            onClick={() => setLightboxIndex(null)}
          >
            <button
              type="button"
              onClick={() => setLightboxIndex(null)}
              className="absolute right-5 top-5 z-101 flex h-10 w-10 items-center justify-center rounded-full bg-white/10 text-white/80 transition hover:bg-white/20 hover:text-white"
            >
              <X size={18} />
            </button>
            {lightboxIndex > 0 && (
              <button
                type="button"
                onClick={(event) => {
                  event.stopPropagation();
                  setLightboxIndex(lightboxIndex - 1);
                }}
                className="absolute left-5 top-1/2 z-101 flex h-10 w-10 -translate-y-1/2 items-center justify-center rounded-full bg-white/10 text-white/80 transition hover:bg-white/20 hover:text-white"
              >
                <ChevronLeft size={18} />
              </button>
            )}
            {lightboxIndex < entry.images.length - 1 && (
              <button
                type="button"
                onClick={(event) => {
                  event.stopPropagation();
                  setLightboxIndex(lightboxIndex + 1);
                }}
                className="absolute right-5 top-1/2 z-101 flex h-10 w-10 -translate-y-1/2 items-center justify-center rounded-full bg-white/10 text-white/80 transition hover:bg-white/20 hover:text-white"
              >
                <ChevronRight size={18} />
              </button>
            )}
            <LightboxImage image={entry.images[lightboxIndex]} />
            {entry.images.length > 1 && (
              <span className="absolute bottom-5 left-1/2 -translate-x-1/2 rounded-full bg-white/10 px-2.5 py-1 text-[11px] tabular-nums text-white/70">
                {lightboxIndex + 1} / {entry.images.length}
              </span>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
