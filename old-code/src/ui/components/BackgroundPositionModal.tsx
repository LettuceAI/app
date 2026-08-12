import { useState, useCallback, useRef, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { X, ZoomIn, ZoomOut, Check, RotateCcw } from "lucide-react";
import { cn, typography, radius, interactive, shadows } from "../design-tokens";
import { useI18n } from "../../core/i18n/context";

interface Position {
    x: number;
    y: number;
}

interface BackgroundPositionModalProps {
    isOpen: boolean;
    onClose: () => void;
    imageSrc: string;
    onConfirm: (croppedImageDataUrl: string) => void;
}

// Portrait aspect ratio (9:16 for mobile backgrounds)
const ASPECT_WIDTH = 9;
const ASPECT_HEIGHT = 16;
const OUTPUT_WIDTH = 720;
const OUTPUT_HEIGHT = 1280;

export function BackgroundPositionModal({
    isOpen,
    onClose,
    imageSrc,
    onConfirm,
}: BackgroundPositionModalProps) {
    const { t } = useI18n();
    const [scale, setScale] = useState(1);
    const [position, setPosition] = useState<Position>({ x: 0, y: 0 });
    const [isDragging, setIsDragging] = useState(false);
    const [dragStart, setDragStart] = useState<Position>({ x: 0, y: 0 });
    const [imageLoaded, setImageLoaded] = useState(false);
    const [imageSize, setImageSize] = useState<{ width: number; height: number } | null>(null);

    const containerRef = useRef<HTMLDivElement>(null);
    const imageRef = useRef<HTMLImageElement>(null);
    const canvasRef = useRef<HTMLCanvasElement>(null);

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
                    const centered = centerImage(img.naturalWidth, img.naturalHeight, 1);
                    setPosition(centered);
                }, 50);
            };
            img.onerror = () => setImageLoaded(true);
            img.src = imageSrc;
        }
    }, [isOpen, imageSrc, centerImage]);

    const zoomToCenter = useCallback((newScale: number) => {
        if (!containerRef.current || !imageSize) return;

        const containerRect = containerRef.current.getBoundingClientRect();
        const containerCenterX = containerRect.width / 2;
        const containerCenterY = containerRect.height / 2;

        const imageCenterX = (containerCenterX - position.x) / scale;
        const imageCenterY = (containerCenterY - position.y) / scale;

        const newX = containerCenterX - imageCenterX * newScale;
        const newY = containerCenterY - imageCenterY * newScale;

        setScale(newScale);
        setPosition({ x: newX, y: newY });
    }, [scale, position, imageSize]);

    const handleZoomIn = useCallback(() => {
        const newScale = Math.min(scale + 0.1, 4);
        zoomToCenter(newScale);
    }, [scale, zoomToCenter]);

    const handleZoomOut = useCallback(() => {
        const newScale = Math.max(scale - 0.1, 0.1);
        zoomToCenter(newScale);
    }, [scale, zoomToCenter]);

    const handleReset = useCallback(() => {
        if (!imageSize) return;
        setScale(1);
        const centered = centerImage(imageSize.width, imageSize.height, 1);
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
        [position]
    );

    const handleDragMove = useCallback(
        (clientX: number, clientY: number) => {
            if (!isDragging) return;
            setPosition({
                x: clientX - dragStart.x,
                y: clientY - dragStart.y,
            });
        },
        [isDragging, dragStart]
    );

    const handleDragEnd = useCallback(() => {
        setIsDragging(false);
    }, []);

    const handleMouseDown = useCallback(
        (e: React.MouseEvent) => {
            e.preventDefault();
            handleDragStart(e.clientX, e.clientY);
        },
        [handleDragStart]
    );

    const handleMouseMove = useCallback(
        (e: React.MouseEvent) => {
            handleDragMove(e.clientX, e.clientY);
        },
        [handleDragMove]
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
        [handleDragStart]
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
                    const newScale = Math.min(Math.max(scale + scaleDelta, 0.1), 4);

                    const containerRect = containerRef.current.getBoundingClientRect();
                    const containerCenterX = containerRect.width / 2;
                    const containerCenterY = containerRect.height / 2;
                    const imageCenterX = (containerCenterX - position.x) / scale;
                    const imageCenterY = (containerCenterY - position.y) / scale;
                    const newX = containerCenterX - imageCenterX * newScale;
                    const newY = containerCenterY - imageCenterY * newScale;

                    setScale(newScale);
                    setPosition({ x: newX, y: newY });
                }
                lastPinchDistance.current = distance;
            }
        },
        [handleDragMove, scale, position, imageSize]
    );

    const handleTouchEnd = useCallback(() => {
        handleDragEnd();
        lastPinchDistance.current = null;
    }, [handleDragEnd]);

    const handleWheel = useCallback((e: React.WheelEvent) => {
        e.preventDefault();
        if (!containerRef.current || !imageSize) return;

        const delta = e.deltaY > 0 ? -0.08 : 0.08;
        const newScale = Math.min(Math.max(scale + delta, 0.1), 4);

        const containerRect = containerRef.current.getBoundingClientRect();
        const containerCenterX = containerRect.width / 2;
        const containerCenterY = containerRect.height / 2;
        const imageCenterX = (containerCenterX - position.x) / scale;
        const imageCenterY = (containerCenterY - position.y) / scale;
        const newX = containerCenterX - imageCenterX * newScale;
        const newY = containerCenterY - imageCenterY * newScale;

        setScale(newScale);
        setPosition({ x: newX, y: newY });
    }, [scale, position, imageSize]);

    const handleConfirm = useCallback(async () => {
        if (!imageRef.current || !canvasRef.current || !containerRef.current) return;

        const canvas = canvasRef.current;
        const ctx = canvas.getContext("2d");
        if (!ctx) return;

        const img = imageRef.current;
        const container = containerRef.current;

        canvas.width = OUTPUT_WIDTH;
        canvas.height = OUTPUT_HEIGHT;

        const containerRect = container.getBoundingClientRect();

        // Calculate the crop rectangle dimensions based on the container
        const containerWidth = containerRect.width;
        const containerHeight = containerRect.height;

        // The visible crop area (portrait rectangle in center)
        const cropAspect = ASPECT_WIDTH / ASPECT_HEIGHT;
        let cropWidth: number, cropHeight: number;

        if (containerWidth / containerHeight > cropAspect) {
            // Container is wider than crop aspect
            cropHeight = containerHeight * 0.85;
            cropWidth = cropHeight * cropAspect;
        } else {
            // Container is taller than crop aspect
            cropWidth = containerWidth * 0.85;
            cropHeight = cropWidth / cropAspect;
        }

        const cropCenterX = containerWidth / 2;
        const cropCenterY = containerHeight / 2;

        // Convert container coordinates to image coordinates
        const sourceX = (cropCenterX - cropWidth / 2 - position.x) / scale;
        const sourceY = (cropCenterY - cropHeight / 2 - position.y) / scale;
        const sourceWidth = cropWidth / scale;
        const sourceHeight = cropHeight / scale;

        ctx.clearRect(0, 0, OUTPUT_WIDTH, OUTPUT_HEIGHT);
        ctx.drawImage(
            img,
            sourceX,
            sourceY,
            sourceWidth,
            sourceHeight,
            0,
            0,
            OUTPUT_WIDTH,
            OUTPUT_HEIGHT
        );

        const dataUrl = canvas.toDataURL("image/webp", 0.9);
        onConfirm(dataUrl);
        onClose();
    }, [scale, position, onConfirm, onClose]);

    // Calculate crop rectangle dimensions for the SVG mask
    const getCropDimensions = useCallback(() => {
        const cropAspect = ASPECT_WIDTH / ASPECT_HEIGHT;
        // Container is assumed square (100x100 in viewBox)
        const containerAspect = 1;

        if (containerAspect > cropAspect) {
            // Container is wider
            const height = 85;
            const width = height * cropAspect;
            return { width, height, x: (100 - width) / 2, y: (100 - height) / 2 };
        } else {
            // Container is taller
            const width = 85;
            const height = width / cropAspect;
            return { width, height, x: (100 - width) / 2, y: (100 - height) / 2 };
        }
    }, []);

    const cropDims = getCropDimensions();

    return (
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
                            shadows.xl
                        )}
                        initial={{ opacity: 0, scale: 0.9 }}
                        animate={{ opacity: 1, scale: 1 }}
                        exit={{ opacity: 0, scale: 0.9 }}
                        transition={{ type: "spring", damping: 25, stiffness: 300 }}
                    >
                        {/* Header */}
                        <div className="flex items-center justify-between border-b border-white/10 px-5 py-4">
                            <h3 className={cn(typography.h3.size, typography.h3.weight, "text-white")}>
                                {t("components.backgroundPosition.title")}
                            </h3>
                            <button
                                onClick={onClose}
                                className={cn(
                                    "flex h-12 w-12 items-center justify-center text-white/50",
                                    radius.full,
                                    interactive.transition.default,
                                    "hover:bg-white/10 hover:text-white active:scale-95"
                                )}
                            >
                                <X size={18} />
                            </button>
                        </div>

                        {/* Image Area with Portrait Rectangle Guide */}
                        <div className="relative px-5 py-6">
                            <p className={cn(typography.bodySmall.size, "mb-4 text-center text-white/50")}>
                                {t("components.backgroundPosition.instructions")}
                            </p>

                            {/* Full image container */}
                            <div
                                ref={containerRef}
                                className={cn(
                                    "relative mx-auto aspect-3/4 w-full max-w-70 overflow-hidden",
                                    radius.lg,
                                    "cursor-move touch-none select-none",
                                    "border border-white/10"
                                )}
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
                                    alt={t("components.extra.backgroundAlt")}
                                    className="absolute pointer-events-none"
                                    style={{
                                        transform: `translate(${position.x}px, ${position.y}px) scale(${scale})`,
                                        transformOrigin: "top left",
                                        maxWidth: "none",
                                        transition: isDragging ? "none" : "transform 0.1s ease-out",
                                    }}
                                    onLoad={() => setImageLoaded(true)}
                                    draggable={false}
                                />

                                {/* Darkening overlay with portrait rectangle cutout */}
                                <svg
                                    className="absolute inset-0 h-full w-full pointer-events-none"
                                    viewBox="0 0 100 133"
                                    preserveAspectRatio="none"
                                >
                                    <defs>
                                        <mask id="portraitMask">
                                            <rect x="0" y="0" width="100" height="133" fill="white" />
                                            <rect
                                                x={cropDims.x}
                                                y={(133 - cropDims.height * 1.33) / 2}
                                                width={cropDims.width}
                                                height={cropDims.height * 1.33}
                                                rx="2"
                                                fill="black"
                                            />
                                        </mask>
                                    </defs>
                                    {/* Dark overlay outside rectangle */}
                                    <rect
                                        x="0"
                                        y="0"
                                        width="100"
                                        height="133"
                                        fill="rgba(0, 0, 0, 0.7)"
                                        mask="url(#portraitMask)"
                                    />
                                    {/* Rectangle border */}
                                    <rect
                                        x={cropDims.x}
                                        y={(133 - cropDims.height * 1.33) / 2}
                                        width={cropDims.width}
                                        height={cropDims.height * 1.33}
                                        rx="2"
                                        fill="none"
                                        stroke="rgba(34, 211, 238, 0.6)"
                                        strokeWidth="0.8"
                                    />
                                    {/* Rule of thirds grid */}
                                    <line
                                        x1={cropDims.x + cropDims.width / 3}
                                        y1={(133 - cropDims.height * 1.33) / 2}
                                        x2={cropDims.x + cropDims.width / 3}
                                        y2={(133 + cropDims.height * 1.33) / 2}
                                        stroke="rgba(255, 255, 255, 0.15)"
                                        strokeWidth="0.3"
                                    />
                                    <line
                                        x1={cropDims.x + (cropDims.width * 2) / 3}
                                        y1={(133 - cropDims.height * 1.33) / 2}
                                        x2={cropDims.x + (cropDims.width * 2) / 3}
                                        y2={(133 + cropDims.height * 1.33) / 2}
                                        stroke="rgba(255, 255, 255, 0.15)"
                                        strokeWidth="0.3"
                                    />
                                    <line
                                        x1={cropDims.x}
                                        y1={(133 - cropDims.height * 1.33) / 2 + (cropDims.height * 1.33) / 3}
                                        x2={cropDims.x + cropDims.width}
                                        y2={(133 - cropDims.height * 1.33) / 2 + (cropDims.height * 1.33) / 3}
                                        stroke="rgba(255, 255, 255, 0.15)"
                                        strokeWidth="0.3"
                                    />
                                    <line
                                        x1={cropDims.x}
                                        y1={(133 - cropDims.height * 1.33) / 2 + (cropDims.height * 1.33 * 2) / 3}
                                        x2={cropDims.x + cropDims.width}
                                        y2={(133 - cropDims.height * 1.33) / 2 + (cropDims.height * 1.33 * 2) / 3}
                                        stroke="rgba(255, 255, 255, 0.15)"
                                        strokeWidth="0.3"
                                    />
                                </svg>

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
                                        "hover:bg-white/10 hover:text-white active:scale-95"
                                    )}
                                >
                                    <ZoomOut size={18} />
                                </button>

                                <div
                                    className={cn(
                                        "flex h-12 min-w-20 items-center justify-center border border-white/10 bg-white/5 px-3",
                                        radius.md
                                    )}
                                >
                                    <span className="text-sm font-medium text-white/70">{Math.round(scale * 100)}%</span>
                                </div>

                                <button
                                    onClick={handleZoomIn}
                                    className={cn(
                                        "flex h-12 w-12 items-center justify-center border border-white/10 bg-white/5 text-white/60",
                                        radius.md,
                                        interactive.transition.default,
                                        "hover:bg-white/10 hover:text-white active:scale-95"
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
                                        "hover:bg-white/10 hover:text-white active:scale-95"
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
                                    "hover:bg-white/10 hover:text-white active:scale-[0.98]"
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
                                        : "cursor-not-allowed opacity-50"
                                )}
                            >
                                <Check size={18} />
                                {t("common.buttons.confirm")}
                            </button>
                        </div>

                        {/* Hidden canvas for cropping */}
                        <canvas ref={canvasRef} className="hidden" />
                    </motion.div>
                </>
            )}
        </AnimatePresence>
    );
}
