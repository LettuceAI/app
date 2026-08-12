import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Copy, Minus, Plus, Square, X } from "lucide-react";
import { getCurrentWindow, PhysicalSize } from "@tauri-apps/api/window";

type ResizeDirection =
  | "East"
  | "North"
  | "NorthEast"
  | "NorthWest"
  | "South"
  | "SouthEast"
  | "SouthWest"
  | "West";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import { getPlatform } from "../../../core/utils/platform";

const appPlatform = getPlatform();
const isDesktop = appPlatform.type === "desktop";
const isMacOS = appPlatform.os === "macos";

export const dragRegionAttr: Record<string, string> = isDesktop
  ? { "data-tauri-drag-region": "" }
  : {};

export type TitleBarDesign =
  | "classic"
  | "lights"
  | "lights_dimmed"
  | "minimal"
  | "native"
  | "hidden";
export type TitleBarSide = "left" | "right";
export type TitleBarSize = "small" | "medium" | "large";
export type WindowCorners = "off" | "small" | "medium" | "large";

const SELECTABLE_DESIGNS: TitleBarDesign[] = [
  "classic",
  "lights",
  "lights_dimmed",
  "minimal",
  "native",
];
const TITLE_BAR_SIZES: TitleBarSize[] = ["small", "medium", "large"];
const WINDOW_CORNERS: WindowCorners[] = ["off", "small", "medium", "large"];
const CORNER_RADII: Record<WindowCorners, string> = {
  off: "0px",
  small: "8px",
  medium: "12px",
  large: "16px",
};
const DESIGN_KEY = "lettuce.titleBarDesign";
const SIDE_KEY = "lettuce.titleBarSide";
const SIZE_KEY = "lettuce.titleBarSize";
const CORNERS_KEY = "lettuce.windowCorners";
const CHROME_EVENT = "lettuce:titleBarChrome";

export function readTitleBarDesign(): TitleBarDesign {
  try {
    const stored = localStorage.getItem(DESIGN_KEY) as TitleBarDesign | null;
    if (stored && SELECTABLE_DESIGNS.includes(stored)) return stored;
  } catch {
    // localStorage unavailable; fall through to default
  }
  return "classic";
}

export function readTitleBarSide(): TitleBarSide {
  try {
    const stored = localStorage.getItem(SIDE_KEY);
    if (stored === "left" || stored === "right") return stored;
  } catch {
    // localStorage unavailable; fall through to default
  }
  return "right";
}

export function readTitleBarSize(): TitleBarSize {
  try {
    const stored = localStorage.getItem(SIZE_KEY) as TitleBarSize | null;
    if (stored && TITLE_BAR_SIZES.includes(stored)) return stored;
  } catch {
    // localStorage unavailable; fall through to default
  }
  return "medium";
}

export function readWindowCorners(): WindowCorners {
  try {
    const stored = localStorage.getItem(CORNERS_KEY) as WindowCorners | null;
    if (stored && WINDOW_CORNERS.includes(stored)) return stored;
  } catch {
    // localStorage unavailable; fall through to default
  }
  return "medium";
}

async function applyDecorations(design: TitleBarDesign) {
  if (!isDesktop) return;
  if (isMacOS) return;
  try {
    await getCurrentWindow().setDecorations(design === "native");
  } catch {
    // window may be tearing down
  }
}

function persistChrome(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // localStorage unavailable; value applies for this session only
  }
  window.dispatchEvent(new CustomEvent(CHROME_EVENT));
}

export function setTitleBarDesign(design: TitleBarDesign) {
  if (design === "native") {
    document.documentElement.classList.remove("window-rounded");
    document.body.style.clipPath = "";
  }
  persistChrome(DESIGN_KEY, design);
  void applyDecorations(design);
}

export function setTitleBarSide(side: TitleBarSide) {
  persistChrome(SIDE_KEY, side);
}

export function setTitleBarSize(size: TitleBarSize) {
  persistChrome(SIZE_KEY, size);
}

export function setWindowCorners(corners: WindowCorners) {
  persistChrome(CORNERS_KEY, corners);
}

// WebKitGTK keeps stale opaque surface pixels when clip-path or background
// transparency changes at runtime; two spaced-out resizes force the surface to
// rebuild (back-to-back setSize calls get coalesced by GTK into a no-op).
async function nudgeWindowSurface() {
  if (!isDesktop) return;
  try {
    const win = getCurrentWindow();
    if (await win.isMaximized()) return;
    const size = await win.innerSize();
    await win.setSize(new PhysicalSize(size.width, size.height + 1));
    await new Promise((resolve) => setTimeout(resolve, 60));
    await win.setSize(size);
  } catch {
    // window may be tearing down
  }
}

