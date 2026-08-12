import type {
  BoxNode,
  BoxVariant,
} from "../../../../../core/storage/chatWidgetSchemas";
import { cn } from "../../../../design-tokens";
import { WidgetRenderer } from "./WidgetRenderer";
import { useWidgetContext } from "./WidgetContext";

interface WidgetBoxProps {
  node: BoxNode;
}

const VARIANT_STYLES: Record<BoxVariant, string> = {
  default: "border-fg/10 bg-fg/5",
  subtle: "border-fg/8 bg-fg/[0.03]",
  info: "border-info/30 bg-info/10",
  warning: "border-warning/30 bg-warning/10",
  success: "border-accent/30 bg-accent/10",
  danger: "border-danger/30 bg-danger/10",
};

const VARIANT_STYLES_BG: Record<BoxVariant, string> = {
  default: "border-fg/12 bg-surface-el/80",
  subtle: "border-fg/10 bg-surface-el/60",
  info: "border-info/40 bg-info/25",
  warning: "border-warning/40 bg-warning/25",
  success: "border-accent/40 bg-accent/25",
  danger: "border-danger/40 bg-danger/25",
};

const VARIANT_TITLE: Record<BoxVariant, string> = {
  default: "text-fg/75",
  subtle: "text-fg/55",
  info: "text-info",
  warning: "text-warning",
  success: "text-accent",
  danger: "text-danger",
};

export function WidgetBox({ node }: WidgetBoxProps) {
  const { hasBackground } = useWidgetContext();
  const variant: BoxVariant = node.variant ?? "default";
  return (
    <section
      className={cn(
        "flex flex-col gap-2 rounded-xl border px-3 py-3",
        hasBackground
          ? cn(VARIANT_STYLES_BG[variant], "backdrop-blur-md")
          : VARIANT_STYLES[variant],
      )}
    >
      {(node.title || node.description) && (
        <header className="flex flex-col gap-0.5">
          {node.title && (
            <h3 className={`text-sm font-semibold ${VARIANT_TITLE[variant]}`}>
              {node.title}
            </h3>
          )}
          {node.description && (
            <p className="text-[11px] leading-snug text-fg/45">{node.description}</p>
          )}
        </header>
      )}
      {node.children.length > 0 && (
        <div className="flex flex-col gap-2">
          {node.children.map((child) => (
            <WidgetRenderer key={child.id} node={child} />
          ))}
        </div>
      )}
    </section>
  );
}
