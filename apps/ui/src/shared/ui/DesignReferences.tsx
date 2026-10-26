import { useState } from "react";
import { ImagePlus, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, Field, icon, IconButton, Textarea, toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { AssetRef } from "@/api/generated/bindings";

/** A design note and reference images that describe how someone looks, for generated scene images. */
export function DesignReferences({ api, note, onNoteChange, references, onReferencesChange, autoFocus }: {
  api: ApiClient;
  note: string;
  onNoteChange: (note: string) => void;
  references: AssetRef[];
  onReferencesChange: (change: (references: AssetRef[]) => AssetRef[]) => void;
  autoFocus?: boolean;
}) {
  const { t } = useTranslation();
  const [picking, setPicking] = useState(false);
  const add = async () => {
    if (picking) return;
    setPicking(true);
    try {
      const picked = await api.call("filesPickOpen", { kinds: ["image"], multiple: false });
      const source = picked.sources[0];
      if (!source) return;
      const asset = await api.call("assetsIngest", { source, role: "reference_image" });
      onReferencesChange((current) => [...current, asset]);
    } catch (error) {
      toast.error(t(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally { setPicking(false); }
  };

  return <div className="grid grid-cols-1 gap-6 @4xl:grid-cols-2 @4xl:gap-x-10">
    <Field label={t("designReferences.note")} hint={t("designReferences.noteHint")}>
      <Textarea value={note} rows={4} autoGrow maxRows={10} placeholder={t("designReferences.notePlaceholder")} onChange={(event) => onNoteChange(event.target.value)} {...(autoFocus ? { "data-autofocus": true } : {})} />
    </Field>
    <Field label={t("designReferences.images")} hint={t("designReferences.imagesHint")}>
      <div className="grid grid-cols-3 gap-2 @xl:grid-cols-4">
        {references.map((asset) => <div key={asset.asset_id} className="relative aspect-square overflow-hidden rounded-xl ring-1 ring-inset ring-line-2">
          <img src={asset.url} alt="" className="h-full w-full object-cover" />
          <IconButton label={t("designReferences.remove")} size="sm" variant="overlay" className="absolute right-1.5 top-1.5" onClick={() => onReferencesChange((current) => current.filter((entry) => entry.asset_id !== asset.asset_id))}><Trash2 size={icon.sm} /></IconButton>
        </div>)}
        <Button variant="secondary" disabled={picking} loading={picking} className="aspect-square h-auto flex-col gap-1.5 whitespace-normal border-dashed bg-surface-1 text-sm" leading={<ImagePlus size={icon.lg} />} onClick={() => void add()}>{t("designReferences.add")}</Button>
      </div>
    </Field>
  </div>;
}
