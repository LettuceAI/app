import { useCallback, useRef, useState } from "react";

type BindProps = {
  onTouchStart: (event: React.TouchEvent) => void;
  onTouchMove: (event: React.TouchEvent) => void;
  onTouchEnd: (event: React.TouchEvent) => void;
};

export function useRocketEasterEgg(): { bind: BindProps; isLaunched: boolean } {
  const startYRef = useRef<number | null>(null);
  const lastLaunchRef = useRef(0);
  const [isLaunched, setIsLaunched] = useState(false);

  const onTouchStart = useCallback((event: React.TouchEvent) => {
    startYRef.current = event.touches[0]?.clientY ?? null;
  }, []);

  const triggerLaunch = useCallback((delta: number) => {
    const now = Date.now();
    if (delta > 60 && now - lastLaunchRef.current > 2500) {
      lastLaunchRef.current = now;
      setIsLaunched(true);
      window.setTimeout(() => setIsLaunched(false), 1100);
      return true;
    }
    return false;
  }, []);

  const onTouchMove = useCallback(
    (event: React.TouchEvent) => {
      const startY = startYRef.current;
      if (startY == null) return;
      const currentY = event.touches[0]?.clientY ?? null;
      if (currentY == null) return;
      const delta = currentY - startY;
      if (triggerLaunch(delta)) {
        startYRef.current = null;
      }
    },
    [triggerLaunch],
  );

  const onTouchEnd = useCallback(
    (event: React.TouchEvent) => {
      const startY = startYRef.current;
      const endY = event.changedTouches[0]?.clientY ?? null;
      startYRef.current = null;
      if (startY == null || endY == null) return;
      triggerLaunch(endY - startY);
    },
    [triggerLaunch],
  );

  return { bind: { onTouchStart, onTouchMove, onTouchEnd }, isLaunched };
}
