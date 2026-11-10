import { useState, type ReactNode } from "react";
import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import { Check, Pencil, Plus, RotateCcw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, cn, EmptyState, icon, motion as presets, Skeleton } from "@lettuceai/crisp";
import { FailureState } from "@/shared/ui/FailureState";
import type { WidgetLayout } from "./layout";
import { availableNodes, createActionsPreset, createWidgetNode, fold, splitFolded, updateNode, type DefaultLabel, type WidgetNode, type WidgetPlace, type WidgetSlots, type WidgetType } from "./nodes";
import { WidgetConfigSheet } from "./WidgetConfigSheet";
import { WidgetEditList } from "./WidgetEditList";
import { WidgetRenderer } from "./WidgetRenderer";
import { WidgetTypeSheet } from "./WidgetTypeSheet";

type Side = "left" | "right";

export interface WidgetAreaProps {
  layout: WidgetLayout;
  slots: WidgetSlots | null;
  /** Why the slots are missing: still loading, or a failed load with a way to try again. */
  failure: { error: unknown; retry: () => void } | null;
  where: WidgetPlace;
  save: (mutate: (slots: WidgetSlots) => WidgetSlots) => Promise<boolean>;
  children: ReactNode;
}

/**
 * The chat column with the widget panels beside it. The column element stays the same whether or not panels show, so toggling widgets never remounts the timeline. Owns edit mode: a draft of both sides, saved together on Done.
 */
export function WidgetArea({ layout, slots, failure, where, save, children }: WidgetAreaProps) {
  const { t } = useTranslation("conversations");
  const [draft, setDraft] = useState<WidgetSlots | null>(null);
  const [saving, setSaving] = useState(false);
  const [adding, setAdding] = useState<{ add: (node: WidgetNode) => void } | null>(null);
  const [configuring, setConfiguring] = useState<WidgetNode | null>(null);
  const bothSides = layout.showLeft && layout.showRight;
  if (!layout.enabled && draft !== null) setDraft(null);

  const label = (key: DefaultLabel) => (key === "stat" ? t("widgets.config.statTracker.defaultStat") : key === "snippet" ? t("widgets.config.quickSnippets.newSnippet") : t(`widgets.types.${key}.label`));
  const done = async () => {
    if (!draft) return;
    if (JSON.stringify(draft) === JSON.stringify(slots)) {
      setDraft(null);
      return;
    }
    setSaving(true);
    const saved = await save(() => draft);
    setSaving(false);
    if (saved) setDraft(null);
  };
  /** Rewrites one panel's widgets; with a single panel showing, that panel holds both sides and the result is split back. */
  const edit = (side: Side, change: (nodes: WidgetNode[]) => WidgetNode[]) =>
    setDraft((current) => {
      if (!current) return current;
      const next = change(fold(current, layout)[side]);
      return bothSides ? { ...current, [side]: next } : splitFolded(current, next);
    });
  const pick = (type: WidgetType | "actions") => {
    adding?.add(type === "actions" ? createActionsPreset(t("widgets.presets.actions.label")) : createWidgetNode(type, label));
    setAdding(null);
  };
  const edited = (node: WidgetNode) => {
    setDraft((current) => current && { left: updateNode(current.left, node.id, () => node), right: updateNode(current.right, node.id, () => node) });
    setConfiguring(null);
  };
  const move = (from: Side, node: WidgetNode) =>
    setDraft((current) => {
      if (!current) return current;
      const to: Side = from === "left" ? "right" : "left";
      return { ...current, [from]: current[from].filter((entry) => entry.id !== node.id), [to]: [...current[to], node] };
    });

  const panel = (side: Side) => {
    const nodes = draft ? fold(draft, layout)[side] : slots ? availableNodes(fold(slots, layout)[side], where) : [];
    return (
      <aside
        key={side}
        aria-label={t(side === "left" ? "widgets.area.left" : "widgets.area.right")}
        className={cn("scrollbar-thin relative flex min-w-0 flex-1 basis-0 flex-col overflow-y-auto", side === "left" ? "border-r border-line" : "border-l border-line")}
      >
        {draft && (
          <div className="sticky top-0 z-20 flex flex-wrap items-center justify-between gap-1.5 border-b border-line bg-surface px-3 py-2">
            <Button variant="secondary" size="sm" leading={<Plus size={icon.sm} />} onClick={() => setAdding({ add: (node) => edit(side, (shown) => [...shown, node]) })}>
              {t("widgets.list.add")}
            </Button>
            <div className="flex items-center gap-1.5">
              <Button variant="ghost" size="sm" disabled={saving} leading={<RotateCcw size={icon.sm} />} onClick={() => setDraft(null)}>
                {t("widgets.list.revert")}
              </Button>
              <Button size="sm" loading={saving} leading={<Check size={icon.sm} />} onClick={() => void done()}>
                {t("widgets.list.done")}
              </Button>
            </div>
          </div>
        )}
        <div className="flex min-h-0 flex-1 flex-col gap-3 p-3">
          {draft ? (
            nodes.length === 0 ? (
              <EmptyState size="sm" title={t("widgets.empty.editingTitle")} action={<Button variant="secondary" size="sm" onClick={() => setAdding({ add: (node) => edit(side, (shown) => [...shown, node]) })}>{t("widgets.empty.addWidget")}</Button>} />
            ) : (
              <WidgetEditList
                nodes={nodes}
                onChange={(next) => edit(side, () => next)}
                onEdit={setConfiguring}
                onAdd={(add) => setAdding({ add })}
                {...(bothSides ? { move: { side, onMove: (node: WidgetNode) => move(side, node) } } : {})}
              />
            )
          ) : failure && !slots ? (
            <FailureState size="sm" error={failure.error} onRetry={failure.retry} />
          ) : !slots ? (
            <>
              <Skeleton shape="block" height={96} />
              <Skeleton shape="block" height={64} />
            </>
          ) : (
            <>
              {nodes.length === 0 ? <EmptyState size="sm" title={t("widgets.empty.title")} /> : <WidgetList nodes={nodes} />}
              <Button variant="ghost" size="sm" block leading={<Pencil size={icon.xs} />} className="mt-auto border border-dashed border-line-2 text-fg-3" onClick={() => setDraft(slots)}>
                {t("widgets.list.editWidgets")}
              </Button>
            </>
          )}
        </div>
      </aside>
    );
  };

  return (
    <div className="relative flex min-h-0 flex-1">
      {layout.showLeft && panel("left")}
      <div
        key="column"
        className={cn("relative flex min-h-0 min-w-0 flex-col", layout.enabled ? "shrink-0" : "flex-1")}
        style={layout.enabled && layout.columnPx !== null ? { width: layout.columnPx, maxWidth: "100%" } : undefined}
      >
        {children}
      </div>
      {layout.showRight && panel("right")}
      <WidgetTypeSheet open={adding !== null} where={where} onPick={pick} onClose={() => setAdding(null)} />
      <WidgetConfigSheet node={configuring} onSave={edited} onClose={() => setConfiguring(null)} />
    </div>
  );
}

function WidgetList({ nodes }: { nodes: WidgetNode[] }) {
  const reduced = useReducedMotion();
  return (
    <div className="flex flex-col gap-3">
      <AnimatePresence initial={false} mode="popLayout">
        {nodes.map((node) => (
          <motion.div
            key={node.id}
            layout={!reduced}
            initial={reduced ? false : { opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={reduced ? { opacity: 0 } : { opacity: 0, y: -4 }}
            transition={presets.quick}
          >
            <WidgetRenderer node={node} />
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  );
}
