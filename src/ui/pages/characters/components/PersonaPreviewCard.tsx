import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { User } from "lucide-react";
import { cn, typography } from "../../../design-tokens";

interface PersonaPreview {
  id?: string;
  title?: string;
  description?: string;
  avatarPath?: string | null;
  isDefault?: boolean;
}

interface UploadedImage {
  data: string;
}

export function PersonaPreviewCard({
  persona,
  sessionId,
}: {
  persona: PersonaPreview | null;
  sessionId?: string;
}) {
  const [avatarSrc, setAvatarSrc] = useState<string | null>(null);

  const title = persona?.title?.trim() || "Unnamed Persona";
  const description = persona?.description?.trim() || "No description yet";
  const avatarPath = persona?.avatarPath ?? null;
  const isDefault = persona?.isDefault ?? false;

  useEffect(() => {
    const resolveImage = async (path: string | null, setSrc: (s: string | null) => void) => {
      if (!path) {
        setSrc(null);
        return;
      }

      if (path.startsWith("data:") || path.startsWith("http") || path.startsWith("blob:")) {
        setSrc(path);
        return;
      }

      if (sessionId && path.length < 100) {
        try {
          const img = await invoke<UploadedImage | null>("creation_helper_get_uploaded_image", {
            sessionId,
            imageId: path,
          });
          if (img) {
            setSrc(img.data.startsWith("data:") ? img.data : `data:image/png;base64,${img.data}`);
            return;
          }
        } catch (e) {
          console.error("Failed to resolve persona avatar:", path, e);
        }
      }

      setSrc(`data:image/png;base64,${path}`);
    };

    resolveImage(avatarPath, setAvatarSrc);
  }, [avatarPath, sessionId]);

  return (
    <div className="rounded-2xl border border-fg/10 bg-fg/5 overflow-hidden">
      <div className="p-4">
        <div className="flex items-start gap-3">
          <div
            className={cn(
              "shrink-0 overflow-hidden rounded-full border-2",
              avatarSrc ? "border-fg/20" : "border-fg/10 bg-fg/5",
              "h-16 w-16",
            )}
          >
            {avatarSrc ? (
              <img src={avatarSrc} alt={title} className="h-full w-full object-cover" />
            ) : (
              <div className="h-full w-full flex items-center justify-center">
                <User className="h-6 w-6 text-fg/30" />
              </div>
            )}
          </div>

          <div className="flex-1 min-w-0">
            <h3 className={cn(typography.h2.size, typography.h2.weight, "text-fg truncate")}>
              {title}
            </h3>
            <div className="flex items-center gap-2 mt-1">
              {isDefault && (
                <span className="text-[10px] px-2 py-0.5 rounded-full bg-accent/15 border border-accent/30 text-accent/80">
                  Default
                </span>
              )}
            </div>
          </div>
        </div>

        <div className="mt-4">
          <p className="text-xs text-fg/40 uppercase tracking-wider mb-1">Description</p>
          <p className="text-sm text-fg/70 line-clamp-4">{description}</p>
        </div>
      </div>
    </div>
  );
}
