import { useState, useCallback, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  Sparkles,
  Loader2,
  RefreshCw,
  Check,
  X,
  HelpCircle,
  ChevronLeft,
  ChevronRight,
  Wand2,
  ArrowLeft,
} from "lucide-react";

import { BottomMenu } from "../BottomMenu";
import { cn, typography, radius, interactive, animations } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import {
  buildAvatarEditPrompt,
  buildAvatarGenerationPrompt,
  generateImage,
  type ImageGenerationRequest,
  type GeneratedImage,
  resolveGeneratedImageUrl,
  resolveAvatarGenerationOptions,
} from "../../../core/image-generation";
import { readSettings, SETTINGS_UPDATED_EVENT } from "../../../core/storage/repo";
import type { Model, ProviderCredential } from "../../../core/storage/schemas";
import { openDocs } from "../../../core/utils/docs";

interface AvatarGenerationSheetProps {
  isOpen: boolean;
  onClose: () => void;
  onImageGenerated: (imageDataUrl: string) => void;
  subjectName?: string;
  subjectDescription?: string;
  initialImageSrc?: string | null;
  startInEditMode?: boolean;
  hidePromptNavigation?: boolean;
}

interface AvatarVariant {
  image: GeneratedImage;
  renderedPrompt: string;
  imageUrl?: string | null;
}

type ViewMode = "initial" | "result";

function getErrorMessage(error: unknown, fallback: string): string {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }

  if (typeof error === "string" && error.trim()) {
    return error;
  }

  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string" && message.trim()) {
      return message;
    }
  }

  return fallback;
}

