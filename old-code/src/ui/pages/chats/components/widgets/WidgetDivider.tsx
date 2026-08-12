import type { DividerNode } from "../../../../../core/storage/chatWidgetSchemas";

interface WidgetDividerProps {
  node: DividerNode;
}

export function WidgetDivider({ node }: WidgetDividerProps) {
  if (node.style === "space") {
    return <div aria-hidden className="h-4" />;
  }
  return <div aria-hidden className="h-px w-full bg-fg/10" />;
}