// CLI overrides: --osdecorations forces the OS title bar, --nobuttons hides all chrome.
let flagOverrideCache: TitleBarDesign | null = null;
const flagOverridePromise: Promise<TitleBarDesign | null> = isDesktop
  ? invoke<[boolean, boolean]>("get_window_chrome_flags")
      .then(([osDecorations, noButtons]) => {
        flagOverrideCache = osDecorations ? "native" : noButtons ? "hidden" : null;
        return flagOverrideCache;
      })
      .catch(() => null)
  : Promise.resolve(null);

export function useTitleBarChrome(): {
  design: TitleBarDesign;
  side: TitleBarSide;
  size: TitleBarSize;
  corners: WindowCorners;
} {
  const [design, setDesign] = useState<TitleBarDesign>(readTitleBarDesign);
  const [side, setSide] = useState<TitleBarSide>(readTitleBarSide);
  const [size, setSize] = useState<TitleBarSize>(readTitleBarSize);
  const [corners, setCorners] = useState<WindowCorners>(readWindowCorners);
  const [override, setOverride] = useState<TitleBarDesign | null>(flagOverrideCache);

  useEffect(() => {
    const sync = () => {
      setDesign(readTitleBarDesign());
      setSide(readTitleBarSide());
      setSize(readTitleBarSize());
      setCorners(readWindowCorners());
    };
    window.addEventListener(CHROME_EVENT, sync);
    void flagOverridePromise.then(setOverride);
    return () => window.removeEventListener(CHROME_EVENT, sync);
  }, []);

  return { design: override ?? design, side, size, corners };
}

