import { z } from "zod";

import {
  HeaderStyleSchema,
  NavigationSideSchema,
  NavigationStyleSchema,
  NavItemIdSchema,
  NavAlignSchema,
  NavEdgeSchema,
  type HeaderStyle,
  type NavigationSide,
  type NavigationStyle,
  type NavItemId,
  type NavAlign,
  type NavEdge,
} from "../../../core/storage/schemas";

const STYLE_KEY = "appnav.style";
const SIDE_KEY = "appnav.side";
const HEADER_KEY = "appnav.header";
const ITEMS_KEY = "appnav.items";
const ALIGN_KEY = "appnav.align";
const EDGE_KEY = "appnav.edge";

const NavItemsSchema = z.array(NavItemIdSchema);

export const CONTENT_COLUMN_MAX_W = "max-w-[1600px]";
export const CONTENT_COLUMN_MAX_W_LG = "lg:max-w-[1600px]";
export const CONTENT_INNER_MAX_W_LG = "lg:max-w-[1536px]";

export const CONTENT_COLUMN_ROUTES: readonly string[] = [
  "/",
  "/chat",
  "/group-chats",
  "/library",
];

export type NavPrefs = {
  style: NavigationStyle;
  side: NavigationSide;
  header: HeaderStyle;
  items: NavItemId[] | null;
  align: NavAlign;
  edge: NavEdge;
};

export function readCachedNavPrefs(): NavPrefs {
  const fallback: NavPrefs = {
    style: "bottom",
    side: "left",
    header: "auto",
    items: null,
    align: "start",
    edge: "bottom",
  };
  if (typeof window === "undefined") return fallback;
  try {
    const style = NavigationStyleSchema.safeParse(window.localStorage.getItem(STYLE_KEY));
    const side = NavigationSideSchema.safeParse(window.localStorage.getItem(SIDE_KEY));
    const header = HeaderStyleSchema.safeParse(window.localStorage.getItem(HEADER_KEY));
    const align = NavAlignSchema.safeParse(window.localStorage.getItem(ALIGN_KEY));
    const edge = NavEdgeSchema.safeParse(window.localStorage.getItem(EDGE_KEY));
    const rawItems = window.localStorage.getItem(ITEMS_KEY);
    const items = rawItems ? NavItemsSchema.safeParse(JSON.parse(rawItems)) : null;
    return {
      style: style.success ? style.data : fallback.style,
      side: side.success ? side.data : fallback.side,
      header: header.success ? header.data : fallback.header,
      items: items && items.success ? items.data : null,
      align: align.success ? align.data : fallback.align,
      edge: edge.success ? edge.data : fallback.edge,
    };
  } catch {
    return fallback;
  }
}

export function writeCachedNavPrefs(prefs: NavPrefs): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(STYLE_KEY, prefs.style);
    window.localStorage.setItem(SIDE_KEY, prefs.side);
    window.localStorage.setItem(HEADER_KEY, prefs.header);
    window.localStorage.setItem(ALIGN_KEY, prefs.align);
    window.localStorage.setItem(EDGE_KEY, prefs.edge);
    if (prefs.items && prefs.items.length > 0) {
      window.localStorage.setItem(ITEMS_KEY, JSON.stringify(prefs.items));
    } else {
      window.localStorage.removeItem(ITEMS_KEY);
    }
  } catch {
    /* ignore */
  }
}
