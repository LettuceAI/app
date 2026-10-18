import { FileUp } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, icon, ListGroup, ListRow } from "@lettuceai/crisp";
import { withImage } from "../data/form";
import { CardStyleRows } from "./CardStyleRows";
import { CharacterMode } from "./CharacterMode";
import { AvatarPicker, CardPreview, ImageField } from "./ImageFields";
import { useImagePicker } from "./use-image-picker";
import { NameField } from "./ProfileFields";
import type { StepProps } from "./step-props";

export function IdentityStep({ api, form, update, onImport, importing, onModeLoadingChange }: StepProps & { onImport: () => void; importing: boolean; onModeLoadingChange: (loading: boolean) => void }) {
  const { t } = useTranslation("characters");
  const { picking, choose } = useImagePicker(api, update);
  const { draft, images } = form;
  const busy = picking || importing;
  const image = (slot: "banner" | "background", title: string, hint: string) => <ImageField title={title} hint={hint} image={images[slot]} busy={busy} onChoose={() => void choose(slot)} onRemove={() => update((current) => withImage(current, slot, null))} />;

  return <div className="grid gap-6 @4xl:grid-cols-2 @4xl:gap-x-10">
    <div className="space-y-6">
      <div className="flex flex-col items-center gap-5 @xl:flex-row @xl:items-start">
        <AvatarPicker name={draft.profile.name.trim()} image={images.avatar_original} busy={busy} onChoose={() => void choose("avatar_original")} onRemove={() => update((current) => withImage(current, "avatar_original", null))} />
        <div className="w-full min-w-0 space-y-4 @xl:flex-1">
          <NameField form={form} update={update} focus />
          <CharacterMode api={api} form={form} update={update} onLoadingChange={onModeLoadingChange} />
        </div>
      </div>
      <CardPreview api={api} form={form} />
      <ListGroup>
        <CardStyleRows api={api} form={form} update={update} />
      </ListGroup>
      {draft.presentation.card_style === "banner" && image("banner", t("identity.banner"), t("identity.bannerBody"))}
    </div>
    <div className="space-y-6">
      {image("background", t("identity.background"), t("identity.backgroundBody"))}
      <ListGroup><ListRow icon={<FileUp size={icon.lg} />} title={t("identity.import")} description={t("identity.importBody")} trailing={<Button size="sm" variant="tonal" disabled={busy} onClick={onImport}>{t("identity.chooseFile")}</Button>} /></ListGroup>
    </div>
  </div>;
}
