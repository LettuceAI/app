import type { ImageNode } from "../../../../../core/storage/chatWidgetSchemas";
import { cn } from "../../../../design-tokens";
import { useI18n } from "../../../../../core/i18n/context";
import { useAvatar } from "../../../../hooks/useAvatar";
import { useImageData } from "../../../../hooks/useImageData";
import { useWidgetContext } from "./WidgetContext";
import { widgetCardClass } from "./widgetSurface";

interface WidgetImageProps {
  node: ImageNode;
}

export function WidgetImage({ node }: WidgetImageProps) {
  const { t } = useI18n();
  const { character, persona, hasBackground } = useWidgetContext();
  const shape = node.shape ?? "auto";
  const characterAvatarUrl = useAvatar(
    "character",
    character?.id,
    character?.avatarPath,
    "base",
  );
  const personaAvatarUrl = useAvatar(
    "persona",
    persona?.id,
    persona?.avatarPath,
    "base",
  );
  const libraryPath = node.source.kind === "library" ? node.source.path : undefined;
  const uploadPath = node.source.kind === "upload" ? node.source.path : undefined;
  const libraryUrl = useImageData(libraryPath);
  const uploadUrl = useImageData(uploadPath);

  let url: string | undefined;
  let alt = node.title ?? t("chats.widgets.image.alt");
  switch (node.source.kind) {
    case "character_avatar":
      url = characterAvatarUrl;
      alt = character?.name ?? alt;
      break;
    case "persona_avatar":
      url = personaAvatarUrl;
      alt = persona?.title ?? alt;
      break;
    case "library":
      url = libraryUrl;
      break;
    case "upload":
      url = uploadUrl;
      break;
  }

  return (
    <section className="flex flex-col gap-1.5">
      {(node.title || node.description) && (
        <header className="flex flex-col gap-0.5 px-0.5">
          {node.title && (
            <h3 className="text-sm font-semibold text-fg/75">{node.title}</h3>
          )}
          {node.description && (
            <p className="text-[11px] leading-snug text-fg/45">{node.description}</p>
          )}
        </header>
      )}
      <div
        className={cn(
          "overflow-hidden",
          shape === "circle" ? "mx-auto rounded-full" : "rounded-xl",
          shape === "circle" && "aspect-square w-1/2 max-w-[160px]",
          widgetCardClass(hasBackground, node.design),
        )}
      >
        {url ? (
          <img
            src={url}
            alt={alt}
            className={cn(
              "block w-full",
              shape === "auto" ? "h-auto object-cover" : "h-full object-cover",
              shape === "square" && "aspect-square",
              shape === "wide" && "aspect-video",
              shape === "circle" && "aspect-square",
            )}
          />
        ) : (
          <div
            className={cn(
              "flex items-center justify-center text-[12px] italic text-fg/35",
              shape === "circle" ? "aspect-square" : "aspect-video",
            )}
          >
            {t("chats.widgets.image.noImage")}
          </div>
        )}
      </div>
    </section>
  );
}
