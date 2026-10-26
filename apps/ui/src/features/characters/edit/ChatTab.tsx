import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ListGroup, ListRow, ListRowButton, Switch } from "@lettuceai/crisp";
import { draftSetters } from "../components/draft-setters";
import { LorebooksRow, MemoryGroup, PromptsGroup } from "../components/ChatSettings";
import { ModelChoiceSheet, VoiceChoiceSheet } from "../components/Pickers";
import { useModelName } from "../components/use-model-name";
import { voiceLabel } from "../data/voice";
import type { StepProps } from "../components/step-props";

export function ChatTab({ api, form, update }: StepProps) {
  const { t } = useTranslation("characters");
  const [sheet, setSheet] = useState<"model" | "voice" | null>(null);
  const { setDefaults } = draftSetters(update);
  const { defaults } = form.draft;
  const modelName = useModelName(api, defaults.model_profile_id);
  return <div className="grid grid-cols-1 gap-6 @4xl:grid-cols-2 @4xl:gap-x-10">
    <div className="min-w-0 space-y-6">
      <ListGroup label={t("details.model")}>
        <ListRowButton title={t("details.model")} description={modelName} onClick={() => setSheet("model")} />
      </ListGroup>
      <PromptsGroup api={api} form={form} update={update} />
    </div>
    <div className="min-w-0 space-y-6">
      <MemoryGroup api={api} form={form} update={update} />
      <ListGroup label={t("voice.group")}>
        <ListRowButton title={t("voice.title")} description={voiceLabel(defaults.voice, t("voice.saved")) ?? t("voice.none")} onClick={() => setSheet("voice")} />
        <ListRow title={t("voice.autoplay")} description={t("voice.autoplayBody")} trailing={<Switch checked={defaults.voice_autoplay} onChange={(voice_autoplay) => setDefaults({ voice_autoplay })} aria-label={t("voice.autoplay")} />} />
      </ListGroup>
      <LorebooksRow api={api} form={form} update={update} />
    </div>
    <ModelChoiceSheet api={api} open={sheet === "model"} value={defaults.model_profile_id} onChange={(model_profile_id) => setDefaults({ model_profile_id })} onClose={() => setSheet(null)} />
    <VoiceChoiceSheet api={api} open={sheet === "voice"} value={defaults.voice} onChange={(voice) => setDefaults({ voice })} onClose={() => setSheet(null)} />
  </div>;
}
