import { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { ArrowUpRight, X } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { getEmbeddingModelInfo } from "../../core/storage/repo";
import { useI18n } from "../../core/i18n/context";

/**
 * A toast that appears once on app launch if legacy v2 embedding model is detected.
 * Prompts user to upgrade to v4.
 */
export function V2UpgradeToast() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [isVisible, setIsVisible] = useState(false);
  const [isDismissed, setIsDismissed] = useState(false);

  useEffect(() => {
    const dismissed = sessionStorage.getItem("v2_upgrade_toast_dismissed");
    if (dismissed) return;

    const checkVersion = async () => {
      try {
        const modelInfo = await getEmbeddingModelInfo();
        const available = modelInfo.availableVersions ?? [];
        if (modelInfo.installed && modelInfo.version === "v2" && !available.includes("v4")) {
          setTimeout(() => setIsVisible(true), 1800);
        }
      } catch (err) {
        console.error("Failed to check embedding model source version:", err);
      }
    };

    checkVersion();
  }, []);

  const handleDismiss = () => {
    setIsDismissed(true);
    sessionStorage.setItem("v2_upgrade_toast_dismissed", "true");
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
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 20 }}
          transition={{ duration: 0.25, ease: [0.4, 0, 0.2, 1] }}
          className="fixed bottom-24 inset-x-4 z-50"
        >
          <div className="pointer-events-auto mx-auto max-w-md rounded-xl border border-white/10 bg-[#0a0a0a]/95 p-4 shadow-2xl backdrop-blur-md">
            <div className="flex items-start gap-3.5">
              {/* Simple indicator dot */}
              <div className="mt-1 h-1.5 w-1.5 shrink-0 rounded-full bg-blue-400" />

              <div className="flex-1 min-w-0">
                <div className="flex items-start justify-between gap-3">
                  <div className="flex-1">
                    <div className="flex items-baseline gap-2">
                      <h3 className="text-[15px] font-medium text-white">{t("components.v2UpgradeToast.title")}</h3>
                      <span className="text-xs font-medium text-white/40">{t("components.v2UpgradeToast.badge")}</span>
                    </div>
                    <p className="mt-1 text-[13px] leading-relaxed text-white/60">
                      {t("components.v2UpgradeToast.message")}
                    </p>
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
                    {t("components.v2UpgradeToast.upgrade")}
                    <ArrowUpRight className="h-3.5 w-3.5 transition-transform group-hover:translate-x-0.5 group-hover:-translate-y-0.5" />
                  </button>

                  <button
                    onClick={handleDismiss}
                    className="rounded-lg px-3.5 py-2 text-[13px] font-medium text-white/60 transition-colors hover:bg-white/5 hover:text-white/80 active:scale-[0.98]"
                  >
                    {t("components.v2UpgradeToast.dismiss")}
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
