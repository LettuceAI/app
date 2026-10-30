import { useState } from "react";
import { Check, Users } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, icon, ListGroup, ListRowButton, SearchField, Sheet, Text } from "@lettuceai/crisp";

export interface Speaker {
  id: string;
  name: string;
  avatar: string | null;
  characterId: string | null;
  description?: string | null;
  muted: boolean;
}

export interface SpeakerSheetProps {
  open: boolean;
  purpose?: "speak" | "branch" | "regenerate";
  selectedId?: string | null;
  originalName?: string;
  speakers: readonly Speaker[];
  onClose: () => void;
  onPick: (speaker: Speaker | null) => void;
}

export function SpeakerSheet({ open, purpose = "speak", speakers, selectedId, originalName, onClose, onPick }: SpeakerSheetProps) {
  const { t } = useTranslation("conversations");
  const [query, setQuery] = useState("");
  const [wasOpen, setWasOpen] = useState(open);
  if (open !== wasOpen) {
    setWasOpen(open);
    if (open) setQuery("");
  }
  const needle = query.trim().toLocaleLowerCase();
  const shown = speakers.filter((speaker) => speaker.name.toLocaleLowerCase().includes(needle));
  const pick = (speaker: Speaker | null) => {
    onClose();
    onPick(speaker);
  };
  return (
    <Sheet open={open} onClose={onClose} size="sm" title={t(`speaker.${purpose}.title`)} description={t(`speaker.${purpose}.body`)}>
      <div className="space-y-3">
      {purpose === "regenerate" && speakers.length > 8 && <SearchField value={query} onChange={setQuery} placeholder={t("speaker.regenerate.search")} aria-label={t("speaker.regenerate.search")} />}
      <ListGroup className="mb-1">
        {purpose !== "branch" && (
          <ListRowButton
            icon={<Users size={icon.lg} />}
            title={t(purpose === "regenerate" ? "speaker.regenerate.same" : "speaker.auto")}
            description={purpose === "regenerate" ? originalName : t("speaker.autoBody")}
            trailing={selectedId === null ? <Check size={icon.md} /> : undefined}
            chevron={false}
            onClick={() => pick(null)}
          />
        )}
        {shown.map((speaker) => (
          <ListRowButton
            key={speaker.id}
            icon={<Avatar src={speaker.avatar} name={speaker.name} size="sm" />}
            title={speaker.name}
            {...(speaker.muted && purpose !== "branch" ? { description: t("speaker.muted") } : {})}
            trailing={selectedId === speaker.id ? <Check size={icon.md} /> : undefined}
            chevron={false}
            onClick={() => pick(speaker)}
          />
        ))}
      </ListGroup>
      {query && shown.length === 0 && <Text variant="hint">{t("speaker.regenerate.noMatches")}</Text>}
      </div>
    </Sheet>
  );
}
