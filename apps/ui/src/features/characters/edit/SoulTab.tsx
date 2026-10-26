import { useState } from "react";
import { Sparkles } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Accordion, AccordionItem, Button, Field, icon, ListGroup, ListRow, Slider, Switch, Textarea } from "@lettuceai/crisp";
import type { CompanionEmotionVector, CompanionRegulationStyle, CompanionRelationshipDefaults, CompanionSoulConfigView, CompanionSoulIdentity } from "@/api/generated/bindings";
import type { StepProps } from "../components/step-props";
import { GenerateSoulSheet } from "./GenerateSoulSheet";
import { NotesGroup } from "./NotesGroup";

type TextField = { [K in keyof CompanionSoulIdentity]: CompanionSoulIdentity[K] extends string ? K : never }[keyof CompanionSoulIdentity];

const textFields: readonly TextField[] = ["essence", "traits", "backstory", "appearance", "goals", "likes", "voice", "relational_style", "vulnerabilities", "fears", "habits", "boundaries"];

const affect: readonly (keyof CompanionEmotionVector)[] = ["warmth", "trust", "calm", "vulnerability", "longing", "hurt", "tension", "irritation", "affection_intensity", "reassurance_need"];
const regulation: readonly (keyof CompanionRegulationStyle)[] = ["suppression", "volatility", "recovery_speed", "conflict_avoidance", "reassurance_seeking", "protest_behavior", "emotional_transparency", "attachment_activation", "pride"];
const relationship: readonly (keyof CompanionRelationshipDefaults)[] = ["closeness", "trust", "affection", "tension"];

type Sharing = "time_awareness" | "share_memory_across_chats" | "share_soul_growth_across_chats";
const sharing = [
  { key: "time_awareness", label: "timeAwareness" },
  { key: "share_memory_across_chats", label: "shareMemory" },
  { key: "share_soul_growth_across_chats", label: "shareGrowth" },
] as const satisfies readonly { key: Sharing; label: string }[];

function Sliders<K extends string>({ label, keys, values, onChange }: { label: (key: K) => string; keys: readonly K[]; values: Record<K, number | null>; onChange: (key: K, value: number) => void }) {
  return <div className="grid gap-x-8 gap-y-4 @3xl:grid-cols-2">
    {keys.map((key) => <Field key={key} label={label(key)}>
      <Slider min={0} max={100} step={1} value={Math.round((values[key] ?? 0) * 100)} showValue onChange={(value) => onChange(key, Math.round(value) / 100)} />
    </Field>)}
  </div>;
}

export function SoulTab({ api, form, update, characterId }: StepProps & { characterId: string }) {
  const { t } = useTranslation("characters");
  const [generating, setGenerating] = useState(false);
  const config = form.draft.defaults.companion_soul;
  if (!config) return null;
  const change = (edit: (current: CompanionSoulConfigView) => CompanionSoulConfigView) => update((current) => {
    const existing = current.draft.defaults.companion_soul;
    return existing ? { ...current, draft: { ...current.draft, defaults: { ...current.draft.defaults, companion_soul: edit(existing) } } } : current;
  });
  const { soul } = config;
  const missing = !form.draft.profile.name.trim() ? "generateSoul.addName" : !(form.draft.profile.definition ?? "").trim() ? "generateSoul.addDefinition" : null;
  return <div className="space-y-8">
    <ListGroup>
      <ListRow icon={<Sparkles size={icon.lg} />} title={t("generateSoul.title")} description={missing ? t(missing) : t("generateSoul.body")} trailing={<Button size="sm" variant="tonal" disabled={missing !== null} onClick={() => setGenerating(true)}>{t("generateSoul.generate")}</Button>} />
    </ListGroup>
    <GenerateSoulSheet api={api} open={generating} form={form} onClose={() => setGenerating(false)} onApply={(next) => change(() => next)} />
    <section className="space-y-4">
      <h2 className="text-base font-semibold text-fg">{t("soul.identity")}</h2>
      <div className="grid gap-x-10 gap-y-5 @4xl:grid-cols-2">
        {textFields.map((key) => <Field key={key} label={t(`soul.fields.${key}.label`)}>
          <Textarea rows={3} autoGrow maxRows={10} value={soul[key]} placeholder={t(`soul.fields.${key}.placeholder`)} onChange={(event) => { const value = event.target.value; change((current) => ({ ...current, soul: { ...current.soul, [key]: value } })); }} />
        </Field>)}
      </div>
    </section>
    <section className="space-y-4">
      <h2 className="text-base font-semibold text-fg">{t("soul.feelings")}</h2>
      <Accordion multiple>
        <AccordionItem value="affect" title={t("soul.affectTitle")} description={t("soul.affectBody")}>
          <Sliders label={(key) => t(`soul.affect.${key}`)} keys={affect} values={soul.baseline_affect} onChange={(key, value) => change((current) => ({ ...current, soul: { ...current.soul, baseline_affect: { ...current.soul.baseline_affect, [key]: value } } }))} />
        </AccordionItem>
        <AccordionItem value="regulation" title={t("soul.regulationTitle")} description={t("soul.regulationBody")}>
          <Sliders label={(key) => t(`soul.regulation.${key}`)} keys={regulation} values={soul.regulation_style} onChange={(key, value) => change((current) => ({ ...current, soul: { ...current.soul, regulation_style: { ...current.soul.regulation_style, [key]: value } } }))} />
        </AccordionItem>
        <AccordionItem value="relationship" title={t("soul.relationshipTitle")} description={t("soul.relationshipBody")}>
          <Sliders label={(key) => t(`soul.relationship.${key}`)} keys={relationship} values={config.relationship_defaults} onChange={(key, value) => change((current) => ({ ...current, relationship_defaults: { ...current.relationship_defaults, [key]: value } }))} />
        </AccordionItem>
      </Accordion>
    </section>
    <div className="grid grid-cols-1 gap-6 @4xl:grid-cols-2 @4xl:gap-x-10">
      <div className="min-w-0 space-y-6">
        <Field label={t("soul.styleNotes")} hint={t("soul.styleNotesHint")}>
          <Textarea rows={4} autoGrow maxRows={12} value={config.prompting.style_notes} placeholder={t("soul.styleNotesPlaceholder")} onChange={(event) => { const style_notes = event.target.value; change((current) => ({ ...current, prompting: { ...current.prompting, style_notes } })); }} />
        </Field>
        <NotesGroup api={api} characterId={characterId} />
      </div>
      <ListGroup label={t("soul.acrossChats")}>
        {sharing.map(({ key, label }) => <ListRow key={key} title={t(`soul.${label}`)} description={t(`soul.${label}Body`)} trailing={<Switch checked={config[key]} onChange={(value) => change((current) => ({ ...current, [key]: value }))} aria-label={t(`soul.${label}`)} />} />)}
      </ListGroup>
    </div>
  </div>;
}
