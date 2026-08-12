import { useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Check, Pencil, Plus, RotateCcw } from "lucide-react";
import type { WidgetNode } from "../../../../../core/storage/schemas";
import { useI18n } from "../../../../../core/i18n/context";
import { WidgetRenderer } from "./WidgetRenderer";
import { WidgetEditList } from "./WidgetEditList";
import { WidgetEmptyState } from "./WidgetEmptyState";
import { useWidgetEdit, type WidgetSide } from "./WidgetEditContext";
import { WidgetTypePickerSheet } from "./editor/WidgetTypePickerSheet";
import { createWidgetNode, filterAvailableWidgets } from "./editor/widgetFactories";
import { useWidgetAvailability } from "./WidgetContext";

interface WidgetListProps {
  nodes: WidgetNode[];
  side: WidgetSide;
  canMove: boolean;
}

export function WidgetList({ nodes, side, canMove }: WidgetListProps) {
  const edit = useWidgetEdit();
  const availability = useWidgetAvailability();
  const [topPickerOpen, setTopPickerOpen] = useState(false);
  const displayNodes = edit.editing
    ? edit.getNodes(side)
    : filterAvailableWidgets(nodes, availability);

  return (
    <div className="flex h-full w-full flex-col overflow-y-auto">
      {edit.editing && (
        <Toolbar
          saving={edit.saving}
          onAdd={() => setTopPickerOpen(true)}
          onRevert={edit.revert}
          onDone={edit.done}
        />
      )}
      <WidgetTypePickerSheet
        open={topPickerOpen}
        onClose={() => setTopPickerOpen(false)}
        onPick={(type) => edit.addNode(side, createWidgetNode(type))}
      />

      {!edit.editing && (
        <div className="px-3 pb-3 pt-3">
          <EditWidgetsButton onClick={edit.enterEdit} />
        </div>
      )}

      {edit.editing ? (
        <WidgetEditList
          nodes={displayNodes}
          onChange={(next) => edit.setNodes(side, next)}
          side={canMove ? side : undefined}
        />
      ) : displayNodes.length === 0 ? (
        <AnimatePresence>
          <WidgetEmptyState key="empty" editing={false} />
        </AnimatePresence>
      ) : (
        <div className="flex flex-col gap-3 px-3 pb-4">
          <AnimatePresence initial={false} mode="popLayout">
            {displayNodes.map((node) => (
              <motion.div
                key={node.id}
                layout
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -6, transition: { duration: 0.14 } }}
                transition={{ duration: 0.22, ease: "easeOut" }}
              >
                <WidgetRenderer node={node} />
              </motion.div>
            ))}
          </AnimatePresence>
        </div>
      )}

      {!edit.editing && displayNodes.length > 0 && (
        <div className="mt-auto px-3 pb-4 pt-2">
          <EditWidgetsButton onClick={edit.enterEdit} />
        </div>
      )}
    </div>
  );
}

function EditWidgetsButton({ onClick }: { onClick: () => void }) {
  const { t } = useI18n();
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex w-full items-center justify-center gap-1.5 rounded-lg border border-dashed border-fg/15 py-2 text-[11px] font-medium text-fg/50 transition hover:border-fg/30 hover:text-fg/75"
    >
      <Pencil size={12} strokeWidth={2.2} />
      {t("chats.widgets.list.editWidgets")}
    </button>
  );
}

interface ToolbarProps {
  saving: boolean;
  onAdd: () => void;
  onRevert: () => void;
  onDone: () => void;
}

function Toolbar({ saving, onAdd, onRevert, onDone }: ToolbarProps) {
  const { t } = useI18n();
  return (
    <div className="sticky top-0 z-20 flex min-h-[44px] items-center justify-end gap-2 border-b border-fg/10 bg-surface/95 px-2 py-2 backdrop-blur-md">
      <motion.div
        initial={{ opacity: 0, y: -4 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.16, ease: "easeOut" }}
        className="flex w-full items-center justify-between gap-1"
      >
        <button
          type="button"
          onClick={onAdd}
          className="flex items-center gap-1 rounded-md border border-accent/30 bg-accent/10 px-2.5 py-1.5 text-[11px] font-medium text-accent shadow-sm transition hover:bg-accent/20"
        >
          <Plus size={12} strokeWidth={2.4} />
          {t("common.buttons.add")}
        </button>
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={onRevert}
            disabled={saving}
            className="flex items-center gap-1 rounded-md border border-fg/20 bg-surface-el px-2.5 py-1.5 text-[11px] text-fg/80 shadow-sm transition hover:bg-fg/15 disabled:opacity-50"
          >
            <RotateCcw size={11} strokeWidth={2.2} />
            {t("chats.widgets.list.revert")}
          </button>
          <button
            type="button"
            onClick={onDone}
            disabled={saving}
            className="flex items-center gap-1 rounded-md bg-accent px-2.5 py-1.5 text-[11px] font-semibold text-black shadow-sm transition hover:brightness-110 disabled:opacity-50"
          >
            <Check size={12} strokeWidth={2.4} />
            {t("common.buttons.done")}
          </button>
        </div>
      </motion.div>
    </div>
  );
}
