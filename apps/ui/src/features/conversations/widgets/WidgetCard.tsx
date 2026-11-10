import { createContext, useContext, type ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { cn, icon, useOnSurface } from "@lettuceai/crisp";
import type { LibraryBoxVariant, LibraryWidgetDesign } from "@/api/generated/bindings";

const InsideBox = createContext(false);

/** Widgets inside a box drop their own frame, so a box never holds cards. */
export const BoxScope = InsideBox.Provider;

const tints: Record<LibraryBoxVariant, string> = {
  default: "",
  subtle: "border-line bg-fill",
  info: "border-info/30 bg-info/10",
  warning: "border-warning/30 bg-warning/10",
  success: "border-accent/30 bg-accent/10",
  danger: "border-danger/30 bg-danger/10",
};

function designFrame(design: LibraryWidgetDesign | null | undefined, hasBackground: boolean, tone: "light" | "dark"): string {
  switch (design) {
    case "minimal":
      return "border-transparent bg-transparent";
    case "outline":
      return "border-line-3 bg-transparent";
    case "solid":
      return "border-transparent bg-surface-el";
    default:
      if (!hasBackground) return "border-line bg-fill";
      return tone === "light" ? "border-black/10 bg-white/55" : "border-white/10 bg-black/40";
  }
}

export interface WidgetCardProps {
  design?: LibraryWidgetDesign | null;
  tint?: LibraryBoxVariant | null;
  title?: ReactNode;
  description?: ReactNode;
  icon?: LucideIcon;
  trailing?: ReactNode;
  /** Keep the frame even inside a box. */
  framed?: boolean;
  /** The body is a row that brings its own padding. */
  flush?: boolean;
  className?: string;
  children?: ReactNode;
}

/**
 * The card every widget sits in: a title with one small icon, and the body. On a background image it turns to glass like the message bubbles; `design` picks the frame and `tint` colours a box.
 */
export function WidgetCard({ design, tint, title, description, icon: Icon, trailing, flush, framed, className, children }: WidgetCardProps) {
  const { hasBackground, tone } = useOnSurface();
  const nested = useContext(InsideBox) && !framed;
  const heading = (title || trailing || description) && (
    <div className={cn("min-w-0", flush ? "px-4 pt-3" : "mb-2.5")}>
      {(title || trailing) && (
        <header className="flex items-center gap-2">
          {Icon && <Icon size={icon.sm} className="shrink-0 text-fg-3" aria-hidden />}
          {title && <h3 className="min-w-0 flex-1 truncate text-sm font-semibold text-fg-2">{title}</h3>}
          {trailing}
        </header>
      )}
      {description && <p className="mt-0.5 text-2xs text-fg-3">{description}</p>}
    </div>
  );
  if (nested) {
    return (
      <section className={className}>
        {heading}
        {children}
      </section>
    );
  }
  const frame = (tint ? tints[tint] : "") || designFrame(design, hasBackground, tone);
  const glass = hasBackground && (design === undefined || design === null || design === "default");
  return (
    <section className={cn("rounded-xl border", flush ? "overflow-hidden" : "px-3 py-3", glass && "backdrop-blur-md", frame, className)}>
      {heading}
      {children}
    </section>
  );
}
