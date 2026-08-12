import { useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { useI18n } from "../../../../core/i18n/context";
import { GroupChatSettingsPage } from "../GroupChatSettingsPage";

interface GroupChatSettingsDrawerProps {
  isOpen: boolean;
  onClose: () => void;
  groupSessionId: string;
}

export function GroupChatSettingsDrawer({
  isOpen,
  onClose,
  groupSessionId,
}: GroupChatSettingsDrawerProps) {
  const { t } = useI18n();

  useEffect(() => {
    if (!isOpen) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [isOpen, onClose]);

  return (
    <AnimatePresence>
      {isOpen && (
        <>
          <motion.div
            className="fixed inset-0 z-40 bg-black/50"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
            onClick={onClose}
          />

          <motion.aside
            className="fixed bottom-0 right-0 top-[var(--titlebar-h,0px)] z-50 flex w-[640px] max-w-[90vw] flex-col border-l border-fg/10 bg-surface shadow-2xl"
            initial={{ x: "100%" }}
            animate={{ x: 0 }}
            exit={{ x: "100%" }}
            transition={{ type: "spring", damping: 28, stiffness: 320 }}
          >
            <div
              className="flex shrink-0 items-center justify-between border-b border-fg/10 px-4 py-3"
            >
              <div>
                <p className="text-base font-bold text-fg">{t("common.nav.settings")}</p>
                <p className="text-xs text-fg/50">{t("groupChats.sessionSettings.subtitle")}</p>
              </div>
              <div className="flex items-center gap-1">
              </div>
            </div>

            <div className="flex-1 overflow-hidden">
              <GroupChatSettingsPage
                mode="drawer"
                onClose={onClose}
                groupSessionId={groupSessionId}
              />
            </div>
          </motion.aside>
        </>
      )}
    </AnimatePresence>
  );
}
