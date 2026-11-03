import { AssetAudio } from "@/shared/ui/AssetAudio";
import { memo, useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { Badge, cn, ImageViewer, MediaTile, Message, Prose, relativeTime, type MessageVariants } from "@lettuceai/crisp";
import type { ParticipantView, TimelineMessage } from "@/api/generated/bindings";
import { useChatLook, useMessageAppearance } from "@/entities/preferences";
import {
  authorOf,
  mediaOf,
  type MediaItem,
  spacing,
  textOf,
  type Group,
} from "../data/messages";
import { ReplyDetails } from "./ReplyDetails";
import { SceneImageStatus } from "../scene-images/SceneImageStatus";
import { MessageImageViewer } from "./MessageImageViewer";
import { useOverlayBack } from "../companion/use-overlay-back";

function EmbeddedImageViewer({ image, onClose }: { image: { src: string; alt: string }; onClose: () => void }) {
  useOverlayBack(true, onClose);
  return <ImageViewer images={[image]} index={0} onIndexChange={onClose} />;
}

export function MessageBody({ text, query }: { text: string; query: string }) {
  const [image, setImage] = useState<{ src: string; alt: string } | null>(null);
  const closeImage = useCallback(() => setImage(null), []);
  return (
    <>
      <Prose size="inherit" quotes query={query} onImageClick={setImage} className="select-text">{text}</Prose>
      {image && <EmbeddedImageViewer image={image} onClose={closeImage} />}
    </>
  );
}

export function Attached({ media }: { media: readonly MediaItem[] }) {
  const { t } = useTranslation("conversations");
  const pictures = media.filter((item) => item.url && (item.kind === "image" || item.kind === null));
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const selectedIndex = pictures.findIndex((picture) => picture.id === selectedId);
  return (
    <div className="space-y-1.5">
      {pictures.length > 0 && (
        <div className={cn("grid gap-1.5", pictures.length > 1 ? "grid-cols-2" : "grid-cols-1")}>
          {pictures.map((item) => (
              <MediaTile
                key={item.id}
                src={item.url!}
                alt={item.name ?? t("message.image")}
                className="max-h-56"
                onClick={() => setSelectedId(item.id)}
              />
          ))}
        </div>
      )}
      {selectedIndex >= 0 && <MessageImageViewer images={pictures} index={selectedIndex} onIndexChange={(index) => setSelectedId(index === null ? null : pictures[index]?.id ?? null)} />}
      {media.map((item) =>
        item.kind === "audio" ? (
          <AssetAudio key={item.id} title={item.name ?? t("message.audio")} src={item.url} />
        ) : item.kind === "video" || item.kind === "document" ? (
          <Badge key={item.id} tone="neutral">
            {item.name ?? t(`message.${item.kind}`)}
          </Badge>
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
  scene?: MessageVariants;
  conversationId: string;
  onSceneImage: (message: TimelineMessage) => void;
}

function Row({
  message,
  group,
  participants,
  locale,
  query,
  dimmed,
  held,
  latest,
  onMenu,
  onVariant,
  scene,
  conversationId,
  onSceneImage,
}: MessageRowProps) {
  const look = useChatLook();
  const appearance = useMessageAppearance();
  const author = authorOf(participants, message);
  const media = mediaOf(message);
  const picturesBelow = message.role === "assistant" ? media.filter((item) => item.kind === "image" || item.kind === null) : [];
  const attachments = message.role === "assistant" ? media.filter((item) => item.kind !== "image" && item.kind !== null) : media;
  const index = message.candidate_index;
  return (
    <div id={`m-${message.id}`} className={spacing(group, look.messageGap)}>
      <Message
        appearance={appearance}
        {...(message.role === "assistant" && look.details.length
          ? { details: <ReplyDetails messageId={message.id} /> }
          : {})}
        role={message.role}
        {...(author ? { author } : {})}
        timestamp={
          look.timestampFormat === "relative"
            ? relativeTime(message.created_at, undefined, locale)
            : new Intl.DateTimeFormat(
                locale,
                look.timestampFormat === "time"
                  ? { hour: "numeric", minute: "2-digit" }
                  : { dateStyle: "medium", timeStyle: "short" },
              ).format(message.created_at)
        }
        group={group}
        pinned={message.pinned}
        {...(message.reasoning
          ? { reasoning: { text: <MessageBody text={message.reasoning} query="" /> } }
          : {})}
        {...(scene ? { variants: scene } : {})}
        {...(latest && index !== null && message.candidate_count > 1
          ? {
              variants: {
                index,
                total: message.candidate_count,
                onPrev: () => onVariant(message, -1),
                onNext: () => onVariant(message, 1),
              },
            }
          : {})}
        {...(attachments.length > 0 ? { attachments: <Attached media={attachments} /> } : {})}
        onMenu={() => onMenu(message)}
        held={held}
        className={cn("mt-0! transition-opacity", dimmed && "opacity-25")}
      >
        <MessageBody text={textOf(message)} query={query} />
        {picturesBelow.length > 0 && <div className="mt-2"><Attached media={picturesBelow} /></div>}
        {message.scene_image && <SceneImageStatus conversationId={conversationId} message={message} onReview={onSceneImage} />}
      </Message>
    </div>
  );
}

export const MessageRow = memo(Row);
