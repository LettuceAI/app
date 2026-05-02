import { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Sparkles, ArrowRight, X } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { getEmbeddingModelInfo } from "../../core/storage/repo";
import { useI18n } from "../../core/i18n/context";

/**
 * A toast that appears once on app launch if v1 embedding model is detected.
 * Prompts user to upgrade to v4.
 */
export function V1UpgradeToast() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [isVisible, setIsVisible] = useState(false);
  const [isDismissed, setIsDismissed] = useState(false);

  useEffect(() => {
    const dismissed = sessionStorage.getItem("v1_upgrade_toast_dismissed");
    if (dismissed) return;

    const checkVersion = async () => {
      try {
        const modelInfo = await getEmbeddingModelInfo();
        const available = modelInfo.availableVersions ?? [];
        if (modelInfo.installed && modelInfo.version === "v1" && !available.includes("v4")) {
          setTimeout(() => setIsVisible(true), 1500);
        }
      } catch (err) {
        console.error("Failed to check embedding model version:", err);
      }
    };

    checkVersion();
  }, []);

  const handleDismiss = () => {
    setIsDismissed(true);
    sessionStorage.setItem("v1_upgrade_toast_dismissed", "true");
  };

  const handleUpgrade = () => {
    handleDismiss();
    navigate("/settings/embedding-download?upgrade=true&version=v4&returnTo=/chat");
  };

  if (isDismissed) return null;

  return (
    <AnimatePresence>
      {isVisible && (
        <motion.div
          initial={{ opacity: 0, y: 50, scale: 0.95 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: 50, scale: 0.95 }}
          transition={{ duration: 0.3, ease: "easeOut" }}
          className="fixed bottom-24 inset-x-4 z-50"
        >
          <div className="mx-auto max-w-md rounded-2xl border border-amber-500/30 bg-black/90 backdrop-blur-lg p-4 shadow-2xl shadow-amber-500/10">
            <div className="flex items-start gap-3">
              <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-amber-500/20">
                <Sparkles className="h-5 w-5 text-amber-400" />
              </div>
              <div className="flex-1 min-w-0">
                <div className="flex items-start justify-between gap-2">
                  <div>
                    <h3 className="font-semibold text-amber-200 text-sm">
                      {t("components.v1UpgradeToast.title")}
                    </h3>
                    <p className="mt-1 text-xs text-amber-200/70">
                      {t("components.v1UpgradeToast.message")}
                    </p>
                  </div>
                  <button
                    onClick={handleDismiss}
                    className="shrink-0 p-1 text-amber-400/60 hover:text-amber-400 transition-colors"
                    aria-label="Dismiss"
                  >
                    <X className="h-4 w-4" />
                  </button>
                </div>
                <div className="mt-3 flex gap-2">
                  <button
                    onClick={handleUpgrade}
                    className="flex items-center gap-1.5 rounded-lg bg-amber-500 px-3 py-1.5 text-xs font-medium text-black transition-all hover:bg-amber-400"
                  >
                    {t("components.v1UpgradeToast.upgrade")}
                    <ArrowRight className="h-3.5 w-3.5" />
                  </button>
                  <button
                    onClick={handleDismiss}
                    className="px-3 py-1.5 text-xs text-amber-200/60 hover:text-amber-200 transition-colors"
                  >
                    {t("components.v1UpgradeToast.dismiss")}
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
