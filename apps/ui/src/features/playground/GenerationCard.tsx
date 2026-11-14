import { useEffect, useRef, useState, type ReactNode } from "react";
import { motion } from "framer-motion";
import { useTranslation } from "react-i18next";
import { Copy, Dices, ImageOff, ImageUp, Maximize2, RefreshCw, Trash2 } from "lucide-react";
import { Alert, Badge, cn, icon, IconButton, Panel, motion as m, toast } from "@lettuceai/crisp";
import { canRegenerate, entryShape, useFailureText, type PlaygroundAsset, type PlaygroundEntry, type PlaygroundImage } from "./playground-data";

const collapsedPrompt = 36;
const expandedPromptLimit = 240;

export interface CardActions {
  disabled: boolean;
  onOpenImage: (entry: PlaygroundEntry, index: number) => void;
  onDelete: (entry: PlaygroundEntry) => void;
  onReuseSeed: (entry: PlaygroundEntry) => void;
  onRegenerate: (entry: PlaygroundEntry) => void;
  onUpscale?: (entry: PlaygroundEntry, asset: PlaygroundAsset) => void;
  onUseAsStart?: (asset: PlaygroundAsset) => void;
}

function CardImage({ image, alt, onClick, overlay }: {
  image: PlaygroundImage;
  alt: string;
  onClick: () => void;
  overlay?: ReactNode;
}) {
  const url = image.asset?.url ?? null;
  const wrapperRef = useRef<HTMLDivElement | null>(null);
  const imageRef = useRef<HTMLImageElement | null>(null);
  const [corner, setCorner] = useState<{ right: number; bottom: number } | null>(null);
  const [broken, setBroken] = useState(false);

  useEffect(() => {
    const wrapper = wrapperRef.current;
    const element = imageRef.current;
    if (!url || !wrapper || !element) return;
    const compute = () => {
      const wrapperRect = wrapper.getBoundingClientRect();
      const imageRect = element.getBoundingClientRect();
      if (!imageRect.width || !imageRect.height) return;
      setCorner({
        right: Math.max(0, wrapperRect.right - imageRect.right),
        bottom: Math.max(0, wrapperRect.bottom - imageRect.bottom),
      });
    };
    compute();
    const observer = new ResizeObserver(compute);
    observer.observe(wrapper);
    observer.observe(element);
    return () => observer.disconnect();
  }, [url]);

  if (!url || broken) {
    return (
      <div className="absolute inset-0 flex items-center justify-center">
        <div className="flex h-40 w-40 items-center justify-center rounded-xl border border-line bg-surface-1 text-fg-4">
          <ImageOff size={icon.xl} />
        </div>
      </div>
    );
  }
  return (
    <div ref={wrapperRef} className="absolute inset-0">
      <button type="button" onClick={onClick} className="group absolute inset-0 flex cursor-zoom-in items-center justify-center">
        <img
          ref={imageRef}
          src={url}
          alt={alt}
          loading="lazy"
          decoding="async"
          onError={() => setBroken(true)}
          className="max-h-full max-w-full rounded-xl border border-line object-contain shadow-raised transition group-hover:brightness-105"
        />
      </button>
      {overlay && (
        <div style={{ right: (corner?.right ?? 0) + 8, bottom: (corner?.bottom ?? 0) + 8 }} className="absolute z-10">
          {overlay}
        </div>
      )}
    </div>
  );
}

