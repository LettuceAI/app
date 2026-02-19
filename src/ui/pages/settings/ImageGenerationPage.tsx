import { useState, useCallback, useEffect } from "react";
import { Image, Loader2, Download, Sparkles, AlertCircle, ChevronDown } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { convertFileSrc } from "@tauri-apps/api/core";

import {
  generateImage,
  getModelSizes,
  type ImageGenerationRequest,
  type GeneratedImage,
} from "../../../core/image-generation";
import { readSettings } from "../../../core/storage/repo";
import type { Model, ProviderCredential } from "../../../core/storage/schemas";

interface ImageGenerationState {
  loading: boolean;
  generating: boolean;
  error: string | null;
  models: Model[];
  providers: ProviderCredential[];
  selectedModel: Model | null;
  selectedProvider: ProviderCredential | null;
  generatedImages: GeneratedImage[];
}

export function ImageGenerationPage() {
  const [state, setState] = useState<ImageGenerationState>({
    loading: true,
    generating: false,
    error: null,
    models: [],
    providers: [],
    selectedModel: null,
    selectedProvider: null,
    generatedImages: [],
  });

  const [prompt, setPrompt] = useState("");
  const [size, setSize] = useState("1024x1024");
  const [quality, setQuality] = useState("standard");
  const [style, setStyle] = useState("vivid");
  const [showAdvanced, setShowAdvanced] = useState(false);

  // Load image generation models
  useEffect(() => {
    (async () => {
      try {
        const settings = await readSettings();
        const imageModels = settings.models.filter((m) => m.outputScopes?.includes("image"));
        const providers = settings.providerCredentials;

        const selectedModel = imageModels[0] ?? null;
        const selectedProvider = selectedModel
          ? providers.find(
              (p) =>
                p.providerId === selectedModel.providerId &&
                p.label === selectedModel.providerLabel
            ) ?? providers.find((p) => p.providerId === selectedModel.providerId) ?? null
          : null;

        setState((prev) => ({
          ...prev,
          loading: false,
          models: imageModels,
          providers,
          selectedModel,
          selectedProvider,
        }));
      } catch (err) {
        console.error("Failed to load image generation settings:", err);
        setState((prev) => ({
          ...prev,
          loading: false,
          error: err instanceof Error ? err.message : "Failed to load settings",
        }));
      }
    })();
  }, []);

  const handleModelChange = useCallback(
    (modelId: string) => {
      const model = state.models.find((m) => m.id === modelId) ?? null;
      const provider = model
        ? state.providers.find(
            (p) =>
              p.providerId === model.providerId && p.label === model.providerLabel
          ) ?? state.providers.find((p) => p.providerId === model.providerId) ?? null
        : null;

      setState((prev) => ({
        ...prev,
        selectedModel: model,
        selectedProvider: provider,
      }));

      // Update size options for the new model
      if (model) {
        const sizes = getModelSizes(model.providerId, model.name);
        if (sizes.length > 0 && !sizes.includes(size)) {
          setSize(sizes[0]);
        }
      }
    },
    [state.models, state.providers, size]
  );

  const handleGenerate = useCallback(async () => {
    if (!state.selectedModel || !state.selectedProvider || !prompt.trim()) {
      return;
    }

    setState((prev) => ({ ...prev, generating: true, error: null }));

    try {
      const request: ImageGenerationRequest = {
        prompt: prompt.trim(),
        model: state.selectedModel.name,
        providerId: state.selectedModel.providerId,
        credentialId: state.selectedProvider.id,
        size,
        quality: state.selectedModel.providerId === "openai" ? quality : undefined,
        style: state.selectedModel.providerId === "openai" ? style : undefined,
        n: 1,
      };

      const response = await generateImage(request);

      setState((prev) => ({
        ...prev,
        generating: false,
        generatedImages: [...response.images, ...prev.generatedImages],
      }));

      setPrompt("");
    } catch (err) {
      console.error("Image generation failed:", err);
      setState((prev) => ({
        ...prev,
        generating: false,
        error: err instanceof Error ? err.message : "Image generation failed",
      }));
    }
  }, [state.selectedModel, state.selectedProvider, prompt, size, quality, style]);

  const availableSizes = state.selectedModel
    ? getModelSizes(state.selectedModel.providerId, state.selectedModel.name)
    : ["1024x1024"];

  const isOpenAI = state.selectedModel?.providerId === "openai";

  if (state.loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-fg/10 border-t-fg/60" />
      </div>
    );
  }

  if (state.models.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center px-6">
        <div className="flex h-16 w-16 items-center justify-center rounded-2xl border border-fg/10 bg-fg/5 mb-4">
          <Image className="h-8 w-8 text-fg/40" />
        </div>
        <h2 className="text-lg font-semibold text-fg mb-2">No Image Models</h2>
        <p className="text-center text-sm text-fg/50 mb-4">
          Add an image generation model from the Models page to start generating images.
        </p>
        <p className="text-center text-xs text-fg/40">
          Supported providers: OpenAI (DALL-E), Google (Imagen), OpenRouter
        </p>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      {/* Main Content */}
      <div className="flex-1 overflow-y-auto px-4 py-4 space-y-4">
        {/* Error Banner */}
        <AnimatePresence>
          {state.error && (
            <motion.div
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: "auto" }}
              exit={{ opacity: 0, height: 0 }}
              className="rounded-xl border border-danger/30 bg-danger/10 px-4 py-3 flex items-start gap-3"
            >
              <AlertCircle className="h-5 w-5 text-danger/80 shrink-0 mt-0.5" />
              <div className="flex-1">
                <p className="text-sm font-medium text-danger/80">Generation Failed</p>
                <p className="text-xs text-danger/70 mt-0.5">{state.error}</p>
              </div>
              <button
                onClick={() => setState((prev) => ({ ...prev, error: null }))}
                className="text-danger/80/60 hover:text-danger/80 text-xs"
              >
                Dismiss
              </button>
            </motion.div>
          )}
        </AnimatePresence>

        {/* Model Selection */}
        <div className="space-y-2">
          <label className="text-[11px] font-medium text-fg/70">MODEL</label>
          <select
            value={state.selectedModel?.id ?? ""}
            onChange={(e) => handleModelChange(e.target.value)}
            className="w-full rounded-xl border border-fg/10 bg-surface-el/20 px-3 py-2.5 text-fg transition focus:border-fg/30 focus:outline-none"
          >
            {state.models.map((model) => (
              <option key={model.id} value={model.id} className="bg-surface-el">
                {model.displayName}
              </option>
            ))}
          </select>
        </div>

        {/* Prompt Input */}
        <div className="space-y-2">
          <label className="text-[11px] font-medium text-fg/70">PROMPT</label>
          <textarea
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            placeholder="Describe the image you want to generate..."
            rows={4}
            className="w-full rounded-xl border border-fg/10 bg-surface-el/20 px-3 py-2.5 text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none resize-none"
          />
        </div>

        {/* Advanced Settings Toggle */}
        <button
          onClick={() => setShowAdvanced(!showAdvanced)}
          className="flex w-full items-center justify-between rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-left transition active:bg-fg/10"
        >
          <span className="text-sm font-medium text-fg">Advanced Settings</span>
          <ChevronDown
            className={`h-4 w-4 text-fg/50 transition-transform ${
              showAdvanced ? "rotate-180" : ""
            }`}
          />
        </button>

        {/* Advanced Settings */}
        <AnimatePresence>
          {showAdvanced && (
            <motion.div
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: "auto" }}
              exit={{ opacity: 0, height: 0 }}
              className="space-y-4 overflow-hidden"
            >
              {/* Size */}
              <div className="space-y-2">
                <label className="text-[11px] font-medium text-fg/70">SIZE</label>
                <div className="flex flex-wrap gap-2">
                  {availableSizes.map((s) => (
                    <button
                      key={s}
                      onClick={() => setSize(s)}
                      className={`rounded-lg px-3 py-1.5 text-sm font-medium transition ${
                        size === s
                          ? "border border-accent/40 bg-accent/20 text-accent/80"
                          : "border border-fg/10 bg-fg/5 text-fg/60 active:bg-fg/10"
                      }`}
                    >
                      {s}
                    </button>
                  ))}
                </div>
              </div>

              {/* OpenAI-specific options */}
              {isOpenAI && (
                <>
                  {/* Quality */}
                  <div className="space-y-2">
                    <label className="text-[11px] font-medium text-fg/70">QUALITY</label>
                    <div className="flex gap-2">
                      {["standard", "hd"].map((q) => (
                        <button
                          key={q}
                          onClick={() => setQuality(q)}
                          className={`flex-1 rounded-lg px-3 py-2 text-sm font-medium transition ${
                            quality === q
                              ? "border border-info/40 bg-info/20 text-info"
                              : "border border-fg/10 bg-fg/5 text-fg/60 active:bg-fg/10"
                          }`}
                        >
                          {q.toUpperCase()}
                        </button>
                      ))}
                    </div>
                  </div>

                  {/* Style */}
                  <div className="space-y-2">
                    <label className="text-[11px] font-medium text-fg/70">STYLE</label>
                    <div className="flex gap-2">
                      {["vivid", "natural"].map((s) => (
                        <button
                          key={s}
                          onClick={() => setStyle(s)}
                          className={`flex-1 rounded-lg px-3 py-2 text-sm font-medium transition ${
                            style === s
                              ? "border border-secondary/40 bg-secondary/20 text-secondary"
                              : "border border-fg/10 bg-fg/5 text-fg/60 active:bg-fg/10"
                          }`}
                        >
                          {s.charAt(0).toUpperCase() + s.slice(1)}
                        </button>
                      ))}
                    </div>
                  </div>
                </>
              )}
            </motion.div>
          )}
        </AnimatePresence>

        {/* Generate Button */}
        <button
          onClick={handleGenerate}
          disabled={state.generating || !prompt.trim() || !state.selectedModel}
          className="w-full rounded-xl border border-accent/40 bg-accent/20 px-4 py-3 text-sm font-semibold text-accent/90 transition hover:bg-accent/30 disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
        >
          {state.generating ? (
            <>
              <Loader2 className="h-4 w-4 animate-spin" />
              Generating...
            </>
          ) : (
            <>
              <Sparkles className="h-4 w-4" />
              Generate Image
            </>
          )}
        </button>

        {/* Generated Images */}
        {state.generatedImages.length > 0 && (
          <div className="space-y-3">
            <h3 className="text-sm font-medium text-fg/70">Generated Images</h3>
            <div className="grid grid-cols-2 gap-3">
              {state.generatedImages.map((img, idx) => (
                <div
                  key={`${img.filePath}-${idx}`}
                  className="relative group rounded-xl overflow-hidden border border-fg/10 bg-fg/5"
                >
                  <img
                    src={convertFileSrc(img.filePath)}
                    alt={`Generated image ${idx + 1}`}
                    className="w-full aspect-square object-cover"
                    loading="lazy"
                  />
                  {img.text && (
                    <div className="absolute bottom-0 left-0 right-0 bg-surface-el/70 px-2 py-1 text-xs text-fg/70 truncate">
                      {img.text}
                    </div>
                  )}
                  <div className="absolute inset-0 bg-surface-el/40 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
                    <button
                      onClick={() => {
                        // Open image in default viewer
                        window.open(convertFileSrc(img.filePath), "_blank");
                      }}
                      className="rounded-full bg-fg/20 p-2 hover:bg-fg/30 transition"
                    >
                      <Download className="h-4 w-4 text-fg" />
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
