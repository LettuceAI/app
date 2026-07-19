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
      <div className="flex aspect-square w-full items-center justify-center rounded-lg border border-fg/8 bg-fg/4 text-fg/20">
        <ImageOff size={18} />
      </div>
    );
  }
  return (
    <button type="button" onClick={onClick} className="group block w-full cursor-zoom-in">
      <img
        src={url}
        alt=""
        loading="lazy"
        decoding="async"
        className="w-full rounded-lg border border-fg/8 object-cover transition group-hover:brightness-105"
        style={{
          aspectRatio:
            image.width && image.height && image.width > 0 && image.height > 0
              ? `${image.width} / ${image.height}`
              : undefined,
        }}
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
  const timestamp = new Date(entry.createdAt).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });

  return (
    <div className="rounded-xl border border-fg/8 bg-fg/[0.02] p-3">
      {entry.images.length > 0 && (
        <div
          className={cn(
            "mb-3 grid gap-2",
            entry.images.length === 1
              ? "grid-cols-1 sm:max-w-[420px]"
              : entry.images.length === 2
                ? "grid-cols-2"
                : "grid-cols-2 sm:grid-cols-3",
          )}
        >
          {entry.images.map((image, index) => (
            <CardImage
              key={image.assetId || image.filePath || index}
              image={image}
              onClick={() => setLightboxIndex(index)}
            />
          ))}
        </div>
      )}
      {(failed || cancelled || interrupted) && (
        <div
          className={cn(
            "mb-3 flex items-start gap-2 rounded-lg border px-3 py-2.5 text-[12px] leading-relaxed",
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
      <p className="text-[12.5px] leading-relaxed text-fg/75 line-clamp-3">{entry.prompt}</p>
      <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[10.5px] text-fg/40">
        <span className="truncate">{entry.modelName}</span>
        {entry.seed != null && (
          <span className="flex items-center gap-1 font-mono">
            <Dices size={10} />
            {entry.seed}
          </span>
        )}
        <span className="flex items-center gap-1">
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
          {actions.onUpscale && entry.status === "complete" && entry.images[0] && (
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
          {actions.onSendToImg2img && entry.status === "complete" && entry.images[0] && (
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