export function AvatarGenerationSheet({
  isOpen,
  onClose,
  onImageGenerated,
  subjectName,
  subjectDescription,
  initialImageSrc,
  startInEditMode = false,
  hidePromptNavigation = false,
}: AvatarGenerationSheetProps) {
  const { t } = useI18n();
  const [prompt, setPrompt] = useState("");
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [variants, setVariants] = useState<AvatarVariant[]>([]);
  const [currentVariantIndex, setCurrentVariantIndex] = useState(0);
  const [editRequest, setEditRequest] = useState("");
  const [operationLabel, setOperationLabel] = useState<string | null>(null);
  const [selectedModel, setSelectedModel] = useState<Model | null>(null);
  const [selectedProvider, setSelectedProvider] = useState<ProviderCredential | null>(null);
  const [loading, setLoading] = useState(true);
  const [isRefining, setIsRefining] = useState(false);
  const [viewMode, setViewMode] = useState<ViewMode>("initial");

  const currentVariant = variants[currentVariantIndex] ?? null;

  useEffect(() => {
    if (!isOpen) return;

    const loadModelState = async () => {
      try {
        setLoading(true);
        const settings = await readSettings();
        const options = resolveAvatarGenerationOptions(settings);

        setSelectedModel(options.defaultModel);
        setSelectedProvider(options.defaultProvider);
        setError(null);
        if (!options.enabled || !options.defaultModel || !options.defaultProvider) {
          setError(t("components.avatarGeneration.modelsLoadError"));
        }
      } catch (err) {
        console.error("Failed to load models:", err);
        setError(t("components.avatarGeneration.modelsLoadError"));
        setSelectedModel(null);
        setSelectedProvider(null);
      } finally {
        setLoading(false);
      }
    };

    void loadModelState();
    window.addEventListener(SETTINGS_UPDATED_EVENT, loadModelState);

    return () => {
      window.removeEventListener(SETTINGS_UPDATED_EVENT, loadModelState);
    };
  }, [isOpen, t]);

  useEffect(() => {
    if (!isOpen) {
      setPrompt("");
      setVariants([]);
      setCurrentVariantIndex(0);
      setError(null);
      setEditRequest("");
      setOperationLabel(null);
      setSelectedModel(null);
      setSelectedProvider(null);
      setViewMode("initial");
      setIsRefining(false);
    }
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) return;

    if (startInEditMode && initialImageSrc) {
      setPrompt("");
      setVariants([
        {
          image: {
            assetId: "current-avatar",
            filePath: initialImageSrc,
            mimeType: "image/*",
          },
          renderedPrompt: "",
          imageUrl: initialImageSrc,
        },
      ]);
      setCurrentVariantIndex(0);
      setEditRequest("");
      setError(null);
      setViewMode("result");
      setIsRefining(true);
      return;
    }

    setViewMode("initial");
    setIsRefining(false);
  }, [initialImageSrc, isOpen, startInEditMode]);

  useEffect(() => {
    let cancelled = false;

    if (!currentVariant || currentVariant.imageUrl !== undefined) {
      return;
    }

    void resolveGeneratedImageUrl(currentVariant.image)
      .then((url) => {
        if (!cancelled) {
          setVariants((prev) =>
            prev.map((variant, index) =>
              index === currentVariantIndex ? { ...variant, imageUrl: url ?? null } : variant,
            ),
          );
        }
      })
      .catch((err) => {
        console.error("Failed to resolve generated image:", err);
        if (!cancelled) {
          setVariants((prev) =>
            prev.map((variant, index) =>
              index === currentVariantIndex ? { ...variant, imageUrl: null } : variant,
            ),
          );
        }
      });

    return () => {
      cancelled = true;
    };
  }, [currentVariant, currentVariantIndex]);

  const appendVariant = useCallback(
    (image: GeneratedImage, renderedPrompt: string) => {
      const nextIndex = variants.length;
      setVariants([
        ...variants,
        {
          image,
          renderedPrompt,
        },
      ]);
      setCurrentVariantIndex(nextIndex);
      setViewMode("result");
      setIsRefining(false);
    },
    [variants],
  );

  const handleGenerate = useCallback(async () => {
    if (!selectedModel || !selectedProvider || !prompt.trim()) return;

    setGenerating(true);
    setError(null);
    setOperationLabel(t("components.avatarGeneration.inProgress"));

    try {
      const renderedPrompt = await buildAvatarGenerationPrompt({
        subjectName,
        subjectDescription,
        avatarRequest: prompt.trim(),
      });
      const request: ImageGenerationRequest = {
        prompt: renderedPrompt,
        model: selectedModel.name,
        providerId: selectedModel.providerId,
        credentialId: selectedProvider.id,
        advancedModelSettings: selectedModel.advancedModelSettings ?? null,
        size: selectedModel.advancedModelSettings?.sdSize ?? "1024x1024",
        n: 1,
      };

      const response = await generateImage(request);

      if (response.images.length > 0) {
        appendVariant(response.images[0], renderedPrompt);
      }
    } catch (err) {
      console.error("Image generation failed:", err);
      setError(getErrorMessage(err, "Failed to generate image"));
    } finally {
      setGenerating(false);
      setOperationLabel(null);
    }
  }, [appendVariant, selectedModel, selectedProvider, prompt, subjectDescription, subjectName, t]);

  const resolveCurrentImageAsDataUrl = useCallback(async (): Promise<string | null> => {
    if (!currentVariant) return null;

    const imageUrl =
      currentVariant.imageUrl !== undefined
        ? currentVariant.imageUrl
        : await resolveGeneratedImageUrl(currentVariant.image);
    if (!imageUrl) return null;

    try {
      const response = await fetch(imageUrl);
      const blob = await response.blob();
      return await new Promise<string>((resolve, reject) => {
        const reader = new FileReader();
        reader.onloadend = () => resolve(reader.result as string);
        reader.onerror = () => reject(new Error("Failed to convert image to data URL"));
        reader.readAsDataURL(blob);
      });
    } catch (err) {
      console.error("Failed to resolve generated image as data URL:", err);
      return null;
    }
  }, [currentVariant]);

  const handleApplyEdit = useCallback(async () => {
    if (!selectedModel || !selectedProvider || !editRequest.trim() || !currentVariant) return;

    const sourceImageDataUrl = await resolveCurrentImageAsDataUrl();
    if (!sourceImageDataUrl) {
      setError(t("components.avatarGeneration.editImageLoadError"));
      return;
    }

    setGenerating(true);
    setError(null);
    setOperationLabel(t("components.avatarGeneration.editingInProgress"));

    try {
      const renderedPrompt = await buildAvatarEditPrompt({
        subjectName,
        subjectDescription,
        currentAvatarPrompt: currentVariant.renderedPrompt,
        editRequest: editRequest.trim(),
      });

      const request: ImageGenerationRequest = {
        prompt: renderedPrompt,
        model: selectedModel.name,
        providerId: selectedModel.providerId,
        credentialId: selectedProvider.id,
        advancedModelSettings: selectedModel.advancedModelSettings ?? null,
        inputImages: [sourceImageDataUrl],
        size: selectedModel.advancedModelSettings?.sdSize ?? "1024x1024",
        n: 1,
      };

      const response = await generateImage(request);
      if (response.images.length > 0) {
        appendVariant(response.images[0], renderedPrompt);
        setEditRequest("");
      }
    } catch (err) {
      console.error("Avatar edit generation failed:", err);
      setError(getErrorMessage(err, "Failed to edit avatar"));
    } finally {
      setGenerating(false);
      setOperationLabel(null);
    }
  }, [
    appendVariant,
    currentVariant,
    editRequest,
    resolveCurrentImageAsDataUrl,
    selectedModel,
    selectedProvider,
    subjectDescription,
    subjectName,
    t,
  ]);

  const handleUseImage = useCallback(async () => {
    if (!currentVariant) return;

    const imageUrl =
      currentVariant.imageUrl !== undefined
        ? currentVariant.imageUrl
        : await resolveGeneratedImageUrl(currentVariant.image);
    if (!imageUrl) return;

    try {
      const response = await fetch(imageUrl);
      const blob = await response.blob();
      const reader = new FileReader();
      reader.onloadend = () => {
        const dataUrl = reader.result as string;
        onImageGenerated(dataUrl);
        onClose();
      };
      reader.readAsDataURL(blob);
    } catch (err) {
      console.warn("Failed to convert to data URL, using original:", err);
      onImageGenerated(imageUrl);
      onClose();
    }
  }, [currentVariant, onImageGenerated, onClose]);

  if (loading && isOpen) {
    return (
      <BottomMenu isOpen={isOpen} onClose={onClose} title={t("components.avatarGeneration.title")}>
        <div className="flex h-48 items-center justify-center">
          <motion.div
            animate={{ rotate: 360 }}
            transition={{ duration: 1, repeat: Infinity, ease: "linear" }}
          >
            <Loader2 className="h-8 w-8 text-white/20" />
          </motion.div>
        </div>
      </BottomMenu>
    );
  }

  return (
    <BottomMenu
      isOpen={isOpen}
      onClose={onClose}
      title={t("components.avatarGeneration.title")}
      leftAction={
        viewMode === "result" && !generating && !hidePromptNavigation ? (
          <button
            type="button"
            onClick={() => setViewMode("initial")}
            className="flex items-center gap-2 text-fg/40 transition-colors hover:text-fg/70"
          >
            <ArrowLeft size={16} />
            <span className={typography.bodySmall.size}>
              {t("components.avatarGeneration.backToResults")}
            </span>
          </button>
        ) : null
      }
      rightAction={
        <button
          type="button"
          onClick={() => openDocs("imagegen", "avatar-generation")}
          className="text-fg/40 hover:text-fg/60 transition"
          aria-label={t("components.avatarGeneration.help")}
        >
          <HelpCircle size={18} />
        </button>
      }
    >
      <div className="relative overflow-hidden px-1 pb-2">
        <AnimatePresence mode="wait">
          {viewMode === "initial" && !generating ? (
            <motion.div key="input-view" {...animations.fadeInFast} className="space-y-6">
              <div className="space-y-3">
                <div
                  className={cn(
                    "relative group",
                    "rounded-xl border border-white/10 bg-white/5 p-4",
                    "focus-within:border-white/20 focus-within:bg-white/[0.08]",
                    interactive.transition.default,
                  )}
                >
                  <textarea
                    value={prompt}
                    onChange={(e) => setPrompt(e.target.value)}
                    placeholder={t("components.avatarGeneration.describePlaceholder")}
                    rows={6}
                    className={cn(
                      "min-h-[168px] w-full resize-none bg-transparent text-sm text-white placeholder-white/20",
                      "focus:outline-none",
                    )}
                    autoFocus
                  />

                  <div className="mt-4 flex justify-end">
                    <button
                      onClick={handleGenerate}
                      disabled={!prompt.trim() || !selectedModel || !selectedProvider}
                      className={cn(
                        "relative flex items-center gap-2 px-6 py-2.5",
                        radius.lg,
                        interactive.transition.fast,
                        prompt.trim() && selectedModel && selectedProvider
                          ? "bg-emerald-500/20 text-emerald-100 border border-emerald-500/30 hover:bg-emerald-500/30 active:scale-[0.97]"
                          : "bg-white/5 text-white/20 border border-white/5 cursor-not-allowed",
                      )}
                    >
                      <Sparkles size={16} />
                      <span className="text-sm font-semibold">
                        {t("components.avatarGeneration.title")}
                      </span>
                    </button>
                  </div>
                </div>
              </div>

              {variants.length > 0 && !hidePromptNavigation && (
                <button
                  onClick={() => setViewMode("result")}
                  className={cn(
                    "flex w-full items-center justify-center gap-2 py-3 text-fg/40",
                    typography.bodySmall.size,
                    "hover:text-fg/60 transition-colors",
                  )}
                >
                  <ArrowLeft size={14} className="rotate-180" />
                  {t("components.avatarGeneration.backToResults")}
                </button>
              )}
            </motion.div>
          ) : generating ? (
            <motion.div
              key="loading-view"
              {...animations.scaleIn}
              className="flex min-h-[320px] flex-col items-center justify-center gap-6"
            >
              <div className="flex h-20 w-20 items-center justify-center rounded-full border border-white/10 bg-white/5">
                <Loader2 className="h-8 w-8 animate-spin text-white/20" />
              </div>

              <div className="text-center space-y-1">
                <p className={cn(typography.h3.size, typography.h3.weight, "text-white")}>
                  {operationLabel || t("components.avatarGeneration.inProgress")}
                </p>
                <p className={cn(typography.bodySmall.size, "text-white/40")}>
                  {t("components.avatarGeneration.magicInTheWorks")}
                </p>
              </div>
            </motion.div>
          ) : currentVariant ? (
            <motion.div key="result-view" {...animations.fadeInFast} className="space-y-6">
              {/* Image Preview Area */}
              <div className="relative group">
                <div
                  className={cn(
                    "relative aspect-square w-full overflow-hidden border border-white/10 bg-white/5",
                    radius.lg,
                  )}
                >
                  <AnimatePresence mode="wait">
                    <motion.img
                      key={currentVariantIndex}
                      src={currentVariant.imageUrl ?? ""}
                      alt={t("components.avatarGeneration.alt")}
                      className="h-full w-full object-cover"
                      initial={{ opacity: 0, scale: 1.1 }}
                      animate={{ opacity: 1, scale: 1 }}
                      transition={{ duration: 0.4 }}
                    />
                  </AnimatePresence>
                </div>

                {/* Variant Navigation Overlay */}
                {variants.length > 1 && (
                  <div className="absolute inset-x-0 top-4 flex justify-center gap-2 px-4 opacity-0 group-hover:opacity-100 transition-opacity">
                    <div className="flex items-center gap-1.5 rounded-full bg-black/40 px-3 py-1.5 backdrop-blur-md border border-white/10">
                      <button
                        onClick={() => setCurrentVariantIndex((i) => Math.max(0, i - 1))}
                        disabled={currentVariantIndex === 0}
                        className="text-white/60 hover:text-white disabled:opacity-30 p-0.5"
                      >
                        <ChevronLeft size={16} />
                      </button>
                      <span className="text-[10px] font-bold text-white/90 min-w-[3ch] text-center">
                        {currentVariantIndex + 1} / {variants.length}
                      </span>
                      <button
                        onClick={() =>
                          setCurrentVariantIndex((i) => Math.min(variants.length - 1, i + 1))
                        }
                        disabled={currentVariantIndex === variants.length - 1}
                        className="text-white/60 hover:text-white disabled:opacity-30 p-0.5"
                      >
                        <ChevronRight size={16} />
                      </button>
                    </div>
                  </div>
                )}

                {/* Use This Button (Primary Action) */}
                <div className="absolute bottom-4 inset-x-4">
                  <button
                    onClick={handleUseImage}
                    className={cn(
                      "flex w-full items-center justify-center gap-2 py-3.5",
                      radius.lg,
                      "bg-white text-black font-bold shadow-lg hover:bg-white/90 transition-colors",
                      interactive.transition.default,
                      "active:scale-[0.98]",
                    )}
                  >
                    <Check size={18} strokeWidth={3} />
                    <span className="text-sm">{t("components.avatarGeneration.useThis")}</span>
                  </button>
                </div>
              </div>

              {/* Refinement Actions */}
              <div className="space-y-3">
                <AnimatePresence mode="wait">
                  {!isRefining ? (
                    <motion.div
                      key="actions"
                      initial={{ opacity: 0, y: 10 }}
                      animate={{ opacity: 1, y: 0 }}
                      exit={{ opacity: 0, y: -10 }}
                      className="flex gap-3"
                    >
                      <button
                        onClick={() => setIsRefining(true)}
                        className={cn(
                          "flex-1 flex items-center justify-center gap-2 py-3 border border-white/10 bg-white/5",
                          radius.lg,
                          interactive.transition.default,
                          "hover:bg-white/8 active:scale-[0.98]",
                        )}
                      >
                        <Wand2 size={16} className="text-emerald-400" />
                        <span className={cn(typography.body.size, "font-medium text-white")}>
                          {t("components.avatarGeneration.refine")}
                        </span>
                      </button>
                      <button
                        onClick={handleGenerate}
                        className={cn(
                          "flex items-center justify-center gap-2 px-6 py-3 border border-white/10 bg-white/5",
                          radius.lg,
                          interactive.transition.default,
                          "hover:bg-white/8 active:scale-[0.98]",
                        )}
                      >
                        <RefreshCw size={16} className="text-white/40" />
                      </button>
                    </motion.div>
                  ) : (
                    <motion.div
                      key="refine-input"
                      initial={{ opacity: 0, scale: 0.95 }}
                      animate={{ opacity: 1, scale: 1 }}
                      exit={{ opacity: 0, scale: 0.95 }}
                      className="space-y-3"
                    >
                      <div
                        className={cn(
                          "relative rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-3",
                          interactive.transition.default,
                        )}
                      >
                        <textarea
                          value={editRequest}
                          onChange={(e) => setEditRequest(e.target.value)}
                          placeholder={t("components.avatarGeneration.editRequestPlaceholder")}
                          rows={2}
                          className={cn(
                            "w-full resize-none bg-transparent text-sm text-white placeholder-white/20",
                            "focus:outline-none",
                          )}
                          autoFocus
                        />
                        <div className="mt-2 flex justify-end gap-2">
                          <button
                            onClick={() => setIsRefining(false)}
                            className="px-3 py-1.5 text-white/40 hover:text-white transition-colors text-xs font-medium"
                          >
                            {t("common.buttons.cancel")}
                          </button>
                          <button
                            onClick={handleApplyEdit}
                            disabled={!editRequest.trim() || generating}
                            className={cn(
                              "flex items-center gap-2 px-4 py-1.5 rounded-lg text-xs font-bold transition-all",
                              editRequest.trim()
                                ? "bg-emerald-500/20 text-emerald-100 border border-emerald-500/30 hover:bg-emerald-500/30"
                                : "bg-white/5 text-white/20 border border-white/5",
                            )}
                          >
                            <Sparkles size={14} />
                            <span>{t("components.avatarGeneration.apply")}</span>
                          </button>
                        </div>
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>
            </motion.div>
          ) : null}
        </AnimatePresence>

        {/* Error Display */}
        <AnimatePresence>
          {error && (
            <motion.div
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, scale: 0.95 }}
              className={cn(
                "mt-4 flex items-center gap-3 border border-danger/20 bg-danger/5 px-4 py-3",
                radius.md,
              )}
            >
              <X className="h-4 w-4 shrink-0 text-danger" />
              <p className="text-xs text-danger/80">{error}</p>
              <button onClick={() => setError(null)} className="ml-auto p-1 text-danger/40">
                <X size={14} />
              </button>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </BottomMenu>
  );
}
