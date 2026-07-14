import { Compass, Library, MessageCircle, Users } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import type { TranslationKey } from "../../../core/i18n/context";

export type NavDestination = {
  id: string;
  to: string;
  icon: LucideIcon;
  labelKey: TranslationKey;
  isActive: (pathname: string) => boolean;
  dataTourId: string;
};

export const NAV_DESTINATIONS: readonly NavDestination[] = [
  {
    id: "chats",
    to: "/chat",
    icon: MessageCircle,
    labelKey: "common.bottomNav.chats",
    isActive: (pathname) => pathname === "/" || pathname.startsWith("/chat"),
    dataTourId: "nav-chats",
  },
  {
    id: "groups",
    to: "/group-chats",
    icon: Users,
    labelKey: "common.bottomNav.groups",
    isActive: (pathname) => pathname.startsWith("/group-chats"),
    dataTourId: "nav-groups",
  },
  {
    id: "discover",
    to: "/discover",
    icon: Compass,
    labelKey: "common.bottomNav.discover",
    isActive: (pathname) => pathname.startsWith("/discover"),
    dataTourId: "nav-discover",
  },
  {
    id: "library",
    to: "/library",
    icon: Library,
    labelKey: "common.bottomNav.library",
    isActive: (pathname) => pathname.startsWith("/library"),
    dataTourId: "nav-library",
  },
];

export const NAV_LEADING_DESTINATIONS = NAV_DESTINATIONS.slice(0, 2);
export const NAV_TRAILING_DESTINATIONS = NAV_DESTINATIONS.slice(2);

export function resolveCreateAction(pathname: string, fallback: () => void): void {
  if (typeof window !== "undefined") {
    const globalWindow = window as any;
    if (pathname.startsWith("/settings/providers")) {
      if (typeof globalWindow.__openAddProvider === "function") {
        globalWindow.__openAddProvider();
      } else {
        window.dispatchEvent(new CustomEvent("providers:add"));
      }
      return;
    }

    if (pathname.startsWith("/settings/models")) {
      if (typeof globalWindow.__openAddModel === "function") {
        globalWindow.__openAddModel();
      } else {
        window.dispatchEvent(new CustomEvent("models:add"));
      }
      return;
    }

    if (pathname.startsWith("/settings/prompts")) {
      if (typeof globalWindow.__openAddPromptTemplate === "function") {
        globalWindow.__openAddPromptTemplate();
      } else {
        window.dispatchEvent(new CustomEvent("prompts:add"));
      }
      return;
    }
  }

  fallback();
}
