import { useState } from "react";
import { useTranslation } from "react-i18next";
import { datePickerStrings } from "./date-picker-strings";
import { Button, CrispStringsProvider, DatePicker, Field, ListGroup, ListRow, Segmented, Sheet } from "@lettuceai/crisp";
import type { TimeMode, TimeView } from "@/api/generated/bindings";
import type { PanelCtx } from "./context";
import { useOverlayBack } from "../../companion/use-overlay-back";

export function TimeSheet({ ctx, value }: { ctx: PanelCtx; value: TimeView }) {
  const { t, i18n } = useTranslation("conversations");
  const [mode, setMode] = useState<TimeMode>(value.mode);
  const [anchor, setAnchor] = useState<Date | null>(() => new Date(value.mode === "ticking" ? value.effective_at : value.anchor_at ?? value.effective_at));
  useOverlayBack(ctx.activeSheet === "time", ctx.closeSheet);
  const apply = async (reset: boolean) => {
    const ok = await ctx.save({ time: reset ? { type: "reset" } : { type: "set", mode, anchor_at: mode === "live" ? null : anchor?.getTime() ?? null } });
    if (ok) ctx.closeSheet();
  };
  return <CrispStringsProvider locale={i18n.language} strings={datePickerStrings(t)}><Sheet open={ctx.activeSheet === "time"} onClose={ctx.closeSheet} placement="bottom" size="md" title={t("settings.time.title")} footer={
    <div className="flex items-center gap-2">
      {value.mode !== "live" && <Button variant="ghost" disabled={ctx.busy} onClick={() => void apply(true)}>{t("settings.time.reset")}</Button>}
      <span className="flex-1" />
      <Button variant="ghost" disabled={ctx.busy} onClick={ctx.closeSheet}>{t("settings.time.cancel")}</Button>
      <Button loading={ctx.busy} disabled={ctx.busy || (mode !== "live" && (anchor === null || !Number.isFinite(anchor.getTime())))} onClick={() => void apply(false)}>{t("settings.time.apply")}</Button>
    </div>
  }>
    <div className="space-y-5 pb-2">
      <ListGroup><ListRow title={t("settings.time.mode")} trailing={<fieldset disabled={ctx.busy}><Segmented size="sm" value={mode} aria-label={t("settings.time.mode")} onChange={setMode} options={(["live", "frozen", "ticking"] as const).map((value) => ({ value, label: t(`settings.time.${value}`) }))} /></fieldset>} /></ListGroup>
      {mode !== "live" && <Field label={t("settings.time.anchor")}><DatePicker withTime value={anchor} onChange={setAnchor} locale={i18n.language} disabled={ctx.busy} placeholder={t("settings.time.chooseAnchor")} /></Field>}
    </div>
  </Sheet></CrispStringsProvider>;
}
