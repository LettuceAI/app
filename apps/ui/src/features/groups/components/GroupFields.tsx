import { useState } from "react";
import { Brain, Check, Hand, Image, RefreshCw, Scale } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, icon, ListGroup, ListRow, ListRowButton, Segmented, toast } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import type { SpeakerSelectionMethod } from "@/api/generated/bindings";
import type { StepProps } from "./MembersStep";

const speakers = ["llm", "heuristic", "round_robin", "director"] as const;
const icons = { llm: Brain, heuristic: Scale, round_robin: RefreshCw, director: Hand };

export function BackgroundGroup({ api, form, update }: StepProps) {
  const { t } = useTranslation("groups");
  const { t: common } = useTranslation();
  const [picking, setPicking] = useState(false);
  const pickBackground = async () => {
    if (picking) return;
    setPicking(true);
    try {
      const selected = await api.call("filesPickOpen", { kinds: ["image"], multiple: false });
      if (!selected.sources[0]) return;
      const asset = await api.call("assetsIngest", { source: selected.sources[0], role: "background" });
      update((current) => ({ ...current, background: asset, draft: { ...current.draft, background_asset_id: asset.asset_id } }));
    } catch (error) { toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`)); }
    finally { setPicking(false); }
  };
  return <ListGroup><ListRow icon={<Image size={icon.lg} />} title={t("setup.background")} description={t(form.background ? "setup.backgroundChosen" : "setup.backgroundBody")} trailing={form.background ? <Button size="sm" variant="ghost" onClick={() => update((current) => ({ ...current, background: null, draft: { ...current.draft, background_asset_id: null } }))}>{t("remove")}</Button> : <Button size="sm" variant="tonal" loading={picking} onClick={() => void pickBackground()}>{t("choose")}</Button>} /></ListGroup>;
}

export function SpeakerGroup({ form, update }: StepProps) {
  const { t } = useTranslation("groups");
  const draft = form.draft;
  const setDraft = (patch: Partial<typeof draft>) => update((current) => ({ ...current, draft: { ...current.draft, ...patch } }));
  const director = draft.speaker_selection === "director" || draft.speaker_selection === "director_action";
  return <ListGroup label={t("setup.speaker")}>{speakers.map((id) => {
        const selected = id === "director" ? director : draft.speaker_selection === id;
        const Icon = icons[id];
        return <ListRowButton key={id} role="radio" aria-checked={selected} icon={<Icon size={icon.lg} />} title={t(`speaker.${id}`)} description={t(`speaker.${id}Body`)} chevron={false} trailing={selected ? <Check size={icon.md} /> : undefined} onClick={() => setDraft({ speaker_selection: id })} />;
      })}{director && <ListRow title={t("setup.when")} description={t(draft.speaker_selection === "director" ? "setup.beforeBody" : "setup.afterBody")} trailing={<Segmented<SpeakerSelectionMethod> size="sm" aria-label={t("setup.when")} value={draft.speaker_selection} onChange={(speaker_selection) => setDraft({ speaker_selection })} options={[{ value: "director", label: t("setup.before") }, { value: "director_action", label: t("setup.after") }]} />} />}</ListGroup>;
}

export function MemoryGroup({ form, update, dynamicMemory }: StepProps & { dynamicMemory: boolean }) {
  const { t } = useTranslation("groups");
  const draft = form.draft;
  const setDraft = (patch: Partial<typeof draft>) => update((current) => ({ ...current, draft: { ...current.draft, ...patch } }));
  return <ListGroup label={t("setup.memory")}>{(["dynamic", "manual"] as const).map((policy) => <ListRowButton key={policy} role="radio" aria-checked={draft.memory_policy === policy} disabled={policy === "dynamic" && !dynamicMemory} title={t(`memory.${policy}`)} description={t(policy === "dynamic" && !dynamicMemory ? "memory.unavailable" : `memory.${policy}Body`)} chevron={false} trailing={draft.memory_policy === policy ? <Check size={icon.md} /> : undefined} onClick={() => setDraft({ memory_policy: policy })} />)}</ListGroup>;
}
