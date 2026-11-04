export type DrawerSide = "left" | "right";

export const SIDE_KEY = "chatAppearanceDrawer.side";

export function readSide(): DrawerSide {
  try {
    return window.localStorage.getItem(SIDE_KEY) === "left" ? "left" : "right";
  } catch {
    return "right";
  }
}

export function writeSide(side: DrawerSide) {
  try {
    window.localStorage.setItem(SIDE_KEY, side);
  } catch {
    return;
  }
}
