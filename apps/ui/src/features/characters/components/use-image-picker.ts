import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { toast } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import { isApiFailure } from "@/api/client";
import type { AssetIngestRole, LibraryCharacterMediaSlot } from "@/api/generated/bindings";
import { pickImage } from "../data/actions";
import { withImage } from "../data/form";
import type { FormProps } from "./step-props";

const roles: Record<LibraryCharacterMediaSlot, AssetIngestRole> = { avatar_original: "avatar", banner: "avatar", background: "background", design_reference: "reference_image" };

export function useImagePicker(api: ApiClient, update: FormProps["update"]) {
  const { t: common } = useTranslation("common");
  const queryClient = useQueryClient();
  const [picking, setPicking] = useState(false);
  const choose = async (slot: LibraryCharacterMediaSlot) => {
    if (picking) return;
    setPicking(true);
    try {
      const asset = await pickImage({ api, queryClient }, roles[slot]);
      if (asset) update((current) => withImage(current, slot, asset));
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally { setPicking(false); }
  };
  return { picking, choose };
}
