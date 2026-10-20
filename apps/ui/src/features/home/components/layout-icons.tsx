import type { ReactNode } from "react";
import { History, List } from "lucide-react";
import { icon } from "@lettuceai/crisp";
import type { PhoneLayout } from "../data/layout";

export const layoutIcons: Record<PhoneLayout, ReactNode> = {
  list: <List size={icon.md} />,
  recent: <History size={icon.md} />,
};
