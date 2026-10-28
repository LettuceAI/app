import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Camera } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, Button, Field, icon, IconButton, Input, toast } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import { pickImage } from "@/features/characters";
import { withAvatar } from "../data/form";
import { PersonaDescription } from "./PersonaDescription";
import type { StepProps } from "./step-props";

export function IdentityFields({ api, form, update, disabled = false, autoFocus = false, onBusyChange }: StepProps & { disabled?: boolean; autoFocus?: boolean; onBusyChange?: (busy: boolean) => void }) {
  const { t } = useTranslation("personas");
  const { t: common } = useTranslation("common");
  const queryClient = useQueryClient();
  const [picking, setPicking] = useState(false);
  const { draft } = form;
  const busy = picking || disabled;
  const chooseAvatar = async () => {
    if (busy) return;
    setPicking(true);
    onBusyChange?.(true);
    try {
      const asset = await pickImage({ api, queryClient }, "avatar");
      if (asset) update((current) => withAvatar(current, asset));
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      setPicking(false);
      onBusyChange?.(false);
    }
  };
  const setDraft = (patch: Partial<typeof draft>) => update((current) => ({ ...current, draft: { ...current.draft, ...patch } }));

  return <>
    <div className="flex flex-col items-center gap-5 @xl:flex-row @xl:items-start">
      <div className="flex shrink-0 flex-col items-center gap-2">
        <IconButton label={t("identity.chooseAvatar")} variant="ghost" disabled={busy} className="relative h-28 w-28 overflow-visible rounded-full p-0" onClick={() => void chooseAvatar()}>
          <Avatar src={form.avatar?.url ?? null} name={draft.title.trim() || "?"} size="xl" className="h-28 w-28 text-3xl" />
          <span className="absolute -right-1 -bottom-1 flex h-9 w-9 items-center justify-center rounded-full border border-line-2 bg-surface-2 text-fg shadow-raised"><Camera size={icon.md} /></span>
        </IconButton>
        {form.avatar && <Button size="sm" variant="ghost" disabled={busy} onClick={() => update((current) => withAvatar(current, null))}>{t("identity.removeAvatar")}</Button>}
      </div>
      <div className="w-full min-w-0 space-y-4 @xl:flex-1">
        <Field label={t("identity.name")} required hint={t("identity.nameHint")}>
          <Input value={draft.title} maxLength={200} placeholder={t("identity.namePlaceholder")} onChange={(event) => setDraft({ title: event.target.value })} {...(autoFocus ? { "data-autofocus": true } : {})} />
        </Field>
        <Field label={t("identity.nickname")} hint={t("identity.nicknameHint")}>
          <Input value={draft.nickname ?? ""} maxLength={200} placeholder={t("identity.nicknamePlaceholder")} onChange={(event) => setDraft({ nickname: event.target.value })} />
        </Field>
      </div>
    </div>
    <PersonaDescription api={api} form={form} update={update} />
  </>;
}
