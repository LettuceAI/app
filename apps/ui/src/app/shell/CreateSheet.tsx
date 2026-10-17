import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { BookOpen, FileUp, UserPlus, UserRound } from "lucide-react";
import { useTranslation } from "react-i18next";
import { icon, ListGroup, ListRowButton, Sheet, toast } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import { isApiFailure } from "@/api/client";
import { importCharacterCard } from "@/features/home";

export function CreateSheet({ api, open, onClose, onCreate, onDiscover }: { api: ApiClient; open: boolean; onClose: () => void; onCreate: (kind: "character" | "persona") => void; onDiscover: () => void }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [busy, setBusy] = useState(false);

  const importCard = () => {
    if (busy) return;
    onClose();
    setBusy(true);
    importCharacterCard({ api, queryClient })
      .then((imported) => {
        if (imported) toast.success(t("create.imported"));
      })
      .catch((error: unknown) => {
        toast.error(t(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
      })
      .finally(() => setBusy(false));
  };

  return (
    <Sheet open={open} onClose={onClose} size="sm" title={t("create.title")} description={t("create.description")}>
      <div className="space-y-4 pb-1">
        <ListGroup>
          <ListRowButton
            icon={<UserPlus size={icon.lg} />}
            title={t("create.character")}
            description={t("create.characterBody")}
            onClick={() => {
              onClose();
              onCreate("character");
            }}
          />
          <ListRowButton
            icon={<UserRound size={icon.lg} />}
            title={t("create.persona")}
            description={t("create.personaBody")}
            onClick={() => {
              onClose();
              onCreate("persona");
            }}
          />
          <ListRowButton icon={<FileUp size={icon.lg} />} title={t("create.import")} description={t("create.importBody")} chevron={false} disabled={busy} onClick={importCard} />
        </ListGroup>
        <ListGroup>
          <ListRowButton
            icon={<BookOpen size={icon.lg} />}
            title={t("create.discover")}
            description={t("create.discoverBody")}
            onClick={() => {
              onClose();
              onDiscover();
            }}
          />
        </ListGroup>
      </div>
    </Sheet>
  );
}
