import { useState } from "react";
import { Camera } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, Button, Field, icon, IconButton, Input, toast } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import type { AssetRef } from "@/api/generated/bindings";
import { AvatarGeneration, AvatarLibraryButton, pickImage, useAvatarCrop, useAvatarLibrary } from "@/entities/image-input";
import { withAvatar } from "../data/form";
import { PersonaDescription } from "./PersonaDescription";
import type { StepProps } from "./step-props";

export function IdentityFields({ api, form, update, disabled = false, autoFocus = false, onBusyChange }: StepProps & { disabled?: boolean; autoFocus?: boolean; onBusyChange?: (busy: boolean) => void }) {
  const { t } = useTranslation("personas");
  const { t: common } = useTranslation("common");
  const [picking, setPicking] = useState(false);
  const crop = useAvatarCrop(api);
  const { draft } = form;
  const busy = picking || disabled;
  const chooseAvatar = async (libraryAsset?: AssetRef) => {
    if (busy) return;
    setPicking(true);
    onBusyChange?.(true);
    try {
      const picked = libraryAsset ?? await pickImage(api, "avatar");
      const asset = picked ? await crop.choose(picked) : null;
      if (asset) update((current) => withAvatar(current, asset));
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      setPicking(false);
      onBusyChange?.(false);
    }
  };
  const library = useAvatarLibrary(api, (asset) => void chooseAvatar(asset));
  const setDraft = (patch: Partial<typeof draft>) => update((current) => ({ ...current, draft: { ...current.draft, ...patch } }));

  return <>
    {crop.editor}
    {library.sheet}
    <div className="flex flex-col items-center gap-5 @xl:flex-row @xl:items-start">
      <div className="flex shrink-0 flex-col items-center gap-2">
        <IconButton label={t("identity.chooseAvatar")} variant="ghost" disabled={busy} className="relative h-28 w-28 overflow-visible rounded-full p-0" onClick={() => void chooseAvatar()}>
          <Avatar src={form.avatar?.url ?? null} name={draft.title.trim() || "?"} size="xl" className="h-28 w-28 text-3xl" />
          <span className="absolute -right-1 -bottom-1 flex h-9 w-9 items-center justify-center rounded-full border border-line-2 bg-surface-2 text-fg shadow-raised"><Camera size={icon.md} /></span>
        </IconButton>
        <AvatarLibraryButton disabled={busy} onClick={() => library.show()} />
        <AvatarGeneration
          api={api}
          disabled={busy}
          readContext={() => ({ subjectName: draft.title.trim(), subjectDescription: draft.description.trim(), currentAvatar: form.avatar })}
          onUse={(asset) => update((current) => withAvatar(current, asset))}
        />
        {form.avatar && <Button size="sm" variant="ghost" disabled={busy} onClick={() => update((current) => withAvatar(current, null))}>{t("identity.removeAvatar")}</Button>}
      </div>
      <div className="w-full min-w-0 space-y-4 @xl:flex-1">
        <Field label={t("identity.name")} required hint={t("identity.nameHint")}>
          <Input value={draft.title} placeholder={t("identity.namePlaceholder")} onChange={(event) => setDraft({ title: event.target.value })} {...(autoFocus ? { "data-autofocus": true } : {})} />
        </Field>
        <Field label={t("identity.nickname")} hint={t("identity.nicknameHint")}>
          <Input value={draft.nickname ?? ""} placeholder={t("identity.nicknamePlaceholder")} onChange={(event) => setDraft({ nickname: event.target.value })} />
        </Field>
      </div>
    </div>
    <PersonaDescription api={api} form={form} update={update} />
  </>;
}
