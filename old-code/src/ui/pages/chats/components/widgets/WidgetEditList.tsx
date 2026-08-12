import { useEffect, useState } from "react";
import { AnimatePresence, Reorder } from "framer-motion";
import { Plus } from "lucide-react";
import type { WidgetNode } from "../../../../../core/storage/schemas";
import { cn } from "../../../../design-tokens";
import { useI18n } from "../../../../../core/i18n/context";
import { WidgetEditWrapper } from "./WidgetEditWrapper";
import { WidgetEmptyState } from "./WidgetEmptyState";
import { useWidgetEdit, type WidgetSide } from "./WidgetEditContext";
import { WidgetTypePickerSheet } from "./editor/WidgetTypePickerSheet";
import { WidgetConfigSheet } from "./editor/WidgetConfigSheet";
import { createWidgetNode } from "./editor/widgetFactories";

interface WidgetEditListProps {
  nodes: WidgetNode[];
  onChange: (nodes: WidgetNode[]) => void;
  nested?: boolean;
  side?: WidgetSide;
}

export function WidgetEditList({ nodes, onChange, nested, side }: WidgetEditListProps) {
  const { t } = useI18n();
  const { pendingOpenNodeId, clearPendingOpen, moveToOtherSlot } = useWidgetEdit();
  const [pickerOpen, setPickerOpen] = useState(false);
  const [editingNode, setEditingNode] = useState<WidgetNode | null>(null);

  useEffect(() => {
    if (!pendingOpenNodeId) return;
    const match = nodes.find((n) => n.id === pendingOpenNodeId);
    if (match) {
      setEditingNode(match);
      clearPendingOpen();
    }
  }, [pendingOpenNodeId, nodes, clearPendingOpen]);

  const updateNode = (next: WidgetNode) =>
    onChange(nodes.map((n) => (n.id === next.id ? next : n)));

  return (
    <div
      className={cn(
        "flex flex-col gap-3",
        !nested && "min-h-0 flex-1 px-3 pb-4",
      )}
    >
      <Reorder.Group
        axis="y"
        values={nodes}
        onReorder={onChange}
        className="flex flex-col gap-3"
      >
        <AnimatePresence initial={false}>
          {nodes.map((node) => (
            <WidgetEditWrapper
              key={node.id}
              node={node}
              onEdit={() => setEditingNode(node)}
              onDelete={() => onChange(nodes.filter((n) => n.id !== node.id))}
              moveSide={side}
              onMove={side ? () => moveToOtherSlot(side, node.id) : undefined}
              onChildrenChange={
                node.type === "box"
                  ? (children) => updateNode({ ...node, children })
                  : undefined
              }
            />
          ))}
        </AnimatePresence>
      </Reorder.Group>

      {nodes.length === 0 && !nested ? (
        <WidgetEmptyState editing onAdd={() => setPickerOpen(true)} />
      ) : (
        <button
          type="button"
          onClick={() => setPickerOpen(true)}
          className={cn(
            "flex items-center justify-center gap-1 rounded-lg border border-dashed border-fg/25 bg-surface-el/70 py-2 text-[11px] font-medium text-fg/70 backdrop-blur-md transition hover:border-accent/50 hover:bg-surface-el hover:text-accent",
            nested && "py-1.5",
          )}
        >
          <Plus size={12} strokeWidth={2.4} />
          {t("chats.widgets.list.addWidget")}
        </button>
      )}

      <WidgetTypePickerSheet
        open={pickerOpen}
        onClose={() => setPickerOpen(false)}
        onPick={(type) => onChange([...nodes, createWidgetNode(type)])}
      />
      <WidgetConfigSheet
        open={editingNode !== null}
        node={editingNode}
        onClose={() => setEditingNode(null)}
        onSave={updateNode}
      />
    </div>
  );
}
