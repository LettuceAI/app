import { useEffect, useRef } from "react";

/**
 * Owns the page scroller ref and gives the preview the chrome it needs from it: `--scroller-height` for the sticky panel, and the page made inert while the phone overlay is open.
 */
export function useAppearancePreviewChrome(overlayOpen: boolean) {
  const scroller = useRef<HTMLElement>(null);
  useEffect(() => {
    const page = scroller.current?.parentElement;
    if (!page) return;
    page.inert = overlayOpen;
    return () => { page.inert = false; };
  }, [overlayOpen]);
  useEffect(() => {
    const element = scroller.current;
    if (!element) return;
    const sync = () => element.style.setProperty("--scroller-height", `${element.clientHeight}px`);
    sync();
    const observer = new ResizeObserver(sync);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  return scroller;
}

