import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { BookOpen, ImagePlus, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, Field, icon, IconButton, ListGroup, ListRowButton, Textarea, toast } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import { LorebookChoiceSheet, pickImage } from "@/features/characters";
import { withReferences } from "../data/form";
import type { StepProps } from "./step-props";

export function ExtrasStep({ api, form, update }: StepProps) {
  const { t, i18n } = useTranslation("personas");
  const { t: common } = useTranslation("common");
  const queryClient = useQueryClient();
  const [choosingLorebooks, setChoosingLorebooks] = useState(false);
  const [picking, setPicking] = useState(false);
  const count = form.lorebookIds.length;
  const addReference = async () => {
    if (picking) return;
    setPicking(true);
    try {
      const asset = await pickImage({ api, queryClient }, "reference_image");
      if (asset) update((current) => withReferences(current, [...current.references, asset]));
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally { setPicking(false); }
  };

  return <div className="space-y-6">
    <p className="text-base text-fg-2">{t("extras.body")}</p>
    <div className="grid gap-6 @4xl:grid-cols-2 @4xl:gap-x-10">
      <div className="space-y-6">
        <Field label={t("extras.design")} hint={t("extras.designHint")}>
          <Textarea value={form.draft.design_description ?? ""} rows={4} autoGrow maxRows={10} placeholder={t("extras.designPlaceholder")} onChange={(event) => { const value = event.target.value; update((current) => ({ ...current, draft: { ...current.draft, design_description: value } })); }} data-autofocus />
        </Field>
        <ListGroup>
          <ListRowButton
            icon={<BookOpen size={icon.lg} />}
            title={t("extras.lorebooks")}
            description={count === 0 ? t("extras.lorebooksNone") : t("extras.lorebooksCount", { count, formattedCount: new Intl.NumberFormat(i18n.language).format(count) })}
            onClick={() => setChoosingLorebooks(true)}
          />
        </ListGroup>
      </div>
      <Field label={t("extras.references")} hint={t("extras.referencesHint")}>
        <div className="grid grid-cols-3 gap-2 @xl:grid-cols-4">
          {form.references.map((asset) => <div key={asset.asset_id} className="relative aspect-square overflow-hidden rounded-xl ring-1 ring-inset ring-line-2">
            <img src={asset.url} alt="" className="h-full w-full object-cover" />
            <IconButton label={t("extras.removeReference")} size="sm" variant="overlay" className="absolute right-1.5 top-1.5" onClick={() => update((current) => withReferences(current, current.references.filter((entry) => entry.asset_id !== asset.asset_id)))}><Trash2 size={icon.sm} /></IconButton>
          </div>)}
          <Button variant="secondary" disabled={picking} loading={picking} className="aspect-square h-auto flex-col gap-1.5 whitespace-normal border-dashed bg-surface-1 text-sm" leading={<ImagePlus size={icon.lg} />} onClick={() => void addReference()}>{t("extras.addReference")}</Button>
        </div>
      </Field>
    </div>
    <LorebookChoiceSheet api={api} open={choosingLorebooks} value={form.lorebookIds} onChange={(lorebookIds) => update((current) => ({ ...current, lorebookIds }))} onClose={() => setChoosingLorebooks(false)} />
  </div>;
}
