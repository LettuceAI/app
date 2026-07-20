import { useEffect, useState } from "react";
import { ArrowLeft, SlidersHorizontal, TerminalSquare } from "lucide-react";

import { BottomMenu } from "../../components/BottomMenu";
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

export function PlaygroundPage() {
  const { t } = useI18n();
  const { backOrReplace } = useNavigationManager();
  const settings = usePlaygroundSettings();
  const generation = usePlaygroundGeneration();
  const [prompt, setPrompt] = useState("");
  const [negativePrompt, setNegativePrompt] = useState("");
  const [initImage, setInitImage] = useState<PlaygroundInitImage | null>(null);
  const [promptSheetOpen, setPromptSheetOpen] = useState(false);
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
          onClick={() => backOrReplace(Routes.settingsImageGeneration)}
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
        <aside className="hidden w-[300px] shrink-0 flex-col border-r border-fg/8 bg-surface lg:flex">
          {promptPane}
        </aside>
        <section className="flex min-w-0 flex-1 flex-col bg-bg/40">
          <PlaygroundFeed
            generation={generation}
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
        <aside className="hidden w-[340px] shrink-0 flex-col border-l border-fg/8 bg-surface lg:flex">
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
    </div>
  );
}

export default PlaygroundPage;
