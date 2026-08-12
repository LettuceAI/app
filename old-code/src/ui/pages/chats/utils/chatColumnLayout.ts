import type { CSSProperties } from "react";
import type { ChatAppearanceSettings } from "../../../../core/storage/schemas";

const PRESET_PX: Record<
  Exclude<ChatAppearanceSettings["chatColumnWidth"], "full" | "custom">,
  number
> = {
  narrow: 640,
  normal: 800,
  wide: 1024,
  xl: 1280,
};

const ALIGN_CLASS: Record<ChatAppearanceSettings["chatColumnAlign"], string> = {
  left: "lg:mr-auto lg:ml-0",
  center: "lg:mx-auto",
  right: "lg:ml-auto lg:mr-0",
};

export interface ChatColumnLayout {
  className: string;
  style: CSSProperties;
  isConstrained: boolean;
}

export function getChatColumnLayout(
  appearance: ChatAppearanceSettings,
): ChatColumnLayout {
  const width = appearance.chatColumnWidth;
  if (width === "full") {
    return { className: "", style: {}, isConstrained: false };
  }
  const px =
    width === "custom"
      ? Math.min(2400, Math.max(400, appearance.chatColumnWidthPx ?? 800))
      : PRESET_PX[width];
  return {
    className: `w-full ${ALIGN_CLASS[appearance.chatColumnAlign]}`,
    style: { maxWidth: `min(100%, ${px}px)` },
    isConstrained: true,
  };
}
