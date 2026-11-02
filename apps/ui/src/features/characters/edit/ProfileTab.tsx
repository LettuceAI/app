import { withImage } from "../data/form";
import { CharacterMode } from "../components/CharacterMode";
import { AvatarGenerate } from "../components/AvatarGenerate";
import { AvatarPicker } from "../components/ImageFields";
import { useImagePicker } from "../components/use-image-picker";
import { CreatorField, CreatorNotesField, DefinitionField, DescriptionField, NameField, NicknameField, TagsField } from "../components/ProfileFields";
import type { StepProps } from "../components/step-props";

export function ProfileTab({ api, form, update, characterId, onModeLoadingChange }: StepProps & { characterId: string; onModeLoadingChange: (loading: boolean) => void }) {
  const { picking, choose } = useImagePicker(api, update);
  return <div className="grid grid-cols-1 gap-6 @4xl:grid-cols-2 @4xl:gap-x-10">
    <div className="min-w-0 space-y-6">
      <div className="flex flex-col items-center gap-5 @xl:flex-row @xl:items-start">
        <AvatarPicker name={form.draft.profile.name.trim()} image={form.images.avatar_original} busy={picking} onChoose={() => void choose("avatar_original")} onRemove={() => update((current) => withImage(current, "avatar_original", null))} actions={<AvatarGenerate api={api} form={form} update={update} characterId={characterId} disabled={picking} />} />
        <div className="w-full min-w-0 space-y-4 @xl:flex-1">
          <NameField form={form} update={update} />
          <NicknameField form={form} update={update} />
        </div>
      </div>
      <DefinitionField form={form} update={update} />
    </div>
    <div className="min-w-0 space-y-6">
      <CharacterMode api={api} form={form} update={update} onLoadingChange={onModeLoadingChange} />
      <DescriptionField form={form} update={update} />
      <CreatorField form={form} update={update} />
      <TagsField form={form} update={update} />
      <CreatorNotesField form={form} update={update} />
    </div>
  </div>;
}