export function GenerationCard({ entry, actions }: { entry: PlaygroundEntry; actions: CardActions }) {
  const { t, i18n } = useTranslation("playground");
  const failureText = useFailureText();
  const [expanded, setExpanded] = useState(false);
  const [clamped, setClamped] = useState(true);
  const [slotHeight, setSlotHeight] = useState<number | null>(null);
  const [expandTarget, setExpandTarget] = useState(expandedPromptLimit);
  const infoRef = useRef<HTMLDivElement | null>(null);
  const promptRef = useRef<HTMLParagraphElement | null>(null);
  const overlayActive = expanded || !clamped;

  const togglePrompt = () => {
    if (expanded) {
      setExpanded(false);
      return;
    }
    setSlotHeight(infoRef.current?.offsetHeight ?? null);
    setExpandTarget(Math.min(promptRef.current?.scrollHeight ?? expandedPromptLimit, expandedPromptLimit));
    setClamped(false);
    setExpanded(true);
  };

  const copyPrompt = () => {
    navigator.clipboard.writeText(entry.prompt).then(
      () => toast.success(t("feed.promptCopied")),
      () => toast.error(failureText(null)),
    );
  };

  const images = entry.images;
  const complete = entry.status === "complete";
  const status =
    entry.status === "failed"
      ? entry.failure?.kind === "interrupted"
        ? { tone: "danger" as const, text: t("feed.interrupted") }
        : { tone: "danger" as const, text: entry.failure?.message || t("feed.failed") }
      : entry.status === "cancelled"
        ? { tone: "info" as const, text: t("feed.cancelled") }
        : entry.status === "pending"
          ? { tone: "info" as const, text: t("feed.interrupted") }
          : null;
  const first = images[0];
  const shape = entryShape(entry);
  const dimensions = first?.width && first.height ? `${first.width}x${first.height}` : shape.size;
  const time = new Intl.DateTimeFormat(i18n.language, { hour: "2-digit", minute: "2-digit" }).format(entry.created_at);
  const viewIndex = (index: number) => images.slice(0, index).filter((image) => image.asset).length;
  const singleAsset = complete && images.length === 1 ? first?.asset ?? null : null;
  const action = (label: string, onClick: () => void, children: ReactNode, disabled = actions.disabled) => (
    <IconButton size="sm" label={label} disabled={disabled} onClick={onClick}>
      {children}
    </IconButton>
  );

  return (
    <div className="flex h-full min-h-0 w-full flex-col items-center gap-3 px-4 pb-4 pt-3 @xl:px-8">
      <div className="flex min-h-0 w-full flex-1 items-center justify-center">
        {images.length > 1 ? (
          <div className={cn("grid h-full w-full auto-rows-fr gap-3", images.length === 2 ? "grid-cols-2" : "grid-cols-2 @xl:grid-cols-3")}>
            {images.map((image, index) => {
              const asset = image.asset;
              return (
                <div key={asset?.asset_id ?? index} className="relative min-h-0 min-w-0">
                  <CardImage
                    image={image}
                    alt={entry.prompt}
                    onClick={() => actions.onOpenImage(entry, viewIndex(index))}
                    overlay={
                      complete && asset && (actions.onUpscale || actions.onUseAsStart) ? (
                        <div className="flex items-center gap-1">
                          {actions.onUpscale && (
                            <IconButton size="sm" variant="overlay" label={t("feed.upscale")} disabled={actions.disabled} onClick={() => actions.onUpscale?.(entry, asset)}>
                              <Maximize2 size={icon.sm} />
                            </IconButton>
                          )}
                          {actions.onUseAsStart && (
                            <IconButton size="sm" variant="overlay" label={t("feed.sendToImg2img")} disabled={actions.disabled} onClick={() => actions.onUseAsStart?.(asset)}>
                              <ImageUp size={icon.sm} />
                            </IconButton>
                          )}
                        </div>
                      ) : undefined
                    }
                  />
                </div>
              );
            })}
          </div>
        ) : first ? (
          <div className="relative h-full w-full">
            <CardImage image={first} alt={entry.prompt} onClick={() => actions.onOpenImage(entry, 0)} />
          </div>
        ) : (
          status && <Alert tone={status.tone} title={<span className="[overflow-wrap:anywhere]">{status.text}</span>} className="w-full max-w-2xl min-w-0" />
        )}
      </div>

      <div className="relative w-full max-w-2xl shrink-0" style={overlayActive && slotHeight ? { height: slotHeight } : undefined}>
        <div ref={infoRef} className={cn(overlayActive && "absolute inset-x-0 bottom-0 z-20")}>
          <Panel className={cn(overlayActive && "bg-surface-el shadow-raised")}>
            {status && images.length > 0 && <Alert tone={status.tone} title={<span className="[overflow-wrap:anywhere]">{status.text}</span>} />}
            <button type="button" onClick={togglePrompt} className="block w-full cursor-pointer text-left">
              <motion.p
                ref={promptRef}
                initial={false}
                animate={{ maxHeight: expanded ? expandTarget : collapsedPrompt }}
                transition={m.collapse}
                onAnimationComplete={() => {
                  if (!expanded) {
                    setClamped(true);
                    setSlotHeight(null);
                  }
                }}
                className={cn("select-text text-sm text-fg-2", clamped ? "line-clamp-2" : "pr-1", expanded ? "overflow-y-auto" : "overflow-hidden")}
              >
                {entry.prompt}
              </motion.p>
            </button>
            <div className="flex flex-wrap items-center gap-x-1.5 gap-y-1">
              <Badge>
                <span className="max-w-56 truncate">{entry.model_name}</span>
              </Badge>
              {dimensions && <Badge>{dimensions}</Badge>}
              {entry.seed !== null && <Badge>{entry.seed}</Badge>}
              <span className="px-1 text-xs text-fg-3">{time}</span>
              <span className="ml-auto flex items-center gap-0.5">
                {action(t("feed.copyPrompt"), copyPrompt, <Copy size={icon.sm} />, false)}
                {entry.seed !== null && action(t("feed.reuseSeed"), () => actions.onReuseSeed(entry), <Dices size={icon.sm} />)}
                {canRegenerate(entry) && action(t("feed.regenerate"), () => actions.onRegenerate(entry), <RefreshCw size={icon.sm} />)}
                {singleAsset && actions.onUpscale && action(t("feed.upscale"), () => actions.onUpscale?.(entry, singleAsset), <Maximize2 size={icon.sm} />)}
                {singleAsset && actions.onUseAsStart && action(t("feed.sendToImg2img"), () => actions.onUseAsStart?.(singleAsset), <ImageUp size={icon.sm} />)}
                {action(t("feed.delete"), () => actions.onDelete(entry), <Trash2 size={icon.sm} />)}
              </span>
            </div>
          </Panel>
        </div>
      </div>
    </div>
  );
}
