import type { NavigationSide, NavigationStyle } from "../../../core/storage/schemas";
import { BottomNav } from "./BottomNav";
import { DockNav } from "./DockNav";
import { SidebarNav } from "./SidebarNav";

export function AppNav({
  style,
  side = "left",
  onCreateClick,
}: {
  style: NavigationStyle;
  side?: NavigationSide;
  onCreateClick: () => void;
}) {
  switch (style) {
    case "sidebar":
      return <SidebarNav onCreateClick={onCreateClick} side={side} />;
    case "floatingSidebar":
      return <SidebarNav onCreateClick={onCreateClick} side={side} floating />;
    case "dock":
      return <DockNav onCreateClick={onCreateClick} />;
    case "bottomLabels":
      return <BottomNav onCreateClick={onCreateClick} showLabels />;
    default:
      return <BottomNav onCreateClick={onCreateClick} />;
  }
}
