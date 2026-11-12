import { useTranslation } from "react-i18next";
import { ArrowLeft } from "lucide-react";
import { ActionBar, Alert, Badge, Button, icon, IconButton, ListGroup, ListRowButton, Sheet } from "@lettuceai/crisp";
import type { CreatorDraft, CreatorProposalView, CreatorView } from "@/api/generated/bindings";
import { DraftDiff } from "./DraftDiff";

export function ReviewSheet({ open, phone, work, baseline, complete, canSave, busy, onClose, onApply }: {
  open: boolean;
  phone: boolean;
  work: CreatorView;
  baseline: CreatorDraft;
  complete: boolean;
  canSave: boolean;
  busy: boolean;
  onClose: () => void;
  onApply: (chat: boolean) => void;
}) {
  const { t } = useTranslation("creator");
  const existing = work.summary.target_id !== null;
  const chat = work.summary.kind === "character" && !existing;
  return (
    <Sheet
      open={open}
      onClose={onClose}
      placement={phone ? "bottom" : "side"}
      size="lg"
      title={t("review")}
      description={t(existing ? "reviewEdit" : "reviewNew")}
      footer={
        <ActionBar
          primary={<Button disabled={!canSave} loading={busy} onClick={() => onApply(false)}>{t(existing ? "saveChanges" : "create")}</Button>}
          secondary={chat ? <Button variant="secondary" disabled={!canSave || busy} onClick={() => onApply(true)}>{t("createAndChat")}</Button> : undefined}
        />
      }
    >
      <div className="space-y-5">
        {!complete && <Alert tone="warning" title={t("incomplete")} />}
        <DraftDiff before={baseline} after={work.proposal.draft} kind={work.summary.kind} />
      </div>
    </Sheet>
  );
}

export function HistorySheet({ open, phone, work, baseline, selected, canRestore, onSelect, onClose, onRestore }: {
  open: boolean;
  phone: boolean;
  work: CreatorView;
  baseline: CreatorDraft;
  selected: CreatorProposalView | null;
  canRestore: boolean;
  onSelect: (proposal: CreatorProposalView | null) => void;
  onClose: () => void;
  onRestore: (proposal: CreatorProposalView) => void;
}) {
  const { t, i18n } = useTranslation("creator");
  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" });
  const versions = work.history.filter((proposal) => proposal.source !== "review");
  const number = (proposal: CreatorProposalView) => versions.findIndex((version) => version.id === proposal.id) + 1;
  const currentId = work.proposal.source === "review" ? work.proposal.parent_id : work.proposal.id;
  const parent = selected ? work.history.find((proposal) => proposal.id === selected.parent_id)?.draft ?? baseline : null;
  const current = selected?.id === currentId;
  return (
    <Sheet
      open={open}
      onClose={onClose}
      placement={phone ? "bottom" : "side"}
      size="lg"
      title={selected ? t("version", { number: number(selected) }) : t("history")}
      {...(selected ? { description: `${t(`source.${selected.source}`)} · ${date.format(selected.created_at)}` } : {})}
      leftAction={
        selected ? (
          <IconButton label={t("back")} variant="ghost" size="sm" onClick={() => onSelect(null)}>
            <ArrowLeft size={icon.md} />
          </IconButton>
        ) : undefined
      }
      footer={
        selected && !current && canRestore ? (
          <Button variant="secondary" block onClick={() => onRestore(selected)}>{t("restoreVersion")}</Button>
        ) : undefined
      }
    >
      {selected && parent ? (
        <DraftDiff before={parent} after={selected.draft} kind={work.summary.kind} />
      ) : (
        <ListGroup>
          {[...versions].reverse().map((proposal) => (
            <ListRowButton
              key={proposal.id}
              title={t("version", { number: number(proposal) })}
              description={`${t(`source.${proposal.source}`)} · ${date.format(proposal.created_at)}`}
              trailing={proposal.id === currentId ? <Badge>{t("current")}</Badge> : undefined}
              onClick={() => onSelect(proposal)}
            />
          ))}
        </ListGroup>
      )}
    </Sheet>
  );
}
