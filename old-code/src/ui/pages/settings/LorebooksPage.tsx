import { useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { BookOpen, Workflow } from "lucide-react";

import { cn } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import type { TranslationKey } from "../../../core/i18n/context";
import { LorebookGeneratorPage } from "./LorebookGeneratorPage";
import { LorebookEntryGeneratorPage } from "./LorebookEntryGeneratorPage";

type LorebookTab = "full" | "entry";

const TABS = [
  {
    id: "full",
    labelKey: "lorebookGen.tabs.fullLabel",
    descriptionKey: "lorebookGen.tabs.fullDescription",
    icon: Workflow,
  },
  {
    id: "entry",
    labelKey: "lorebookGen.tabs.entryLabel",
    descriptionKey: "lorebookGen.tabs.entryDescription",
    icon: BookOpen,
  },
] satisfies Array<{
  id: LorebookTab;
  labelKey: TranslationKey;
  descriptionKey: TranslationKey;
  icon: React.ComponentType<{ className?: string }>;
}>;

export function LorebooksPage() {
  const { t } = useI18n();
  const [activeTab, setActiveTab] = useState<LorebookTab>("full");

  return (
    <div className="flex min-h-screen flex-col">
      <div className="px-4 pt-4">
        <div className="mx-auto w-full max-w-5xl">
          <div className="grid grid-cols-2 gap-2 rounded-xl border border-fg/10 bg-fg/3 p-1">
            {TABS.map((tab) => {
              const active = tab.id === activeTab;
              const Icon = tab.icon;
              return (
                <button
                  key={tab.id}
                  type="button"
                  onClick={() => setActiveTab(tab.id)}
                  className={cn(
                    "group relative flex items-center gap-2.5 rounded-lg px-3 py-2.5 text-left transition-colors",
                    active ? "text-fg" : "text-fg/65 hover:text-fg",
                  )}
                >
                  {active && (
                    <motion.span
                      layoutId="lorebook-tab-pill"
                      className="absolute inset-0 rounded-lg bg-fg/8"
                      transition={{ type: "spring", stiffness: 380, damping: 32 }}
                    />
                  )}
                  <span
                    className={cn(
                      "relative flex h-7 w-7 shrink-0 items-center justify-center rounded-lg border transition-colors",
                      active
                        ? "border-accent/40 bg-accent/15 text-accent"
                        : "border-fg/10 bg-fg/5 text-fg/55",
                    )}
                  >
                    <Icon className="h-4 w-4" />
                  </span>
                  <span className="relative min-w-0 flex-1">
                    <span className="block text-sm font-medium">{t(tab.labelKey)}</span>
                    <span className="mt-0.5 block truncate text-[11px] text-fg/45">
                      {t(tab.descriptionKey)}
                    </span>
                  </span>
                </button>
              );
            })}
          </div>
        </div>
      </div>

      <div className="relative flex-1">
        <AnimatePresence mode="wait" initial={false}>
          <motion.div
            key={activeTab}
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1] }}
          >
            {activeTab === "full" ? <LorebookGeneratorPage /> : <LorebookEntryGeneratorPage />}
          </motion.div>
        </AnimatePresence>
      </div>
    </div>
  );
}
