import { Pin, PinOff } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, EmptyState, icon, IconButton, ListGroup } from "@lettuceai/crisp";
import type { ParticipantView, TimelineMessage } from "@/api/generated/bindings";
import { authorOf, textOf } from "../data/messages";

export interface PinnedTabProps {
  messages: readonly TimelineMessage[];
  participants: readonly ParticipantView[];
  hasMore: boolean;
  loadingMore: boolean;
  unpinning: string | null;
  onLoadMore: () => void;
  onOpen: (message: TimelineMessage) => void;
  onUnpin: (message: TimelineMessage) => void;
}

export function PinnedTab({ messages, participants, hasMore, loadingMore, unpinning, onLoadMore, onOpen, onUnpin }: PinnedTabProps) {
  const { t, i18n } = useTranslation("conversations");
  const when = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" });
  if (messages.length === 0) {
    return <EmptyState icon={<Pin size={icon.xl} />} title={t("memory.pinnedTab.empty")} description={t("memory.pinnedTab.emptyBody")} />;
  }
  return (
    <div className="space-y-4">
      <ListGroup label={t("memory.pinnedTab.label")}>
        {messages.map((message) => {
          const speaker = message.role === "user" ? t("memory.pinnedTab.you") : (authorOf(participants, message)?.name ?? "");
          return (
            <div key={message.id} className="flex items-start gap-3 px-4 py-3.5">
              <button type="button" onClick={() => onOpen(message)} className="min-w-0 flex-1 text-left">
                <p className="truncate text-sm text-fg-3">{[speaker, when.format(message.created_at)].filter(Boolean).join(" · ")}</p>
                <p className="mt-1 line-clamp-3 text-base leading-snug text-fg">{textOf(message) || t("memory.pinnedTab.noText")}</p>
              </button>
              <IconButton label={t("memory.pinnedTab.unpin")} size="sm" variant="ghost" disabled={unpinning !== null} onClick={() => onUnpin(message)}>
                <PinOff size={icon.sm} />
              </IconButton>
            </div>
          );
        })}
      </ListGroup>
      {hasMore && (
        <div className="flex justify-center">
          <Button variant="ghost" loading={loadingMore} onClick={onLoadMore}>
            {t("memory.pinnedTab.loadMore")}
          </Button>
        </div>
      )}
    </div>
  );
}
