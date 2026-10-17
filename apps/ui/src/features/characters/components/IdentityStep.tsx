import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Camera, FileUp, Image as ImageIcon, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, Button, CharacterCard, Field, icon, IconButton, Input, ListGroup, ListRow, Segmented, toast } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import type { AssetIngestRole, CharacterSummary, LibraryCharacterMediaSlot } from "@/api/generated/bindings";
import { useCardColors } from "@/features/home";
import { EditorRow } from "@/shared/ui/EditorRow";
import { pickImage } from "../data/actions";
import { withImage } from "../data/form";
import { CharacterMode } from "./CharacterMode";
import { GradientSettings } from "./GradientSettings";
import type { StepProps } from "./step-props";

const roles: Record<LibraryCharacterMediaSlot, AssetIngestRole> = { avatar_original: "avatar", banner: "avatar", background: "background", design_reference: "reference_image" };

export function IdentityStep({ api, form, update, onImport, importing, onModeLoadingChange }: StepProps & { onImport: () => void; importing: boolean; onModeLoadingChange: (loading: boolean) => void }) {
  const { t } = useTranslation("characters");
  const { t: common } = useTranslation("common");
  const queryClient = useQueryClient();
  const [picking, setPicking] = useState<LibraryCharacterMediaSlot | null>(null);
  const { draft, images } = form;
  const name = draft.profile.name;
  const unnamed = name.trim() === "";
  const summary: CharacterSummary = {
    id: "", name: name.trim(), description: draft.profile.description ?? draft.profile.definition, avatar: images.avatar_original ?? null, banner: images.banner ?? null,
    card_style: draft.presentation.card_style, gradient: draft.presentation.custom_gradient_enabled && draft.presentation.custom_gradient_colors.length ? { type: "custom", colors: draft.presentation.custom_gradient_colors, text_color: draft.presentation.primary_text_color, text_secondary: draft.presentation.secondary_text_color } : draft.presentation.disable_gradient ? { type: "none" } : { type: "avatar" }, revision: 0, created_at: 0, updated_at: 0,
  };
  const colors = useCardColors(api, summary);
  const busy = picking !== null || importing;
  const choose = async (slot: LibraryCharacterMediaSlot) => {
    if (busy) return;
    setPicking(slot);
    try {
      const asset = await pickImage({ api, queryClient }, roles[slot]);
      if (asset) update((current) => withImage(current, slot, asset));
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally { setPicking(null); }
  };
  const imageField = (slot: "banner" | "background", title: string, hint: string) => {
    const image = images[slot];
    return <Field label={title} hint={hint}>
      {image ? <div className="relative overflow-hidden rounded-xl ring-1 ring-inset ring-line-2">
        <img src={image.url} alt={title} className="h-36 w-full object-cover @2xl:h-52" />
        <div className="absolute right-2 top-2 flex gap-1.5">
          <Button size="sm" variant="tonal" disabled={busy} onClick={() => void choose(slot)}>{t("identity.changeAvatar")}</Button>
          <IconButton label={t("identity.remove")} size="sm" variant="tonal" disabled={busy} onClick={() => update((current) => withImage(current, slot, null))}><Trash2 size={icon.sm} /></IconButton>
        </div>
      </div> : <Button variant="secondary" block disabled={busy} className="h-36 whitespace-normal border-dashed bg-surface-1 @2xl:h-52" leading={<ImageIcon size={icon.xl} />} onClick={() => void choose(slot)}>{t("identity.chooseImage")}</Button>}
    </Field>;
  };

  return <div className="grid gap-6 @4xl:grid-cols-2 @4xl:gap-x-10">
    <div className="space-y-6">
      <div className="flex flex-col items-center gap-5 @xl:flex-row @xl:items-start">
        <div className="flex shrink-0 flex-col items-center gap-2">
          <IconButton label={t("identity.chooseAvatar")} variant="ghost" disabled={busy} className="relative h-28 w-28 overflow-visible rounded-full p-0" onClick={() => void choose("avatar_original")}>
            <Avatar src={images.avatar_original?.url ?? null} name={name.trim() || "?"} size="xl" className="h-28 w-28 text-3xl" />
            <span className="absolute -right-1 -bottom-1 flex h-9 w-9 items-center justify-center rounded-full border border-line-2 bg-surface-2 text-fg shadow-raised"><Camera size={icon.md} /></span>
          </IconButton>
          {images.avatar_original && <Button size="sm" variant="ghost" disabled={busy} onClick={() => update((current) => withImage(current, "avatar_original", null))}>{t("identity.removeAvatar")}</Button>}
        </div>
        <div className="w-full min-w-0 space-y-4 @xl:flex-1">
          <Field label={t("identity.name")} required hint={t("identity.nameHint")}>
            <Input value={name} maxLength={200} placeholder={t("identity.namePlaceholder")} onChange={(event) => { const value = event.target.value; update((current) => ({ ...current, draft: { ...current.draft, profile: { ...current.draft.profile, name: value } } })); }} data-autofocus />
          </Field>
          <CharacterMode api={api} form={form} update={update} onLoadingChange={onModeLoadingChange} />
        </div>
      </div>
      <div inert>
        <CharacterCard name={unnamed ? t("identity.previewName") : name.trim()} {...(unnamed ? { initials: "?" } : {})} description={draft.profile.definition?.trim().split(".")[0] || t("identity.previewDescription")} avatar={images.avatar_original?.url ?? null} variant={draft.presentation.card_style === "banner" ? "banner" : "row"} gradient={colors?.css ?? (unnamed ? "var(--color-surface-1)" : null)} fg={colors?.fg ?? null} fgMuted={colors?.fgMuted ?? null} tabIndex={-1} />
      </div>
      <ListGroup>
        <EditorRow label={t("identity.cardStyle")} hint={t("identity.cardStyleBody")}>
          <Segmented aria-label={t("identity.cardStyle")} block value={draft.presentation.card_style} options={[{ value: "circle", label: t("identity.cardCircle") }, { value: "banner", label: t("identity.cardBanner") }]} onChange={(card_style) => update((current) => ({ ...current, draft: { ...current.draft, presentation: { ...current.draft.presentation, card_style } } }))} />
        </EditorRow>
        <GradientSettings api={api} form={form} update={update} />
      </ListGroup>
      {draft.presentation.card_style === "banner" && imageField("banner", t("identity.banner"), t("identity.bannerBody"))}
    </div>
    <div className="space-y-6">
      {imageField("background", t("identity.background"), t("identity.backgroundBody"))}
      <ListGroup><ListRow icon={<FileUp size={icon.lg} />} title={t("identity.import")} description={t("identity.importBody")} trailing={<Button size="sm" variant="tonal" disabled={busy} onClick={onImport}>{t("identity.chooseFile")}</Button>} /></ListGroup>
    </div>
  </div>;
}
