import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { AlertTriangle, Check, PenLine, Plus, Wand2 } from "lucide-react";
import {
  Badge,
  Button,
  Checkbox,
  ChoiceCard,
  Collapsible,
  EmptyState,
  icon,
  ListGroup,
  ListRow,
  ListRowButton,
  Panel,
  Progress,
  Skeleton,
  Text,
} from "@lettuceai/crisp";
import type {
  LorebookCoherenceChangeView,
  LorebookDraftView,
  LorebookPlanView,
  LorebookProjectView_Serialize,
  LorebookSummary_Serialize,
  LorebookPage_Serialize,
} from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { useApi } from "@/api/use-api";
import { FailureState } from "@/shared/ui/FailureState";
import { categories, category, draftCounts, sourceLabels } from "../data/lorebook-projects";

export function Keywords({ values }: { values: string[] }) {
  return (
    <span className="flex flex-wrap gap-1">
      {values.map((value, index) => (
        <Text
          key={`${value}-${index}`}
          as="span"
          variant="caption"
          className="rounded-md bg-fill-2 px-1.5 py-0.5 font-mono text-fg-2"
        >
          {value}
        </Text>
      ))}
    </span>
  );
}
export function CategoryCounts({ plans }: { plans: LorebookPlanView[] }) {
  const { t } = useTranslation("lorebook-create");
  return (
    <div className="flex flex-wrap gap-1.5">
      {categories.map((value) => {
        const count = plans.filter((plan) => category(plan.category) === value).length;
        return count > 0 ? (
          <Badge key={value}>{t(`categoryCounts.${value}`, { count })}</Badge>
        ) : null;
      })}
    </div>
  );
}
export function Outline({
  project,
  editable,
  onEdit,
}: {
  project: LorebookProjectView_Serialize;
  editable: boolean;
  onEdit: (plan: LorebookPlanView | null) => void;
}) {
  const { t } = useTranslation("lorebook-create");
  return (
    <div className="space-y-5">
      <div className="space-y-3">
        <Text variant="secondary">
          {t(editable ? "outlineHint" : "outlineApproved", { count: project.outline.length })}
        </Text>
        <CategoryCounts plans={project.outline} />
      </div>
      <ListGroup>
        {project.outline.map((plan, index) => (
          <ListRowButton
            key={plan.plan_id}
            title={
              <span className="flex min-w-0 flex-wrap items-center gap-2">
                <span className="text-sm tabular-nums text-fg-4">{index + 1}</span>
                <span className="min-w-0 break-words">{plan.title}</span>
                <Badge>{t(`categories.${category(plan.category)}`)}</Badge>
              </span>
            }
            description={
              <span className="space-y-2">
                <Text as="span" variant="secondary" className="block">
                  {plan.rationale}
                </Text>
                <Keywords values={plan.proposed_keys} />
                {plan.source_refs.length > 0 && (
                  <Text as="span" variant="hint" className="block">
                    {t("plannedFrom", {
                      value: sourceLabels(project, plan.source_refs).join(", "),
                    })}
                  </Text>
                )}
              </span>
            }
            chevron={false}
            disabled={!editable}
            onClick={() => onEdit(plan)}
          />
        ))}
      </ListGroup>
      {editable && (
        <Button variant="ghost" leading={<Plus size={icon.md} />} onClick={() => onEdit(null)}>
          {t("addEntry")}
        </Button>
      )}
    </div>
  );
}
function CoherenceText({
  change,
  project,
}: {
  change: LorebookCoherenceChangeView;
  project: LorebookProjectView_Serialize;
}) {
  const { t } = useTranslation("lorebook-create");
  const title = (id: string) =>
    project.drafts.find((draft) => draft.plan_id === id)?.title ??
    project.outline.find((plan) => plan.plan_id === id)?.title ??
    t("anEntry");
  const label =
    change.kind === "merge_keys"
      ? "keywords"
      : change.kind === "rename_term"
        ? "naming"
        : change.kind === "toggle_always_active"
          ? "alwaysActive"
          : "conflict";
  const text =
    change.kind === "merge_keys"
      ? t("mergeKeys", {
          keys: change.remove_keys.join(", "),
          name: title(change.plan_id),
          reason: change.reason,
        })
      : change.kind === "rename_term"
        ? t("renameTerm", {
            old: change.old_term,
            next: change.new_term,
            entries: change.plan_ids ? change.plan_ids.map(title).join(", ") : t("everyEntry"),
            reason: change.reason,
          })
        : change.kind === "toggle_always_active"
          ? t(change.new_value ? "keepAlways" : "stopAlways", {
              name: title(change.plan_id),
              reason: change.reason,
            })
          : t("contradiction", {
              entries: change.plan_ids.map(title).join(", "),
              description: change.description,
            });
  return (
    <span className="min-w-0 break-words text-sm text-fg-2">
      <Badge tone={change.kind === "flag_contradiction" ? "warning" : "neutral"} className="mr-2">
        {t(label)}
      </Badge>
      {text}
    </span>
  );
}
export function Entries({
  project,
  busy,
  onApprove,
  onEdit,
  onRefine,
  checking,
  coherence,
  selected,
  onSelected,
  onFix,
}: {
  project: LorebookProjectView_Serialize;
  busy: boolean;
  onApprove: (draft: LorebookDraftView) => Promise<boolean>;
  onEdit: (draft: LorebookDraftView) => void;
  onRefine: (draft: LorebookDraftView) => void;
  checking: boolean;
  coherence: boolean;
  selected: Set<string>;
  onSelected: (values: Set<string>) => void;
  onFix: (draft: LorebookDraftView) => void;
}) {
  const { t } = useTranslation("lorebook-create");
  const counts = draftCounts(project);
  const [expanded, setExpanded] = useState<string | null>(null);
  const writing = project.active_job_ids.length > 0 && !checking;
  return (
    <div className="space-y-5">
      <Panel
        title={t(writing ? "writingProgress" : "writtenProgress", {
          current: Math.min(counts.written + 1, counts.total),
          written: counts.written,
          count: counts.total,
        })}
        action={
          <Text variant="hint">
            {t("reviewProgress", { approved: counts.approved, failed: counts.failed })}
          </Text>
        }
      >
        <div className="space-y-2">
          <Progress
            value={counts.total ? (counts.written / counts.total) * 100 : 0}
            label={t("entriesWritten")}
          />
          <Text variant="hint">{t(writing ? "whileWriting" : "reviewHint")}</Text>
        </div>
      </Panel>
      {checking && (
        <Panel title={t("checking")}>
          <Progress label={t("checking")} />
          <Text variant="hint">{t("checkingBody")}</Text>
        </Panel>
      )}
      {coherence && (
        <Panel
          title={t("coherenceSuggestions", { count: project.coherence_changes.length })}
          description={t("coherenceHint")}
        >
          <div className="divide-y divide-line">
            {project.coherence_changes.map((change) => (
              <div key={change.id} className="flex items-start gap-3 py-3">
                {change.kind === "flag_contradiction" ? (
                  <AlertTriangle size={icon.md} className="mt-0.5 shrink-0 text-warning" />
                ) : (
                  <Checkbox
                    checked={selected.has(change.id)}
                    onChange={(checked) => {
                      const next = new Set(selected);
                      if (checked) next.add(change.id);
                      else next.delete(change.id);
                      onSelected(next);
                    }}
                    aria-label={t("selectSuggestion", { id: change.id })}
                    disabled={busy}
                  />
                )}
                <div className="min-w-0 flex-1 space-y-2">
                  <CoherenceText change={change} project={project} />
                  {change.kind === "flag_contradiction" &&
                    change.plan_ids.map((id) => {
                      const draft = project.drafts.find((entry) => entry.plan_id === id);
                      return draft ? (
                        <Button
                          key={id}
                          size="sm"
                          variant="ghost"
                          disabled={busy}
                          onClick={() => onFix(draft)}
                        >
                          {t("editNamed", { name: draft.title })}
                        </Button>
                      ) : null;
                    })}
                </div>
              </div>
            ))}
          </div>
        </Panel>
      )}
      <div className="grid gap-2">
        {project.drafts.map((draft) => {
          const written = draft.status === "drafted" || draft.status === "approved";
          const status = (
            <Badge
              tone={
                draft.status === "failed"
                  ? "danger"
                  : draft.status === "approved"
                    ? "accent"
                    : draft.status === "drafted"
                      ? "secondary"
                      : "neutral"
              }
            >
              {t(`draftStatuses.${draft.status}`)}
            </Badge>
          );
          if (!written)
            return (
              <ListGroup key={draft.plan_id}>
                <ListRow
                  title={draft.title}
                  description={t(
                    draft.status === "failed"
                      ? "draftFailed"
                      : draft.status === "drafting"
                        ? "draftWriting"
                        : "draftWaiting",
                  )}
                  trailing={status}
                />
              </ListGroup>
            );
          return (
            <Collapsible
              key={draft.plan_id}
              title={draft.title}
              description={expanded === draft.plan_id ? undefined : draft.content}
              trailing={
                <span className="flex flex-wrap gap-1.5">
                  {draft.always_active && <Badge tone="info">{t("alwaysActive")}</Badge>}
                  {status}
                </span>
              }
              open={expanded === draft.plan_id}
              onOpenChange={(open) => setExpanded(open ? draft.plan_id : null)}
            >
              <div className="space-y-3">
                <Text className="whitespace-pre-wrap break-words">{draft.content}</Text>
                <Keywords values={draft.keywords} />
                {draft.revisions.length > 0 && (
                  <Collapsible
                    title={t("revisionCount", { count: draft.revisions.length })}
                    variant="plain"
                  >
                    {draft.revisions.map((revision, index) => (
                      <div key={index} className="space-y-2 border-t border-line py-3">
                        <Text variant="hint">{revision.feedback}</Text>
                        <Text variant="secondary" className="whitespace-pre-wrap">
                          {revision.content}
                        </Text>
                      </div>
                    ))}
                  </Collapsible>
                )}
                <div className="flex flex-wrap gap-2">
                  <Button
                    size="sm"
                    variant={draft.status === "approved" ? "tonal" : "secondary"}
                    leading={<Check size={icon.sm} />}
                    disabled={busy}
                    onClick={() =>
                      void onApprove(draft).then((success) => {
                        if (success && draft.status !== "approved")
                          setExpanded(
                            project.drafts.find(
                              (item) => item.status === "drafted" && item.plan_id !== draft.plan_id,
                            )?.plan_id ?? null,
                          );
                      })
                    }
                  >
                    {t(draft.status === "approved" ? "looksGood" : "markGood")}
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    leading={<Wand2 size={icon.sm} />}
                    disabled={busy}
                    onClick={() => onRefine(draft)}
                  >
                    {t("askChanges")}
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    leading={<PenLine size={icon.sm} />}
                    disabled={busy}
                    onClick={() => onEdit(draft)}
                  >
                    {t("edit")}
                  </Button>
                </div>
              </div>
            </Collapsible>
          );
        })}
      </div>
    </div>
  );
}
export function SaveProject({
  project,
  target,
  onTarget,
}: {
  project: LorebookProjectView_Serialize;
  target: string;
  onTarget: (id: string) => void;
}) {
  const { t } = useTranslation("lorebook-create");
  const api = useApi();
  const [existing, setExisting] = useState(target !== "new");
  const books = useQuery({
    queryKey: queryKeys.lorebooks.list(),
    queryFn: async () => {
      const items: LorebookSummary_Serialize[] = [];
      let cursor: string | null = null;
      do {
        const page: LorebookPage_Serialize = await api.call("lorebooksList", {
          query: null,
          lifecycle: "active",
          cursor,
          limit: 100,
        });
        items.push(...page.items);
        cursor = page.next_cursor;
      } while (cursor !== null);
      return items;
    },
    enabled: existing,
  });
  const saved = project.drafts.filter((draft) => draft.title.trim() || draft.content.trim());
  const plans = project.outline.filter((plan) =>
    saved.some((draft) => draft.plan_id === plan.plan_id),
  );
  return (
    <div className="space-y-6">
      <div className="space-y-3">
        <Text variant="secondary">
          {t("readyToSave", { count: saved.length, omitted: project.drafts.length - saved.length })}
        </Text>
        {saved.some((draft) => !draft.content.trim()) && (
          <Text variant="hint" className="text-warning">
            {t("titleOnlySave", { count: saved.filter((draft) => !draft.content.trim()).length })}
          </Text>
        )}
        <CategoryCounts plans={plans} />
        <div className="flex flex-wrap gap-1.5">
          <Badge>
            {t("alwaysCount", { count: saved.filter((draft) => draft.always_active).length })}
          </Badge>
          <Badge>
            {t("keywordCount", {
              count: saved.reduce((sum, draft) => sum + draft.keywords.length, 0),
            })}
          </Badge>
        </div>
      </div>
      <div className="grid gap-2">
        <ChoiceCard
          selected={!existing}
          onClick={() => {
            setExisting(false);
            onTarget("new");
          }}
          title={t("saveNew")}
          description={project.lorebook_name ?? t("untitled")}
        />
        <ChoiceCard
          selected={existing}
          onClick={() => {
            setExisting(true);
            onTarget(books.data?.[0]?.id ?? "");
          }}
          title={t("saveExisting")}
          description={t("saveExistingBody")}
        />
      </div>
      {existing &&
        (books.isPending ? (
          <Skeleton className="h-24 w-full" />
        ) : books.isError ? (
          <FailureState error={books.error} onRetry={() => void books.refetch()} />
        ) : books.data.length === 0 ? (
          <EmptyState title={t("noLorebooks")} />
        ) : (
          <ListGroup label={t("yourLorebooks")}>
            {books.data.map((book) => (
              <ListRowButton
                key={book.id}
                title={book.name}
                chevron={false}
                trailing={
                  book.id === target ? <Check size={icon.md} className="text-accent" /> : undefined
                }
                onClick={() => onTarget(book.id)}
              />
            ))}
          </ListGroup>
        ))}
      <Text variant="hint">{t("savedEntryHint")}</Text>
    </div>
  );
}
