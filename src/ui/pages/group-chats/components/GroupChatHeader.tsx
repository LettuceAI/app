import { ArrowLeft, Settings, Brain } from "lucide-react";

import type { GroupSession, Character } from "../../../../core/storage/schemas";
import { AvatarImage } from "../../../components/AvatarImage";
import { cn } from "../../../design-tokens";
import { useAvatar } from "../../../hooks/useAvatar";

export function GroupChatHeader({
  session,
  characters,
  onBack,
  onSettings,
  onMemories,
  hasBackgroundImage,
}: {
  session: GroupSession;
  characters: Character[];
  onBack: () => void;
  onSettings: () => void;
  onMemories: () => void;
  hasBackgroundImage?: boolean;
}) {
  return (
    <header
      className={cn(
        "border-b border-fg/10 px-4 pb-3 pt-3",
        hasBackgroundImage && "backdrop-blur-xl",
      )}
    >
      <div className="flex items-center">
        <button
          onClick={onBack}
          className="flex shrink-0 px-[0.6em] py-[0.3em] items-center justify-center -ml-2 text-fg transition hover:text-fg/80"
          aria-label="Back"
        >
          <ArrowLeft size={14} strokeWidth={2.5} />
        </button>

        <div className="min-w-0 flex-1 ml-2">
          <h1 className="h1-group-chat truncate text-lg font-bold text-fg/90">{session.name}</h1>
          <p className="truncate text-xs text-fg/50">{characters.length} characters</p>
        </div>

        <div className="relative flex items-center mr-3">
          {characters.slice(0, 4).map((char, index) => (
            <CharacterMiniAvatar
              key={char.id}
              character={char}
              index={index}
              total={Math.min(characters.length, 4)}
            />
          ))}
          {characters.length > 4 && (
            <div
              className={cn(
                "h-8 w-8 rounded-full",
                "bg-linear-to-br from-secondary/30 to-info/80/30",
                "flex items-center justify-center",
                "text-[10px] font-semibold text-fg shadow-lg",
                "ring-1 ring-white/20",
              )}
              style={{ marginLeft: "-8px", zIndex: 0 }}
            >
              +{characters.length - 4}
            </div>
          )}
        </div>

        <button
          onClick={onMemories}
          className="flex items-center px-[0.6em] py-[0.3em] justify-center text-fg/70 hover:text-fg transition"
          aria-label="Memories"
        >
          <Brain size={14} />
        </button>

        <button
          onClick={onSettings}
          className="flex items-center px-[0.6em] py-[0.3em] justify-center text-fg/70 hover:text-fg transition"
          aria-label="Settings"
        >
          <Settings size={14} />
        </button>
      </div>
    </header>
  );
}

function CharacterMiniAvatar({
  character,
  index,
  total,
}: {
  character: Character;
  index: number;
  total: number;
}) {
  const avatarUrl = useAvatar("character", character.id, character.avatarPath, "round");

  return (
    <div
      className={cn(
        "h-8 w-8 rounded-full overflow-hidden",
        "bg-linear-to-br from-white/10 to-white/5",
        "shadow-lg ring-1 ring-white/20",
        "transition-transform hover:scale-110 hover:z-50",
      )}
      style={{
        marginLeft: index > 0 ? "-10px" : "0",
        zIndex: total - index,
      }}
    >
      {avatarUrl ? (
        <AvatarImage src={avatarUrl} alt={character.name} crop={character.avatarCrop} applyCrop />
      ) : (
        <div className="flex h-full w-full items-center justify-center bg-linear-to-br from-secondary/40 to-info/80/40 text-[11px] font-bold text-fg">
          {character.name.slice(0, 1).toUpperCase()}
        </div>
      )}
    </div>
  );
}
