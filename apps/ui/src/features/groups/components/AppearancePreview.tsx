import { ArrowLeft } from "lucide-react";
import { useTranslation } from "react-i18next";
import { icon, IconButton, Text } from "@lettuceai/crisp";
import type { ChatLook } from "@/entities/preferences";
import { ChatPreview, PreviewBackground } from "@/features/settings";

export function AppearancePreviewPanel({ look, image, onImage, group = true }: { look: ChatLook; image: boolean; onImage: (image: boolean) => void; group?: boolean }) {
  const { t } = useTranslation("groups");
  return <aside className="sticky top-[var(--page-bar-height)] flex h-[calc(var(--scroller-height)-var(--page-bar-height))] w-130 shrink-0 flex-col gap-2 pb-4">
    <div className="flex items-center justify-between gap-3 px-1">
      <Text variant="label">{t("appearance.preview")}</Text>
      <PreviewBackground image={image} onChange={onImage} />
    </div>
    <ChatPreview className="flex-1" look={look} group={group} image={image} />
  </aside>;
}

export function AppearancePreviewOverlay({ look, image, onImage, onClose, group = true }: { look: ChatLook; image: boolean; onImage: (image: boolean) => void; onClose: () => void; group?: boolean }) {
  const { t } = useTranslation("groups");
  return <div role="dialog" aria-modal="true" aria-label={t("appearance.preview")} className="absolute inset-0 z-40 flex flex-col bg-surface pt-[var(--safe-top)]">
    <div className="flex shrink-0 items-center gap-2 border-b border-line px-4 py-2">
      <IconButton label={t("create.back")} shape="round" onClick={onClose}><ArrowLeft size={icon.lg} /></IconButton>
      <Text variant="title" className="min-w-0 flex-1 truncate">{t("appearance.preview")}</Text>
      <PreviewBackground image={image} onChange={onImage} />
    </div>
    <ChatPreview className="min-h-0 flex-1 p-4" look={look} group={group} image={image} />
  </div>;
}
