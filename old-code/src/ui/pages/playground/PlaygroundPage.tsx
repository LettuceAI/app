import { useEffect, useRef, useState } from "react";
import { ArrowLeft, SlidersHorizontal, TerminalSquare } from "lucide-react";

import { BottomMenu } from "../../components/BottomMenu";
import { GuidedTour, useGuidedTour } from "../../components/GuidedTour";
import { useI18n } from "../../../core/i18n/context";
import { Routes, useNavigationManager } from "../../navigation";
import { convertFilePathToDataUrl } from "../../../core/storage/images";
import { toast } from "../../components/toast";
import {
  getSdcppUpscalerInventory,
  resolveProviderCredential,
  upscaleLocalImage,
} from "../../../core/image-generation";
import { getPlatform } from "../../../core/utils/platform";
import {
  savePlaygroundHistoryEntry,
  type PlaygroundGenerationEntry,
  type PlaygroundGenerationImage,
} from "../../../core/image-generation/playground";
import { PlaygroundFeed } from "./PlaygroundFeed";
import { PlaygroundPromptPane, type PlaygroundInitImage } from "./PlaygroundPromptPane";
import { PlaygroundSettingsPane } from "./PlaygroundSettingsPane";
import { usePlaygroundGeneration } from "./usePlaygroundGeneration";
import { usePlaygroundSettings } from "./usePlaygroundSettings";

const NEGATIVE_PROMPT_PROVIDERS = new Set(["sdcpp", "comfyui", "automatic1111", "diffusers"]);

const LEFT_PANE_DEFAULT = 300;
const RIGHT_PANE_DEFAULT = 340;
const LEFT_PANE_RANGE = { min: 230, max: 480 };
const RIGHT_PANE_RANGE = { min: 270, max: 540 };

function clampWidth(value: number, range: { min: number; max: number }): number {
  return Math.min(range.max, Math.max(range.min, value));
}

function usePaneWidth(
  storageKey: string,
  defaultWidth: number,
  range: { min: number; max: number },
) {
  const [width, setWidth] = useState(() => {
    try {
      const stored = Number(localStorage.getItem(storageKey));
      return Number.isFinite(stored) && stored > 0 ? clampWidth(stored, range) : defaultWidth;
    } catch {
      return defaultWidth;
    }
  });
  const resize = (delta: number) => {
    setWidth((current) => {
      const next = clampWidth(current + delta, range);
      try {
        localStorage.setItem(storageKey, String(next));
      } catch {
        return next;
      }
      return next;
    });
  };
  const reset = () => {
    setWidth(defaultWidth);
    try {
      localStorage.removeItem(storageKey);
    } catch {
      return;
    }
  };
  return { width, resize, reset };
}

function PaneResizeHandle({
  onDelta,
  onReset,
}: {
  onDelta: (delta: number) => void;
  onReset: () => void;
}) {
  const lastX = useRef(0);
  return (
    <div
      role="separator"
      aria-orientation="vertical"
      onDoubleClick={onReset}
      onPointerDown={(event) => {
        event.preventDefault();
        lastX.current = event.clientX;
        event.currentTarget.setPointerCapture(event.pointerId);
      }}
      onPointerMove={(event) => {
        if (!event.currentTarget.hasPointerCapture(event.pointerId)) return;
        const delta = event.clientX - lastX.current;
        if (delta !== 0) {
          lastX.current = event.clientX;
          onDelta(delta);
        }
      }}
      onPointerUp={(event) => {
        if (event.currentTarget.hasPointerCapture(event.pointerId)) {
          event.currentTarget.releasePointerCapture(event.pointerId);
        }
      }}
      className="group hidden w-1.5 shrink-0 cursor-col-resize touch-none items-stretch justify-center lg:flex"
    >
      <div className="w-px bg-fg/8 transition-colors group-hover:bg-accent/50 group-active:bg-accent/70" />
    </div>
  );
}

