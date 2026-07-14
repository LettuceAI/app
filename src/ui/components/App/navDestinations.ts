import { Compass, Library, MessageCircle, Search, Settings, Users } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import type { TranslationKey } from "../../../core/i18n/context";
import { DEFAULT_NAV_ITEMS, type NavItemId } from "../../../core/storage/schemas";

export type NavDestination = {
  id: NavItemId;
  to: string;
  icon: LucideIcon;
  labelKey: TranslationKey;
  isActive: (pathname: string) => boolean;
  dataTourId?: string;
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
  {
    id: "search",
    to: "/search",
    icon: Search,
    labelKey: "topNav.search",
    isActive: (pathname) => pathname === "/search",
  },
  {
    id: "settings",
    to: "/settings",
    icon: Settings,
    labelKey: "common.nav.settings",
    isActive: (pathname) => pathname.startsWith("/settings"),
  },
];

export type NavEntry =
  | { kind: "destination"; destination: NavDestination }
  | { kind: "create" };

export function resolveNavEntries(ids?: readonly NavItemId[] | null): NavEntry[] {
  const source = ids && ids.length > 0 ? ids : DEFAULT_NAV_ITEMS;
  const seen = new Set<NavItemId>();
  const entries: NavEntry[] = [];
  for (const id of source) {
    if (seen.has(id)) continue;
    seen.add(id);
    if (id === "create") {
      entries.push({ kind: "create" });
      continue;
    }
    const destination = NAV_DESTINATIONS.find((entry) => entry.id === id);
    if (destination) entries.push({ kind: "destination", destination });
  }
  if (!seen.has("create")) {
    entries.push({ kind: "create" });
  }
  return entries;
}

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
