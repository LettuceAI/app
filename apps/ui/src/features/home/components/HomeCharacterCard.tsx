import { useTranslation } from "react-i18next";
import { CharacterCard, cn, InlineMarkdown, useLongPress } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { CharacterSummary } from "@/api/generated/bindings";
import { useCardColors } from "../data/gradient";

export interface HomeCharacterCardProps {
  api: ApiClient;
  character: CharacterSummary;
  onMenu: () => void;
  held: boolean;
}

export function HomeCharacterCard({ api, character, onMenu, held }: HomeCharacterCardProps) {
  const { t } = useTranslation("home");
  const colors = useCardColors(api, character);
  const press = useLongPress(onMenu);
  return (
    <CharacterCard
      name={character.name}
      description={character.description ? <InlineMarkdown>{character.description}</InlineMarkdown> : t("noDescription")}
      avatar={character.avatar?.url ?? null}
      gradient={colors?.css ?? null}
      fg={colors?.fg ?? null}
      fgMuted={colors?.fgMuted ?? null}
      variant={character.card_style === "banner" ? "banner" : "row"}
      aria-haspopup="dialog"
      {...press.handlers}
      className={cn(held && "ring-2 ring-fg/40")}
    />
  );
}
