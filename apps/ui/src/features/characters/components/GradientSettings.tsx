import { useId } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Plus, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, ColorPicker, icon, IconButton, ListRow, Segmented } from "@lettuceai/crisp";
import { avatarGradientQuery } from "@/features/home";
import { EditorRow } from "@/shared/ui/EditorRow";
import type { StepProps } from "./step-props";

type GradientMode = "off" | "auto" | "custom";

const fallbackColors = ["#4f46e5", "#7c3aed"];
const middleColor = "#a855f7";

export function GradientSettings({ api, form, update }: StepProps) {
  const { t } = useTranslation("characters");
  const queryClient = useQueryClient();
  const id = useId();
  const { presentation } = form.draft;
  const palette = presentation.custom_gradient_colors;
  const mode: GradientMode = presentation.custom_gradient_enabled && palette.length > 0 ? "custom" : presentation.disable_gradient ? "off" : "auto";
  const selectMode = (next: GradientMode) => {
    const assetId = form.images.avatar_original?.asset_id;
    const detected = assetId ? queryClient.getQueryData(avatarGradientQuery(api, assetId).queryKey)?.colors.map((color) => color.hex).slice(0, 3) : undefined;
    update((current) => {
      const colors = current.draft.presentation.custom_gradient_colors;
      return { ...current, draft: { ...current.draft, presentation: {
        ...current.draft.presentation,
        disable_gradient: next === "off",
        custom_gradient_enabled: next === "custom",
        custom_gradient_colors: next === "custom" && colors.length < 2 ? detected && detected.length >= 2 ? detected : [...fallbackColors] : colors,
      } } };
    });
  };
  const changeColors = (change: (colors: string[]) => string[]) => update((current) => ({ ...current, draft: { ...current.draft, presentation: { ...current.draft.presentation, custom_gradient_colors: change(current.draft.presentation.custom_gradient_colors) } } }));
  const colorField = (index: number, label: string) => <div className="flex items-center justify-between gap-4 px-4 py-3">
    <div className="flex min-w-0 items-center gap-1">
      <label htmlFor={`${id}-${index}`} className="truncate text-sm font-medium text-fg-2">{label}</label>
      {index === 2 && <IconButton size="sm" label={t("identity.removeMiddleColor")} onClick={() => changeColors((colors) => colors.slice(0, 2))}><X size={icon.sm} /></IconButton>}
    </div>
    <ColorPicker id={`${id}-${index}`} value={palette[index] ?? fallbackColors[index] ?? middleColor} onChange={(value) => changeColors((colors) => colors.map((color, position) => position === index ? value : color))} />
  </div>;

  return <>
    <EditorRow label={t("identity.gradient")} hint={t("identity.gradientBody")}>
      <Segmented<GradientMode> block aria-label={t("identity.gradient")} value={mode} options={[{ value: "off", label: t("identity.gradientOff") }, { value: "auto", label: t("identity.gradientAuto") }, { value: "custom", label: t("identity.gradientCustom") }]} onChange={selectMode} />
    </EditorRow>
    {mode === "custom" && <>
      {colorField(0, t("identity.gradientStart"))}
      {palette.length >= 3 ? colorField(2, t("identity.gradientMiddle")) : <ListRow title={t("identity.gradientMiddle")} trailing={<Button size="sm" variant="ghost" leading={<Plus size={icon.sm} />} onClick={() => changeColors((colors) => [...colors.slice(0, 2), middleColor])}>{t("identity.addMiddleColor")}</Button>} />}
      {colorField(1, t("identity.gradientEnd"))}
    </>}
  </>;
}
