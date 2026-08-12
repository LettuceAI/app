import { useState, useCallback, useRef, useEffect } from "react";
import { createPortal } from "react-dom";
import { motion, AnimatePresence } from "framer-motion";
import { X, ZoomIn, ZoomOut, Check, RotateCcw } from "lucide-react";
import { cn, typography, radius, interactive, shadows } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import type { AvatarCrop } from "../../../core/storage/schemas";

interface Position {
  x: number;
  y: number;
}

interface AvatarPositionModalProps {
  isOpen: boolean;
  onClose: () => void;
  imageSrc: string;
  shape?: "circle" | "banner";
  onConfirm: (result: {
    baseImageData: string;
    shapeImageData: string;
    crop: AvatarCrop;
  }) => void;
}

const GUIDE_CENTER = 50;
const GUIDE_RADIUS = 49;

export function AvatarPositionModal({
  isOpen,
  onClose,
  imageSrc,
  shape = "circle",
  onConfirm,
}: AvatarPositionModalProps) {
  const { t } = useI18n();
  const [scale, setScale] = useState(1);
  const [position, setPosition] = useState<Position>({ x: 0, y: 0 });
  const [isDragging, setIsDragging] = useState(false);
  const [dragStart, setDragStart] = useState<Position>({ x: 0, y: 0 });
  const [imageLoaded, setImageLoaded] = useState(false);
  const [imageSize, setImageSize] = useState<{ width: number; height: number } | null>(null);
  const [zoomInput, setZoomInput] = useState(`${Math.round(scale * 100)}`);
  const [isInputFocused, setIsInputFocused] = useState(false);

  const containerRef = useRef<HTMLDivElement>(null);
  const imageRef = useRef<HTMLImageElement>(null);

  const centerImage = useCallback((imgWidth: number, imgHeight: number, newScale: number) => {
    if (!containerRef.current) return { x: 0, y: 0 };
    const containerRect = containerRef.current.getBoundingClientRect();
    const containerCenterX = containerRect.width / 2;
    const containerCenterY = containerRect.height / 2;
    const scaledWidth = imgWidth * newScale;
    const scaledHeight = imgHeight * newScale;
    return {
      x: containerCenterX - scaledWidth / 2,
      y: containerCenterY - scaledHeight / 2,
    };
  }, []);

  // Smallest scale at which the image still fully covers the cropper container.
  // Zooming below this would leave transparent pixels in the saved canvas — those
  // show as empty/dark areas on the displayed card.
  const computeCoverScale = useCallback((imgWidth: number, imgHeight: number) => {
    if (!containerRef.current || !imgWidth || !imgHeight) return 0.02;
    const containerRect = containerRef.current.getBoundingClientRect();
    if (!containerRect.width || !containerRect.height) return 0.02;
    return Math.max(containerRect.width / imgWidth, containerRect.height / imgHeight);
  }, []);

  // Constrain (x, y) so the scaled image still fully covers the cropper container.
  const clampPosition = useCallback(
    (x: number, y: number, s: number, imgWidth: number, imgHeight: number) => {
      if (!containerRef.current) return { x, y };
      const containerRect = containerRef.current.getBoundingClientRect();
      const scaledWidth = imgWidth * s;
      const scaledHeight = imgHeight * s;
      // image left must be <= 0, image right (x + scaledWidth) must be >= containerWidth
      const maxX = 0;
      const minX = containerRect.width - scaledWidth;
      const maxY = 0;
      const minY = containerRect.height - scaledHeight;
      // If image is somehow smaller than the container (shouldn't happen above cover-scale),
      // fall back to center to avoid NaN ranges.
      const clampedX =
        scaledWidth >= containerRect.width
          ? Math.min(maxX, Math.max(minX, x))
          : (containerRect.width - scaledWidth) / 2;
      const clampedY =
        scaledHeight >= containerRect.height
          ? Math.min(maxY, Math.max(minY, y))
          : (containerRect.height - scaledHeight) / 2;
      return { x: clampedX, y: clampedY };
    },
    [],
  );

  const applyInitialCrop = useCallback(
    (imgWidth: number, imgHeight: number) => {
      if (!containerRef.current) return;
      if (!imgWidth || !imgHeight) return;

      const containerRect = containerRef.current.getBoundingClientRect();
      if (!containerRect.width || !containerRect.height) return;

      // Compute "cover" fit: smallest scale such that the image fully covers
      // the cropper container. Matches how `object-cover` would frame the image,
      // so the cropper opens with the same useful framing as the bare avatar.
      const coverScale = Math.max(
        containerRect.width / imgWidth,
        containerRect.height / imgHeight,
      );

      // Start a touch zoomed in beyond cover-fit so faces are more prominent in
      // typical full-body source photos. User can always zoom out to cover-fit.
      const initialScale = Math.min(coverScale * 1.3, 4);

      const centered = centerImage(imgWidth, imgHeight, initialScale);
      setScale(initialScale);
      setPosition(centered);
      setZoomInput((initialScale * 100).toFixed(2).replace(/\.?0+$/, ""));
    },
    [centerImage],
  );

  useEffect(() => {
    if (isOpen && imageSrc) {
      setScale(1);
      setPosition({ x: 0, y: 0 });
      setImageLoaded(false);
      setImageSize(null);

      const img = new Image();
      img.onload = () => {
        setImageSize({ width: img.naturalWidth, height: img.naturalHeight });
        setImageLoaded(true);
        setTimeout(() => {
          applyInitialCrop(img.naturalWidth, img.naturalHeight);
        }, 50);
      };
      img.onerror = () => setImageLoaded(true);
      img.src = imageSrc;
    }
  }, [isOpen, imageSrc, applyInitialCrop]);

  const zoomToCenter = useCallback(
    (newScale: number) => {
      if (!containerRef.current || !imageSize) return;

      const minScale = computeCoverScale(imageSize.width, imageSize.height);
      const clampedScale = Math.min(Math.max(newScale, minScale), 4);

      const containerRect = containerRef.current.getBoundingClientRect();
      const containerCenterX = containerRect.width / 2;
      const containerCenterY = containerRect.height / 2;

      const imageCenterX = (containerCenterX - position.x) / scale;
      const imageCenterY = (containerCenterY - position.y) / scale;

      const newX = containerCenterX - imageCenterX * clampedScale;
      const newY = containerCenterY - imageCenterY * clampedScale;

      const clamped = clampPosition(newX, newY, clampedScale, imageSize.width, imageSize.height);
      setScale(clampedScale);
      setPosition(clamped);
    },
    [scale, position, imageSize, computeCoverScale, clampPosition],
  );

  const handleZoomIn = useCallback(() => {
    const newScale = Math.min(scale + 0.1, 4);
    zoomToCenter(newScale);
  }, [scale, zoomToCenter]);

  const handleZoomOut = useCallback(() => {
    const newScale = scale - 0.1;
    zoomToCenter(newScale);
  }, [scale, zoomToCenter]);

  useEffect(() => {
    if (!isInputFocused) {
      setZoomInput((scale * 100).toFixed(2).replace(/\.?0+$/, ""));
    }
  }, [scale, isInputFocused]);

  const handleReset = useCallback(() => {
    if (!imageSize || !containerRef.current) return;
    const containerRect = containerRef.current.getBoundingClientRect();
    if (!containerRect.width || !containerRect.height) return;
    const coverScale = Math.max(
      containerRect.width / imageSize.width,
      containerRect.height / imageSize.height,
    );
    setScale(coverScale);
    setZoomInput((coverScale * 100).toFixed(2).replace(/\.?0+$/, ""));
    const centered = centerImage(imageSize.width, imageSize.height, coverScale);
    setPosition(centered);
  }, [imageSize, centerImage]);

  const handleDragStart = useCallback(
    (clientX: number, clientY: number) => {
      setIsDragging(true);
      setDragStart({
        x: clientX - position.x,
        y: clientY - position.y,
      });
    },
    [position],
  );

  const handleDragMove = useCallback(
    (clientX: number, clientY: number) => {
      if (!isDragging) return;
      if (!imageSize) {
        setPosition({ x: clientX - dragStart.x, y: clientY - dragStart.y });
        return;
      }
      const clamped = clampPosition(
        clientX - dragStart.x,
        clientY - dragStart.y,
        scale,
        imageSize.width,
        imageSize.height,
      );
      setPosition(clamped);
    },
    [isDragging, dragStart, imageSize, scale, clampPosition],
  );

  const handleDragEnd = useCallback(() => {
    setIsDragging(false);
  }, []);

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      handleDragStart(e.clientX, e.clientY);
    },
    [handleDragStart],
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      handleDragMove(e.clientX, e.clientY);
    },
    [handleDragMove],
  );

  const handleMouseUp = useCallback(() => {
    handleDragEnd();
  }, [handleDragEnd]);

  const lastPinchDistance = useRef<number | null>(null);

  const handleTouchStart = useCallback(
    (e: React.TouchEvent) => {
      if (e.touches.length === 1) {
        const touch = e.touches[0];
        handleDragStart(touch.clientX, touch.clientY);
        lastPinchDistance.current = null;
      } else if (e.touches.length === 2) {
        setIsDragging(false);
        const dx = e.touches[0].clientX - e.touches[1].clientX;
        const dy = e.touches[0].clientY - e.touches[1].clientY;
        lastPinchDistance.current = Math.sqrt(dx * dx + dy * dy);
      }
    },
    [handleDragStart],
  );

  const handleTouchMove = useCallback(
    (e: React.TouchEvent) => {
      if (e.touches.length === 1 && lastPinchDistance.current === null) {
        const touch = e.touches[0];
        handleDragMove(touch.clientX, touch.clientY);
      } else if (e.touches.length === 2 && containerRef.current && imageSize) {
        const dx = e.touches[0].clientX - e.touches[1].clientX;
        const dy = e.touches[0].clientY - e.touches[1].clientY;
        const distance = Math.sqrt(dx * dx + dy * dy);

        if (lastPinchDistance.current !== null) {
          const scaleDelta = (distance - lastPinchDistance.current) * 0.004;
          const minScale = computeCoverScale(imageSize.width, imageSize.height);
          const newScale = Math.min(Math.max(scale + scaleDelta, minScale), 4);

          const containerRect = containerRef.current.getBoundingClientRect();
          const containerCenterX = containerRect.width / 2;
          const containerCenterY = containerRect.height / 2;
          const imageCenterX = (containerCenterX - position.x) / scale;
          const imageCenterY = (containerCenterY - position.y) / scale;
          const newX = containerCenterX - imageCenterX * newScale;
          const newY = containerCenterY - imageCenterY * newScale;

          const clamped = clampPosition(newX, newY, newScale, imageSize.width, imageSize.height);
          setScale(newScale);
          setPosition(clamped);
        }
        lastPinchDistance.current = distance;
      }
    },
    [handleDragMove, scale, position, imageSize, computeCoverScale, clampPosition],
  );

  const handleTouchEnd = useCallback(() => {
    handleDragEnd();
    lastPinchDistance.current = null;
  }, [handleDragEnd]);

  const handleWheel = useCallback(
    (e: React.WheelEvent) => {
      e.preventDefault();
      if (!containerRef.current || !imageSize) return;

      const delta = e.deltaY > 0 ? -0.08 : 0.08;
      const minScale = computeCoverScale(imageSize.width, imageSize.height);
      const newScale = Math.min(Math.max(scale + delta, minScale), 4);

      const containerRect = containerRef.current.getBoundingClientRect();
      const containerCenterX = containerRect.width / 2;
      const containerCenterY = containerRect.height / 2;
      const imageCenterX = (containerCenterX - position.x) / scale;
      const imageCenterY = (containerCenterY - position.y) / scale;
      const newX = containerCenterX - imageCenterX * newScale;
      const newY = containerCenterY - imageCenterY * newScale;

      const clamped = clampPosition(newX, newY, newScale, imageSize.width, imageSize.height);
      setScale(newScale);
      setPosition(clamped);
    },
    [scale, position, imageSize, computeCoverScale, clampPosition],
  );

  const handleZoomInput = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    if (val === "" || /^\d*\.?\d*$/.test(val)) {
      setZoomInput(val);

      const num = parseFloat(val);
      if (!isNaN(num) && num >= 10 && num <= 400) {
        const newScale = num / 100;
        zoomToCenter(newScale);
      }
    }
  };

  const handleZoomBlur = () => {
    setIsInputFocused(false);
    let num = parseFloat(zoomInput);
    if (isNaN(num)) num = scale * 100;

    const constrainedNum = Math.min(Math.max(num, 2), 400);
    const newScale = constrainedNum / 100;

    zoomToCenter(newScale);
    setZoomInput(constrainedNum.toFixed(2).replace(/\.?0+$/, ""));
  };

  const handleConfirm = useCallback(() => {
    if (!containerRef.current || !imageRef.current || !imageSize) return;
    const containerRect = containerRef.current.getBoundingClientRect();
    if (!containerRect.width || !containerRect.height) return;
    if (!imageSize.width || !imageSize.height) return;

    // Banner saves a square image (same shape as base avatar). The BannerCharacterCard
    // object-covers it into its wide slot at display time, which preserves vertical
    // headroom — matching the look you get when falling back to the base avatar.
    const outputWidth = 1024;
    const outputHeight = 1024;
    const exportScale = outputWidth / containerRect.width;
    const squareCanvas = document.createElement("canvas");
    squareCanvas.width = outputWidth;
    squareCanvas.height = outputHeight;
    const squareCtx = squareCanvas.getContext("2d");
    if (!squareCtx) return;

    squareCtx.clearRect(0, 0, outputWidth, outputHeight);
    squareCtx.setTransform(
      scale * exportScale,
      0,
      0,
      scale * exportScale,
      position.x * exportScale,
      position.y * exportScale,
    );
    squareCtx.drawImage(imageRef.current, 0, 0);

    const baseImageData = squareCanvas.toDataURL("image/png");
    let shapeImageData = baseImageData;

    if (shape === "circle") {
      const roundCanvas = document.createElement("canvas");
      roundCanvas.width = outputWidth;
      roundCanvas.height = outputHeight;
      const roundCtx = roundCanvas.getContext("2d");
      if (!roundCtx) return;

      roundCtx.clearRect(0, 0, outputWidth, outputHeight);
      roundCtx.save();
      roundCtx.beginPath();
      roundCtx.arc(outputWidth / 2, outputHeight / 2, outputWidth / 2, 0, Math.PI * 2);
      roundCtx.clip();
      roundCtx.drawImage(squareCanvas, 0, 0);
      roundCtx.restore();
      shapeImageData = roundCanvas.toDataURL("image/png");
    }

    onConfirm({
      baseImageData,
      shapeImageData,
      crop: {
        x: position.x,
        y: position.y,
        scale,
      },
    });
    onClose();
  }, [scale, position, onConfirm, onClose, imageSize, shape]);

  return createPortal(
    <AnimatePresence>
      {isOpen && (
        <>
          <motion.div
            className="fixed inset-0 z-200 bg-black/80 backdrop-blur-sm"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={onClose}
          />

          {/* Modal */}
          <motion.div
            className={cn(
              "fixed left-1/2 top-1/2 z-210 w-[90vw] max-w-md -translate-x-1/2 -translate-y-1/2",
              "flex flex-col overflow-hidden border border-white/10 bg-[#0a0a0c]",
              "rounded-xl",
              shadows.xl,
            )}
            initial={{ opacity: 0, scale: 0.9 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.9 }}
            transition={{ type: "spring", damping: 25, stiffness: 300 }}
          >
            {/* Header */}
            <div className="flex items-center justify-between border-b border-white/10 px-5 py-4">
              <h3 className={cn(typography.h3.size, typography.h3.weight, "text-white")}>
                {t("components.avatarPosition.title")}
              </h3>
              <button
                onClick={onClose}
                className={cn(
                  "flex h-12 w-12 items-center justify-center text-white/50",
                  radius.full,
                  interactive.transition.default,
                  "hover:bg-white/10 hover:text-white active:scale-95",
                )}
              >
                <X size={18} />
              </button>
            </div>

            {/* Image Area with Circular Guide */}
            <div className="relative px-5 py-6">
              <p className={cn(typography.bodySmall.size, "mb-4 text-center text-white/50")}>
                {t("components.avatarPosition.instructions")}
              </p>

              {/* Full image container */}
              <div
                ref={containerRef}
                className={cn(
                  "relative mx-auto w-full max-w-70 overflow-hidden",
                  "aspect-square",
                  radius.lg,
                  "cursor-move touch-none select-none",
                  "border border-white/10",
                )}
                style={{
                  transform: "translateZ(0)",
                  isolation: "isolate",
                }}
                onMouseDown={handleMouseDown}
                onMouseMove={handleMouseMove}
                onMouseUp={handleMouseUp}
                onMouseLeave={handleMouseUp}
                onTouchStart={handleTouchStart}
                onTouchMove={handleTouchMove}
                onTouchEnd={handleTouchEnd}
                onWheel={handleWheel}
              >
                {/* Image */}
                <img
                  ref={imageRef}
                  src={imageSrc}
                  alt={t("components.avatarPosition.alt")}
                  className="absolute pointer-events-none"
                  style={{
                    top: 0,
                    left: 0,
                    transform: `translate(${position.x}px, ${position.y}px) scale(${scale})`,
                    transformOrigin: "top left",
                    maxWidth: "none",
                    transition: isDragging ? "none" : "transform 0.1s ease-out",
                  }}
                  onLoad={() => setImageLoaded(true)}
                  draggable={false}
                />

                {/* Darkening overlay — circular cutout for circle shape, none for banner (full square is captured) */}
                <div
                  className="absolute inset-0 z-10 pointer-events-none"
                  style={{
                    background:
                      shape === "banner"
                        ? "transparent"
                        : "radial-gradient(circle closest-side, transparent 98%, rgba(0, 0, 0, 0.7) 98%)",
                    WebkitBackdropFilter: "none",
                  }}
                />

                {/* SVG for guides and border */}
                <div className="absolute inset-0 z-20 pointer-events-none">
                  <svg className="h-full w-full" viewBox="0 0 100 100" preserveAspectRatio="none">
                    {shape === "banner" ? (
                      <>
                        {/* Full square border — what gets saved */}
                        <rect
                          x="1"
                          y="1"
                          width="98"
                          height="98"
                          rx="2"
                          fill="none"
                          stroke="rgba(52, 211, 153, 0.55)"
                          strokeWidth="0.6"
                        />
                        {/* Rule-of-thirds */}
                        <line
                          x1="33.33"
                          y1="2"
                          x2="33.33"
                          y2="98"
                          stroke="rgba(255, 255, 255, 0.12)"
                          strokeWidth="0.25"
                        />
                        <line
                          x1="66.66"
                          y1="2"
                          x2="66.66"
                          y2="98"
                          stroke="rgba(255, 255, 255, 0.12)"
                          strokeWidth="0.25"
                        />
                        <line
                          x1="2"
                          y1="33.33"
                          x2="98"
                          y2="33.33"
                          stroke="rgba(255, 255, 255, 0.12)"
                          strokeWidth="0.25"
                        />
                        <line
                          x1="2"
                          y1="66.66"
                          x2="98"
                          y2="66.66"
                          stroke="rgba(255, 255, 255, 0.12)"
                          strokeWidth="0.25"
                        />
                      </>
                    ) : (
                      <>
                        <circle
                          cx={GUIDE_CENTER}
                          cy={GUIDE_CENTER}
                          r={GUIDE_RADIUS}
                          fill="none"
                          stroke="rgba(52, 211, 153, 0.6)"
                          strokeWidth="0.8"
                        />
                        <line
                          x1={GUIDE_CENTER}
                          y1={GUIDE_CENTER - GUIDE_RADIUS}
                          x2={GUIDE_CENTER}
                          y2={GUIDE_CENTER + GUIDE_RADIUS}
                          stroke="rgba(255, 255, 255, 0.15)"
                          strokeWidth="0.3"
                        />
                        <line
                          x1={GUIDE_CENTER - GUIDE_RADIUS}
                          y1={GUIDE_CENTER}
                          x2={GUIDE_CENTER + GUIDE_RADIUS}
                          y2={GUIDE_CENTER}
                          stroke="rgba(255, 255, 255, 0.15)"
                          strokeWidth="0.3"
                        />
                      </>
                    )}
                    {/* Center point */}
                    <circle
                      cx={GUIDE_CENTER}
                      cy={GUIDE_CENTER}
                      r="1.5"
                      fill="rgba(52, 211, 153, 0.5)"
                    />
                  </svg>
                </div>

                {/* Loading overlay */}
                {!imageLoaded && (
                  <div className="absolute inset-0 flex items-center justify-center bg-black/50">
                    <div className="h-8 w-8 animate-spin rounded-full border-2 border-white/20 border-t-white/70" />
                  </div>
                )}
              </div>

              {/* Zoom Controls */}
              <div className="mt-4 flex items-center justify-center gap-2">
                <button
                  onClick={handleZoomOut}
                  className={cn(
                    "flex h-12 w-12 items-center justify-center border border-white/10 bg-white/5 text-white/60",
                    radius.md,
                    interactive.transition.default,
                    "hover:bg-white/10 hover:text-white active:scale-95",
                  )}
                >
                  <ZoomOut size={18} />
                </button>

                <div
                  className={cn(
                    "flex h-12 min-w-20 items-center justify-center border border-white/10 bg-white/5",
                    radius.md,
                    "focus-within:border-white/20 focus-within:bg-white/8 transition-colors",
                  )}
                >
                  <div className="flex items-center px-3">
                    <input
                      type="text"
                      inputMode="decimal"
                      value={zoomInput}
                      onChange={handleZoomInput}
                      onFocus={() => setIsInputFocused(true)}
                      onBlur={handleZoomBlur}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") {
                          handleZoomBlur();
                          (e.target as HTMLInputElement).blur();
                        }
                      }}
                      className="w-12 bg-transparent text-center text-sm font-medium text-white/70 outline-none"
                    />
                    <span className="text-sm font-medium text-white/40">%</span>
                  </div>
                </div>

                <button
                  onClick={handleZoomIn}
                  className={cn(
                    "flex h-12 w-12 items-center justify-center border border-white/10 bg-white/5 text-white/60",
                    radius.md,
                    interactive.transition.default,
                    "hover:bg-white/10 hover:text-white active:scale-95",
                  )}
                >
                  <ZoomIn size={18} />
                </button>

                <button
                  onClick={handleReset}
                  className={cn(
                    "flex h-12 items-center justify-center gap-1.5 border border-white/10 bg-white/5 px-3 text-white/60",
                    radius.md,
                    interactive.transition.default,
                    "hover:bg-white/10 hover:text-white active:scale-95",
                  )}
                >
                  <RotateCcw size={18} />
                </button>
              </div>
            </div>

            {/* Actions */}
            <div className="flex gap-3 border-t border-white/10 p-5">
              <button
                onClick={onClose}
                className={cn(
                  "flex-1 py-3 font-medium text-white/70",
                  radius.md,
                  "border border-white/10 bg-white/5",
                  interactive.transition.default,
                  "hover:bg-white/10 hover:text-white active:scale-[0.98]",
                )}
              >
                {t("common.buttons.cancel")}
              </button>
              <button
                onClick={handleConfirm}
                disabled={!imageLoaded}
                className={cn(
                  "flex flex-1 items-center justify-center gap-2 py-3 font-semibold text-white",
                  radius.md,
                  "border border-emerald-400/40 bg-emerald-500/80",
                  interactive.transition.default,
                  imageLoaded
                    ? "hover:bg-emerald-500/90 active:scale-[0.98]"
                    : "cursor-not-allowed opacity-50",
                )}
              >
                <Check size={18} />
                {t("common.buttons.confirm")}
              </button>
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>,
    document.body,
  );
}
