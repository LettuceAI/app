import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, CrispStringsProvider, DatePicker, Field, ListGroup, ListRow, Segmented, Sheet } from "@lettuceai/crisp";
import type { CompanionTimeMode, CompanionTimeView } from "@/api/generated/bindings";
import type { PanelCtx } from "./context";
import { useOverlayBack } from "../../companion/use-overlay-back";

export function CompanionTimeSheet({ ctx, value }: { ctx: PanelCtx; value: CompanionTimeView }) {
  const { t, i18n } = useTranslation("conversations");
  const [mode, setMode] = useState<CompanionTimeMode>(value.mode);
  const [anchor, setAnchor] = useState<Date | null>(() => new Date(value.mode === "ticking" ? value.effective_at : value.anchor_at ?? value.effective_at));
  useOverlayBack(ctx.activeSheet === "companion_time", ctx.closeSheet);
  const apply = async (reset: boolean) => {
    const ok = await ctx.save({ companion_time: reset ? { type: "reset" } : { type: "set", mode, anchor_at: mode === "live" ? null : anchor?.getTime() ?? null } });
    if (ok) ctx.closeSheet();
  };
  return <CrispStringsProvider locale={i18n.language} strings={{ datePickerPreviousMonth: t("settings.companionTime.previousMonth"), datePickerNextMonth: t("settings.companionTime.nextMonth"), datePickerChooseMonth: t("settings.companionTime.chooseMonth"), datePickerPreviousYear: t("settings.companionTime.previousYear"), datePickerNextYear: t("settings.companionTime.nextYear"), datePickerYear: t("settings.companionTime.year"), datePickerTime: t("settings.companionTime.time") }}><Sheet open={ctx.activeSheet === "companion_time"} onClose={ctx.closeSheet} placement="bottom" size="md" title={t("settings.companionTime.title")} footer={
    <div className="flex items-center gap-2">
      {value.mode !== "live" && <Button variant="ghost" disabled={ctx.busy} onClick={() => void apply(true)}>{t("settings.companionTime.reset")}</Button>}
      <span className="flex-1" />
      <Button variant="ghost" disabled={ctx.busy} onClick={ctx.closeSheet}>{t("settings.companionTime.cancel")}</Button>
      <Button loading={ctx.busy} disabled={ctx.busy || (mode !== "live" && (anchor === null || !Number.isFinite(anchor.getTime())))} onClick={() => void apply(false)}>{t("settings.companionTime.apply")}</Button>
    </div>
  }>
    <div className="space-y-5 pb-2">
      <ListGroup><ListRow title={t("settings.companionTime.mode")} trailing={<fieldset disabled={ctx.busy}><Segmented size="sm" value={mode} aria-label={t("settings.companionTime.mode")} onChange={setMode} options={(["live", "frozen", "ticking"] as const).map((value) => ({ value, label: t(`settings.companionTime.${value}`) }))} /></fieldset>} /></ListGroup>
      {mode !== "live" && <Field label={t("settings.companionTime.anchor")}><DatePicker withTime value={anchor} onChange={setAnchor} locale={i18n.language} disabled={ctx.busy} placeholder={t("settings.companionTime.chooseAnchor")} /></Field>}
    </div>
  </Sheet></CrispStringsProvider>;
}
