import { Reorder, useDragControls } from "framer-motion";
import { ArrowLeftToLine, ArrowRightToLine, GripVertical, Pencil, Trash2 } from "lucide-react";
import type { WidgetNode } from "../../../../../core/storage/schemas";
import type { BoxNode } from "../../../../../core/storage/chatWidgetSchemas";
import { useI18n } from "../../../../../core/i18n/context";
import type { WidgetSide } from "./WidgetEditContext";
import { WidgetRenderer } from "./WidgetRenderer";
import { WidgetEditList } from "./WidgetEditList";

interface WidgetEditWrapperProps {
  node: WidgetNode;
  onEdit: () => void;
  onDelete: () => void;
  moveSide?: WidgetSide;
  onMove?: () => void;
  onChildrenChange?: (children: WidgetNode[]) => void;
}

export function WidgetEditWrapper({
  node,
  onEdit,
  onDelete,
  moveSide,
  onMove,
  onChildrenChange,
}: WidgetEditWrapperProps) {
  const { t } = useI18n();
  const controls = useDragControls();
  const isEditableBox = node.type === "box" && !!onChildrenChange;
  return (
    <Reorder.Item
      value={node}
      dragListener={false}
      dragControls={controls}
      dragMomentum={false}
      dragElastic={0}
      layout="position"
      initial={{ opacity: 0, y: 8, scale: 0.98 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, scale: 0.96, transition: { duration: 0.14 } }}
      whileDrag={{ zIndex: 30, boxShadow: "0 16px 32px rgba(0,0,0,0.28)" }}
      transition={{ layout: { duration: 0.18, ease: "easeOut" }, duration: 0.2, ease: "easeOut" }}
      className="relative rounded-xl border border-dashed border-fg/30 bg-black/30 p-2"
      style={{ position: "relative" }}
    >
      <div className="mb-1.5 flex items-center justify-between gap-2">
        <button
          type="button"
          onPointerDown={(e) => controls.start(e)}
          className="flex h-7 w-7 cursor-grab items-center justify-center rounded-md border border-fg/20 bg-surface-el text-fg/80 shadow-sm transition hover:bg-fg/15 hover:text-fg active:cursor-grabbing"
          aria-label={t("chats.widgets.wrapper.dragToReorder")}
        >
          <GripVertical size={14} />
        </button>
        <div className="flex items-center gap-1">
          {onMove && (
            <button
              type="button"
              onClick={onMove}
              className="flex h-7 w-7 items-center justify-center rounded-md border border-fg/20 bg-surface-el text-fg/80 shadow-sm transition hover:bg-fg/15 hover:text-fg"
              aria-label={moveSide === "left" ? t("chats.widgets.wrapper.moveRight") : t("chats.widgets.wrapper.moveLeft")}
            >
              {moveSide === "left" ? (
                <ArrowRightToLine size={13} strokeWidth={2.2} />
              ) : (
                <ArrowLeftToLine size={13} strokeWidth={2.2} />
              )}
            </button>
          )}
          <button
            type="button"
            onClick={onEdit}
            className="flex h-7 w-7 items-center justify-center rounded-md border border-fg/20 bg-surface-el text-fg/80 shadow-sm transition hover:bg-fg/15 hover:text-fg"
            aria-label={t("chats.widgets.wrapper.editWidget")}
          >
            <Pencil size={13} strokeWidth={2.2} />
          </button>
          <button
            type="button"
            onClick={onDelete}
            className="flex h-7 w-7 items-center justify-center rounded-md border border-fg/20 bg-surface-el text-fg/80 shadow-sm transition hover:border-danger/50 hover:bg-danger/25 hover:text-danger"
            aria-label={t("chats.widgets.wrapper.deleteWidget")}
          >
            <Trash2 size={13} strokeWidth={2.2} />
          </button>
        </div>
      </div>
      {isEditableBox ? (
        <BoxEditShell node={node as BoxNode} onChildrenChange={onChildrenChange!} />
      ) : (
        <div className="pointer-events-none select-none">
          <WidgetRenderer node={node} />
        </div>
      )}
    </Reorder.Item>
  );
}

function BoxEditShell({
  node,
  onChildrenChange,
}: {
  node: BoxNode;
  onChildrenChange: (children: WidgetNode[]) => void;
}) {
  return (
    <div className="flex flex-col gap-2 rounded-lg border border-fg/15 bg-surface-el/90 px-2.5 py-2.5 backdrop-blur-md">
      {(node.title || node.description) && (
        <header className="flex flex-col gap-0.5">
          {node.title && (
            <h3 className="text-sm font-semibold text-fg/75">{node.title}</h3>
          )}
          {node.description && (
            <p className="text-[11px] leading-snug text-fg/45">{node.description}</p>
          )}
        </header>
      )}
      <WidgetEditList nodes={node.children} onChange={onChildrenChange} nested />
    </div>
  );
}
