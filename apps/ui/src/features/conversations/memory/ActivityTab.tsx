import { useState } from "react";
import { History, Undo2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Accordion, AccordionItem, Badge, Button, cn, confirm, EmptyState, icon, LoadingState, Text } from "@lettuceai/crisp";
import type { MemoryCycleView } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";

export interface ActivityTabProps {
  companion?: boolean;
  cycles: readonly MemoryCycleView[];
  hasMore: boolean;
  loadingMore: boolean;
  loading: boolean;
  error: unknown;
  onRetry: () => void;
  onLoadMore: () => void;
  onRevert: (cycle: MemoryCycleView) => Promise<boolean>;
}

export function ActivityTab({ cycles, hasMore, loadingMore, loading, error, onRetry, onLoadMore, onRevert, companion = false }: ActivityTabProps) {
  const { t, i18n } = useTranslation("conversations");
  const [reverting, setReverting] = useState<string | null>(null);
  const when = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" });
  const windowText = (cycle: Pick<MemoryCycleView, "window_start" | "window_end">) => t("memory.window", { from: cycle.window_start, to: cycle.window_end });

  if (loading) return <LoadingState />;
  if (cycles.length === 0 && error) return <FailureState error={error} onRetry={onRetry} />;
  if (cycles.length === 0) {
    return <EmptyState icon={<History size={icon.xl} />} title={t("memory.activity.empty")} description={t("memory.activity.emptyBody")} />;
  }

  const askRevert = async (cycle: MemoryCycleView) => {
    const sure = await confirm({
      title: t(companion ? "companion.activity.undoTitle" : "memory.activity.undoTitle", { window: windowText(cycle) }),
      message: t(companion ? "companion.activity.undoBody" : "memory.activity.undoBody"),
      confirmLabel: t(companion ? "companion.activity.undoConfirm" : "memory.activity.undoConfirm"),
      cancelLabel: t("memory.activity.keep"),
      destructive: true,
    });
    if (!sure) return;
    setReverting(cycle.run_id);
    await onRevert(cycle);
    setReverting(null);
  };

  const blockedText = (cycle: MemoryCycleView) => {
    const later = cycles.find((entry) => entry.run_id === cycle.blocked_by);
    return later ? t(companion ? "companion.activity.blockedBy" : "memory.activity.blockedBy", { window: windowText(later) }) : t(companion ? "companion.activity.blocked" : "memory.activity.blocked");
  };

  return (
    <div className="space-y-4">
      <Accordion>
        {cycles.map((cycle) => {
          const failed = cycle.status === "failed" || cycle.status === "interrupted";
          const live = cycle.status === "queued" || cycle.status === "processing";
          return (
            <AccordionItem
              key={cycle.run_id}
              value={cycle.run_id}
              title={
                <span className="flex flex-wrap items-center gap-2">
                  {windowText(cycle)}
                  {cycle.reverted && <Badge>{t("memory.activity.badgeUndone")}</Badge>}
                  {cycle.status === "failed" && <Badge tone="danger">{t("memory.activity.badgeFailed")}</Badge>}
                  {cycle.status === "interrupted" && <Badge tone="warning">{t("memory.activity.badgeInterrupted")}</Badge>}
                  {cycle.status === "cancelled" && <Badge>{t("memory.activity.badgeStopped")}</Badge>}
                  {live && <Badge tone="info">{t("memory.activity.badgeRunning")}</Badge>}
                </span>
              }
              description={when.format(cycle.started_at)}
            >
              <div className="space-y-3 pb-1">
                {failed ? (
                  <Text variant="secondary">{cycle.failure ? t(`memory.failure.${cycle.failure}`) : t("memory.activity.noDetails")}</Text>
                ) : live ? (
                  <Text variant="secondary">{t("memory.activity.running")}</Text>
                ) : cycle.status === "cancelled" ? (
                  <Text variant="secondary">{t("memory.activity.stopped")}</Text>
                ) : null}
                {!failed && !live && cycle.summary && <Text variant="secondary">{cycle.summary}</Text>}
                {cycle.actions.length > 0 && (
                  <ul className="space-y-1.5">
                    {cycle.actions.map((action, index) => (
                      <li key={index} className="flex gap-2 text-sm">
                        <span className={cn("w-24 shrink-0 text-fg-3", action.kind === "created" && "text-accent")}>{t(`memory.activity.actions.${action.kind}`)}</span>
                        {action.text && <span className="min-w-0 text-fg-2">{action.text}</span>}
                      </li>
                    ))}
                  </ul>
                )}
                {!cycle.reverted && cycle.revertable && (
                  <Button size="sm" variant="secondary" loading={reverting === cycle.run_id} disabled={reverting !== null} leading={<Undo2 size={icon.sm} />} onClick={() => void askRevert(cycle)}>
                    {t(companion ? "companion.activity.undo" : "memory.activity.undo")}
                  </Button>
                )}
                {!cycle.reverted && !cycle.revertable && cycle.blocked_by !== null && <Text variant="hint">{blockedText(cycle)}</Text>}
              </div>
            </AccordionItem>
          );
        })}
      </Accordion>
      {error ? <FailureState error={error} onRetry={onRetry} size="sm" /> : hasMore && (
        <div className="flex justify-center">
          <Button variant="ghost" loading={loadingMore} onClick={onLoadMore}>
            {t("memory.activity.loadMore")}
          </Button>
        </div>
      )}
    </div>
  );
}
