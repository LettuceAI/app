import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { User, MessageSquare, Sparkles } from "lucide-react";
import { cn, typography } from "../../../design-tokens";
import { useI18n } from "../../../../core/i18n/context";
import { loadAvatar } from "../../../../core/storage/avatars";
import { convertToImageUrl } from "../../../../core/storage/images";
import { isRenderableImageUrl } from "../../../../core/utils/image";

interface DraftScene {
  id: string;
  content: string;
  direction: string | null;
}

interface DraftCharacter {
  name: string | null;
  definition?: string | null;
  description: string | null;
  scenes: DraftScene[];
  defaultSceneId: string | null;
  avatarPath: string | null;
  backgroundImagePath: string | null;
  disableAvatarGradient: boolean;
  defaultModelId: string | null;
  promptTemplateId: string | null;
}

interface CharacterPreviewCardProps {
  draft: DraftCharacter;
  compact?: boolean;
  sessionId?: string;
  targetCharacterId?: string;
}

interface UploadedImage {
  data: string;
  assetId?: string | null;
}

export function CharacterPreviewCard({
  draft,
  compact = false,
  sessionId,
  targetCharacterId,
}: CharacterPreviewCardProps) {
  const { t } = useI18n();
  const hasAvatar = draft.avatarPath && draft.avatarPath.length > 0;
  const defaultScene = draft.scenes.find((s) => s.id === draft.defaultSceneId) || draft.scenes[0];
  const previewDescription = draft.description || draft.definition || null;

  const [avatarSrc, setAvatarSrc] = useState<string | null>(null);
  const [bgSrc, setBgSrc] = useState<string | null>(null);

  // Resolve images
  useEffect(() => {
    const resolveImage = async (
      path: string | null,
      kind: "avatar" | "background",
      setSrc: (s: string | null) => void,
    ) => {
      if (!path) {
        setSrc(null);
        return;
      }

      if (isRenderableImageUrl(path)) {
        setSrc(path);
        return;
      }

      // If it's a UUID (short string) and we have a session, try to fetch it
      if (sessionId && path.length < 100) {
        // UUID is 36 chars
        try {
          const img = await invoke<UploadedImage | null>("creation_helper_get_uploaded_image", {
            sessionId,
            imageId: path,
          });
          if (img) {
            if (img.data && isRenderableImageUrl(img.data)) {
              setSrc(img.data);
              return;
            }
            if (img.assetId) {
              setSrc((await convertToImageUrl(img.assetId)) ?? null);
              return;
            }
            return;
          }
        } catch (e) {
          console.error("Failed to resolve image:", path, e);
        }
      }

      try {
        if (kind === "avatar" && targetCharacterId) {
          setSrc((await loadAvatar("character", targetCharacterId, path)) ?? null);
          return;
        }
        if (kind === "background") {
          setSrc((await convertToImageUrl(path)) ?? null);
          return;
        }
      } catch (error) {
        console.error("Failed to resolve persisted preview image:", path, error);
      }

      setSrc(null);
    };

    resolveImage(draft.avatarPath, "avatar", setAvatarSrc);
    resolveImage(draft.backgroundImagePath, "background", setBgSrc);
  }, [draft.avatarPath, draft.backgroundImagePath, sessionId, targetCharacterId]);

  return (
    <div className="rounded-2xl border border-fg/10 bg-fg/5 overflow-hidden">
      {/* Background Image Preview */}
      {bgSrc && (
        <div
          className="h-24 bg-cover bg-center"
          style={{
            backgroundImage: `linear-gradient(rgba(5,5,5,0.3), rgba(5,5,5,0.8)), url(${bgSrc})`,
          }}
        />
      )}

      <div className={cn("p-4", bgSrc && "-mt-8 relative")}>
        {/* Avatar + Name Row */}
        <div className="flex items-start gap-3">
          {/* Avatar */}
          <div
            className={cn(
              "shrink-0 overflow-hidden rounded-full border-2",
              hasAvatar ? "border-fg/20" : "border-fg/10 bg-fg/5",
              compact ? "h-12 w-12" : "h-16 w-16",
            )}
          >
            {avatarSrc ? (
              <img
                src={avatarSrc}
                alt={draft.name || t("characters.preview.characterFallback")}
                className="h-full w-full object-cover"
              />
            ) : (
              <div className="h-full w-full flex items-center justify-center">
                <User className="h-6 w-6 text-fg/30" />
              </div>
            )}
          </div>

          {/* Name + Stats */}
          <div className="flex-1 min-w-0">
            <h3 className={cn(typography.h2.size, typography.h2.weight, "text-fg truncate")}>
              {draft.name || t("characters.preview.unnamedCharacter")}
            </h3>

            <div className="flex items-center gap-3 mt-1">
              {draft.scenes.length > 0 && (
                <span className="flex items-center gap-1 text-xs text-fg/50">
                  <MessageSquare className="h-3 w-3" />
                  {t("characters.preview.sceneCount", { count: draft.scenes.length })}
                </span>
              )}
              {draft.promptTemplateId && (
                <span className="flex items-center gap-1 text-xs text-fg/50">
                  <Sparkles className="h-3 w-3" />
                  {t("characters.preview.customPrompt")}
                </span>
              )}
            </div>
          </div>
        </div>

        {/* Description Preview */}
        {!compact && previewDescription && (
          <div className="mt-4">
            <p className="text-xs text-fg/40 uppercase tracking-wider mb-1">
              {t("characters.preview.description")}
            </p>
            <p className="text-sm text-fg/70 line-clamp-3">{previewDescription}</p>
          </div>
        )}

        {/* Scene Preview */}
        {!compact && defaultScene && (
          <div className="mt-4">
            <p className="text-xs text-fg/40 uppercase tracking-wider mb-1">
              {t("characters.preview.startingScene")}
            </p>
            <p className="text-sm text-fg/60 italic line-clamp-2">
              "{defaultScene.content.slice(0, 150)}
              {defaultScene.content.length > 150 ? "..." : ""}"
            </p>
          </div>
        )}

        {/* Settings Badges */}
        {!compact && (draft.defaultModelId || !draft.disableAvatarGradient) && (
          <div className="mt-4 flex flex-wrap gap-1.5">
            {!draft.disableAvatarGradient && (
              <span className="text-[10px] px-2 py-0.5 rounded-full bg-fg/5 border border-fg/10 text-fg/50">
                {t("characters.preview.gradientEnabled")}
              </span>
            )}
            {draft.defaultModelId && (
              <span className="text-[10px] px-2 py-0.5 rounded-full bg-fg/5 border border-fg/10 text-fg/50">
                {t("characters.preview.customModel")}
              </span>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
