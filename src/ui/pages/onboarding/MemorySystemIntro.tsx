import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { ArrowLeft, Check, Brain, Edit3, Loader, ArrowRight } from "lucide-react";
import { storageBridge } from "../../../core/storage/files";
import { checkEmbeddingModel } from "../../../core/storage/repo";
import { setOnboardingCompleted } from "../../../core/storage/appState";
import { Settings } from "../../../core/storage/schemas";
import { MemorySetupPrompt } from "./MemorySetupPrompt";
import { cn, radius, typography, interactive } from "../../design-tokens";
import { getPlatform } from "../../../core/utils/platform";
import { useI18n } from "../../../core/i18n/context";

export function MemorySystemIntro() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const platform = getPlatform();
  const isDesktop = platform.type === "desktop";

  const [isProcessing, setIsProcessing] = useState(false);
  const [selectedType, setSelectedType] = useState<"manual" | "dynamic" | null>(null);
  const [showDownloadModal, setShowDownloadModal] = useState(false);

  const saveSettings = async (enableDynamic: boolean) => {
    setIsProcessing(true);
    try {
      const currentSettings = await storageBridge.readSettings<Settings | null>(null);
      if (currentSettings) {
        const advancedSettings = (currentSettings.advancedSettings || {}) as any;
        const dynamicMemory = advancedSettings.dynamicMemory || {
          enabled: false,
          summaryMessageInterval: 20,
          maxEntries: 50,
          minSimilarityThreshold: 0.32,
          hotMemoryTokenBudget: 2000,
          decayRate: 0.08,
          coldThreshold: 0.3,
        };

        dynamicMemory.enabled = enableDynamic;

        await storageBridge.settingsSetAdvanced({
          ...advancedSettings,
          dynamicMemory,
        });
      }
      await setOnboardingCompleted(true);
    } catch (error) {
      console.error("Failed to save memory settings:", error);
    } finally {
      setIsProcessing(false);
    }
  };

  const handleFinish = async () => {
    if (!selectedType) return;

    if (selectedType === "dynamic") {
      try {
        const modelExists = await checkEmbeddingModel();
        if (modelExists) {
          await saveSettings(true);
          navigate("/chat?firstTime=true");
        } else {
          setShowDownloadModal(true);
        }
      } catch (error) {
        console.error("Failed to check embedding model:", error);
        setShowDownloadModal(true);
      }
    } else {
      await saveSettings(false);
      navigate("/chat?firstTime=true");
    }
  };

  // Memory option cards - shared component
  const DynamicMemoryCard = ({ compact = false }: { compact?: boolean }) => (
    <button
      onClick={() => setSelectedType("dynamic")}
      className={cn(
        "w-full h-full flex flex-col text-left relative overflow-hidden group transition-all duration-300",
        radius.lg,
        "border p-1",
        selectedType === "dynamic"
          ? "border-emerald-500/50 bg-emerald-500/5 ring-1 ring-emerald-500/20"
          : "border-white/10 bg-white/5 hover:border-white/20 hover:bg-white/8",
      )}
    >
      <div
        className={cn(
          "absolute inset-0 bg-linear-to-br from-emerald-500/10 via-transparent to-transparent opacity-0 transition-opacity duration-500",
          selectedType === "dynamic" ? "opacity-100" : "group-hover:opacity-30",
        )}
      />

      <div className={cn("relative z-10", compact ? "p-4" : "p-5")}>
        <div className="flex items-start justify-between mb-2">
          <div className="flex items-center gap-3">
            <div
              className={cn(
                "w-10 h-10 rounded-xl flex items-center justify-center border transition-colors duration-300",
                selectedType === "dynamic"
                  ? "bg-emerald-500/20 border-emerald-500/30 text-emerald-400"
                  : "bg-white/10 border-white/10 text-gray-400 group-hover:text-emerald-300",
              )}
            >
              <Brain size={20} />
            </div>
            <div>
              <h3 className={cn("font-semibold text-white", compact ? "text-base" : "text-lg")}>
                Dynamic Memory
              </h3>
              <div className="flex items-center gap-1.5 mt-0.5 h-5">
                <span className="text-[10px] uppercase tracking-wider font-medium text-emerald-400 bg-emerald-500/10 px-1.5 py-0.5 rounded border border-emerald-500/20">
                  Recommended
                </span>
              </div>
            </div>
          </div>
          <div
            className={cn(
              "w-6 h-6 rounded-full border flex items-center justify-center transition-all duration-300",
              selectedType === "dynamic"
                ? "bg-emerald-500 border-emerald-500 text-black scale-100"
                : "border-white/20 bg-transparent text-transparent scale-90",
            )}
          >
            <Check size={14} strokeWidth={3} />
          </div>
        </div>

        <p
          className={cn(
            "text-gray-400 mb-4 leading-relaxed pl-13",
            compact ? "text-xs" : "text-sm",
          )}
        >
          Uses a <b>local embedding model</b> to smartly manage context. This cuts token costs while
          maintaining high quality, even in long chats.
        </p>

        <div
          className={cn("grid gap-2 pl-13", compact ? "grid-cols-1" : "grid-cols-1 sm:grid-cols-2")}
        >
          <div className="flex items-center gap-2 text-xs text-gray-300">
            <div className="w-1 h-1 rounded-full bg-emerald-500" />
            Maintains quality in long chats
          </div>
          <div className="flex items-center gap-2 text-xs text-gray-300">
            <div className="w-1 h-1 rounded-full bg-emerald-500" />
            Reduces API costs significantly
          </div>
          <div className="flex items-center gap-2 text-xs text-gray-300">
            <div className="w-1 h-1 rounded-full bg-emerald-500" />
            Automatic context management
          </div>
          <div className="flex items-center gap-2 text-xs text-gray-300">
            <div className="w-1 h-1 rounded-full bg-emerald-500" />
            Zero configuration needed
          </div>
        </div>
      </div>
    </button>
  );

  const ManualMemoryCard = ({ compact = false }: { compact?: boolean }) => (
    <button
      onClick={() => setSelectedType("manual")}
      className={cn(
        "w-full h-full flex flex-col text-left relative overflow-hidden group transition-all duration-300",
        radius.lg,
        "border p-1",
        selectedType === "manual"
          ? "border-blue-500/50 bg-blue-500/5 ring-1 ring-blue-500/20"
          : "border-white/10 bg-white/5 hover:border-white/20 hover:bg-white/8",
      )}
    >
      <div
        className={cn(
          "absolute inset-0 bg-linear-to-br from-blue-500/10 via-transparent to-transparent opacity-0 transition-opacity duration-500",
          selectedType === "manual" ? "opacity-100" : "group-hover:opacity-30",
        )}
      />

      <div className={cn("relative z-10", compact ? "p-4" : "p-5")}>
        <div className="flex items-start justify-between mb-2">
          <div className="flex items-center gap-3">
            <div
              className={cn(
                "w-10 h-10 rounded-xl flex items-center justify-center border transition-colors duration-300",
                selectedType === "manual"
                  ? "bg-blue-500/20 border-blue-500/30 text-blue-400"
                  : "bg-white/10 border-white/10 text-gray-400 group-hover:text-blue-300",
              )}
            >
              <Edit3 size={20} />
            </div>
            <div>
              <h3 className={cn("font-semibold text-white", compact ? "text-base" : "text-lg")}>
                Manual Memory
              </h3>
              <p className="text-xs text-gray-500 mt-0.5 h-5 flex items-center">
                Classic experience
              </p>
            </div>
          </div>
          <div
            className={cn(
              "w-6 h-6 rounded-full border flex items-center justify-center transition-all duration-300",
              selectedType === "manual"
                ? "bg-blue-500 border-blue-500 text-black scale-100"
                : "border-white/20 bg-transparent text-transparent scale-90",
            )}
          >
            <Check size={14} strokeWidth={3} />
          </div>
        </div>

        <p
          className={cn(
            "text-gray-400 mb-4 leading-relaxed pl-13",
            compact ? "text-xs" : "text-sm",
          )}
        >
          You explicitly pin messages and edit the "World Info" or character definitions yourself.
          Good for total control.
        </p>

        <div
          className={cn("grid gap-2 pl-13", compact ? "grid-cols-1" : "grid-cols-1 sm:grid-cols-2")}
        >
          <div className="flex items-center gap-2 text-xs text-gray-300">
            <div className="w-1 h-1 rounded-full bg-blue-500" />
            Total control over facts
          </div>
          <div className="flex items-center gap-2 text-xs text-gray-300">
            <div className="w-1 h-1 rounded-full bg-blue-500" />
            Best for specific scenarios
          </div>
        </div>
      </div>
    </button>
  );

  const FinishButton = () => (
    <button
      onClick={handleFinish}
      disabled={!selectedType || isProcessing}
      className={cn(
        "w-full h-14 rounded-2xl font-bold text-lg transition-all duration-300 active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-50 flex items-center justify-center gap-3 group relative overflow-hidden",
        selectedType === "dynamic"
          ? "bg-emerald-500 text-black shadow-[0_4px_20px_rgba(16,185,129,0.25)] hover:shadow-[0_4px_24px_rgba(16,185,129,0.4)] hover:bg-emerald-400"
          : selectedType === "manual"
            ? "bg-blue-500 text-black shadow-[0_4px_20px_rgba(59,130,246,0.25)] hover:shadow-[0_4px_24px_rgba(59,130,246,0.4)] hover:bg-blue-400"
            : "bg-white/5 text-gray-500 border border-white/5",
      )}
    >
      {isProcessing ? (
        <>
          <Loader size={20} className="animate-spin" />
          <span>Setting up...</span>
        </>
      ) : (
        <>
          <span>Finish Setup</span>
          {selectedType && (
            <ArrowRight
              size={20}
              className="transition-transform duration-300 group-hover:translate-x-1"
            />
          )}
        </>
      )}
    </button>
  );

  // Desktop Layout
  if (isDesktop) {
    return (
      <div className="flex min-h-screen flex-col text-gray-200 overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-8 py-6 border-b border-white/10">
          <button
            onClick={() => navigate("/onboarding/models")}
            className="flex items-center justify-center w-11 h-11 rounded-full border border-white/15 bg-white/8 text-white transition-all duration-200 hover:border-white/30 hover:bg-white/15 active:scale-[0.98]"
          >
            <ArrowLeft size={18} />
          </button>
          <div className="text-center">
            <p
              className={cn(
                typography.caption.size,
                "font-medium uppercase tracking-[0.25em] text-gray-500",
              )}
            >
              Step 3 of 3
            </p>
            <p className="text-xs text-gray-400 mt-0.5">Memory System</p>
          </div>
          <div className="w-11" />
        </div>

        {/* Main Content - Centered with side-by-side cards */}
        <div className="flex-1 flex flex-col items-center justify-center p-8 overflow-y-auto">
          <div className="w-full max-w-4xl">
            {/* Title */}
            <div className="text-center space-y-3 mb-10">
              <h1 className="text-2xl font-bold text-white">{t("onboarding.steps.memory")}</h1>
              <p className="text-sm text-gray-400 max-w-md mx-auto leading-relaxed">
                How should your AI companions remember details about you and your conversations?
              </p>
            </div>

            {/* Side-by-side cards */}
            <div className="grid grid-cols-2 gap-6 mb-8">
              <DynamicMemoryCard compact />
              <ManualMemoryCard compact />
            </div>

            {/* Action Button */}
            <div className="max-w-md mx-auto">
              <FinishButton />
            </div>
          </div>
        </div>

        {/* Download Confirmation Modal */}
        <MemorySetupPrompt
          isOpen={showDownloadModal}
          onClose={() => setShowDownloadModal(false)}
          onConfirm={() => navigate("/settings/embedding-download?returnTo=/chat?firstTime=true")}
          onSkip={async () => {
            await saveSettings(false);
            navigate("/chat?firstTime=true");
          }}
        />
      </div>
    );
  }

  // Mobile Layout (original)
  return (
    <div className="flex min-h-screen flex-col text-gray-200 px-4 pt-8 pb-8 overflow-y-auto">
      <div className="flex flex-col items-center max-w-md mx-auto w-full">
        {/* Header */}
        <div className="flex w-full items-center justify-between mb-8">
          <button
            onClick={() => navigate("/onboarding/models")}
            className={cn(
              "flex h-10 w-10 items-center justify-center rounded-full border border-white/15 bg-white/8 text-white",
              interactive.transition.default,
              interactive.hover.brightness,
            )}
          >
            <ArrowLeft size={16} />
          </button>
          <div className="text-center">
            <p
              className={cn(
                typography.caption.size,
                "font-medium uppercase tracking-[0.25em] text-gray-500",
              )}
            >
              Step 3 of 3
            </p>
            <p className="text-xs text-gray-400 mt-0.5">Memory System</p>
          </div>
          <div className="w-10" />
        </div>

        {/* Title */}
        <div className="text-center space-y-3 mb-10">
          <h1 className={cn(typography.h3.size, typography.h3.weight, "text-white")}>
            Choose your memory style
          </h1>
          <p className="text-sm text-gray-400 max-w-xs mx-auto leading-relaxed">
            How should your AI companions remember details about you and your conversations?
          </p>
        </div>

        {/* Comparison Cards - Stacked */}
        <div className="w-full space-y-4 mb-8">
          <DynamicMemoryCard />
          <ManualMemoryCard />
        </div>

        {/* Action Button */}
        <FinishButton />
      </div>

      {/* Download Confirmation Modal */}
      <MemorySetupPrompt
        isOpen={showDownloadModal}
        onClose={() => setShowDownloadModal(false)}
        onConfirm={() => navigate("/settings/embedding-download?returnTo=/chat?firstTime=true")}
        onSkip={async () => {
          await saveSettings(false);
          navigate("/chat?firstTime=true");
        }}
      />
    </div>
  );
}
