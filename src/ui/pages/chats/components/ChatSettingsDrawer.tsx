import { useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import type { Character } from "../../../../core/storage/schemas";
import { ChatSettingsContent } from "../ChatSettings";
import { WindowControlButtons, useDragRegionProps } from "../../../components/App/TopNav";

interface ChatSettingsDrawerProps {
  isOpen: boolean;
  onClose: () => void;
  character: Character;
  onOpenAuthorNote?: () => void;
}

export function ChatSettingsDrawer({
  isOpen,
  onClose,
  character,
  onOpenAuthorNote,
}: ChatSettingsDrawerProps) {
  const dragRegionProps = useDragRegionProps();
  // Close on Escape
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
          {/* Backdrop */}
          <motion.div
            className="fixed inset-0 z-40 bg-black/50"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
            onClick={onClose}
          />

          {/* Drawer panel */}
          <motion.aside
            className="fixed inset-y-0 right-0 z-50 flex w-[640px] max-w-[90vw] flex-col border-l border-fg/10 bg-surface shadow-2xl"
            initial={{ x: "100%" }}
            animate={{ x: 0 }}
            exit={{ x: "100%" }}
            transition={{ type: "spring", damping: 28, stiffness: 320 }}
          >
            {/* Drawer header */}
            <div className="flex shrink-0 items-center justify-between border-b border-fg/10 px-4 py-3" {...dragRegionProps}>
              <div>
                <p className="text-base font-bold text-fg">Chat Settings</p>
                <p className="text-xs text-fg/50">Manage conversation preferences</p>
              </div>
              <div className="flex items-center gap-1">
                <WindowControlButtons />
              </div>
            </div>

            {/* Settings content */}
            <div className="flex-1 overflow-hidden">
              <ChatSettingsContent
                character={character}
                mode="drawer"
                onClose={onClose}
                onOpenAuthorNote={onOpenAuthorNote}
              />
            </div>
          </motion.aside>
        </>
      )}
    </AnimatePresence>
  );
}
