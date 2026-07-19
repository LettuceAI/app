import { useState } from "react";
import { ArrowLeft, SlidersHorizontal, TerminalSquare } from "lucide-react";

import { BottomMenu } from "../../components/BottomMenu";
import { useI18n } from "../../../core/i18n/context";
import { Routes, useNavigationManager } from "../../navigation";
import { PlaygroundFeed } from "./PlaygroundFeed";
import { PlaygroundPromptPane } from "./PlaygroundPromptPane";
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
  const [promptSheetOpen, setPromptSheetOpen] = useState(false);
  const [settingsSheetOpen, setSettingsSheetOpen] = useState(false);

  const showNegativePrompt = NEGATIVE_PROMPT_PROVIDERS.has(
    settings.selectedModel?.providerId ?? "",
  );
  const canGenerate = prompt.trim().length > 0 && settings.selectedModel !== null;

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
      initImage: null,
    });
  };

  const promptPane = (
    <PlaygroundPromptPane
      prompt={prompt}
      onPromptChange={setPrompt}
      negativePrompt={negativePrompt}
      onNegativePromptChange={setNegativePrompt}
      showNegativePrompt={showNegativePrompt}
      canGenerate={canGenerate}
      generating={generation.generating}
      onGenerate={handleGenerate}
    />
  );

  const settingsPane = <PlaygroundSettingsPane controller={settings} />;

  return (
    <div className="flex h-full flex-col bg-bg">
      <header className="flex h-12 shrink-0 items-center gap-2 border-b border-fg/8 px-3">
        <button
          type="button"
          onClick={() => backOrReplace(Routes.settingsImageGeneration)}
          aria-label={t("playground.back")}
          className="flex h-8 w-8 items-center justify-center rounded-lg text-fg/50 transition hover:bg-fg/8 hover:text-fg"
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
        <aside className="hidden w-[300px] shrink-0 flex-col border-r border-fg/8 lg:flex">
          {promptPane}
        </aside>
        <section className="flex min-w-0 flex-1 flex-col">
          <PlaygroundFeed generation={generation} />
        </section>
        <aside className="hidden w-[340px] shrink-0 flex-col border-l border-fg/8 lg:flex">
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
