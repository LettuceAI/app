import { useRef, useState, type PointerEvent, type ReactNode } from "react";
import type { ChatWidgetLayout } from "../utils/chatWidgetLayout";
import type { WidgetNode } from "../../../../core/storage/schemas";
import { cn } from "../../../design-tokens";
import { useI18n } from "../../../../core/i18n/context";
import { WidgetList } from "./widgets";

const MIN_COLUMN_PX = 400;
const MAX_COLUMN_PX = 2400;

interface WidgetAreaPanelProps {
  side: "left" | "right";
  nodes: WidgetNode[];
  withBorder: boolean;
  canMove: boolean;
}

function WidgetAreaPanel({ side, nodes, withBorder, canMove }: WidgetAreaPanelProps) {
  const { t } = useI18n();
  return (
    <aside
      className={cn(
        "relative z-10 flex flex-1 basis-0 flex-col self-stretch",
        withBorder && (side === "left" ? "border-r border-fg/10" : "border-l border-fg/10"),
      )}
      style={{ minWidth: 0 }}
      aria-label={
        side === "left"
          ? t("chats.widgetArea.leftLabel")
          : t("chats.widgetArea.rightLabel")
      }
    >
      <WidgetList nodes={nodes} side={side} canMove={canMove} />
    </aside>
  );
}

interface ResizeHandleProps {
  onDrag: (clientX: number) => void;
  onStart: (clientX: number) => void;
  onEnd: () => void;
}

function ResizeHandle({ onDrag, onStart, onEnd }: ResizeHandleProps) {
  const [active, setActive] = useState(false);
  const handlePointerDown = (e: PointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    setActive(true);
    onStart(e.clientX);
  };
  return (
    <div
      role="separator"
      aria-orientation="vertical"
      onPointerDown={handlePointerDown}
      onPointerMove={(e) => {
        if (active) onDrag(e.clientX);
      }}
      onPointerUp={(e) => {
        if (!active) return;
        e.currentTarget.releasePointerCapture(e.pointerId);
        setActive(false);
        onEnd();
      }}
      className="group relative z-20 flex w-2 shrink-0 cursor-col-resize items-center justify-center self-stretch"
    >
      <span
        className={cn(
          "h-12 w-1 rounded-full transition-colors",
          active ? "bg-accent" : "bg-fg/20 group-hover:bg-fg/40",
        )}
      />
    </div>
  );
}

interface ChatWidgetAreaProps {
  widgetLayout: ChatWidgetLayout;
  leftNodes: WidgetNode[];
  rightNodes: WidgetNode[];
  resizable: boolean;
  viewportWidth: number;
  onResizeColumn: (px: number) => void;
  children: ReactNode;
}

export function ChatWidgetArea({
  widgetLayout,
  leftNodes,
  rightNodes,
  resizable,
  viewportWidth,
  onResizeColumn,
  children,
}: ChatWidgetAreaProps) {
  const [liveColumnPx, setLiveColumnPx] = useState<number | null>(null);
  const dragRef = useRef<{ startX: number; startPx: number; factor: number } | null>(null);

  if (!widgetLayout.enabled || widgetLayout.columnPx == null) {
    return <>{children}</>;
  }

  const baseColumnPx = widgetLayout.columnPx;
  const columnPx = liveColumnPx ?? baseColumnPx;
  const bothSides = widgetLayout.showLeft && widgetLayout.showRight;
  const factor = bothSides ? 2 : 1;
  const sidesShown = (widgetLayout.showLeft ? 1 : 0) + (widgetLayout.showRight ? 1 : 0);
  const maxPx = Math.max(
    MIN_COLUMN_PX,
    Math.min(MAX_COLUMN_PX, viewportWidth - sidesShown * widgetLayout.panelMinWidth),
  );

  const clamp = (px: number) => Math.max(MIN_COLUMN_PX, Math.min(maxPx, Math.round(px)));

  const startDrag = (clientX: number, sign: 1 | -1) => {
    dragRef.current = { startX: clientX, startPx: baseColumnPx, factor: factor * sign };
    setLiveColumnPx(baseColumnPx);
  };
  const onDrag = (clientX: number) => {
    const drag = dragRef.current;
    if (!drag) return;
    const dx = clientX - drag.startX;
    setLiveColumnPx(clamp(drag.startPx + drag.factor * dx));
  };
  const endDrag = () => {
    if (liveColumnPx != null && liveColumnPx !== baseColumnPx) {
      onResizeColumn(liveColumnPx);
    }
    dragRef.current = null;
    setLiveColumnPx(null);
  };

  return (
    <div className="relative z-10 flex min-h-0 flex-1 flex-row">
      {widgetLayout.showLeft && (
        <WidgetAreaPanel
          side="left"
          nodes={leftNodes}
          withBorder={!resizable}
          canMove={bothSides}
        />
      )}
      {widgetLayout.showLeft && resizable && (
        <ResizeHandle
          onStart={(x) => startDrag(x, -1)}
          onDrag={onDrag}
          onEnd={endDrag}
        />
      )}
      <div
        className="relative flex shrink-0 flex-col"
        style={{ width: columnPx, maxWidth: "100%" }}
      >
        {children}
      </div>
      {widgetLayout.showRight && resizable && (
        <ResizeHandle
          onStart={(x) => startDrag(x, 1)}
          onDrag={onDrag}
          onEnd={endDrag}
        />
      )}
      {widgetLayout.showRight && (
        <WidgetAreaPanel
          side="right"
          nodes={rightNodes}
          withBorder={!resizable}
          canMove={bothSides}
        />
      )}
    </div>
  );
}
