import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Field, Segmented, toast } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import type { LibraryInteractionMode } from "@/api/generated/bindings";
import { EmotionDownloadSheet, emotionStatusQuery } from "@/entities/download";
import { draftDefaultQuery } from "../data/queries";
import type { StepProps } from "./step-props";

export function CharacterMode({ api, form, update, onLoadingChange }: StepProps & { onLoadingChange: (loading: boolean) => void }) {
  const { t } = useTranslation("characters");
  const { t: common } = useTranslation("common");
  const queryClient = useQueryClient();
  const [loading, setLoading] = useState(false);
  const [emotionMissing, setEmotionMissing] = useState(false);
  const select = async (interaction_mode: LibraryInteractionMode) => {
    if (loading || interaction_mode === form.draft.defaults.interaction_mode) return;
    setLoading(true);
    onLoadingChange(true);
    try {
      const config = interaction_mode === "companion" && !form.draft.defaults.companion_soul
        ? (await queryClient.ensureQueryData(draftDefaultQuery(api, "companion"))).defaults.companion_soul
        : form.draft.defaults.companion_soul;
      if (interaction_mode === "companion" && !config) throw new Error("Companion defaults are missing");
      update((current) => ({ ...current, draft: { ...current.draft, defaults: { ...current.draft.defaults, interaction_mode, companion_soul: current.draft.defaults.companion_soul ?? config } } }));
      if (interaction_mode === "companion") setEmotionMissing((await queryClient.fetchQuery(emotionStatusQuery(api))).type !== "installed");
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      setLoading(false);
      onLoadingChange(false);
    }
  };

  return <>
    <Field label={t("identity.mode")} hint={t(form.draft.defaults.interaction_mode === "companion" ? "identity.companionBody" : "identity.roleplayBody")}>
      <fieldset disabled={loading} aria-busy={loading}>
        <Segmented<LibraryInteractionMode> block aria-label={t("identity.mode")} value={form.draft.defaults.interaction_mode} options={[{ value: "roleplay", label: t("identity.roleplay") }, { value: "companion", label: t("identity.companion") }]} onChange={(mode) => void select(mode)} />
      </fieldset>
    </Field>
    <EmotionDownloadSheet api={api} open={emotionMissing} onClose={() => setEmotionMissing(false)} />
  </>;
}
