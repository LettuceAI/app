import { useState } from "react";
import { useInfiniteQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Check, User, UserCircle } from "lucide-react";
import {
  Avatar,
  Button,
  EmptyState,
  icon,
  ListGroup,
  ListRowButton,
  SearchField,
  Sheet,
  Skeleton,
} from "@lettuceai/crisp";
import { useApi } from "@/api/use-api";
import { FailureState } from "@/shared/ui/FailureState";
import { queryKeys } from "@/api/query-keys";

export function SourcePicker({
  kind,
  current,
  onClose,
  onChoose,
}: {
  kind: "character" | "persona";
  current: string | null;
  onClose: () => void;
  onChoose: (id: string | null) => void;
}) {
  const { t } = useTranslation("lorebook-create");
  const api = useApi();
  const [search, setSearch] = useState("");
  const query = useInfiniteQuery({
    queryKey: [
      ...(kind === "character" ? queryKeys.characters.all : queryKeys.personas.all),
      "lorebook-source",
      search,
    ],
    initialPageParam: null as string | null,
    queryFn: async ({ pageParam }) => {
      if (kind === "character") {
        const page = await api.call("charactersList", {
          query: search || null,
          cursor: pageParam,
          limit: 24,
          search_description: true,
        });
        return {
          items: page.items.map((item) => ({
            id: item.id,
            name: item.name,
            description: item.description ?? "",
            avatar: item.avatar,
          })),
          next: page.next_cursor,
        };
      }
      const page = await api.call("personasList", {
        query: search || null,
        cursor: pageParam,
        limit: 24,
        sort: null,
        search_description: true,
        search_nickname: true,
      });
      return {
        items: page.items.map((item) => ({
          id: item.id,
          name: item.title,
          description: item.description,
          avatar: item.avatar,
        })),
        next: page.next_cursor,
      };
    },
    getNextPageParam: (page) => page.next,
  });
  const rows = query.data?.pages.flatMap((page) => page.items) ?? [];
  const mark = <Check size={icon.md} className="text-accent" />;
  return (
    <Sheet open onClose={onClose} title={t(kind)} description={t("sourcePickerBody")}>
      <div className="space-y-4">
        <SearchField
          value={search}
          onChange={setSearch}
          placeholder={t("searchLibrary")}
          aria-label={t("searchLibrary")}
        />
        <ListGroup>
          <ListRowButton
            icon={kind === "character" ? <User size={icon.lg} /> : <UserCircle size={icon.lg} />}
            title={t("none")}
            chevron={false}
            trailing={current === null ? mark : undefined}
            onClick={() => onChoose(null)}
          />
          {rows.map((row) => (
            <ListRowButton
              key={row.id}
              icon={
                <Avatar
                  src={row.avatar?.preview_url ?? row.avatar?.url ?? null}
                  name={row.name}
                  size="sm"
                />
              }
              title={row.name}
              description={row.description}
              chevron={false}
              trailing={current === row.id ? mark : undefined}
              onClick={() => onChoose(row.id)}
            />
          ))}
        </ListGroup>
        {query.isPending && <Skeleton className="h-24 w-full" />}
        {query.isError && (
          <FailureState error={query.error} onRetry={() => void query.refetch()} size="sm" />
        )}
        {query.isSuccess && rows.length === 0 && <EmptyState title={t("noMatches")} />}{" "}
        {query.hasNextPage && (
          <Button
            variant="ghost"
            loading={query.isFetchingNextPage}
            onClick={() => void query.fetchNextPage()}
          >
            {t("showMore")}
          </Button>
        )}
      </div>
    </Sheet>
  );
}
