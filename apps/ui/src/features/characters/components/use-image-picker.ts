import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import { isApiFailure } from "@/api/client";
import type { LibraryCharacterMediaSlot } from "@/api/generated/bindings";
import { pickImage, useAvatarCrop, type ImageInputRole } from "@/entities/image-input";
import { withBanner, withImage, type CharacterForm } from "../data/form";
import type { FormProps } from "./step-props";

const roles: Record<LibraryCharacterMediaSlot, ImageInputRole> = { avatar_original: "avatar", banner: "avatar", background: "background", design_reference: "reference_image" };

export function useImagePicker(api: ApiClient, update: FormProps["update"], form: CharacterForm) {
  const { t: common } = useTranslation("common");
  const [picking, setPicking] = useState(false);
  const crop = useAvatarCrop(api);
  const choose = async (slot: LibraryCharacterMediaSlot) => {
    if (picking) return;
    setPicking(true);
    try {
      const picked = await pickImage(api, roles[slot]);
      if (picked && slot === "banner") {
        const selection = await crop.chooseBanner(picked);
        if (selection) update((current) => withBanner(current, selection));
        return;
      }
      const asset = picked && roles[slot] === "avatar" ? await crop.choose(picked) : picked;
      if (asset) update((current) => withImage(current, slot, asset));
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally { setPicking(false); }
  };
  const repositionBanner = async () => {
    const source = form.bannerSource ?? form.images.banner;
    if (picking || !source) return;
    setPicking(true);
    try {
      const selection = await crop.chooseBanner(source, form.bannerSource ? form.draft.presentation.banner_crop : null);
      if (selection) update((current) => withBanner(current, selection));
    } finally { setPicking(false); }
  };
  return { picking, choose, repositionBanner, cropEditor: crop.editor };
}
