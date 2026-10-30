import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Collapsible, ListGroup, ListRow, PartitionBar, Skeleton, type Partition, type PartitionTone } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { PromptSection, PromptSectionKind } from "@/api/generated/bindings";
import { promptSnapshotQuery } from "../data/queries";

type Group = "character" | "lorebook" | "memories" | "history" | "other";

const groupOf: Record<PromptSectionKind, Group> = {
  character: "character",
  persona: "character",
  scene: "character",
  group_cast: "character",
  lorebook: "lorebook",
  memories: "memories",
  history: "history",
  author_note: "other",
  companion_state: "other",
  scheduled_notes: "other",
  prompt_entry: "other",
  media: "other",
  tools: "other",
};

const toneOf: Record<Group, PartitionTone> = {
  character: "info",
  lorebook: "warning",
  memories: "secondary",
  history: "neutral",
  other: "accent",
};

const order: Group[] = ["character", "lorebook", "memories", "history", "other"];

function partitions(sections: readonly PromptSection[], label: (group: Group) => string): Partition[] {
  const totals = new Map<Group, number>();
  for (const section of sections) {
    const group = groupOf[section.kind];
    totals.set(group, (totals.get(group) ?? 0) + section.estimated_tokens);
  }
  return order.flatMap((group) => {
    const value = totals.get(group) ?? 0;
    return value > 0 ? [{ id: group, label: label(group), value, tone: toneOf[group] }] : [];
  });
}

function Details({ api, conversationId, messageId }: { api: ApiClient; conversationId: string; messageId: string }) {
  const { t, i18n } = useTranslation("conversations");
  const snapshot = useQuery(promptSnapshotQuery(api, conversationId, messageId));
  const number = new Intl.NumberFormat(i18n.language);
  if (snapshot.isPending) {
    return (
      <div className="space-y-3 p-4" aria-hidden="true">
        <Skeleton height={10} width="100%" />
        <Skeleton height={32} width="100%" />
      </div>
    );
  }
  if (snapshot.isError) return <p className="p-4 text-sm text-fg-3">{t("menu.details.unavailable")}</p>;
  const { sections } = snapshot.data;
  const entries = (sections ?? []).filter((section) => section.kind === "lorebook");
  return (
    <div className="space-y-4 p-4">
      {sections ? (
        <PartitionBar
          segments={partitions(sections, (group) => t(`menu.details.${group}`))}
          labels="legend"
          size="md"
          format={(value) => t("menu.details.tokens", { count: number.format(value) })}
          aria-label={t("menu.details.partsLabel")}
        />
      ) : (
        <p className="text-sm text-fg-3">{t("menu.details.noBreakdown")}</p>
      )}
      {entries.length > 0 && (
        <ListGroup label={t("menu.details.entries")}>
          {entries.map((entry, index) => (
            <ListRow key={`${entry.label ?? ""}-${index}`} title={entry.label ?? t("menu.details.untitled")} trailing={t("menu.details.tokens", { count: entry.estimated_tokens })} />
          ))}
        </ListGroup>
      )}
    </div>
  );
}

export function SpeakerSelectionReason({ api, conversationId, messageId }: { api: ApiClient; conversationId: string; messageId: string }) {
  const { t } = useTranslation("conversations");
  const snapshot = useQuery(promptSnapshotQuery(api, conversationId, messageId));
  const selection = snapshot.data?.speaker_selection;
  if (!selection || (!selection.reason && !selection.heuristic_fallback)) return null;
  return (
    <ListGroup>
      <ListRow title={t("menu.selection.title")} description={selection.reason ?? t("menu.selection.fallback")} />
      {selection.reason && selection.heuristic_fallback && <p className="px-4 pb-3 text-xs text-fg-3">{t("menu.selection.fallback")}</p>}
    </ListGroup>
  );
}

export function MessagePromptDetails({ api, conversationId, messageId }: { api: ApiClient; conversationId: string; messageId: string }) {
  const { t } = useTranslation("conversations");
  const [open, setOpen] = useState(false);
  return (
    <Collapsible title={t("menu.details.title")} description={t("menu.details.hint")} open={open} onOpenChange={setOpen}>
      {open && <Details api={api} conversationId={conversationId} messageId={messageId} />}
    </Collapsible>
  );
}
