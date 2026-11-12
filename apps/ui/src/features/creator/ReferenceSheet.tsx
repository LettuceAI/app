import { useState } from "react";
import { useInfiniteQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Check } from "lucide-react";
import { Avatar, Button, EmptyState, icon, ListGroup, ListRowButton, LoadingState, SearchField, Sheet } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { CreatorKind } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";

export interface PickedReference {
  kind: CreatorKind;
  id: string;
  name: string;
}

interface Row {
  id: string;
  name: string;
  detail: string | null;
  avatar: string | null;
}

/** Characters or personas to attach to the next message, any number of them. */
export function ReferenceSheet({ api, kind, value, onChange, onClose }: {
  api: ApiClient;
  kind: CreatorKind | null;
  value: readonly PickedReference[];
  onChange: (references: PickedReference[]) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation("creator");
  const [query, setQuery] = useState("");
  const search = query.trim() || null;
  const rows = useInfiniteQuery({
    queryKey: ["creator", "references", kind, search],
    enabled: kind !== null,
    initialPageParam: null as string | null,
    queryFn: async ({ pageParam }): Promise<{ items: Row[]; next: string | null }> => {
      if (kind === "persona") {
        const page = await api.call("personasList", { cursor: pageParam, limit: null, query: search, sort: null });
        return { items: page.items.map((item) => ({ id: item.id, name: item.title, detail: item.description || null, avatar: item.avatar?.url ?? null })), next: page.next_cursor };
      }
      const page = await api.call("charactersList", { cursor: pageParam, limit: null, query: search });
      return { items: page.items.map((item) => ({ id: item.id, name: item.name, detail: item.description, avatar: item.avatar?.url ?? null })), next: page.next_cursor };
    },
    getNextPageParam: (page) => page.next ?? undefined,
  });
  const items = rows.data?.pages.flatMap((page) => page.items) ?? [];
  const picked = (id: string) => value.some((entry) => entry.kind === kind && entry.id === id);
  const toggle = (row: Row) => {
    if (!kind) return;
    onChange(picked(row.id) ? value.filter((entry) => !(entry.kind === kind && entry.id === row.id)) : [...value, { kind, id: row.id, name: row.name }]);
  };
  return (
    <Sheet
      open={kind !== null}
      onClose={onClose}
      size="sm"
      title={t(kind === "persona" ? "references.personaTitle" : "references.characterTitle")}
      description={t("references.description")}
      footer={<Button variant="primary" className="w-full" onClick={onClose}>{t("references.done")}</Button>}
    >
      <div className="space-y-4 pb-1">
        <SearchField value={query} onChange={setQuery} placeholder={t("references.search")} aria-label={t("references.search")} />
        {rows.isPending ? (
          <LoadingState />
        ) : rows.isError ? (
          <FailureState size="sm" error={rows.error} onRetry={() => void rows.refetch()} />
        ) : items.length === 0 ? (
          <EmptyState size="sm" title={t(kind === "persona" ? "references.noPersonas" : "references.noCharacters")} />
        ) : (
          <ListGroup>
            {items.map((row) => (
              <ListRowButton
                key={row.id}
                icon={<Avatar src={row.avatar} name={row.name || "?"} size="sm" />}
                title={row.name}
                description={row.detail ? <span className="line-clamp-2">{row.detail}</span> : undefined}
                aria-pressed={picked(row.id)}
                chevron={false}
                trailing={picked(row.id) ? <Check size={icon.md} className="text-accent" /> : undefined}
                onClick={() => toggle(row)}
              />
            ))}
          </ListGroup>
        )}
        {rows.hasNextPage && (
          <Button variant="ghost" block loading={rows.isFetchingNextPage} onClick={() => void rows.fetchNextPage()}>
            {t("loadMore")}
          </Button>
        )}
      </div>
    </Sheet>
  );
}
