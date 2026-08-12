import { cn } from "../../../../design-tokens";
import type { CharacterInfoNode } from "../../../../../core/storage/chatWidgetSchemas";
import { useI18n } from "../../../../../core/i18n/context";
import { CharacterAvatar } from "../CharacterAvatar";
import { useWidgetContext } from "./WidgetContext";
import { widgetCardClass } from "./widgetSurface";

export function WidgetCharacterInfo({ node }: { node: CharacterInfoNode }) {
  const { t } = useI18n();
  const { character: contextCharacter, characters, hasBackground } = useWidgetContext();
  const character = node.characterId
    ? (characters?.find((c) => c.id === node.characterId) ?? contextCharacter)
    : contextCharacter;

  if (!character) {
    return (
      <section
        className={cn(
          "rounded-xl px-3 py-3 text-[12px] italic text-fg/40",
          widgetCardClass(hasBackground, node.design),
        )}
      >
        {t("chats.widgets.characterInfo.noCharacter")}
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
        <div
          className={cn(
            "relative h-12 w-12 shrink-0 overflow-hidden rounded-full ring-1 ring-white/15",
          )}
        >
          <CharacterAvatar character={character} />
        </div>
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-semibold text-fg/85">
            {character.name}
          </div>
          {character.nickname && (
            <div className="truncate text-[11px] text-fg/50">
              {character.nickname}
            </div>
          )}
        </div>
      </header>
      {character.description && (
        <p className="line-clamp-6 text-[12px] leading-relaxed text-fg/65">
          {character.description}
        </p>
      )}
    </section>
  );
}
