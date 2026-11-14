import { useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import { History, ImageUp, Sparkles, X } from "lucide-react";
import { Button, Field, icon, Slider, Text, Textarea, toast } from "@lettuceai/crisp";
import { useApi } from "@/api/use-api";
import { pickImage } from "@/entities/image-input";
import { editorFields } from "@/entities/model/model-editor-fields";
import { InitImagePicker } from "./InitImagePicker";
import { defaultStrength, type PlaygroundAsset, type StartImage } from "./playground-data";

const strengthRange = editorFields.sdDenoisingStrength;

export function PromptPane({
  prompt,
  onPromptChange,
  negative,
  onNegativeChange,
  showNegative,
  showStartImage,
  startImage,
  onStartImageChange,
  canGenerate,
  generating,
  onGenerate,
}: {
  prompt: string;
  onPromptChange: (value: string) => void;
  negative: string;
  onNegativeChange: (value: string) => void;
  showNegative: boolean;
  showStartImage: boolean;
  startImage: StartImage | null;
  onStartImageChange: (value: StartImage | null) => void;
  canGenerate: boolean;
  generating: boolean;
  onGenerate: () => void;
}) {
  const { t } = useTranslation("playground");
  const api = useApi();
  const [historyOpen, setHistoryOpen] = useState(false);

  const start = (asset: PlaygroundAsset) =>
    onStartImageChange({ assetId: asset.asset_id, url: asset.preview_url ?? asset.url, strength: startImage?.strength ?? defaultStrength });

  const pickFile = async () => {
    try {
      const asset = await pickImage(api, "reference_image");
      if (asset) start(asset);
    } catch {
      toast.error(t("prompt.initImageFailed"));
    }
  };

  const submitShortcut = (event: KeyboardEvent) => {
    if ((event.ctrlKey || event.metaKey) && event.key === "Enter" && canGenerate) {
      event.preventDefault();
      onGenerate();
    }
  };

  return (
    <div className="flex min-h-full flex-col gap-4">
      <div className="flex min-h-0 flex-1 flex-col gap-4">
        <Field label={t("prompt.label")} className="flex flex-1 flex-col">
          <Textarea
            value={prompt}
            onChange={(event) => onPromptChange(event.target.value)}
            onKeyDown={submitShortcut}
            placeholder={t("prompt.placeholder")}
            className="min-h-36 flex-1"
          />
        </Field>
        {showNegative && (
          <Field label={t("prompt.negativeLabel")}>
            <Textarea
              value={negative}
              onChange={(event) => onNegativeChange(event.target.value)}
              onKeyDown={submitShortcut}
              placeholder={t("prompt.negativePlaceholder")}
              rows={4}
            />
          </Field>
        )}
        {showStartImage && (
          <div className="space-y-1.5">
            <span className="block text-sm font-medium text-fg-2">{t("prompt.initImageLabel")}</span>
            {startImage ? (
              <div className="flex items-start gap-3 rounded-xl border border-line bg-surface-1 p-2.5">
                <img src={startImage.url} alt="" className="h-16 w-16 shrink-0 rounded-lg border border-line object-cover" />
                <div className="min-w-0 flex-1 space-y-1">
                  <Slider
                    aria-label={t("prompt.strength")}
                    value={startImage.strength}
                    min={strengthRange.min}
                    max={strengthRange.max}
                    step={strengthRange.step}
                    showValue={(value) => value.toFixed(2)}
                    onChange={(value) => onStartImageChange({ ...startImage, strength: value })}
                  />
                  <Button size="sm" variant="ghost" leading={<X size={icon.sm} />} onClick={() => onStartImageChange(null)}>
                    {t("prompt.clearInitImage")}
                  </Button>
                </div>
              </div>
            ) : (
              <div className="grid grid-cols-2 gap-2">
                <Button variant="secondary" size="sm" leading={<ImageUp size={icon.sm} />} onClick={() => void pickFile()}>
                  {t("prompt.pickInitImageFile")}
                </Button>
                <Button variant="secondary" size="sm" leading={<History size={icon.sm} />} onClick={() => setHistoryOpen(true)}>
                  {t("prompt.pickInitImageHistory")}
                </Button>
              </div>
            )}
          </div>
        )}
      </div>
      <InitImagePicker open={historyOpen} onClose={() => setHistoryOpen(false)} onPick={start} />
      <Button
        variant="primary"
        size="lg"
        block
        loading={generating}
        disabled={!canGenerate}
        leading={generating ? undefined : <Sparkles size={icon.md} />}
        onClick={onGenerate}
      >
        {t(generating ? "prompt.generating" : "prompt.generate")}
      </Button>
      <Text variant="hint" className="text-center">{t("prompt.shortcutHint")}</Text>
    </div>
  );
}
