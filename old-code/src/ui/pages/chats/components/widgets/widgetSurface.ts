import type { WidgetDesign } from "../../../../../core/storage/chatWidgetSchemas";

export function widgetCardClass(
  hasBackground: boolean,
  design: WidgetDesign = "default",
): string {
  switch (design) {
    case "minimal":
      return "border border-transparent bg-transparent";
    case "outline":
      return "border border-fg/25 bg-transparent";
    case "solid":
      return hasBackground
        ? "border border-transparent bg-surface-el/95 backdrop-blur-md"
        : "border border-transparent bg-surface-el";
    default:
      return hasBackground
        ? "border border-fg/12 bg-surface-el/80 backdrop-blur-md"
        : "border border-fg/10 bg-fg/5";
  }
}
