import { useState } from "react";
import { Minus, Plus } from "lucide-react";
import type { StatTrackerNode } from "../../../../../core/storage/chatWidgetSchemas";
import { cn } from "../../../../design-tokens";
import { NumberInput } from "../../../../components/NumberInput";
import { useI18n } from "../../../../../core/i18n/context";
import { useWidgetContext } from "./WidgetContext";
import { useWidgetEdit } from "./WidgetEditContext";
import { widgetCardClass } from "./widgetSurface";

export function WidgetStatTracker({ node }: { node: StatTrackerNode }) {
  const { t } = useI18n();
  const { hasBackground, onUpdateNode } = useWidgetContext();
  const { editing: areaEditing } = useWidgetEdit();
  const interactive = !areaEditing;
  const [editingId, setEditingId] = useState<string | null>(null);

  const setValue = (statId: string, next: number) => {
    if (!interactive) return;
    const stats = node.stats.map((s) => {
      if (s.id !== statId) return s;
      let clamped = next;
      if (s.min !== undefined) clamped = Math.max(s.min, clamped);
      if (s.max !== undefined) clamped = Math.min(s.max, clamped);
      return { ...s, value: clamped };
    });
    void onUpdateNode(node.id, { stats });
  };

  return (
    <section
      className={cn(
        "flex flex-col gap-2 rounded-xl px-3 py-3",
        widgetCardClass(hasBackground, node.design),
      )}
    >
      {(node.title || node.description) && (
        <header className="flex flex-col gap-0.5">
          {node.title && (
            <h3 className="text-sm font-semibold text-fg/75">{node.title}</h3>
          )}
          {node.description && (
            <p className="text-[11px] leading-snug text-fg/45">{node.description}</p>
          )}
        </header>
      )}
      {node.stats.length === 0 ? (
        <p className="text-[12px] italic text-fg/40">{t("chats.widgets.statTracker.empty")}</p>
      ) : (
        <div className="flex flex-col gap-1.5">
          {node.stats.map((stat) => (
            <div key={stat.id} className="flex items-center gap-2">
              <span className="min-w-0 flex-1 truncate text-[12px] text-fg/70">
                {stat.label}
              </span>
              <div className="flex items-center gap-1">
                <button
                  type="button"
                  onClick={() => setValue(stat.id, stat.value - 1)}
                  disabled={!interactive || (stat.min !== undefined && stat.value <= stat.min)}
                  className="flex h-6 w-6 items-center justify-center rounded-md border border-fg/15 bg-fg/5 text-fg/60 transition hover:bg-fg/10 disabled:opacity-40"
                  aria-label={t("chats.widgets.statTracker.decrease", { label: stat.label })}
                >
                  <Minus size={12} strokeWidth={2.4} />
                </button>
                {interactive && editingId === stat.id ? (
                  <NumberInput
                    value={stat.value}
                    min={stat.min}
                    max={stat.max}
                    autoFocus
                    className="w-12 rounded-md border border-fg/15 bg-fg/5 px-1 py-0.5 text-center text-sm font-semibold tabular-nums text-fg/85 focus:border-accent/40 focus:outline-none"
                    onChange={(next) => setValue(stat.id, next ?? stat.value)}
                    onBlur={() => setEditingId(null)}
                  />
                ) : (
                  <button
                    type="button"
                    onClick={() => interactive && setEditingId(stat.id)}
                    disabled={!interactive}
                    className="w-8 text-center text-sm font-semibold tabular-nums text-fg/85 disabled:cursor-default"
                  >
                    {stat.value}
                  </button>
                )}
                <button
                  type="button"
                  onClick={() => setValue(stat.id, stat.value + 1)}
                  disabled={!interactive || (stat.max !== undefined && stat.value >= stat.max)}
                  className="flex h-6 w-6 items-center justify-center rounded-md border border-fg/15 bg-fg/5 text-fg/60 transition hover:bg-fg/10 disabled:opacity-40"
                  aria-label={t("chats.widgets.statTracker.increase", { label: stat.label })}
                >
                  <Plus size={12} strokeWidth={2.4} />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
