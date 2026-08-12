import { useEffect, useRef } from "react";

export function InfiniteScrollSentinel({
  onReach,
  disabled = false,
}: {
  onReach: () => void;
  disabled?: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el || disabled) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) onReach();
      },
      { rootMargin: "600px" },
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [onReach, disabled]);

  return <div ref={ref} className="h-px" />;
}
