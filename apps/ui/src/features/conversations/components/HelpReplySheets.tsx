import { PenLine, RefreshCw, Sparkles } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Alert, Button, icon, ListGroup, ListRowButton, Sheet, Spinner } from "@lettuceai/crisp";
import type { HelpMeReplyMode } from "@/api/generated/bindings";
import type { HelpReplyState } from "../data/use-help-reply";

export function HelpChoiceSheet({ open, onChoose, onClose }: { open: boolean; onChoose: (mode: HelpMeReplyMode) => void; onClose: () => void }) {
  const { t } = useTranslation("conversations");
  return (
    <Sheet open={open} onClose={onClose} size="sm" title={t("help.title")} description={t("help.draftPrompt")}>
      <ListGroup>
        <ListRowButton icon={<PenLine size={icon.lg} />} title={t("help.useMyText")} description={t("help.useMyTextHint")} chevron={false} onClick={() => onChoose("enrich")} />
        <ListRowButton icon={<Sparkles size={icon.lg} />} title={t("help.writeNew")} description={t("help.writeNewHint")} chevron={false} onClick={() => onChoose("new")} />
      </ListGroup>
    </Sheet>
  );
}

export interface HelpResultSheetProps {
  open: boolean;
  state: HelpReplyState;
  onRegenerate: () => void;
  onUse: () => void;
  onClose: () => void;
}

export function HelpResultSheet({ open, state, onRegenerate, onUse, onClose }: HelpResultSheetProps) {
  const { t } = useTranslation("conversations");
  const writing = state.status === "writing";
  return (
    <Sheet
      open={open}
      onClose={onClose}
      size="sm"
      title={t("help.resultTitle")}
      footer={
        <div className="flex gap-2">
          <Button variant="secondary" className="flex-1" disabled={writing} leading={<RefreshCw size={icon.md} />} onClick={onRegenerate}>{t("help.regenerate")}</Button>
          <Button variant="primary" className="flex-1" disabled={writing || state.status !== "ready"} onClick={onUse}>{t("help.use")}</Button>
        </div>
      }
    >
      <div className="pb-1">
        {state.status === "failed" ? (
          <Alert tone="danger" title={t(`help.error.${state.error ?? "failed"}`, { defaultValue: t("help.error.failed") })} />
        ) : writing && !state.text ? (
          <div className="flex flex-col items-center gap-3 py-8" role="status">
            <Spinner />
            <p className="text-center text-sm text-fg-muted">{state.thinking ? t("help.thinking") : t("help.writing")}</p>
          </div>
        ) : state.text ? (
          <div className="max-h-[40vh] overflow-y-auto rounded-xl border border-line p-4">
            <p className="whitespace-pre-wrap text-sm text-fg">{state.text}</p>
          </div>
        ) : null}
      </div>
    </Sheet>
  );
}
