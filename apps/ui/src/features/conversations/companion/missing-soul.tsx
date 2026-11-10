import { useState, type ReactNode } from "react";
import { useQueries, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Button, Dialog, ListGroup, ListRow } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { CompanionSoulConfigView } from "@/api/generated/bindings";
import { characterQuery } from "../data/settings-queries";

const soulFields = ["essence", "traits", "backstory", "appearance", "goals", "likes", "voice", "relational_style", "vulnerabilities", "fears", "habits", "boundaries"] as const;

export function hasSoul(config: CompanionSoulConfigView | null): boolean {
  if (config === null) return false;
  return config.authored_facts.length > 0 || soulFields.some((field) => config.soul[field].trim().length > 0);
}

/** The ids among `characterIds` whose character has no Soul written. */
export function useMissingSouls(api: ApiClient, characterIds: readonly string[], enabled = true): ReadonlySet<string> {
  const characters = useQueries({ queries: characterIds.map((id) => ({ ...characterQuery(api, id), enabled })) });
  return new Set(characters.flatMap((character, index) => character.data && !hasSoul(character.data.draft.defaults.companion_soul) ? [characterIds[index]!] : []));
}

interface Missing {
  id: string;
  name: string;
}

export function useMissingSoulWarning(api: ApiClient, { editable = true }: { editable?: boolean } = {}): { warn: (characterIds: readonly string[]) => Promise<void>; dialog: ReactNode } {
  const { t, i18n } = useTranslation("conversations");
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [missing, setMissing] = useState<Missing[]>([]);
  const warn = async (characterIds: readonly string[]) => {
    const found = await Promise.all([...new Set(characterIds)].map(async (id): Promise<Missing | null> => {
      try {
        const character = await queryClient.fetchQuery(characterQuery(api, id));
        return hasSoul(character.draft.defaults.companion_soul) ? null : { id, name: character.draft.profile.name };
      } catch {
        return null;
      }
    }));
    const list = found.filter((entry) => entry !== null);
    if (list.length > 0) setMissing(list);
  };
  const close = () => setMissing([]);
  const editSoul = (characterId: string) => {
    close();
    void navigate({ to: "/characters/$characterId/edit", params: { characterId }, search: { tab: "soul" } });
  };
  const dialog = <Dialog open={missing.length > 0} onClose={close} title={t("companion.soulWarning.title", { count: missing.length })} description={t("companion.soulWarning.body", { count: missing.length, names: new Intl.ListFormat(i18n.language, { style: "long", type: "conjunction" }).format(missing.map((entry) => entry.name)) })} footer={<div className="flex justify-end gap-2"><Button variant="primary" onClick={close}>{t("companion.soulWarning.continue")}</Button></div>}>
    <ListGroup>{missing.map((entry) => <ListRow key={entry.id} title={entry.name} trailing={editable ? <Button size="sm" variant="secondary" aria-label={t("companion.soulWarning.editFor", { name: entry.name })} onClick={() => editSoul(entry.id)}>{t("companion.soulWarning.edit")}</Button> : undefined} />)}</ListGroup>
  </Dialog>;
  return { warn, dialog };
}
