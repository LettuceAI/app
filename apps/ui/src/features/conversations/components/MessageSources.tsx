import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  Button, Collapsible, ListGroup, ListRow, SearchField, Skeleton, Text,
} from "@lettuceai/crisp";
import { useState } from "react";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { PromptSources, PromptSection } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { promptSnapshotQuery } from "../data/queries";

function MoreSources({ shown, total, onMore }: { shown: number; total: number; onMore: () => void }) {
  const { t, i18n } = useTranslation("conversations");
  const numbers = new Intl.NumberFormat(i18n.language);
  return <ListRow
    title={t("menu.sources.showing", { shown: numbers.format(shown), total: numbers.format(total) })}
    trailing={<Button size="sm" variant="ghost" onClick={onMore}>{t("menu.sources.more")}</Button>}
  />;
}

function LorebookSources({ sources, sections }: { sources: PromptSources | undefined; sections: PromptSection[] | null }) {
  const { t, i18n } = useTranslation("conversations");
  const [query, setQuery] = useState("");
  const [limit, setLimit] = useState(40);
  const entries = sources?.lorebook_entries;
  const historicalLabels = sections?.filter((section) => section.kind === "lorebook") ?? [];
  const count = entries?.length ?? historicalLabels.length;
  const needle = query.trim().toLocaleLowerCase();
  const shown = entries?.filter((entry) =>
    [entry.title, entry.lorebook_name].some((text) => text.toLocaleLowerCase().includes(needle)),
  ) ?? [];
  return <Collapsible variant="plain" contentClassName="pl-0" title={t("menu.sources.lorebooks")} trailing={sources || count > 0 ? new Intl.NumberFormat(i18n.language).format(count) : undefined}>
    <div className="space-y-3">
      {entries ? entries.length === 0 ? <Text variant="hint">{t("menu.sources.noLorebooks")}</Text> : <>
        {entries.length > 8 && <SearchField value={query} onChange={(value) => { setQuery(value); setLimit(40); }} placeholder={t("menu.sources.searchEntries")} aria-label={t("menu.sources.searchEntries")} />}
        {shown.length === 0 ? <Text variant="hint">{t("menu.sources.noMatches")}</Text> : <ListGroup>
          {shown.slice(0, limit).map((entry) => <ListRow
            key={`${entry.lorebook_id}:${entry.entry_id}`}
            title={entry.title || t("menu.details.untitled")}
            description={<Text as="span" variant="caption" className="block truncate">{entry.lorebook_name}</Text>}
          />)}
          {shown.length > limit && <MoreSources shown={limit} total={shown.length} onMore={() => setLimit((value) => value + 40)} />}
        </ListGroup>}
      </> : <>
        {historicalLabels.length > 0 && <ListGroup>{historicalLabels.map((entry, index) => <ListRow key={index} title={entry.label || t("menu.details.untitled")} />)}</ListGroup>}
        <Text variant="hint">{t("menu.sources.notRecorded")}</Text>
      </>}
    </div>
  </Collapsible>;
}

function MemorySources({ sources }: { sources: PromptSources | undefined }) {
  const { t, i18n } = useTranslation("conversations");
  const [query, setQuery] = useState("");
  const [limit, setLimit] = useState(40);
  const memories = sources?.memories ?? [];
  const needle = query.trim().toLocaleLowerCase();
  const shown = memories.filter((memory) => memory.text.toLocaleLowerCase().includes(needle));
  const percent = new Intl.NumberFormat(i18n.language, { style: "percent", maximumFractionDigits: 0 });
  return <Collapsible variant="plain" contentClassName="pl-0" title={t("menu.sources.memories")} trailing={sources ? new Intl.NumberFormat(i18n.language).format(memories.length) : undefined}>
    <div className="space-y-3">
      {!sources ? <Text variant="hint">{t("menu.sources.notRecorded")}</Text> : <>
        {memories.length === 0 ? <Text variant="hint">{t("menu.sources.noMemories")}</Text> : <>
          {memories.length > 8 && <SearchField value={query} onChange={(value) => { setQuery(value); setLimit(40); }} placeholder={t("menu.sources.searchMemories")} aria-label={t("menu.sources.searchMemories")} />}
          {shown.length === 0 ? <Text variant="hint">{t("menu.sources.noMatches")}</Text> : <ListGroup>
            {shown.slice(0, limit).map((memory, index) => <ListRow
              key={memory.memory_id ?? index}
              title={memory.match_score_basis_points === null ? t("menu.sources.memory") : t("menu.sources.match", {
                score: percent.format(memory.match_score_basis_points / 10_000),
              })}
              description={<Text as="span" variant="secondary" className="line-clamp-2 break-words">{memory.text}</Text>}
            />)}
            {shown.length > limit && <MoreSources shown={limit} total={shown.length} onMore={() => setLimit((value) => value + 40)} />}
          </ListGroup>}
        </>}
      </>}
    </div>
  </Collapsible>;
}

export function MessageSources({ api, conversationId, messageId }: { api: ApiClient; conversationId: string; messageId: string }) {
  const { t } = useTranslation("conversations");
  const snapshot = useQuery(promptSnapshotQuery(api, conversationId, messageId));
  if (snapshot.isPending) return <div className="space-y-2" aria-hidden="true"><Skeleton className="h-9 w-full" /><Skeleton className="h-9 w-full" /></div>;
  if (snapshot.isError) return isApiFailure(snapshot.error) && snapshot.error.code === "not_found"
    ? <Text variant="hint">{t("menu.sources.notRecorded")}</Text>
    : <FailureState size="sm" error={snapshot.error} onRetry={() => void snapshot.refetch()} />;
  const sources = snapshot.data.used_sources ?? undefined;
  return <div className="space-y-2">
    <LorebookSources sources={sources} sections={snapshot.data.sections} />
    <MemorySources sources={sources} />
  </div>;
}
