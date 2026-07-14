import type { NavAlign, NavEdge, NavigationSide, NavigationStyle, NavItemId } from "../../../core/storage/schemas";
import { BottomNav } from "./BottomNav";
import { DockNav } from "./DockNav";
import { SidebarNav } from "./SidebarNav";

export function AppNav({
  style,
  side = "left",
  align = "start",
  edge = "bottom",
  onCreateClick,
  items,
}: {
  style: NavigationStyle;
  side?: NavigationSide;
  align?: NavAlign;
  edge?: NavEdge;
  onCreateClick: () => void;
  items?: readonly NavItemId[] | null;
}) {
  switch (style) {
    case "header":
      return null;
    case "sidebar":
      return <SidebarNav onCreateClick={onCreateClick} side={side} align={align} items={items} />;
    case "floatingSidebar":
      return <SidebarNav onCreateClick={onCreateClick} side={side} floating align={align} items={items} />;
    case "dock":
      return <DockNav onCreateClick={onCreateClick} align={align} edge={edge} items={items} />;
    case "bottomLabels":
      return <BottomNav onCreateClick={onCreateClick} showLabels items={items} />;
    default:
      return <BottomNav onCreateClick={onCreateClick} items={items} />;
  }
}
