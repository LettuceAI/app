import type { ReactNode } from "react";
import { GalleryVerticalEnd, History, LayoutGrid, List } from "lucide-react";
import { icon } from "@lettuceai/crisp";
import type { PhoneLayout } from "../data/layout";

export const layoutIcons: Record<PhoneLayout, ReactNode> = {
  list: <List size={icon.md} />,
  gallery: <LayoutGrid size={icon.md} />,
  hero: <GalleryVerticalEnd size={icon.md} />,
  recent: <History size={icon.md} />,
};
