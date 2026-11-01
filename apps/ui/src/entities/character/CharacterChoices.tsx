import { Avatar, ChoiceCard } from "@lettuceai/crisp";

export interface CharacterChoice {
  id: string;
  name: string;
  description: string | null;
  avatar: { url: string } | null;
}

export function CharacterChoices({ choices, selected, locked = [], disabled = false, onPick }: {
  choices: readonly CharacterChoice[];
  selected: readonly string[];
  locked?: readonly string[];
  disabled?: boolean;
  onPick: (id: string) => void;
}) {
  return <div className="grid grid-cols-1 gap-2 @3xl:grid-cols-2 @6xl:grid-cols-3">
    {choices.map((character) => <ChoiceCard
      key={character.id}
      selected={selected.includes(character.id)}
      indicator="checkbox"
      disabled={disabled || locked.includes(character.id)}
      icon={<Avatar src={character.avatar?.url ?? null} name={character.name} size="md" />}
      title={<span className="block truncate">{character.name}</span>}
      description={<span className="line-clamp-1">{character.description}</span>}
      className="p-3"
      onClick={() => onPick(character.id)}
    />)}
  </div>;
}
