import { User } from "lucide-react";
import { cn } from "../../../../design-tokens";
import type { PersonaInfoNode } from "../../../../../core/storage/chatWidgetSchemas";
import { AvatarImage } from "../../../../components/AvatarImage";
import { useI18n } from "../../../../../core/i18n/context";
import { useAvatar } from "../../../../hooks/useAvatar";
import { useWidgetContext } from "./WidgetContext";
import { widgetCardClass } from "./widgetSurface";

export function WidgetPersonaInfo({ node }: { node: PersonaInfoNode }) {
  const { t } = useI18n();
  const { persona, hasBackground } = useWidgetContext();
  const avatarUrl = useAvatar("persona", persona?.id ?? "", persona?.avatarPath, "round");

  if (!persona) {
    return (
      <section
        className={cn(
          "rounded-xl px-3 py-3 text-[12px] italic text-fg/40",
          widgetCardClass(hasBackground, node.design),
        )}
      >
        {t("chats.widgets.personaInfo.noPersona")}
      </section>
    );
  }

  return (
    <section
      className={cn(
        "flex flex-col gap-2 rounded-xl px-3 py-3",
        widgetCardClass(hasBackground, node.design),
      )}
    >
      <header className="flex items-center gap-3">
        <div className="relative flex h-12 w-12 shrink-0 items-center justify-center overflow-hidden rounded-full border border-fg/15 bg-fg/5 ring-1 ring-white/15">
          {avatarUrl ? (
            <AvatarImage
              src={avatarUrl}
              alt={persona.title}
              crop={persona.avatarCrop}
              applyCrop
            />
          ) : (
            <User className="h-5 w-5 text-fg/60" />
          )}
        </div>
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-semibold text-fg/85">
            {persona.title}
          </div>
          {persona.nickname && (
            <div className="truncate text-[11px] text-fg/50">{persona.nickname}</div>
          )}
        </div>
      </header>
      {persona.description && (
        <p className="line-clamp-6 text-[12px] leading-relaxed text-fg/65">
          {persona.description}
        </p>
      )}
    </section>
  );
}
