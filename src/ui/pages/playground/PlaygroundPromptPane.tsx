import { Loader, Sparkles } from "lucide-react";

import { cn } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";

export function PlaygroundPromptPane({
  prompt,
  onPromptChange,
  negativePrompt,
  onNegativePromptChange,
  showNegativePrompt,
  canGenerate,
  generating,
  onGenerate,
}: {
  prompt: string;
  onPromptChange: (value: string) => void;
  negativePrompt: string;
  onNegativePromptChange: (value: string) => void;
  showNegativePrompt: boolean;
  canGenerate: boolean;
  generating: boolean;
  onGenerate: () => void;
}) {
  const { t } = useI18n();

  const handleKeyDown = (event: React.KeyboardEvent) => {
    if ((event.ctrlKey || event.metaKey) && event.key === "Enter" && canGenerate && !generating) {
      event.preventDefault();
      onGenerate();
    }
  };

  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto p-4">
      <div className="flex min-h-0 flex-1 flex-col gap-4">
        <div className="flex min-h-0 flex-1 flex-col">
          <p className="mb-1.5 text-[11px] font-medium uppercase tracking-wide text-fg/40">
            {t("playground.prompt.label")}
          </p>
          <textarea
            value={prompt}
            onChange={(event) => onPromptChange(event.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t("playground.prompt.placeholder")}
            className="min-h-[140px] w-full flex-1 resize-none rounded-xl border border-fg/10 bg-surface px-3 py-2.5 text-[13px] leading-relaxed text-fg outline-none transition focus:border-accent/40"
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
              className="w-full resize-none rounded-xl border border-fg/10 bg-surface px-3 py-2.5 text-[13px] leading-relaxed text-fg outline-none transition focus:border-accent/40"
            />
          </div>
        )}
      </div>
      <button
        type="button"
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
