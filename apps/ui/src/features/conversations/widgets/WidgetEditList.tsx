import { ArrowLeftToLine, ArrowRightToLine, Pencil, Plus, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, EmptyState, icon, IconButton, ReorderableList } from "@lettuceai/crisp";
import { WidgetCard } from "./WidgetCard";
import type { WidgetNode } from "./nodes";
import { WidgetRenderer } from "./WidgetRenderer";

export interface WidgetEditListProps {
  nodes: WidgetNode[];
  onChange: (nodes: WidgetNode[]) => void;
  onEdit: (node: WidgetNode) => void;
  /** Asks for a new widget; the owner hands the picked one to `add`. */
  onAdd: (add: (node: WidgetNode) => void) => void;
  /** Set on a panel's own list when both sides show: moves a widget to the other side. */
  move?: { side: "left" | "right"; onMove: (node: WidgetNode) => void };
  nested?: boolean;
}

/** One panel's widgets in edit mode: each can be reordered by its handle, edited, deleted and, between sides, moved. Boxes edit their children in place. */
export function WidgetEditList({ nodes, onChange, onEdit, onAdd, move, nested }: WidgetEditListProps) {
  const { t } = useTranslation("conversations");
  const add = (
    <Button variant="ghost" size="sm" block leading={<Plus size={icon.sm} />} className="border border-dashed border-line-2" onClick={() => onAdd((node) => onChange([...nodes, node]))}>
      {t("widgets.list.addWidget")}
    </Button>
  );
  if (nodes.length === 0 && !nested) {
    return (
      <EmptyState
        size="sm"
        title={t("widgets.empty.editingTitle")}
        action={<Button variant="secondary" size="sm" onClick={() => onAdd((node) => onChange([...nodes, node]))}>{t("widgets.empty.addWidget")}</Button>}
      />
    );
  }
  return (
    <div className="space-y-2">
      <ReorderableList
        items={nodes}
        onReorder={onChange}
        keyOf={(node) => node.id}
        aria-label={t("widgets.list.editWidgets")}
        itemClassName="rounded-xl"
      >
        {(node, handle) => (
          <div className="rounded-xl border border-dashed border-line-2 p-2">
            <div className="mb-1.5 flex items-center justify-between gap-1">
              {handle}
              <div className="flex items-center gap-0.5">
                {move && (
                  <IconButton size="sm" label={t(move.side === "left" ? "widgets.wrapper.moveRight" : "widgets.wrapper.moveLeft")} onClick={() => move.onMove(node)}>
                    {move.side === "left" ? <ArrowRightToLine size={icon.sm} /> : <ArrowLeftToLine size={icon.sm} />}
                  </IconButton>
                )}
                <IconButton size="sm" label={t("widgets.wrapper.editWidget")} onClick={() => onEdit(node)}>
                  <Pencil size={icon.sm} />
                </IconButton>
                <IconButton size="sm" label={t("widgets.wrapper.deleteWidget")} className="hover:text-danger" onClick={() => onChange(nodes.filter((entry) => entry.id !== node.id))}>
                  <Trash2 size={icon.sm} />
                </IconButton>
              </div>
            </div>
            {node.type === "box" ? (
              <WidgetCard tint={node.variant ?? "default"} title={node.title} description={node.description}>
                <WidgetEditList
                  nested
                  nodes={node.children}
                  onChange={(children) => onChange(nodes.map((entry) => (entry.id === node.id ? { ...node, children } : entry)))}
                  onEdit={onEdit}
                  onAdd={onAdd}
                />
              </WidgetCard>
            ) : (
              <div inert className="pointer-events-none select-none">
                <WidgetRenderer node={node} />
              </div>
            )}
          </div>
        )}
      </ReorderableList>
      {add}
    </div>
  );
}