export function PlaygroundPage() {
  const { t } = useI18n();
  const { go } = useNavigationManager();
  const settings = usePlaygroundSettings();
  const generation = usePlaygroundGeneration();
  const [prompt, setPrompt] = useState("");
  const [negativePrompt, setNegativePrompt] = useState("");
  const [initImage, setInitImage] = useState<PlaygroundInitImage | null>(null);
  const [promptSheetOpen, setPromptSheetOpen] = useState(false);
  const { shouldShow: showPlaygroundTour, dismiss: dismissPlaygroundTour } =
    useGuidedTour("playground");
  const [feedStepActive, setFeedStepActive] = useState(false);

  useEffect(() => {
    if (!showPlaygroundTour) {
      setFeedStepActive(false);
      return;
    }
    const handler = (event: Event) => {
      const { tour, stepId } = (event as CustomEvent).detail ?? {};
      if (tour !== "playground") return;
      setFeedStepActive(stepId === "playground-feed");
    };
    window.addEventListener("tour:step", handler);
    return () => window.removeEventListener("tour:step", handler);
  }, [showPlaygroundTour]);
  const [settingsSheetOpen, setSettingsSheetOpen] = useState(false);

  const showNegativePrompt = NEGATIVE_PROMPT_PROVIDERS.has(
    settings.selectedModel?.providerId ?? "",
  );
  const showInitImage =
    NEGATIVE_PROMPT_PROVIDERS.has(settings.selectedModel?.providerId ?? "") ||
    settings.selectedModel?.inputScopes?.includes("image") === true;
  const canGenerate = prompt.trim().length > 0 && settings.selectedModel !== null;

  const [upscalerReady, setUpscalerReady] = useState(false);
  const [upscaling, setUpscaling] = useState(false);
  const leftPane = usePaneWidth("playground:pane:left", LEFT_PANE_DEFAULT, LEFT_PANE_RANGE);
  const rightPane = usePaneWidth("playground:pane:right", RIGHT_PANE_DEFAULT, RIGHT_PANE_RANGE);

  useEffect(() => {
    if (getPlatform().type === "mobile") return;
    let cancelled = false;
    getSdcppUpscalerInventory()
      .then((inventory) => {
        if (!cancelled) setUpscalerReady(inventory.models.length > 0);
      })
      .catch(() => {
        if (!cancelled) setUpscalerReady(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const upscaleImage = async (entry: PlaygroundGenerationEntry, image: PlaygroundGenerationImage) => {
    if (upscaling || generation.generating) return;
    setUpscaling(true);
    try {
      const dataUrl = await convertFilePathToDataUrl(image.filePath);
      if (!dataUrl) throw new Error(t("playground.prompt.initImageFailed"));
      const result = await upscaleLocalImage(dataUrl);
      const upscaledEntry: PlaygroundGenerationEntry = {
        id: crypto.randomUUID(),
        createdAt: Date.now(),
        providerId: "sdcpp",
        modelId: entry.modelId,
        modelName: entry.modelName,
        prompt: entry.prompt,
        negativePrompt: entry.negativePrompt,
        seed: entry.seed,
        params: { upscaleOf: entry.id },
        status: "complete",
        error: null,
        images: [
          {
            assetId: result.assetId,
            filePath: result.filePath,
            mimeType: result.mimeType,
            url: result.url ?? null,
            width: result.width ?? null,
            height: result.height ?? null,
          },
        ],
      };
      await savePlaygroundHistoryEntry(upscaledEntry);
      generation.pushEntry(upscaledEntry);
    } catch (error) {
      toast.error(
        t("playground.feed.upscaleFailed"),
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setUpscaling(false);
    }
  };

  const reuseSeed = (entry: PlaygroundGenerationEntry) => {
    if (entry.seed == null) return;
    settings.updateDraft({ seed: entry.seed });
    toast.success(t("playground.feed.seedReused", { seed: entry.seed }));
  };

  const regenerateEntry = (entry: PlaygroundGenerationEntry) => {
    if (generation.generating) return;
    const model = settings.models.find((candidate) => candidate.id === entry.modelId);
    if (!model) {
      toast.error(t("playground.feed.modelMissing"));
      return;
    }
    const credential = resolveProviderCredential(
      settings.providers,
      model.providerId,
      model.providerLabel,
    );
    if (!credential) {
      toast.error(t("playground.feed.modelMissing"));
      return;
    }
    const advanced = { ...(entry.params.advancedModelSettings ?? {}) };
    delete advanced.sdSeed;
    void generation.generate({
      base: {
        model: model.name,
        providerId: model.providerId,
        credentialId: credential.id,
        advancedModelSettings: advanced,
        size: entry.params.size ?? undefined,
        n: entry.params.n ?? undefined,
        quality: entry.params.quality ?? undefined,
        style: entry.params.style ?? undefined,
      },
      modelDbId: model.id,
      modelDisplayName: model.displayName || model.name,
      prompt: entry.prompt,
      negativePrompt: entry.negativePrompt,
      loras: entry.params.loras ?? [],
      initImage: null,
    });
  };

  const sendToImg2img = async (image: PlaygroundGenerationImage) => {
    const dataUrl = await convertFilePathToDataUrl(image.filePath);
    if (!dataUrl) {
      toast.error(t("playground.prompt.initImageFailed"));
      return;
    }
    setInitImage({
      dataUrl,
      assetId: image.assetId || null,
      denoisingStrength: initImage?.denoisingStrength ?? 0.6,
    });
    toast.success(t("playground.prompt.initImageSet"));
  };

  const handleGenerate = () => {
    const base = settings.buildRequestBase();
    if (!base || !settings.selectedModel || generation.generating) return;
    setPromptSheetOpen(false);
    void generation.generate({
      base,
      modelDbId: settings.selectedModel.id,
      modelDisplayName: settings.selectedModel.displayName || settings.selectedModel.name,
      prompt: prompt.trim(),
      negativePrompt: showNegativePrompt ? negativePrompt.trim() || null : null,
      loras: settings.isLocal ? (settings.draft.loras ?? []) : [],
      initImage: showInitImage && initImage
        ? {
          dataUrl: initImage.dataUrl,
          assetId: initImage.assetId,
          denoisingStrength: initImage.denoisingStrength,
        }
        : null,
    });
  };

  if (getPlatform().type === "mobile") {
    return (
      <main className="flex h-full items-center justify-center px-6 text-center">
        <div>
          <h1 className="text-lg font-semibold text-fg">
            {t("imageGeneration.local.hub.desktopOnlyTitle")}
          </h1>
          <p className="mt-2 text-sm text-fg/50">
            {t("imageGeneration.local.hub.desktopOnlyBody")}
          </p>
        </div>
      </main>
    );
  }

  const promptPane = (
    <PlaygroundPromptPane
      prompt={prompt}
      onPromptChange={setPrompt}
      negativePrompt={negativePrompt}
      onNegativePromptChange={setNegativePrompt}
      showNegativePrompt={showNegativePrompt}
      showInitImage={showInitImage}
      initImage={initImage}
      onInitImageChange={setInitImage}
      canGenerate={canGenerate}
      generating={generation.generating}
      onGenerate={handleGenerate}
    />
  );

  const settingsPane = <PlaygroundSettingsPane controller={settings} />;

  return (
    <div className="flex h-full flex-col bg-surface">
      <header className="flex h-12 shrink-0 items-center gap-2 border-b border-fg/8 bg-surface/95 px-4 backdrop-blur-md">
        <button
          type="button"
          onClick={() => go(Routes.settingsImageGeneration, { replace: true })}
          aria-label={t("playground.back")}
          className="flex h-8 w-8 items-center justify-center rounded-full text-fg/50 transition-all hover:bg-fg/10 hover:text-fg active:scale-95"
        >
          <ArrowLeft size={16} />
        </button>
        <h1 className="text-[14px] font-semibold tracking-tight text-fg">
          {t("playground.title")}
        </h1>
        <div className="ml-auto flex items-center gap-1 lg:hidden">
          <button
            type="button"
            onClick={() => setPromptSheetOpen(true)}
            aria-label={t("playground.promptTab")}
            className="flex h-8 w-8 items-center justify-center rounded-lg text-fg/50 transition hover:bg-fg/8 hover:text-fg"
          >
            <TerminalSquare size={15} />
          </button>
          <button
            type="button"
            onClick={() => setSettingsSheetOpen(true)}
            aria-label={t("playground.settingsTab")}
            className="flex h-8 w-8 items-center justify-center rounded-lg text-fg/50 transition hover:bg-fg/8 hover:text-fg"
          >
            <SlidersHorizontal size={15} />
          </button>
        </div>
      </header>
      <div className="flex min-h-0 flex-1">
        <aside
          style={{ width: leftPane.width }}
          className="hidden shrink-0 flex-col bg-surface lg:flex"
        >
          {promptPane}
        </aside>
        <PaneResizeHandle onDelta={leftPane.resize} onReset={leftPane.reset} />
        <section data-tour-id="playground-feed" className="flex min-w-0 flex-1 flex-col">
          <PlaygroundFeed
            generation={generation}
            showDemo={showPlaygroundTour && feedStepActive}
            onSendToImg2img={showInitImage ? (image) => void sendToImg2img(image) : undefined}
            onUpscale={
              upscalerReady
                ? (entry, image) => void upscaleImage(entry, image)
                : undefined
            }
            onReuseSeed={reuseSeed}
            onRegenerate={regenerateEntry}
            busy={upscaling}
          />
        </section>
        <PaneResizeHandle onDelta={(delta) => rightPane.resize(-delta)} onReset={rightPane.reset} />
        <aside
          style={{ width: rightPane.width }}
          className="hidden shrink-0 flex-col bg-surface lg:flex"
        >
          {settingsPane}
        </aside>
      </div>

      <BottomMenu
        isOpen={promptSheetOpen}
        onClose={() => setPromptSheetOpen(false)}
        title={t("playground.promptTab")}
      >
        <div className="max-h-[70vh]">{promptPane}</div>
      </BottomMenu>
      <BottomMenu
        isOpen={settingsSheetOpen}
        onClose={() => setSettingsSheetOpen(false)}
        title={t("playground.settingsTab")}
      >
        <div className="max-h-[70vh]">{settingsPane}</div>
      </BottomMenu>

      {showPlaygroundTour && (
        <GuidedTour tour="playground" onDismiss={dismissPlaygroundTour} />
      )}
    </div>
  );
}

export default PlaygroundPage;
