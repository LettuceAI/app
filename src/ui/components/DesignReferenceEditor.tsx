import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { ImagePlus, Loader2, Sparkles, X } from "lucide-react";
import { abortMessage, generateDesignReferenceDescription } from "../../core/chat/manager";
import { resolveSceneWriterOptions } from "../../core/image-generation";
import { readSettings, SETTINGS_UPDATED_EVENT } from "../../core/storage/repo";
import { convertToImageUrl } from "../../core/storage/images";
import { isRenderableImageUrl } from "../../core/utils/image";
import { useImageData } from "../hooks/useImageData";
import { BottomMenu } from "./BottomMenu";

function ReferenceThumb({
  value,
  index,
  onRemove,
}: {
  value: string;
  index: number;
  onRemove: (index: number) => void;
}) {
  const imageUrl = useImageData(value);

  return (
    <div className="group relative overflow-hidden rounded-lg border border-fg/10 bg-surface-el/20">
      {imageUrl ? (
        <img
          src={imageUrl}
          alt={`Design reference ${index + 1}`}
          className="h-28 w-full object-cover"
        />
      ) : (
        <div className="flex h-28 items-center justify-center text-xs text-fg/40">Loading...</div>
      )}
      <button
        type="button"
        onClick={() => onRemove(index)}
        className="absolute right-2 top-2 flex h-7 w-7 items-center justify-center rounded-md border border-fg/10 bg-surface/90 text-fg/70 opacity-0 transition hover:border-danger/40 hover:bg-danger/20 hover:text-danger group-hover:opacity-100"
        aria-label="Remove design reference"
      >
        <X size={14} />
      </button>
    </div>
  );
}

async function readFilesAsDataUrls(files: FileList | File[]): Promise<string[]> {
  return Promise.all(
    Array.from(files).map(
      (file) =>
        new Promise<string>((resolve, reject) => {
          const reader = new FileReader();
          reader.onload = () => resolve(reader.result as string);
          reader.onerror = () => reject(new Error(`Failed to read ${file.name}`));
          reader.readAsDataURL(file);
        }),
    ),
  );
}

