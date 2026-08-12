import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { AnimatePresence, motion } from "framer-motion";
import { Cpu, Sparkles } from "lucide-react";

import { cn } from "../../design-tokens";
import { useI18n, type TranslationKey } from "../../../core/i18n/context";
import { CompanionsPage } from "./CompanionsPage";
import { CompanionSoulWriterPage } from "./CompanionSoulWriterPage";

type CompanionTab = "models" | "soulWriter";

const TABS = [
  {
    id: "models",
    labelKey: "companion.hub.modelsLabel",
    descriptionKey: "companion.hub.modelsDescription",
    icon: Cpu,
  },
  {
    id: "soulWriter",
    labelKey: "companion.hub.soulWriterLabel",
    descriptionKey: "companion.hub.soulWriterDescription",
    icon: Sparkles,
  },
] satisfies Array<{
  id: CompanionTab;
  labelKey: TranslationKey;
  descriptionKey: TranslationKey;
  icon: React.ComponentType<{ className?: string }>;
}>;

export function CompanionsHubPage() {
  const { t } = useI18n();
  const location = useLocation();
  const navigate = useNavigate();
  const initialTab: CompanionTab = location.pathname.includes("companion-soul-writer")
    ? "soulWriter"
    : "models";
  const [activeTab, setActiveTab] = useState<CompanionTab>(initialTab);

  useEffect(() => {
    const next: CompanionTab = location.pathname.includes("companion-soul-writer")
      ? "soulWriter"
      : "models";
    setActiveTab(next);
  }, [location.pathname]);

  const handleSelectTab = (tab: CompanionTab) => {
    setActiveTab(tab);
    const targetPath =
      tab === "soulWriter"
        ? "/settings/advanced/companion-soul-writer"
        : "/settings/advanced/companions";
    if (location.pathname !== targetPath) {
      navigate(targetPath, { replace: true });
    }
  };

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
                  onClick={() => handleSelectTab(tab.id)}
                  className={cn(
                    "group relative flex items-center gap-2.5 rounded-lg px-3 py-2.5 text-left transition-colors",
                    active ? "text-fg" : "text-fg/65 hover:text-fg",
                  )}
                >
                  {active && (
                    <motion.span
                      layoutId="companion-tab-pill"
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
            {activeTab === "models" ? <CompanionsPage /> : <CompanionSoulWriterPage />}
          </motion.div>
        </AnimatePresence>
      </div>
    </div>
  );
}
