import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Avatar, EmptyState, ListGroup, ListRowButton, LoadingState, SearchField, Sheet } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import { FailureState } from "@/shared/ui/FailureState";
import { libraryCastQuery } from "../../data/settings-queries";

const searchFrom = 8;

export interface AddMemberSheetProps {
  api: ApiClient;
  open: boolean;
  taken: ReadonlySet<string>;
  busy: boolean;
  onClose: () => void;
  onPick: (characterId: string) => void;
}

export function AddMemberSheet({ api, open, taken, busy, onClose, onPick }: AddMemberSheetProps) {
  const { t } = useTranslation("conversations");
  const cast = useQuery({ ...libraryCastQuery(api), enabled: open });
  const [query, setQuery] = useState("");
  const close = () => {
    setQuery("");
    onClose();
  };
  const candidates = (cast.data ?? []).filter((character) => !taken.has(character.id));
  const needle = query.trim().toLowerCase();
  const shown = candidates.filter((character) => !needle || character.name.toLowerCase().includes(needle));
  return (
    <Sheet open={open} onClose={close} size="sm" title={t("members.add.title")} description={t("members.add.body")}>
      <div className="space-y-3 pb-1">
        {candidates.length > searchFrom && (
          <SearchField value={query} onChange={setQuery} placeholder={t("members.add.search")} aria-label={t("members.add.search")} />
        )}
        {cast.isPending ? (
          <LoadingState />
        ) : cast.isError ? (
          <FailureState error={cast.error} onRetry={() => void cast.refetch()} />
        ) : candidates.length === 0 ? (
          <EmptyState size="sm" title={t("members.add.empty")} />
        ) : shown.length === 0 ? (
          <p className="py-4 text-center text-sm text-fg-3">{t("members.add.noMatch")}</p>
        ) : (
          <ListGroup>
            {shown.map((character) => (
              <ListRowButton
                key={character.id}
                icon={<Avatar src={character.avatar?.url ?? null} name={character.name} size="sm" />}
                title={character.name}
                {...(character.description ? { description: <span className="line-clamp-2">{character.description}</span> } : {})}
                chevron={false}
                disabled={busy}
                onClick={() => {
                  close();
                  onPick(character.id);
                }}
              />
            ))}
          </ListGroup>
        )}
      </div>
    </Sheet>
  );
}
