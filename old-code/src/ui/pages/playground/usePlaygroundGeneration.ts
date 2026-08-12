import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import {
  cancelLocalImageGeneration,
  generateImage,
  isGenerationCancelledError,
  SDCPP_GENERATION_PROGRESS_EVENT,
  type SdcppGenerationProgress,
} from "../../../core/image-generation";
import {
  isLocalSdProvider,
  randomPlaygroundSeed,
  savePlaygroundHistoryEntry,
  type PlaygroundGenerationEntry,
  type PlaygroundGenerationParams,
} from "../../../core/image-generation/playground";
import type { PlaygroundRequestBase } from "./usePlaygroundSettings";

export type PlaygroundGenerationInput = {
  base: PlaygroundRequestBase;
  modelDbId: string;
  modelDisplayName: string;
  prompt: string;
  negativePrompt: string | null;
  loras: { path: string; multiplier: number; isHighNoise?: boolean; keywords?: string[] }[];
  initImage?: { dataUrl: string; assetId: string | null; denoisingStrength: number | null } | null;
};

export type PlaygroundGenerationController = {
  generating: boolean;
  progress: SdcppGenerationProgress | null;
  activeEntry: PlaygroundGenerationEntry | null;
  generate: (input: PlaygroundGenerationInput) => Promise<void>;
  cancel: () => Promise<void>;
  onEntryFinalized: (handler: (entry: PlaygroundGenerationEntry) => void) => void;
  pushEntry: (entry: PlaygroundGenerationEntry) => void;
};

export function usePlaygroundGeneration(): PlaygroundGenerationController {
  const [generating, setGenerating] = useState(false);
  const [progress, setProgress] = useState<SdcppGenerationProgress | null>(null);
  const [activeEntry, setActiveEntry] = useState<PlaygroundGenerationEntry | null>(null);
  const finalizedHandlerRef = useRef<((entry: PlaygroundGenerationEntry) => void) | null>(null);
  const activeIsLocalRef = useRef(false);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    void listen<SdcppGenerationProgress>(SDCPP_GENERATION_PROGRESS_EVENT, (event) => {
      if (activeIsLocalRef.current) setProgress(event.payload);
    }).then((stop) => {
      if (disposed) {
        stop();
      } else {
        unlisten = stop;
      }
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const onEntryFinalized = useCallback((handler: (entry: PlaygroundGenerationEntry) => void) => {
    finalizedHandlerRef.current = handler;
  }, []);

  const generate = useCallback(
    async (input: PlaygroundGenerationInput) => {
      if (generating) return;
      const { base } = input;
      const isLocal = isLocalSdProvider(base.providerId);
      const advanced = { ...base.advancedModelSettings };
      if (isLocal && advanced.sdSeed == null) {
        advanced.sdSeed = randomPlaygroundSeed();
      }
      if (input.negativePrompt?.trim()) {
        advanced.sdNegativePrompt = input.negativePrompt.trim();
      }
      if (input.initImage?.denoisingStrength != null) {
        advanced.sdDenoisingStrength = input.initImage.denoisingStrength;
      }

      const params: PlaygroundGenerationParams = {
        advancedModelSettings: advanced,
        loras: input.loras,
        size: base.size ?? null,
        n: base.n ?? null,
        quality: base.quality ?? null,
        style: base.style ?? null,
        initImageAssetId: input.initImage?.assetId ?? null,
        denoisingStrength: input.initImage?.denoisingStrength ?? null,
      };
      const entry: PlaygroundGenerationEntry = {
        id: crypto.randomUUID(),
        createdAt: Date.now(),
        providerId: base.providerId,
        modelId: input.modelDbId,
        modelName: input.modelDisplayName,
        prompt: input.prompt,
        negativePrompt: input.negativePrompt,
        seed: isLocal ? (advanced.sdSeed ?? null) : null,
        params,
        status: "pending",
        error: null,
        images: [],
      };

      setGenerating(true);
      setProgress(null);
      activeIsLocalRef.current = isLocal;
      setActiveEntry(entry);
      try {
        await savePlaygroundHistoryEntry(entry);
      } catch {
        void 0;
      }

      let finalized: PlaygroundGenerationEntry;
      try {
        const response = await generateImage({
          prompt: input.prompt,
          model: base.model,
          providerId: base.providerId,
          credentialId: base.credentialId,
          advancedModelSettings: advanced,
          loras: input.loras.length > 0 ? input.loras : undefined,
          inputImages: input.initImage ? [input.initImage.dataUrl] : undefined,
          size: base.size,
          n: base.n,
          quality: base.quality,
          style: base.style,
          usageSource: "playground",
        });
        finalized = {
          ...entry,
          status: "complete",
          images: response.images.map((image) => ({
            assetId: image.assetId,
            filePath: image.filePath,
            mimeType: image.mimeType,
            url: image.url ?? null,
            width: image.width ?? null,
            height: image.height ?? null,
          })),
        };
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        finalized = {
          ...entry,
          status: isGenerationCancelledError(error) ? "cancelled" : "failed",
          error: isGenerationCancelledError(error) ? null : message,
        };
      }

      try {
        await savePlaygroundHistoryEntry(finalized);
      } catch {
        void 0;
      }
      setGenerating(false);
      setProgress(null);
      setActiveEntry(null);
      activeIsLocalRef.current = false;
      finalizedHandlerRef.current?.(finalized);
    },
    [generating],
  );

  const cancel = useCallback(async () => {
    try {
      await cancelLocalImageGeneration();
    } catch {
      void 0;
    }
  }, []);

  const pushEntry = useCallback((entry: PlaygroundGenerationEntry) => {
    finalizedHandlerRef.current?.(entry);
  }, []);

  return { generating, progress, activeEntry, generate, cancel, onEntryFinalized, pushEntry };
}
