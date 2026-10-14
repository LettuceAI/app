import { memo } from "react";
import { useTranslation } from "react-i18next";
import { AudioItem, Badge, cn, Highlight, InlineMarkdown, Message } from "@lettuceai/crisp";
import type { ParticipantView, TimelineMessage } from "@/api/generated/bindings";
import { authorOf, formatTime, mediaOf, type MediaItem, paragraphsOf, spacing, textOf, type Group } from "../data/messages";

export function MessageBody({ text, query }: { text: string; query: string }) {
  const needle = query.toLowerCase();
  return (
    <div className="space-y-2">
      {paragraphsOf(text).map((paragraph, index) => (
        <p key={index} className="whitespace-pre-line break-words">
          {needle && paragraph.toLowerCase().includes(needle) ? <Highlight query={query}>{paragraph}</Highlight> : <InlineMarkdown>{paragraph}</InlineMarkdown>}
        </p>
      ))}
    </div>
  );
}

export function Attached({ media }: { media: readonly MediaItem[] }) {
  const { t } = useTranslation("conversations");
  const pictures = media.filter((item) => item.kind === "image" || item.kind === null);
  return (
    <div className="space-y-1.5">
      {pictures.length > 0 && (
        <div className={cn("grid gap-1.5", pictures.length > 1 ? "grid-cols-2" : "grid-cols-1")}>
          {pictures.map((item) =>
            item.url ? <img key={item.id} src={item.url} alt={item.name ?? t("message.image")} className="max-h-56 w-full rounded-lg object-cover" /> : null,
          )}
        </div>
      )}
      {media.map((item) =>
        item.kind === "audio" ? (
          <AudioItem key={item.id} title={item.name ?? t("message.audio")} src={item.url} />
        ) : item.kind === "video" || item.kind === "document" ? (
          <Badge key={item.id} tone="neutral">{item.name ?? t(`message.${item.kind}`)}</Badge>
        ) : null,
      )}
    </div>
  );
}

export interface MessageRowProps {
  message: TimelineMessage;
  group: Group;
  participants: readonly ParticipantView[];
  locale: string;
  query: string;
  dimmed: boolean;
  held: boolean;
  latest: boolean;
  onMenu: (message: TimelineMessage) => void;
  onVariant: (message: TimelineMessage, delta: -1 | 1) => void;
}

function Row({ message, group, participants, locale, query, dimmed, held, latest, onMenu, onVariant }: MessageRowProps) {
  const author = authorOf(participants, message);
  const media = mediaOf(message);
  const index = message.candidate_index;
  return (
    <div id={`m-${message.id}`} className={spacing(group)}>
      <Message
        role={message.role}
        {...(author ? { author } : {})}
        timestamp={formatTime(message.created_at, locale)}
        group={group}
        pinned={message.pinned}
        {...(message.reasoning ? { reasoning: { text: <MessageBody text={message.reasoning} query="" /> } } : {})}
        {...(latest && index !== null && message.candidate_count > 1
          ? { variants: { index, total: message.candidate_count, onPrev: () => onVariant(message, -1), onNext: () => onVariant(message, 1) } }
          : {})}
        {...(media.length > 0 ? { attachments: <Attached media={media} /> } : {})}
        onMenu={() => onMenu(message)}
        held={held}
        className={cn("mt-0! transition-opacity", dimmed && "opacity-25")}
      >
        <MessageBody text={textOf(message)} query={query} />
      </Message>
    </div>
  );
}

export const MessageRow = memo(Row);
