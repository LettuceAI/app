import { Users } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, icon, ListGroup, ListRowButton, Sheet } from "@lettuceai/crisp";

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
  purpose?: "speak" | "branch";
  speakers: readonly Speaker[];
  onClose: () => void;
  onPick: (speaker: Speaker | null) => void;
}

export function SpeakerSheet({ open, purpose = "speak", speakers, onClose, onPick }: SpeakerSheetProps) {
  const { t } = useTranslation("conversations");
  const pick = (speaker: Speaker | null) => {
    onClose();
    onPick(speaker);
  };
  return (
    <Sheet open={open} onClose={onClose} size="sm" title={t(`speaker.${purpose}.title`)} description={t(`speaker.${purpose}.body`)}>
      <ListGroup className="mb-1">
        {purpose === "speak" && (
          <ListRowButton
            icon={<Users size={icon.lg} />}
            title={t("speaker.auto")}
            description={t("speaker.autoBody")}
            chevron={false}
            onClick={() => pick(null)}
          />
        )}
        {speakers.map((speaker) => (
          <ListRowButton
            key={speaker.id}
            icon={<Avatar src={speaker.avatar} name={speaker.name} size="sm" />}
            title={speaker.name}
            {...(speaker.muted && purpose === "speak" ? { description: t("speaker.muted") } : {})}
            chevron={false}
            onClick={() => pick(speaker)}
          />
        ))}
      </ListGroup>
    </Sheet>
  );
}
