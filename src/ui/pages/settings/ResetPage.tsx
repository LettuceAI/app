import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { AlertTriangle, Trash2 } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { exit } from "@tauri-apps/plugin-process";

import { ResetManager } from "../../../core/storage/reset";
import { typography, radius, interactive, shadows, cn } from "../../design-tokens";

export function ResetPage() {
  const navigate = useNavigate();
  const [isResetting, setIsResetting] = useState(false);
  const [showConfirmModal, setShowConfirmModal] = useState(false);

  const handleReset = async () => {
    try {
      setIsResetting(true);
      await ResetManager.resetAllData();

      // Force app restart for clean database state
      // The app will re-initialize fresh when reopened
      await exit(0);
    } catch (error: any) {
      console.error("Reset failed:", error);
      const message =
        error instanceof Error
          ? error.message
          : typeof error === "string"
            ? error
            : "Unknown error";
      alert(`Reset failed: ${message}`);
      setIsResetting(false);
      setShowConfirmModal(false);
    }
  };

  return (
    <div className="px-0 pt-4 pb-4 text-fg" data-settings-scroll>
      <motion.div
        initial={{ opacity: 0, y: 16 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3, ease: "easeOut" }}
        className="mx-auto w-full max-w-md text-center"
      >
        {/* Warning Icon */}
        <motion.div
          initial={{ scale: 0.8, opacity: 0 }}
          animate={{ scale: 1, opacity: 1 }}
          transition={{ delay: 0.1, duration: 0.3 }}
          className={cn(
            "mx-auto mb-8 flex h-24 w-24 items-center justify-center border border-danger/30 bg-danger/10 text-danger/80",
            radius.full,
            shadows.lg,
          )}
        >
          <AlertTriangle size={44} strokeWidth={2} />
        </motion.div>

        {/* Title & Description */}
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ delay: 0.2 }}
          className="mb-10 space-y-3"
        >
          <h2 className={cn(typography.display.size, typography.display.weight, "text-fg")}>
            Reset Everything
          </h2>
          <p className={cn(typography.body.size, typography.body.lineHeight, "text-fg/50")}>
            This will permanently delete all providers, models, characters, chat sessions, and
            preferences from this device.
          </p>
        </motion.div>

        {/* Warning Message */}
        <motion.div
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.3 }}
          className={cn("mb-8 border border-danger/30 bg-danger/5 p-4", radius.lg)}
        >
          <div className="flex items-center justify-center gap-2">
            <AlertTriangle size={16} className="text-danger/80" strokeWidth={2.5} />
            <p className={cn(typography.bodySmall.size, typography.h3.weight, "text-danger/80")}>
              This action cannot be undone
            </p>
          </div>
        </motion.div>

        {/* Action Buttons */}
        <motion.div
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.4 }}
          className="flex flex-col gap-3"
        >
          <button
            onClick={() => setShowConfirmModal(true)}
            disabled={isResetting}
            className={cn(
              "w-full px-6 py-4",
              radius.md,
              typography.body.size,
              typography.h3.weight,
              "border border-danger/40 bg-danger/20 text-danger/90",
              shadows.glow,
              interactive.transition.default,
              "active:scale-[0.97] active:border-danger/60 active:bg-danger/30",
              "disabled:cursor-not-allowed disabled:opacity-50",
            )}
          >
            <div className="flex items-center justify-center gap-2">
              <Trash2 size={18} />
              <span>Reset All Data</span>
            </div>
          </button>

          <button
            onClick={() => navigate("/settings")}
            disabled={isResetting}
            className={cn(
              "w-full px-6 py-3",
              radius.md,
              typography.body.size,
              "border border-fg/10 bg-fg/5 text-fg/60",
              interactive.transition.default,
              "active:scale-[0.97] active:bg-fg/10",
              "disabled:cursor-not-allowed disabled:opacity-50",
            )}
          >
            Cancel
          </button>
        </motion.div>
      </motion.div>

      {/* Confirmation Modal */}
      <AnimatePresence>
        {showConfirmModal && (
          <motion.div
            className="fixed inset-0 z-50 flex items-end justify-center bg-black/70 backdrop-blur-md sm:items-center"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={() => !isResetting && setShowConfirmModal(false)}
          >
            <motion.div
              className={cn(
                "w-full max-w-md border border-fg/10 bg-surface-el p-6",
                "rounded-t-3xl sm:rounded-3xl",
                shadows.xl,
              )}
              initial={{ y: "100%", opacity: 0, scale: 0.95 }}
              animate={{ y: 0, opacity: 1, scale: 1 }}
              exit={{ y: "100%", opacity: 0, scale: 0.95 }}
              transition={{ type: "spring", damping: 30, stiffness: 350 }}
              onClick={(e) => e.stopPropagation()}
            >
              {/* Header */}
              <div className="mb-6 text-center">
                <div
                  className={cn(
                    "mx-auto mb-4 flex h-16 w-16 items-center justify-center border border-danger/40 bg-danger/15 text-danger/80",
                    radius.full,
                  )}
                >
                  <AlertTriangle size={28} strokeWidth={2.5} />
                </div>
                <h3 className={cn(typography.h2.size, typography.h2.weight, "mb-2 text-fg")}>
                  Are You Sure?
                </h3>
                <p className={cn(typography.body.size, "text-fg/50")}>
                  All your data will be permanently deleted. The app will return to first-time
                  setup.
                </p>
              </div>

              {/* Actions */}
              <div className="flex flex-col gap-3">
                <button
                  onClick={handleReset}
                  disabled={isResetting}
                  className={cn(
                    "w-full px-6 py-3.5",
                    radius.md,
                    typography.body.size,
                    typography.h3.weight,
                    "border border-danger/40 bg-danger/20 text-danger/90",
                    interactive.transition.fast,
                    "active:scale-[0.97] active:bg-danger/30",
                    "disabled:cursor-not-allowed disabled:opacity-50",
                  )}
                >
                  {isResetting ? (
                    <div className="flex items-center justify-center gap-2">
                      <div className="h-4 w-4 animate-spin rounded-full border-2 border-danger/30 border-t-danger/90" />
                      <span>Resetting...</span>
                    </div>
                  ) : (
                    "Yes, Reset Everything"
                  )}
                </button>
                <button
                  onClick={() => setShowConfirmModal(false)}
                  disabled={isResetting}
                  className={cn(
                    "w-full px-6 py-3",
                    radius.md,
                    typography.body.size,
                    "text-fg/60",
                    interactive.transition.fast,
                    "active:scale-[0.97] active:text-fg",
                    "disabled:cursor-not-allowed disabled:opacity-50",
                  )}
                >
                  Cancel
                </button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
