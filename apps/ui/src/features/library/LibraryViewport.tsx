import { useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import { VirtualList } from "@lettuceai/crisp";

export function LibraryViewport<T>({
  items,
  itemKey,
  scrollRef,
  gridClass,
  aspectRatio,
  children,
}: {
  items: readonly T[];
  itemKey: (item: T) => string;
  scrollRef: RefObject<HTMLElement | null>;
  gridClass: string;
  aspectRatio?: number;
  children: (item: T) => ReactNode;
}) {
  const probe = useRef<HTMLDivElement>(null);
  const [metrics, setMetrics] = useState({
    columns: aspectRatio ? 2 : 1,
    height: 80,
    gap: aspectRatio === 1 ? 8 : 12,
  });
  useLayoutEffect(() => {
    const element = probe.current;
    if (!element) return;
    const measure = () => {
      const style = getComputedStyle(element);
      const template = style.gridTemplateColumns;
      const columns =
        template && template !== "none" ? template.split(" ").length : aspectRatio ? 2 : 1;
      const gap = Number.parseFloat(style.columnGap) || (aspectRatio === 1 ? 8 : 12);
      const width = element.getBoundingClientRect().width;
      const height =
        aspectRatio && width ? (width - gap * (columns - 1)) / columns / aspectRatio : 80;
      setMetrics((previous) =>
        previous.columns === columns && previous.height === height && previous.gap === gap
          ? previous
          : { columns, height, gap },
      );
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [gridClass, aspectRatio]);
  const rows = [];
  for (let index = 0; index < items.length; index += metrics.columns)
    rows.push(items.slice(index, index + metrics.columns));
  return (
    <>
      <div ref={probe} className={gridClass} aria-hidden style={{ height: 0 }} />
      <VirtualList
        items={rows}
        itemKey={(row) => itemKey(row[0]!)}
        scrollRef={scrollRef}
        estimateSize={metrics.height + metrics.gap}
        overscan={2}
      >
        {(row) => (
          <div className={gridClass} style={{ paddingBottom: metrics.gap }}>
            {row.map((item) => (
              <div key={itemKey(item)}>{children(item)}</div>
            ))}
          </div>
        )}
      </VirtualList>
    </>
  );
}
