import { useMemo } from "react";
import { useI18n } from "../../../../core/i18n";

interface AvatarPreviewProps {
  avatarPath: string;
  name: string;
}

export function AvatarPreview({ avatarPath, name }: AvatarPreviewProps) {
  const { t } = useI18n();

  const preview = useMemo(() => {
    if (!avatarPath) {
      const initial = name.trim().charAt(0) || "?";
      return (
        <div className="flex h-full w-full items-center justify-center bg-gradient-to-br from-accent/20 to-info/80/20">
          <span className="text-3xl font-bold text-fg">{initial.toUpperCase()}</span>
        </div>
      );
    }

    return (
      <img
        src={avatarPath}
        alt={t("characters.preview.avatarAlt")}
        className="h-full w-full object-cover"
      />
    );
  }, [avatarPath, name, t]);

  return <>{preview}</>;
}
