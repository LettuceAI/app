import { Camera, Image as ImageIcon, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, Button, CharacterCard, Field, icon, IconButton } from "@lettuceai/crisp";
import type { AssetRef, CharacterSummary } from "@/api/generated/bindings";
import { useCardColors } from "@/features/home";
import type { StepProps } from "./step-props";

export function AvatarPicker({ name, image, busy, onChoose, onRemove }: { name: string; image: AssetRef | undefined; busy: boolean; onChoose: () => void; onRemove: () => void }) {
  const { t } = useTranslation("characters");
  return <div className="flex shrink-0 flex-col items-center gap-2">
    <IconButton label={t("identity.chooseAvatar")} variant="ghost" disabled={busy} className="relative h-28 w-28 overflow-visible rounded-full p-0" onClick={onChoose}>
      <Avatar src={image?.url ?? null} name={name || "?"} size="xl" className="h-28 w-28 text-3xl" />
      <span className="absolute -right-1 -bottom-1 flex h-9 w-9 items-center justify-center rounded-full border border-line-2 bg-surface-2 text-fg shadow-raised"><Camera size={icon.md} /></span>
    </IconButton>
    {image && <Button size="sm" variant="ghost" disabled={busy} onClick={onRemove}>{t("identity.removeAvatar")}</Button>}
  </div>;
}

export function ImageField({ title, hint, image, busy, onChoose, onRemove }: { title: string; hint: string; image: AssetRef | undefined; busy: boolean; onChoose: () => void; onRemove: () => void }) {
  const { t } = useTranslation("characters");
  return <Field label={title} hint={hint}>
    {image ? <div className="relative overflow-hidden rounded-xl ring-1 ring-inset ring-line-2">
      <img src={image.url} alt={title} className="h-36 w-full object-cover @2xl:h-52" />
      <div className="absolute right-2 top-2 flex gap-1.5">
        <Button size="sm" variant="overlay" disabled={busy} onClick={onChoose}>{t("identity.changeAvatar")}</Button>
        <IconButton label={t("identity.remove")} size="sm" variant="overlay" disabled={busy} onClick={onRemove}><Trash2 size={icon.sm} /></IconButton>
      </div>
    </div> : <Button variant="secondary" block disabled={busy} className="h-36 whitespace-normal border-dashed bg-surface-1 @2xl:h-52" leading={<ImageIcon size={icon.xl} />} onClick={onChoose}>{t("identity.chooseImage")}</Button>}
  </Field>;
}

export function CardPreview({ api, form }: Pick<StepProps, "api" | "form">) {
  const { t } = useTranslation("characters");
  const { draft, images } = form;
  const name = draft.profile.name;
  const unnamed = name.trim() === "";
  const summary: CharacterSummary = {
    id: "", name: name.trim(), description: draft.profile.description ?? draft.profile.definition, avatar: images.avatar_original ?? null, banner: images.banner ?? null,
    card_style: draft.presentation.card_style, gradient: draft.presentation.custom_gradient_enabled && draft.presentation.custom_gradient_colors.length ? { type: "custom", colors: draft.presentation.custom_gradient_colors, text_color: draft.presentation.primary_text_color, text_secondary: draft.presentation.secondary_text_color } : draft.presentation.disable_gradient ? { type: "none" } : { type: "avatar" }, revision: 0, created_at: 0, updated_at: 0,
  };
  const colors = useCardColors(api, summary);
  return <div inert>
    <CharacterCard name={unnamed ? t("identity.previewName") : name.trim()} {...(unnamed ? { initials: "?" } : {})} description={draft.profile.definition?.trim().split(".")[0] || t("identity.previewDescription")} avatar={images.avatar_original?.url ?? null} variant={draft.presentation.card_style === "banner" ? "banner" : "row"} gradient={colors?.css ?? (unnamed ? "var(--color-surface-1)" : null)} fg={colors?.fg ?? null} fgMuted={colors?.fgMuted ?? null} tabIndex={-1} />
  </div>;
}
