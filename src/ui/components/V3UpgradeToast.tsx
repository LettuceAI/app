import { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { ArrowUpRight, X } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { getEmbeddingModelInfo } from "../../core/storage/repo";
import { useI18n } from "../../core/i18n/context";
import {
  isEmbeddingUpgradeDismissed,
  isEmbeddingUpgradePromptForced,
  markEmbeddingUpgradeDismissed,
} from "./EmbeddingUpgradePrompt";

/**
 * App-level toast that appears once on app launch if a v3 embedding model is
 * detected (and v4 is not yet installed). Persists dismissal to localStorage
 * via `markEmbeddingUpgradeDismissed("v4")` so the prompt doesn't reappear
 * until the next major target ships. The debug-only force flag bypasses both
 * the version check and persisted dismissal — see
 * `EmbeddingUpgradePrompt.tsx` for the console snippet.
 */
export function V3UpgradeToast() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [isVisible, setIsVisible] = useState(false);
  const [isDismissed, setIsDismissed] = useState(false);

  useEffect(() => {
    const forced = isEmbeddingUpgradePromptForced();
    if (!forced && isEmbeddingUpgradeDismissed("v4")) {
      return;
    }

    let cancelled = false;
    const checkVersion = async () => {
      try {
        const modelInfo = await getEmbeddingModelInfo();
        const available = modelInfo.availableVersions ?? [];
        const v4Installed =
          available.includes("v4") || modelInfo.sourceVersion === "v4";
        // If v4 is genuinely installed, never show — regardless of any debug
        // flag. Catches the case where the force flag was left on after
        // testing on a now-upgraded system.
        if (v4Installed) {
          return;
        }
        const eligible =
          modelInfo.installed && modelInfo.version === "v3" && !available.includes("v4");
        if (forced || eligible) {
          if (!cancelled) {
            setTimeout(() => {
              if (!cancelled) setIsVisible(true);
            }, 1800);
          }
        }
      } catch (err) {
        console.error("Failed to check embedding model source version:", err);
      }
    };

    void checkVersion();
    return () => {
      cancelled = true;
    };
  }, []);

  const handleDismiss = () => {
    setIsDismissed(true);
    markEmbeddingUpgradeDismissed("v4");
  };

  const handleUpgrade = () => {
    setIsDismissed(true);
    markEmbeddingUpgradeDismissed("v4");
    navigate("/settings/embedding-download?upgrade=true&version=v4&returnTo=/chat");
  };

  if (isDismissed) return null;

  // Locale-fallback for installs that haven't translated the v3 strings yet.
  const fallback = (key: string, en: string) => {
    const value = t(key as Parameters<typeof t>[0]);
    return value === key ? en : value;
  };
  const title = fallback("components.v3UpgradeToast.title", "Memory model v4");
  const badge = fallback("components.v3UpgradeToast.badge", "Available");
  const message = fallback(
    "components.v3UpgradeToast.message",
    "v4 dramatically improves roleplay memory recall over v3. Upgrading is recommended.",
  );
  const upgradeLabel = fallback("components.v3UpgradeToast.upgrade", "Upgrade");
  const dismissLabel = fallback("components.v3UpgradeToast.dismiss", "Later");

  return (
    <AnimatePresence>
      {isVisible && (
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 20 }}
          transition={{ duration: 0.25, ease: [0.4, 0, 0.2, 1] }}
          className="fixed bottom-24 inset-x-4 z-50"
        >
          <div className="pointer-events-auto mx-auto max-w-md rounded-xl border border-white/10 bg-[#0a0a0a]/95 p-4 shadow-2xl backdrop-blur-md">
            <div className="flex items-start gap-3.5">
              <div className="mt-1 h-1.5 w-1.5 shrink-0 rounded-full bg-amber-400" />

              <div className="flex-1 min-w-0">
                <div className="flex items-start justify-between gap-3">
                  <div className="flex-1">
                    <div className="flex items-baseline gap-2">
                      <h3 className="text-[15px] font-medium text-white">{title}</h3>
                      <span className="text-xs font-medium text-white/40">{badge}</span>
                    </div>
                    <p className="mt-1 text-[13px] leading-relaxed text-white/60">{message}</p>
                  </div>

                  <button
                    onClick={handleDismiss}
                    className="shrink-0 -mt-1 -mr-1 rounded-md p-1.5 text-white/40 transition-colors hover:bg-white/5 hover:text-white/60"
                    aria-label="Dismiss"
                  >
                    <X className="h-3.5 w-3.5" />
                  </button>
                </div>

                <div className="mt-3.5 flex gap-2">
                  <button
                    onClick={handleUpgrade}
                    className="group inline-flex items-center gap-1.5 rounded-lg bg-white px-3.5 py-2 text-[13px] font-medium text-black transition-all hover:bg-white/90 active:scale-[0.98]"
                  >
                    {upgradeLabel}
                    <ArrowUpRight className="h-3.5 w-3.5 transition-transform group-hover:translate-x-0.5 group-hover:-translate-y-0.5" />
                  </button>

                  <button
                    onClick={handleDismiss}
                    className="rounded-lg px-3.5 py-2 text-[13px] font-medium text-white/60 transition-colors hover:bg-white/5 hover:text-white/80 active:scale-[0.98]"
                  >
                    {dismissLabel}
                  </button>
                </div>
              </div>
            </div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
