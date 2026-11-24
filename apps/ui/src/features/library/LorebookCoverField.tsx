import { BookOpen, Upload, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, icon, IconButton } from "@lettuceai/crisp";
import type { AssetRef } from "@/api/generated/bindings";
import { AvatarLibraryButton } from "@/entities/image-input";

export function LorebookCoverField({
  cover,
  busy,
  onUpload,
  onChoose,
  onRemove,
}: {
  cover: AssetRef | null;
  busy: boolean;
  onUpload: () => void;
  onChoose: () => void;
  onRemove: () => void;
}) {
  const { t } = useTranslation("lorebook-editor");
  const { t: common } = useTranslation();
  const { t: images } = useTranslation("image-input");

  return (
    <section aria-label={t("coverImage")} aria-busy={busy} className="flex items-center gap-4">
      <div className="relative grid size-20 shrink-0 place-items-center rounded-2xl border border-line bg-surface-2 text-fg-muted">
        {cover ? (
          <img
            alt={t("coverImage")}
            src={cover.preview_url ?? cover.url}
            className="size-full rounded-2xl object-cover"
          />
        ) : (
          <BookOpen size={icon.xl} aria-hidden />
        )}
        {cover && (
          <IconButton
            label={common("designReferences.remove")}
            size="sm"
            className="absolute -end-2 -top-2 rounded-full border border-line bg-surface-2"
            disabled={busy}
            onClick={onRemove}
          >
            <X size={icon.sm} />
          </IconButton>
        )}
      </div>
      <div className="min-w-0 space-y-2">
        <p className="text-sm font-medium text-fg">{t("coverImage")}</p>
        <div className="flex flex-wrap gap-1">
          <Button
            size="sm"
            variant="secondary"
            leading={<Upload size={icon.sm} />}
            disabled={busy}
            onClick={onUpload}
          >
            {images("upload.action")}
          </Button>
          <AvatarLibraryButton disabled={busy} onClick={onChoose} />
        </div>
      </div>
    </section>
  );
}
