import { useState } from "react";
import { History, ImageUp, Loader, Sparkles, X } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";

import { cn } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import { convertFilePathToDataUrl } from "../../../core/storage/images";
import { toast } from "../../components/toast";
import { ADVANCED_SD_DENOISING_STRENGTH_RANGE } from "../../components/AdvancedModelSettingsForm";
import type { PlaygroundGenerationImage } from "../../../core/image-generation/playground";
import { PlaygroundInitImagePicker } from "./PlaygroundInitImagePicker";

export type PlaygroundInitImage = {
  dataUrl: string;
  assetId: string | null;
  denoisingStrength: number;
};

export function PlaygroundPromptPane({
  prompt,
  onPromptChange,
  negativePrompt,
  onNegativePromptChange,
  showNegativePrompt,
  showInitImage,
  initImage,
  onInitImageChange,
  canGenerate,
  generating,
  onGenerate,
}: {
  prompt: string;
  onPromptChange: (value: string) => void;
  negativePrompt: string;
  onNegativePromptChange: (value: string) => void;
  showNegativePrompt: boolean;
  showInitImage: boolean;
  initImage: PlaygroundInitImage | null;
  onInitImageChange: (value: PlaygroundInitImage | null) => void;
  canGenerate: boolean;
  generating: boolean;
  onGenerate: () => void;
}) {
  const { t } = useI18n();
  const [historyPickerOpen, setHistoryPickerOpen] = useState(false);

  const pickFromHistory = async (image: PlaygroundGenerationImage) => {
    const dataUrl = await convertFilePathToDataUrl(image.filePath);
    if (!dataUrl) {
      toast.error(t("playground.prompt.initImageFailed"));
      return;
    }
    onInitImageChange({
      dataUrl,
      assetId: image.assetId || null,
      denoisingStrength: initImage?.denoisingStrength ?? 0.6,
    });
  };

  const pickInitImage = async () => {
    const selection = await open({
      multiple: false,
      filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "webp"] }],
    });
    if (typeof selection !== "string") return;
    const dataUrl = await convertFilePathToDataUrl(selection);
    if (!dataUrl) {
      toast.error(t("playground.prompt.initImageFailed"));
      return;
    }
    onInitImageChange({
      dataUrl,
      assetId: null,
      denoisingStrength: initImage?.denoisingStrength ?? 0.6,
    });
  };

  const handleKeyDown = (event: React.KeyboardEvent) => {
    if ((event.ctrlKey || event.metaKey) && event.key === "Enter" && canGenerate && !generating) {
      event.preventDefault();
      onGenerate();
    }
  };

  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto p-4">
      <div className="flex min-h-0 flex-1 flex-col gap-4">
        <div data-tour-id="playground-prompt" className="flex min-h-0 flex-1 flex-col">
          <p className="mb-1.5 text-[11px] font-medium uppercase tracking-wide text-fg/40">
            {t("playground.prompt.label")}
          </p>
          <textarea
            value={prompt}
            onChange={(event) => onPromptChange(event.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t("playground.prompt.placeholder")}
            className="min-h-[140px] w-full flex-1 resize-none rounded-xl border border-fg/10 bg-fg/5 px-3.5 py-3 text-[13px] leading-relaxed text-fg placeholder-fg/40 transition-all focus:border-fg/20 focus:bg-fg/[0.07] focus:outline-none"
          />
        </div>
        {showNegativePrompt && (
          <div className="flex flex-col">
            <p className="mb-1.5 text-[11px] font-medium uppercase tracking-wide text-fg/40">
              {t("playground.prompt.negativeLabel")}
            </p>
            <textarea
              value={negativePrompt}
              onChange={(event) => onNegativePromptChange(event.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={t("playground.prompt.negativePlaceholder")}
              rows={4}
              className="w-full resize-none rounded-xl border border-fg/10 bg-fg/5 px-3.5 py-3 text-[13px] leading-relaxed text-fg placeholder-fg/40 transition-all focus:border-fg/20 focus:bg-fg/[0.07] focus:outline-none"
            />
          </div>
        )}
        {showInitImage && (
          <div className="flex flex-col">
            <p className="mb-1.5 text-[11px] font-medium uppercase tracking-wide text-fg/40">
              {t("playground.prompt.initImageLabel")}
            </p>
            {initImage ? (
              <div className="rounded-xl border border-fg/10 bg-fg/4 p-2.5">
                <div className="flex items-start gap-2.5">
                  <img
                    src={initImage.dataUrl}
                    alt=""
                    className="h-16 w-16 shrink-0 rounded-lg border border-fg/8 object-cover"
                  />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center justify-between gap-2">
                      <span className="text-[11px] text-fg/50">
                        {t("playground.prompt.strength")}
                      </span>
                      <span className="font-mono text-[11px] tabular-nums text-fg/70">
                        {initImage.denoisingStrength.toFixed(2)}
                      </span>
                    </div>
                    <input
                      type="range"
                      min={ADVANCED_SD_DENOISING_STRENGTH_RANGE.min}
                      max={ADVANCED_SD_DENOISING_STRENGTH_RANGE.max}
                      step={0.05}
                      value={initImage.denoisingStrength}
                      onChange={(event) =>
                        onInitImageChange({
                          ...initImage,
                          denoisingStrength: Number(event.target.value),
                        })
                      }
                      className="mt-1.5 w-full accent-accent"
                    />
                    <button
                      type="button"
                      onClick={() => onInitImageChange(null)}
                      className="mt-1 flex items-center gap-1 text-[11px] font-medium text-fg/45 transition hover:text-danger"
                    >
                      <X size={11} />
                      {t("playground.prompt.clearInitImage")}
                    </button>
                  </div>
                </div>
              </div>
            ) : (
              <div className="grid grid-cols-2 gap-2">
                <button
                  type="button"
                  onClick={() => void pickInitImage()}
                  className="flex items-center justify-center gap-2 rounded-xl border border-dashed border-fg/15 bg-fg/2 px-3 py-3 text-[12px] font-medium text-fg/50 transition-all hover:border-fg/25 hover:bg-fg/5 hover:text-fg/75 active:scale-[0.99]"
                >
                  <ImageUp size={13} />
                  {t("playground.prompt.pickInitImageFile")}
                </button>
                <button
                  type="button"
                  onClick={() => setHistoryPickerOpen(true)}
                  className="flex items-center justify-center gap-2 rounded-xl border border-dashed border-fg/15 bg-fg/2 px-3 py-3 text-[12px] font-medium text-fg/50 transition-all hover:border-fg/25 hover:bg-fg/5 hover:text-fg/75 active:scale-[0.99]"
                >
                  <History size={13} />
                  {t("playground.prompt.pickInitImageHistory")}
                </button>
              </div>
            )}
          </div>
        )}
      </div>
      <PlaygroundInitImagePicker
        isOpen={historyPickerOpen}
        onClose={() => setHistoryPickerOpen(false)}
        onPick={(image) => void pickFromHistory(image)}
      />
      <button
        type="button"
        data-tour-id="playground-generate"
        onClick={onGenerate}
        disabled={!canGenerate || generating}
        className={cn(
          "flex h-11 w-full shrink-0 items-center justify-center gap-2 rounded-xl bg-accent px-4 text-sm font-semibold text-bg transition-[filter]",
          !canGenerate || generating
            ? "cursor-not-allowed opacity-50"
            : "hover:brightness-110 active:scale-[0.99]",
        )}
      >
        {generating ? <Loader size={15} className="animate-spin" /> : <Sparkles size={15} />}
        {generating ? t("playground.prompt.generating") : t("playground.prompt.generate")}
      </button>
      <p className="shrink-0 text-center text-[10.5px] text-fg/35">
        {t("playground.prompt.shortcutHint")}
      </p>
    </div>
  );
}
