import { useMemo, useState } from "react";
import { Brain, Pin, ChevronRight } from "lucide-react";
import type { MemoryNode } from "../../../../../core/storage/chatWidgetSchemas";
import { cn } from "../../../../design-tokens";
import { useI18n } from "../../../../../core/i18n/context";
import { useWidgetContext } from "./WidgetContext";
import { widgetCardClass } from "./widgetSurface";

interface MemoryItem {
  id: string;
  text: string;
  category: string | null;
  importance: number;
  isPinned: boolean;
  isCold: boolean;
  createdAt: number;
}

const CATEGORY_STYLE: Record<string, string> = {
  character_trait: "border-info/30 bg-info/10 text-info",
  relationship: "border-accent/30 bg-accent/10 text-accent",
  plot_event: "border-warning/30 bg-warning/10 text-warning",
  world_detail: "border-secondary/30 bg-secondary/10 text-secondary",
  preference: "border-info/30 bg-info/10 text-info",
  milestone: "border-accent/30 bg-accent/10 text-accent",
  boundary: "border-danger/30 bg-danger/10 text-danger",
  emotional_snapshot: "border-accent/30 bg-accent/10 text-accent",
};

function categoryStyle(category: string): string {
  return CATEGORY_STYLE[category] ?? "border-fg/15 bg-fg/8 text-fg/55";
}

function formatCategory(category: string): string {
  return category.replace(/_/g, " ");
}

export function WidgetMemory({ node }: { node: MemoryNode }) {
  const { t } = useI18n();
  const { character, session, memories, hasBackground, onOpenMemories } =
    useWidgetContext();
  const limit = node.limit ?? 10;
  const [activeCategory, setActiveCategory] = useState<string | null>(null);

  const items = useMemo<MemoryItem[]>(() => {
    const isDynamic = character?.memoryType === "dynamic";
    const embeddings = session?.memoryEmbeddings ?? [];
    if (isDynamic && embeddings.length > 0) {
      return embeddings
        .map((emb, index) => ({
          id: emb.id || `mem-${index}`,
          text: emb.text,
          category: emb.category ?? null,
          importance: emb.importanceScore ?? 1,
          isPinned: emb.isPinned ?? false,
          isCold: emb.isCold ?? false,
          createdAt: emb.createdAt ?? 0,
        }))
        .sort((a, b) => {
          if (a.isPinned !== b.isPinned) return a.isPinned ? -1 : 1;
          if (a.isCold !== b.isCold) return a.isCold ? 1 : -1;
          if (a.importance !== b.importance) return b.importance - a.importance;
          return b.createdAt - a.createdAt;
        });
    }
    return memories.map((text, index) => ({
      id: `mem-${index}`,
      text,
      category: null,
      importance: 1,
      isPinned: false,
      isCold: false,
      createdAt: 0,
    }));
  }, [character?.memoryType, session?.memoryEmbeddings, memories]);

  const categories = useMemo(() => {
    const set = new Set<string>();
    for (const item of items) if (item.category) set.add(item.category);
    return Array.from(set);
  }, [items]);

  const filtered = activeCategory
    ? items.filter((i) => i.category === activeCategory)
    : items;
  const shown = filtered.slice(0, limit);

  return (
    <section
      className={cn(
        "flex flex-col gap-2 rounded-xl px-3 py-3",
        widgetCardClass(hasBackground, node.design),
      )}
    >
      <header className="flex items-center gap-2">
        <Brain size={14} className="text-fg/50" />
        <h3 className="text-sm font-semibold text-fg/75">
          {node.title || t("chats.widgets.memory.defaultTitle")}
        </h3>
        <span className="text-[11px] text-fg/40">{items.length}</span>
        <button
          type="button"
          onClick={onOpenMemories}
          className="ml-auto flex items-center gap-0.5 text-[11px] text-accent/80 transition hover:text-accent"
        >
          {t("chats.widgets.memory.manage")}
          <ChevronRight size={12} />
        </button>
      </header>

      {categories.length > 1 && (
        <div className="flex flex-wrap gap-1">
          <FilterChip
            label={t("chats.widgets.memory.all")}
            active={activeCategory === null}
            onClick={() => setActiveCategory(null)}
          />
          {categories.map((cat) => (
            <FilterChip
              key={cat}
              label={formatCategory(cat)}
              active={activeCategory === cat}
              onClick={() => setActiveCategory(cat)}
            />
          ))}
        </div>
      )}

      {shown.length === 0 ? (
        <p className="text-[12px] italic text-fg/40">{t("chats.widgets.memory.empty")}</p>
      ) : (
        <ul className="flex max-h-64 flex-col gap-1.5 overflow-y-auto pr-1">
          {shown.map((item) => (
            <li key={item.id}>
              <button
                type="button"
                onClick={onOpenMemories}
                title={item.text}
                className={cn(
                  "flex w-full items-start gap-1.5 rounded-md px-1 py-0.5 text-left text-[12px] leading-snug transition hover:bg-fg/5",
                  item.isCold ? "text-fg/40" : "text-fg/75",
                )}
              >
                {item.isPinned ? (
                  <Pin size={10} className="mt-1 shrink-0 text-accent/70" />
                ) : item.category ? (
                  <span
                    className={cn(
                      "mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full border",
                      categoryStyle(item.category),
                    )}
                    title={formatCategory(item.category)}
                  />
                ) : (
                  <span className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-fg/25" />
                )}
                <span className="line-clamp-2 min-w-0">{item.text}</span>
              </button>
            </li>
          ))}
        </ul>
      )}

      {filtered.length > limit && (
        <button
          type="button"
          onClick={onOpenMemories}
          className="text-[11px] text-fg/45 transition hover:text-fg/70"
        >
          {t("chats.widgets.memory.more", { count: filtered.length - limit })}
        </button>
      )}
    </section>
  );
}

function FilterChip({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "rounded-full border px-2 py-0.5 text-[10px] font-medium capitalize transition",
        active
          ? "border-accent/40 bg-accent/15 text-accent"
          : "border-fg/12 bg-fg/5 text-fg/50 hover:bg-fg/10",
      )}
    >
      {label}
    </button>
  );
}
