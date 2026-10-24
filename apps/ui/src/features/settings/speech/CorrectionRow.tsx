import { useTranslation } from "react-i18next";
import { Check, Wand2, X } from "lucide-react";
import { Button, icon, IconButton, ListRow, ListRowButton } from "@lettuceai/crisp";
import type { AsrCorrectionView, AsrIgnoredSuggestionView } from "@/api/generated/bindings";

export function CorrectionRow({ value, view, phone, busy, description, onEdit, onApprove, onIgnore }: {
  value: AsrCorrectionView | AsrIgnoredSuggestionView;
  view: "approved" | "suggested" | "ignored";
  phone: boolean;
  busy: boolean;
  description: string;
  onEdit: (id: string) => void;
  onApprove: (value: AsrCorrectionView | AsrIgnoredSuggestionView) => void;
  onIgnore: (value: AsrCorrectionView) => void;
}) {
  const { t } = useTranslation("settings", { keyPrefix: "speech" });
  const title = <span className="flex flex-wrap items-center gap-x-1.5"><span className="text-fg-3 line-through decoration-fg-4">{value.wrong}</span><span className="text-fg-4">{t("pair", { left: "", right: "" })}</span><span>{value.correct}</span></span>;
  const trailing = view === "approved" ? undefined : <span className="flex shrink-0 gap-1">
    {view === "suggested" && "user_approved" in value && (phone ? <IconButton label={t("ignore")} disabled={busy} onClick={() => onIgnore(value)}><X size={icon.md} /></IconButton> : <Button size="sm" variant="ghost" disabled={busy} onClick={() => onIgnore(value)}>{t("ignore")}</Button>)}
    {phone && view === "suggested" ? <IconButton label={t("approve")} disabled={busy} onClick={() => onApprove(value)}><Check size={icon.md} /></IconButton> : <Button size="sm" variant={view === "ignored" ? "ghost" : "tonal"} disabled={busy} onClick={() => onApprove(value)}>{t("approve")}</Button>}
  </span>;
  return "user_approved" in value ? <ListRowButton icon={<Wand2 size={icon.lg} />} tone={view === "approved" ? "accent" : "neutral"} title={title} description={description} trailing={trailing} chevron={view === "approved"} onClick={() => onEdit(value.id)} /> : <ListRow icon={<Wand2 size={icon.lg} />} title={title} description={description} trailing={trailing} />;
}
