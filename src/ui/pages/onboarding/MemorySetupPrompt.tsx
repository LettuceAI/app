import { Download, Brain } from "lucide-react";
import { BottomMenu } from "../../components/BottomMenu";
import { useI18n } from "../../../core/i18n/context";

interface MemorySetupPromptProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: () => void;
  onSkip: () => void;
}

export function MemorySetupPrompt({ isOpen, onClose, onConfirm, onSkip }: MemorySetupPromptProps) {
  const { t } = useI18n();
  return (
    <BottomMenu
      isOpen={isOpen}
      onClose={onClose}
      title={t("onboarding.memory.promptTitle")}
      includeExitIcon={false}
    >
      <div className="space-y-6">
        {/* Icon and Message */}
        <div className="text-center">
          <div className="mb-4 flex justify-center">
            <div className="flex h-16 w-16 items-center justify-center rounded-full border border-emerald-500/20 bg-emerald-500/10">
              <Brain className="h-8 w-8 text-emerald-400" />
            </div>
          </div>
          <h3 className="text-lg font-semibold text-white">{t("onboarding.memory.oneLastStep")}</h3>
          <p className="mt-2 text-sm text-white/60 leading-relaxed max-w-xs mx-auto">
            {t("onboarding.memory.setupModelMessage")}
          </p>
        </div>

        {/* Info Box */}
        <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4">
          <ul className="space-y-2 text-xs text-emerald-200/80">
            <li className="flex items-start gap-2">
              <span className="mt-0.5">•</span>
              <span>{t("onboarding.memory.setupBullets.offline")}</span>
            </li>
            <li className="flex items-start gap-2">
              <span className="mt-0.5">•</span>
              <span>{t("onboarding.memory.setupBullets.remembering")}</span>
            </li>
            <li className="flex items-start gap-2">
              <span className="mt-0.5">•</span>
              <span>{t("onboarding.memory.setupBullets.disable")}</span>
            </li>
          </ul>
        </div>

        {/* Action Buttons */}
        <div className="space-y-3">
          <button
            onClick={() => {
              onClose();
              onConfirm();
            }}
            className="w-full flex items-center justify-center gap-2 rounded-xl bg-emerald-500 hover:bg-emerald-400 text-black font-bold h-12 transition-colors"
          >
            <Download size={18} />
            {t("onboarding.memory.downloadAndEnable")}
          </button>

          <button
            onClick={() => {
              onClose();
              onSkip();
            }}
            className="w-full h-12 rounded-xl border border-white/10 bg-white/5 text-sm font-medium text-gray-400 hover:text-white hover:bg-white/10 transition-colors"
          >
            {t("onboarding.welcome.skipForNow")}
          </button>
        </div>
      </div>
    </BottomMenu>
  );
}
