import { useInfiniteQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Settings, UserRound, Users } from "lucide-react";
import { Badge, Button, EmptyState, icon, IconButton, ListGroup, ListRowButton, LoadingState, Page } from "@lettuceai/crisp";
import type { CreatorKind, CreatorSummary } from "@/api/generated/bindings";
import { useApi } from "@/api/use-api";
import { FailureState } from "@/shared/ui/FailureState";
import { CreatorStartRow } from "./CreatorStartRow";
import { creatorKeys } from "./data";

const kinds: readonly CreatorKind[] = ["character", "persona"];

function DraftRow({ row, onOpen }: { row: CreatorSummary; onOpen: () => void }) {
  const { t, i18n } = useTranslation("creator");
  const updated = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" }).format(row.updated_at);
  return (
    <ListRowButton
      icon={row.kind === "character" ? <Users size={icon.lg} /> : <UserRound size={icon.lg} />}
      title={row.name || t("untitled")}
      description={`${t(`kind.${row.kind}`)} · ${updated}`}
      trailing={<Badge tone={row.applied_id ? "accent" : "neutral"}>{t(row.applied_id ? "saved" : "notSaved")}</Badge>}
      onClick={onOpen}
    />
  );
}

export function CreatorListPage() {
  const api = useApi();
  const navigate = useNavigate();
  const { t } = useTranslation("creator");
  const query = useInfiniteQuery({
    queryKey: creatorKeys.lists,
    initialPageParam: null as string | null,
    queryFn: ({ pageParam }) => api.call("creatorsList", { cursor: pageParam, limit: null }),
    getNextPageParam: (page) => page.next_cursor ?? undefined,
  });
  const rows = query.data?.pages.flatMap((page) => page.items) ?? [];
  return (
    <Page
      title={t("title")}
      onBack={() => void navigate({ to: "/library" })}
      actions={
        <IconButton label={t("settings")} variant="ghost" onClick={() => void navigate({ to: "/settings/$section", params: { section: "smartCreator" } })}>
          <Settings size={icon.lg} />
        </IconButton>
      }
      contentClassName="space-y-7"
    >
      <ListGroup label={t("create")}>
        {kinds.map((kind) => <CreatorStartRow key={kind} api={api} kind={kind} />)}
      </ListGroup>
      {query.isPending ? (
        <LoadingState />
      ) : query.isError ? (
        <FailureState error={query.error} onRetry={() => void query.refetch()} />
      ) : rows.length === 0 ? (
        <EmptyState title={t("empty")} description={t("emptyBody")} />
      ) : (
        <ListGroup label={t("drafts")}>
          {rows.map((row) => (
            <DraftRow key={row.id} row={row} onOpen={() => void navigate({ to: "/creator/$workflowId", params: { workflowId: row.id } })} />
          ))}
        </ListGroup>
      )}
      {query.hasNextPage && (
        <Button variant="ghost" loading={query.isFetchingNextPage} onClick={() => void query.fetchNextPage()}>
          {t("loadMore")}
        </Button>
      )}
    </Page>
  );
}
