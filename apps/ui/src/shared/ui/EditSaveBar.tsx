import { AnimatePresence, motion } from "framer-motion";
import { Button, motion as m } from "@lettuceai/crisp";

export function EditSaveBar({ visible, phone, saving, canSave, discardDisabled, status, discardLabel, saveLabel, onDiscard, onSave }: {
  visible: boolean;
  phone: boolean;
  saving: boolean;
  canSave: boolean;
  discardDisabled: boolean;
  status: string;
  discardLabel: string;
  saveLabel: string;
  onDiscard: () => void;
  onSave: () => void;
}) {
  return (
    <AnimatePresence initial={false}>
      {visible && (
        <motion.div key="save" initial={{ y: 24, opacity: 0 }} animate={{ y: 0, opacity: 1 }} exit={{ y: 24, opacity: 0 }} transition={m.quick} className={`border-t border-line px-4 py-3 backdrop-blur ${phone ? "pb-[calc(0.75rem+var(--safe-bottom,0px))]" : "px-8"}`}>
          <div className="flex items-center gap-2">
            <span className={phone ? "sr-only" : "mr-auto text-sm text-fg-3"} role="status">{status}</span>
            <Button variant="ghost" className={phone ? "flex-1" : undefined} disabled={discardDisabled} onClick={onDiscard}>{discardLabel}</Button>
            <Button variant="primary" className={phone ? "flex-1" : undefined} loading={saving} disabled={!canSave} onClick={onSave}>{saveLabel}</Button>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
