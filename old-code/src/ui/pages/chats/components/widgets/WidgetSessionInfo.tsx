import { MessageSquare, Hash, Clapperboard } from "lucide-react";
import type { SessionInfoNode } from "../../../../../core/storage/chatWidgetSchemas";
import { cn } from "../../../../design-tokens";
import { useI18n } from "../../../../../core/i18n/context";
import { useWidgetContext } from "./WidgetContext";
import { widgetCardClass } from "./widgetSurface";

export function WidgetSessionInfo({ node }: { node: SessionInfoNode }) {
  const { t } = useI18n();
  const { messageCount, sceneName, session, hasBackground } = useWidgetContext();
  const tokenCount = session?.memorySummaryTokenCount ?? 0;

  const rows: { icon: typeof Hash; label: string; value: string }[] = [
    { icon: MessageSquare, label: t("chats.widgets.sessionInfo.messages"), value: String(messageCount) },
    { icon: Hash, label: t("chats.widgets.sessionInfo.memoryTokens"), value: String(tokenCount) },
    { icon: Clapperboard, label: t("chats.widgets.sessionInfo.scene"), value: sceneName ?? t("common.labels.none") },
  ];

  return (
    <section
      className={cn(
        "flex flex-col gap-2 rounded-xl px-3 py-3",
        widgetCardClass(hasBackground, node.design),
      )}
    >
      {node.title && (
        <h3 className="text-sm font-semibold text-fg/75">{node.title}</h3>
      )}
      <div className="flex flex-col gap-1.5">
        {rows.map((row) => (
          <div key={row.label} className="flex items-center gap-2 text-[12px]">
            <row.icon size={13} className="shrink-0 text-fg/40" />
            <span className="text-fg/50">{row.label}</span>
            <span className="ml-auto truncate font-medium text-fg/80">{row.value}</span>
          </div>
        ))}
      </div>
    </section>
  );
}
