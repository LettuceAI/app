import { useTranslation } from "react-i18next";
import { ListGroup } from "@lettuceai/crisp";
import { DesignReferences } from "@/shared/ui/DesignReferences";
import { CardStyleRows } from "../components/CardStyleRows";
import { draftSetters } from "../components/draft-setters";
import { CardPreview, ImageField } from "../components/ImageFields";
import { useImagePicker } from "../components/use-image-picker";
import type { StepProps } from "../components/step-props";
import { withImage, withReferences } from "../data/form";

export function LookTab({ api, form, update }: StepProps) {
  const { t } = useTranslation("characters");
  const { picking, choose } = useImagePicker(api, update);
  const { setProfile } = draftSetters(update);
  const { images } = form;
  const image = (slot: "banner" | "background", title: string, hint: string) => <ImageField title={title} hint={hint} image={images[slot]} busy={picking} onChoose={() => void choose(slot)} onRemove={() => update((current) => withImage(current, slot, null))} />;
  return <div className="grid grid-cols-1 gap-6 @4xl:grid-cols-2 @4xl:gap-x-10">
    <div className="min-w-0 space-y-6">
      <CardPreview api={api} form={form} />
      <ListGroup>
        <CardStyleRows api={api} form={form} update={update} inline />
      </ListGroup>
      {form.draft.presentation.card_style === "banner" && image("banner", t("identity.banner"), t("identity.bannerBody"))}
    </div>
    <div className="min-w-0 space-y-6">
      {image("background", t("identity.background"), t("identity.backgroundBody"))}
      <div className="@container">
        <DesignReferences
          api={api}
          note={form.draft.profile.design_description ?? ""}
          onNoteChange={(design_description) => setProfile({ design_description })}
          references={form.references}
          onReferencesChange={(change) => update((current) => withReferences(current, change(current.references)))}
        />
      </div>
    </div>
  </div>;
}