export function DesignReferenceEditor({
  designDescription,
  onDesignDescriptionChange,
  referenceImages,
  onReferenceImagesChange,
  subjectName,
  subjectDescription,
  avatarImage,
  showHeader = true,
  title = "Design references",
  description = "Upload a few clear reference images plus one canonical visual description.",
  descriptionPlaceholder = "Describe the stable look: face, hair, build, age presentation, outfit cues, accessories, and art/style direction.",
}: {
  designDescription: string;
  onDesignDescriptionChange: (value: string) => void;
  referenceImages: string[];
  onReferenceImagesChange: (value: string[]) => void;
  subjectName?: string;
  subjectDescription?: string;
  avatarImage?: string | null;
  showHeader?: boolean;
  title?: string;
  description?: string;
  descriptionPlaceholder?: string;
}) {
  const [isGeneratingDescription, setIsGeneratingDescription] = useState(false);
  const [writerModelLabel, setWriterModelLabel] = useState<string | null>(null);
  const [writerAvailable, setWriterAvailable] = useState(false);
  const [showDraftMenu, setShowDraftMenu] = useState(false);
  const [draftText, setDraftText] = useState<string | null>(null);
  const [draftError, setDraftError] = useState<string | null>(null);
  const requestIdRef = useRef<string | null>(null);
  const unlistenRef = useRef<UnlistenFn | null>(null);
  const loadingTimeoutRef = useRef<number | null>(null);
  const helperText = useMemo(
    () =>
      referenceImages.length > 0
        ? `${referenceImages.length} reference image${referenceImages.length === 1 ? "" : "s"} attached`
        : "No reference images attached yet",
    [referenceImages.length],
  );

  const loadWriterModelState = useCallback(async () => {
    try {
      const settings = await readSettings();
      const options = resolveSceneWriterOptions(settings);
      setWriterAvailable(options.models.length > 0);
      setWriterModelLabel(options.defaultModel?.displayName || options.defaultModel?.name || null);
    } catch (error) {
      console.error("Failed to load scene writer settings:", error);
      setWriterAvailable(false);
      setWriterModelLabel(null);
    }
  }, []);

  useEffect(() => {
    void loadWriterModelState();
    window.addEventListener(SETTINGS_UPDATED_EVENT, loadWriterModelState);
    return () => window.removeEventListener(SETTINGS_UPDATED_EVENT, loadWriterModelState);
  }, [loadWriterModelState]);

  const clearDraftRuntime = useCallback(() => {
    if (loadingTimeoutRef.current !== null) {
      window.clearTimeout(loadingTimeoutRef.current);
      loadingTimeoutRef.current = null;
    }
    if (unlistenRef.current) {
      unlistenRef.current();
      unlistenRef.current = null;
    }
    requestIdRef.current = null;
  }, []);

  const cancelDraftGeneration = useCallback(async () => {
    const requestId = requestIdRef.current;
    clearDraftRuntime();
    if (!requestId) return;
    try {
      await abortMessage(requestId);
    } catch {
      // ignore abort failures
    }
  }, [clearDraftRuntime]);

  useEffect(() => {
    return () => {
      void cancelDraftGeneration();
    };
  }, [cancelDraftGeneration]);

  const handleAddImages = async () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/*";
    input.multiple = true;
    input.onchange = async (event) => {
      const files = (event.target as HTMLInputElement).files;
      if (!files?.length) return;
      try {
        const nextImages = await readFilesAsDataUrls(files);
        onReferenceImagesChange([...referenceImages, ...nextImages]);
      } catch (error) {
        console.error("Failed to load design reference images:", error);
      }
    };
    input.click();
  };

  const handleRemoveImage = (index: number) => {
    onReferenceImagesChange(referenceImages.filter((_, currentIndex) => currentIndex !== index));
  };

  const resolveImageToDataUrl = useCallback(async (value: string): Promise<string | null> => {
    const trimmed = value.trim();
    if (!trimmed) return null;
    if (trimmed.startsWith("data:")) return trimmed;

    const resolvedUrl = isRenderableImageUrl(trimmed) ? trimmed : await convertToImageUrl(trimmed);
    if (!resolvedUrl) return null;
    if (resolvedUrl.startsWith("data:")) return resolvedUrl;

    const response = await fetch(resolvedUrl);
    if (!response.ok) {
      throw new Error(`Failed to read image asset (${response.status})`);
    }

    const blob = await response.blob();
    return await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result as string);
      reader.onerror = () => reject(new Error("Failed to convert image asset to data URL"));
      reader.readAsDataURL(blob);
    });
  }, []);

  const handleGenerateDescription = useCallback(async () => {
    if (isGeneratingDescription) return;
    if (!writerAvailable) {
      setDraftError("Add a compatible scene writer model in Image Generation settings first.");
      setShowDraftMenu(true);
      return;
    }

    const hasAvatar = Boolean(avatarImage?.trim());
    const hasReferences = referenceImages.some((value) => value.trim().length > 0);

    if (!hasAvatar && !hasReferences) {
      setDraftError("Add an avatar or at least one reference image before generating.");
      setShowDraftMenu(true);
      return;
    }

    setIsGeneratingDescription(true);
    setDraftText(null);
    setDraftError(null);
    setShowDraftMenu(true);

    try {
      const [resolvedAvatarImage, resolvedReferenceImages] = await Promise.all([
        hasAvatar && avatarImage
          ? resolveImageToDataUrl(avatarImage)
          : Promise.resolve<string | null>(null),
        Promise.all(referenceImages.map((value) => resolveImageToDataUrl(value))),
      ]);
      const usableReferenceImages = resolvedReferenceImages.filter((value): value is string =>
        Boolean(value),
      );

      if (!resolvedAvatarImage && usableReferenceImages.length === 0) {
        throw new Error("No usable reference images were found.");
      }

      const requestId = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
      requestIdRef.current = requestId;
      let streamingText = "";
      let hasStartedStreaming = false;

      loadingTimeoutRef.current = window.setTimeout(() => {
        if (!hasStartedStreaming) {
          setIsGeneratingDescription(false);
        }
      }, 5000);

      const unlisten = await listen<any>(`api-normalized://${requestId}`, (event) => {
        if (requestIdRef.current !== requestId) return;
        try {
          const payload =
            typeof event.payload === "string" ? JSON.parse(event.payload) : event.payload;

          if (payload && payload.type === "delta" && payload.data?.text) {
            if (!hasStartedStreaming) {
              hasStartedStreaming = true;
              setIsGeneratingDescription(false);
              if (loadingTimeoutRef.current !== null) {
                window.clearTimeout(loadingTimeoutRef.current);
                loadingTimeoutRef.current = null;
              }
            }
            streamingText += String(payload.data.text);
            setDraftText(streamingText);
          } else if (payload && payload.type === "error") {
            const message =
              payload.data?.message ||
              payload.data?.error ||
              payload.message ||
              "Draft generation failed.";
            setDraftError(String(message));
            setIsGeneratingDescription(false);
            if (loadingTimeoutRef.current !== null) {
              window.clearTimeout(loadingTimeoutRef.current);
              loadingTimeoutRef.current = null;
            }
          }
        } catch (error) {
          console.error("Failed to parse design description stream:", error);
        }
      });
      unlistenRef.current = unlisten;

      const generated = await generateDesignReferenceDescription({
        subjectName,
        subjectDescription,
        currentDescription: designDescription,
        avatarImage: resolvedAvatarImage,
        referenceImages: usableReferenceImages,
        requestId,
        stream: true,
      });

      if (!streamingText.trim()) {
        setDraftText(generated);
      }

      if (!hasStartedStreaming) {
        setIsGeneratingDescription(false);
        if (loadingTimeoutRef.current !== null) {
          window.clearTimeout(loadingTimeoutRef.current);
          loadingTimeoutRef.current = null;
        }
      }
    } catch (error) {
      console.error("Failed to generate design description:", error);
      setDraftError(
        error instanceof Error ? error.message : "Failed to generate design description.",
      );
    } finally {
      if (requestIdRef.current) {
        clearDraftRuntime();
      }
      setIsGeneratingDescription(false);
    }
  }, [
    avatarImage,
    clearDraftRuntime,
    designDescription,
    isGeneratingDescription,
    referenceImages,
    resolveImageToDataUrl,
    subjectDescription,
    subjectName,
    writerAvailable,
  ]);

  const handleCloseDraftMenu = useCallback(() => {
    setShowDraftMenu(false);
    setDraftText(null);
    setDraftError(null);
    void cancelDraftGeneration();
  }, [cancelDraftGeneration]);

  const handleUseDraft = useCallback(() => {
    if (!draftText?.trim()) return;
    onDesignDescriptionChange(draftText);
    setShowDraftMenu(false);
    setDraftText(null);
    setDraftError(null);
  }, [draftText, onDesignDescriptionChange]);

  return (
    <>
      <section className="space-y-3">
        {showHeader ? (
          <div className="flex items-start justify-between gap-4">
            <div className="space-y-1">
              <h3 className="text-sm font-semibold text-fg">{title}</h3>
              <p className="max-w-2xl text-sm leading-6 text-fg/55">{description}</p>
            </div>
            <button
              type="button"
              onClick={() => void handleAddImages()}
              className="inline-flex shrink-0 items-center gap-2 whitespace-nowrap rounded-lg border border-fg/15 bg-fg/[0.04] px-3 py-2 text-sm font-medium text-fg/80 transition hover:border-fg/25 hover:bg-fg/[0.07]"
            >
              <ImagePlus size={14} />
              Add references
            </button>
          </div>
        ) : null}

        <div className="space-y-2">
          <div className="flex items-center justify-between gap-3">
            <label className="text-sm font-medium text-fg/75">Visual description</label>
            <button
              type="button"
              onClick={() => void handleGenerateDescription()}
              disabled={isGeneratingDescription || !writerAvailable}
              className="inline-flex shrink-0 items-center gap-2 rounded-lg border border-fg/15 bg-fg/[0.04] px-3 py-2 text-sm font-medium text-fg/80 transition hover:border-fg/25 hover:bg-fg/[0.07] disabled:cursor-not-allowed disabled:opacity-50"
            >
              {isGeneratingDescription ? (
                <Loader2 size={14} className="animate-spin" />
              ) : (
                <Sparkles size={14} />
              )}
              Draft with AI
            </button>
          </div>
          <textarea
            value={designDescription}
            onChange={(event) => onDesignDescriptionChange(event.target.value)}
            rows={4}
            placeholder={descriptionPlaceholder}
            className="min-h-[120px] w-full resize-y rounded-lg border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-sm leading-6 text-fg placeholder-fg/35 transition focus:border-fg/25 focus:outline-none"
          />
          <p className="text-xs text-fg/40">
            {writerAvailable
              ? `Uses ${writerModelLabel ?? "the compatible scene writer model"} to draft from your avatar and reference images.`
              : "Add a compatible scene writer model in Image Generation settings to draft this automatically."}
          </p>
        </div>

        <div className="space-y-2">
          <div className="flex items-center justify-between gap-3 border-b border-fg/10 pb-2">
            <div>
              <div className="text-sm font-medium text-fg/75">Reference images</div>
              <div className="text-xs text-fg/40">{helperText}</div>
            </div>
            <div className="flex items-center gap-3">
              {!showHeader ? (
                <button
                  type="button"
                  onClick={() => void handleAddImages()}
                  className="inline-flex shrink-0 items-center gap-2 whitespace-nowrap rounded-lg border border-fg/15 bg-fg/[0.04] px-3 py-2 text-sm font-medium text-fg/80 transition hover:border-fg/25 hover:bg-fg/[0.07]"
                >
                  <ImagePlus size={14} />
                  Add references
                </button>
              ) : null}
            </div>
          </div>

          {referenceImages.length > 0 ? (
            <div className="grid grid-cols-2 gap-3 lg:grid-cols-3">
              {referenceImages.map((value, index) => (
                <ReferenceThumb
                  key={`${value}-${index}`}
                  value={value}
                  index={index}
                  onRemove={handleRemoveImage}
                />
              ))}
            </div>
          ) : (
            <div className="flex h-20 items-center justify-center rounded-lg border border-dashed border-fg/10 bg-fg/[0.02] px-4 text-sm text-fg/35">
              Add a few clear reference shots to lock face, proportions, outfit, and style.
            </div>
          )}
        </div>
      </section>

      <BottomMenu isOpen={showDraftMenu} onClose={handleCloseDraftMenu} title="AI Design Draft">
        <div className="space-y-4">
          <p className="text-sm text-white/60">
            {writerAvailable
              ? `Drafted by ${writerModelLabel ?? "your scene writer model"} from the current avatar and reference images.`
              : "Add a compatible scene writer model before using this helper."}
          </p>

          {draftError ? (
            <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4">
              <p className="text-sm text-red-300">{draftError}</p>
            </div>
          ) : isGeneratingDescription && !draftText ? (
            <div className="flex items-center justify-center py-8">
              <Loader2 className="h-6 w-6 animate-spin text-white/50" />
            </div>
          ) : draftText ? (
            <div className="max-h-[40vh] overflow-y-auto rounded-lg border border-white/10 bg-white/5 p-4">
              <p className="whitespace-pre-wrap text-sm text-white/90">{draftText}</p>
            </div>
          ) : null}

          <div className="flex gap-3">
            <button
              type="button"
              onClick={() => void handleGenerateDescription()}
              disabled={isGeneratingDescription}
              className="flex flex-1 items-center justify-center gap-2 rounded-lg bg-white/10 px-4 py-3 text-white/80 transition hover:bg-white/15 disabled:opacity-50"
            >
              <Sparkles size={18} />
              <span>Regenerate</span>
            </button>
            <button
              type="button"
              onClick={handleUseDraft}
              disabled={isGeneratingDescription || !draftText?.trim()}
              className="flex flex-1 items-center justify-center gap-2 rounded-lg bg-emerald-600 px-4 py-3 text-white transition hover:bg-emerald-500 disabled:opacity-50"
            >
              <span>Use This</span>
            </button>
          </div>
        </div>
      </BottomMenu>
    </>
  );
}
