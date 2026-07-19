import { useState } from "react";
import { ArrowLeft, ImagePlus, SlidersHorizontal, TerminalSquare } from "lucide-react";

import { cn } from "../../design-tokens";
import { BottomMenu } from "../../components/BottomMenu";
import { useI18n } from "../../../core/i18n/context";
import { Routes, useNavigationManager } from "../../navigation";
import { PlaygroundSettingsPane } from "./PlaygroundSettingsPane";
import { usePlaygroundSettings } from "./usePlaygroundSettings";

export function PlaygroundPage() {
  const { t } = useI18n();
  const { backOrReplace } = useNavigationManager();
  const settings = usePlaygroundSettings();
  const [promptSheetOpen, setPromptSheetOpen] = useState(false);
  const [settingsSheetOpen, setSettingsSheetOpen] = useState(false);

  const promptPane = (
    <div className="flex h-full flex-col gap-3 p-4">
      <p className="text-[11px] font-medium uppercase tracking-wide text-fg/40">
        {t("playground.promptTab")}
      </p>
    </div>
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
        <section className={cn("flex min-w-0 flex-1 flex-col")}>
          <div className="flex flex-1 flex-col items-center justify-center gap-2 text-center">
            <ImagePlus size={22} className="text-fg/20" />
            <p className="text-[13px] font-medium text-fg/60">{t("playground.emptyFeed")}</p>
            <p className="text-[12px] text-fg/40">{t("playground.emptyFeedHint")}</p>
          </div>
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
        {promptPane}
      </BottomMenu>
      <BottomMenu
        isOpen={settingsSheetOpen}
        onClose={() => setSettingsSheetOpen(false)}
        title={t("playground.settingsTab")}
      >
        {settingsPane}
      </BottomMenu>
    </div>
  );
}

export default PlaygroundPage;
