import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useBlocker } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Avatar, Button, dismissTopLayer, Field, icon, ListGroup, ListRowButton, LoadingState, Sheet, Text, Textarea } from "@lettuceai/crisp";
import { Cpu as CpuIcon } from "lucide-react";
import type { ApiClient } from "@/api/client";
import { ModelMark } from "@/entities/provider";
import { ModelChoiceSheet } from "@/features/characters";
import { FailureState } from "@/shared/ui/FailureState";
import { settingsQuery } from "../data/queries";
import { useChatModels } from "../data/use-models";
import type { RegenerationOptions } from "../data/use-generation";
import { SpeakerSheet, type Speaker } from "./SpeakerSheet";

export interface RegenerationSheetProps {
  api: ApiClient;
  conversationId: string;
  open: boolean;
  group: boolean;
  speakers: readonly Speaker[];
  originalName: string;
  originalAvatar: string | null;
  busy: boolean;
  onClose: () => void;
  onRegenerate: (options: RegenerationOptions) => Promise<boolean>;
}

export function RegenerationSheet({ api, conversationId, open, group, speakers, originalName, originalAvatar, busy, onClose, onRegenerate }: RegenerationSheetProps) {
  const { t } = useTranslation("conversations");
  const { t: common } = useTranslation("common");
  const [direction, setDirection] = useState("");
  const [modelId, setModelId] = useState<string | null>(null);
  const [speakerId, setSpeakerId] = useState<string | null>(null);
  const [picker, setPicker] = useState<"model" | "speaker" | null>(null);
  const [submitting, setSubmitting] = useState(false);
  if (!open && picker !== null) setPicker(null);
  useBlocker({
    shouldBlockFn: ({ action }) => {
      if (!open || action !== "BACK") return false;
      if (!dismissTopLayer()) onClose();
      return true;
    },
    enableBeforeUnload: false,
  });
  const models = useChatModels(api);
  const settings = useQuery({ ...settingsQuery(api, conversationId), enabled: group });
  const available = group ? speakers.filter((speaker) => settings.data?.members?.participants.some((member) => member.participant_id === speaker.id && member.enabled)) : [];
  const chosenSpeaker = available.find((speaker) => speaker.id === speakerId);
  const chosenModel = models.find(modelId);
  const missingSpeaker = speakerId !== null && !chosenSpeaker;
  const waiting = busy || submitting;
  const submit = async () => {
    if (waiting || missingSpeaker || (group && !settings.data)) return;
    setSubmitting(true);
    try {
      if (await onRegenerate({ direction, modelProfileId: modelId, speakerParticipantId: speakerId })) onClose();
    } finally {
      setSubmitting(false);
    }
  };
  return <>
    <Sheet
      open={open}
      onClose={onClose}
      size="sm"
      title={t("regeneration.title")}
      description={t("regeneration.oneReply")}
      dismissible={!submitting && picker === null}
      footer={<div className="flex gap-2">
        <Button variant="secondary" className="flex-1" disabled={submitting} onClick={onClose}>{common("actions.cancel")}</Button>
        <Button variant="primary" className="flex-1" loading={submitting} disabled={waiting || missingSpeaker || (group && !settings.data)} onClick={() => void submit()}>{t("menu.regenerate")}</Button>
      </div>}
    >
      <div className="space-y-4 pb-1">
        <Field label={t("regeneration.direction")} hint={t("regeneration.directionHint")}>
          <Textarea rows={4} value={direction} onChange={(event) => setDirection(event.target.value)} disabled={submitting} placeholder={t("regeneration.placeholder")} />
        </Field>
        <ListGroup>
          <ListRowButton
            icon={chosenModel ? <ModelMark modelId={chosenModel.externalId} providerKind={chosenModel.providerKind ?? ""} name={chosenModel.name} /> : <CpuIcon size={icon.lg} />}
            title={t("regeneration.model")}
            description={modelId ? chosenModel?.name ?? t("regeneration.modelUnavailable") : t("regeneration.usualModel")}
            disabled={waiting}
            onClick={() => setPicker("model")}
          />
          {group && <ListRowButton
            icon={<Avatar name={chosenSpeaker?.name ?? originalName} src={speakerId ? chosenSpeaker?.avatar ?? null : originalAvatar} size="sm" />}
            title={t("regeneration.character")}
            description={speakerId ? chosenSpeaker?.name ?? t("regeneration.characterUnavailable") : t("regeneration.sameCharacter", { name: originalName })}
            disabled={waiting || !settings.data}
            onClick={() => setPicker("speaker")}
          />}
        </ListGroup>
        {group && settings.isPending && <LoadingState />}
        {group && settings.isError && <FailureState size="sm" error={settings.error} onRetry={() => void settings.refetch()} />}
        {missingSpeaker && <Text variant="hint">{t("regeneration.characterUnavailable")}</Text>}
        {waiting && <Text variant="hint">{t("menu.regenerateBusy")}</Text>}
      </div>
    </Sheet>
    <ModelChoiceSheet api={api} open={open && picker === "model"} value={modelId} defaultChoice={{ title: t("regeneration.usualModel"), description: t("regeneration.oneReply") }} onChange={setModelId} onClose={() => setPicker(null)} />
    <SpeakerSheet open={open && picker === "speaker"} purpose="regenerate" speakers={available} selectedId={speakerId} originalName={originalName} onClose={() => setPicker(null)} onPick={(speaker) => setSpeakerId(speaker?.id ?? null)} />
  </>;
}
