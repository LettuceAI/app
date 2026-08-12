import { motion } from "framer-motion";
import { LayoutPanelLeft, Plus } from "lucide-react";
import { useI18n } from "../../../../../core/i18n/context";

interface WidgetEmptyStateProps {
  editing: boolean;
  onAdd?: () => void;
}

export function WidgetEmptyState({ editing, onAdd }: WidgetEmptyStateProps) {
  const { t } = useI18n();
  return (
    <motion.div
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -6 }}
      transition={{ duration: 0.2, ease: "easeOut" }}
      className="mx-3 mb-4 flex flex-1 flex-col items-center justify-center gap-2 rounded-xl px-4 py-8 text-center"
    >
      <div className="flex h-9 w-9 items-center justify-center rounded-full bg-fg/5 text-fg/30">
        <LayoutPanelLeft size={16} strokeWidth={1.8} />
      </div>
      {editing ? (
        <>
          <p className="text-[12px] text-fg/45">{t("chats.widgets.empty.editingTitle")}</p>
          {onAdd && (
            <button
              type="button"
              onClick={onAdd}
              className="mt-1 flex items-center gap-1 rounded-md border border-accent/30 bg-accent/10 px-2.5 py-1 text-[11px] font-medium text-accent transition hover:bg-accent/20"
            >
              <Plus size={12} strokeWidth={2.4} />
              {t("chats.widgets.empty.addWidget")}
            </button>
          )}
        </>
      ) : (
        <>
          <p className="text-[12px] text-fg/40">{t("chats.widgets.empty.title")}</p>
          <p className="text-[11px] text-fg/30">{t("chats.widgets.empty.hint")}</p>
        </>
      )}
    </motion.div>
  );
}