function useWindowState() {
  const [maximized, setMaximized] = useState(false);
  const [fullscreen, setFullscreen] = useState(false);

  useEffect(() => {
    if (!isDesktop) return;

    const win = getCurrentWindow();
    let disposed = false;
    let unlisten: (() => void) | undefined;

    const sync = async () => {
      try {
        const [isMax, isFull] = await Promise.all([win.isMaximized(), win.isFullscreen()]);
        if (disposed) return;
        setMaximized(isMax);
        setFullscreen(isFull);
      } catch {
        // window queries can fail during teardown; keep last known state
      }
    };

    void sync();
    void win.onResized(() => void sync()).then((stop) => {
      if (disposed) {
        stop();
      } else {
        unlisten = stop;
      }
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  return { maximized, fullscreen };
}

const CLASSIC_SIZES: Record<TitleBarSize, { btn: string; minus: number; square: number; copy: number; x: number }> = {
  small: { btn: "w-9", minus: 12, square: 9, copy: 10, x: 12 },
  medium: { btn: "w-10", minus: 14, square: 11, copy: 12, x: 14 },
  large: { btn: "w-12", minus: 16, square: 13, copy: 14, x: 16 },
};

const LIGHTS_SIZES: Record<TitleBarSize, { dot: string; glyph: number }> = {
  small: { dot: "h-3 w-3", glyph: 8 },
  medium: { dot: "h-3.5 w-3.5", glyph: 9 },
  large: { dot: "h-[18px] w-[18px]", glyph: 11 },
};

const MINIMAL_SIZES: Record<TitleBarSize, { btn: string; minus: number; square: number; copy: number; x: number }> = {
  small: { btn: "h-6 w-6", minus: 11, square: 9, copy: 10, x: 11 },
  medium: { btn: "h-7 w-7", minus: 13, square: 10, copy: 11, x: 13 },
  large: { btn: "h-8 w-8", minus: 15, square: 12, copy: 13, x: 15 },
};

function ClassicButtons({ size, className }: { size: TitleBarSize; className?: string }) {
  const { t } = useI18n();
  const { maximized } = useWindowState();
  const s = CLASSIC_SIZES[size];
  const button = cn(
    "flex h-full items-center justify-center rounded-md text-fg/45 transition",
    s.btn,
  );

  return (
    <div className={cn("flex h-8 items-center", className)}>
      <button
        type="button"
        onClick={() => void getCurrentWindow().minimize()}
        className={cn(button, "hover:bg-fg/10 hover:text-fg")}
        aria-label={t("topNav.extra.minimize")}
      >
        <Minus size={s.minus} strokeWidth={1.5} />
      </button>
      <button
        type="button"
        onClick={() => void getCurrentWindow().toggleMaximize()}
        className={cn(button, "hover:bg-fg/10 hover:text-fg")}
        aria-label={t("topNav.extra.maximize")}
      >
        {maximized ? (
          <Copy size={s.copy} strokeWidth={1.5} className="-scale-x-100" />
        ) : (
          <Square size={s.square} strokeWidth={1.5} />
        )}
      </button>
      <button
        type="button"
        onClick={() => void getCurrentWindow().close()}
        className={cn(button, "hover:bg-red-500/80 hover:text-white")}
        aria-label={t("topNav.extra.close")}
      >
        <X size={s.x} strokeWidth={1.5} />
      </button>
    </div>
  );
}

function LightsButtons({
  size,
  dimmed,
  className,
}: {
  size: TitleBarSize;
  dimmed: boolean;
  className?: string;
}) {
  const { t } = useI18n();
  const s = LIGHTS_SIZES[size];

  const dot = cn(
    "flex items-center justify-center rounded-full text-black/60 transition active:brightness-90",
    s.dot,
  );
  const glyph = "opacity-0 transition-opacity group-hover/lights:opacity-100";

  const colors = dimmed
    ? {
        close: "bg-fg/20 group-hover/lights:bg-[#ff5f57]",
        minimize: "bg-fg/20 group-hover/lights:bg-[#febc2e]",
        maximize: "bg-fg/20 group-hover/lights:bg-[#28c840]",
      }
    : {
        close: "bg-[#ff5f57]",
        minimize: "bg-[#febc2e]",
        maximize: "bg-[#28c840]",
      };

  return (
    <div className={cn("group/lights flex h-8 items-center gap-2 px-2.5", className)}>
      <button
        type="button"
        onClick={() => void getCurrentWindow().close()}
        className={cn(dot, colors.close)}
        aria-label={t("topNav.extra.close")}
      >
        <X size={s.glyph} strokeWidth={2.5} className={glyph} />
      </button>
      <button
        type="button"
        onClick={() => void getCurrentWindow().minimize()}
        className={cn(dot, colors.minimize)}
        aria-label={t("topNav.extra.minimize")}
      >
        <Minus size={s.glyph} strokeWidth={2.5} className={glyph} />
      </button>
      <button
        type="button"
        onClick={() => void getCurrentWindow().toggleMaximize()}
        className={cn(dot, colors.maximize)}
        aria-label={t("topNav.extra.maximize")}
      >
        <Plus size={s.glyph} strokeWidth={2.5} className={glyph} />
      </button>
    </div>
  );
}

function MinimalButtons({ size, className }: { size: TitleBarSize; className?: string }) {
  const { t } = useI18n();
  const { maximized } = useWindowState();
  const s = MINIMAL_SIZES[size];

  const button = cn(
    "flex items-center justify-center rounded-full text-fg/35 transition hover:text-fg",
    s.btn,
  );

  return (
    <div className={cn("flex h-8 items-center gap-0.5 px-1", className)}>
      <button
        type="button"
        onClick={() => void getCurrentWindow().minimize()}
        className={button}
        aria-label={t("topNav.extra.minimize")}
      >
        <Minus size={s.minus} strokeWidth={2} />
      </button>
      <button
        type="button"
        onClick={() => void getCurrentWindow().toggleMaximize()}
        className={button}
        aria-label={t("topNav.extra.maximize")}
      >
        {maximized ? (
          <Copy size={s.copy} strokeWidth={2} className="-scale-x-100" />
        ) : (
          <Square size={s.square} strokeWidth={2} />
        )}
      </button>
      <button
        type="button"
        onClick={() => void getCurrentWindow().close()}
        className={cn(button, "hover:text-danger")}
        aria-label={t("topNav.extra.close")}
      >
        <X size={s.x} strokeWidth={2} />
      </button>
    </div>
  );
}

function WindowButtons({
  design,
  size,
  className,
}: {
  design: TitleBarDesign;
  size: TitleBarSize;
  className?: string;
}) {
  if (!isDesktop || isMacOS) return null;
  if (design === "native" || design === "hidden") return null;
  if (design === "lights" || design === "lights_dimmed") {
    return <LightsButtons size={size} dimmed={design === "lights_dimmed"} className={className} />;
  }
  if (design === "minimal") return <MinimalButtons size={size} className={className} />;
  return <ClassicButtons size={size} className={className} />;
}

const RESIZE_HANDLES: Array<{ dir: ResizeDirection; className: string }> = [
  { dir: "North", className: "left-2 right-2 top-0 h-1.5 cursor-n-resize" },
  { dir: "South", className: "bottom-0 left-2 right-2 h-1.5 cursor-s-resize" },
  { dir: "West", className: "bottom-2 left-0 top-2 w-1.5 cursor-w-resize" },
  { dir: "East", className: "bottom-2 right-0 top-2 w-1.5 cursor-e-resize" },
  { dir: "NorthWest", className: "left-0 top-0 h-3 w-3 cursor-nw-resize" },
  { dir: "NorthEast", className: "right-0 top-0 h-3 w-3 cursor-ne-resize" },
  { dir: "SouthWest", className: "bottom-0 left-0 h-3 w-3 cursor-sw-resize" },
  { dir: "SouthEast", className: "bottom-0 right-0 h-3 w-3 cursor-se-resize" },
];

export function WindowResizeHandles() {
  const { design } = useTitleBarChrome();
  const { maximized, fullscreen } = useWindowState();

  if (!isDesktop || isMacOS) return null;
  if (design === "native" || maximized || fullscreen) return null;

  return (
    <>
      {RESIZE_HANDLES.map(({ dir, className }) => (
        <div
          key={dir}
          onMouseDown={(event) => {
            if (event.button !== 0) return;
            event.preventDefault();
            void getCurrentWindow().startResizeDragging(dir);
          }}
          className={cn("fixed z-[101] select-none", className)}
        />
      ))}
    </>
  );
}

export function TitleBar() {
  const { design, side, size, corners } = useTitleBarChrome();
  const { maximized, fullscreen } = useWindowState();

  const showStrip = isDesktop && !fullscreen && design !== "native" && design !== "hidden";
  const roundedActive =
    appPlatform.os === "linux" &&
    corners !== "off" &&
    design !== "native" &&
    !maximized &&
    !fullscreen;

  useLayoutEffect(() => {
    document.documentElement.style.setProperty("--titlebar-h", showStrip ? "32px" : "0px");
    return () => {
      document.documentElement.style.setProperty("--titlebar-h", "0px");
    };
  }, [showStrip]);

  useEffect(() => {
    void applyDecorations(design);
  }, [design]);

  const prevRoundedKey = useRef<string | null>(null);
  useEffect(() => {
    const root = document.documentElement;
    root.classList.toggle("window-rounded", roundedActive);
    root.style.setProperty("--window-radius", CORNER_RADII[corners]);
    document.body.style.clipPath = roundedActive
      ? `inset(0 round ${CORNER_RADII[corners]})`
      : "";
    const key = `${design}:${roundedActive}:${corners}`;
    const prev = prevRoundedKey.current;
    if (prev !== null && prev !== key) {
      const prevDesign = prev.split(":")[0];
      if (prevDesign === design && design !== "native") {
        void nudgeWindowSurface();
      }
    }
    prevRoundedKey.current = key;
    return () => {
      root.classList.remove("window-rounded");
      document.body.style.clipPath = "";
    };
  }, [design, roundedActive, corners]);

  if (!showStrip) return null;

  const buttonsOnLeft = !isMacOS && side === "left";
  const lightsInset = design === "lights" || design === "lights_dimmed";
  const wordmark = (
    <span className="pointer-events-none text-[11px] font-semibold tracking-wide text-fg/35">
      LettuceAI
    </span>
  );

  return (
    <div
      {...dragRegionAttr}
      className="fixed inset-x-0 top-0 z-[100] flex h-8 select-none items-center justify-between bg-surface"
    >
      <div
        {...dragRegionAttr}
        className={cn(
          "flex h-full items-center",
          isMacOS ? "pl-[78px]" : buttonsOnLeft ? (lightsInset ? "pl-2.5" : "pl-1") : "pl-3",
        )}
      >
        {buttonsOnLeft ? <WindowButtons design={design} size={size} /> : wordmark}
      </div>
      <div {...dragRegionAttr} className="flex h-full items-center">
        {buttonsOnLeft ? (
          <span className="flex h-full items-center pr-3">{wordmark}</span>
        ) : (
          <WindowButtons design={design} size={size} className={lightsInset ? "pr-3.5" : "pr-1"} />
        )}
      </div>
    </div>
  );
}
