import { useEffect, useState } from "react";
import { ArrowDownUp, ChevronDown, KeyRound } from "lucide-react";
import { keepPreviousData, useInfiniteQuery } from "@tanstack/react-query";
import { useNavigate, useRouter } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Button, compactNumber, EmptyState, FilterBar, icon, IconButton, MediaCard, MediaCardSkeleton, Menu, MenuContent, MenuItem, MenuLabel, MenuSeparator, MenuTrigger, SearchField, Text } from "@lettuceai/crisp";
import type { CivitaiLoraSummary } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { CivitaiTokenSheet, TokenAlert } from "./CivitaiTokenSheet";
import { civitaiImage, civitaiTokenNeeded, civitaiWidths, useCivitaiAuth, usePureMode } from "./local-data";

const skeletonCards = 8;

export function CivitaiView() {
  const { t, i18n } = useTranslation("settings");
  const api = useSettingsApi();
  const navigate = useNavigate();
  const router = useRouter();
  const pure = usePureMode();
  const [input, setInput] = useState("");
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState("Highest Rated");
  const [period, setPeriod] = useState("AllTime");
  const [base, setBase] = useState("");
  const [bases, setBases] = useState<string[]>([]);
  const [tokenOpen, setTokenOpen] = useState(false);
  useEffect(() => {
    const timer = window.setTimeout(() => setQuery(input.trim()), 350);
    return () => window.clearTimeout(timer);
  }, [input]);
  const auth = useCivitaiAuth();
  const results = useInfiniteQuery({ queryKey: queryKeys.civitai.search(query, sort, period, base), initialPageParam: null as string | null, queryFn: ({ pageParam }) => api.call("civitaiSearch", { query: query || null, sort, period, base_models: base ? [base] : [], cursor: pageParam, limit: null }), getNextPageParam: (page) => page.next_cursor ?? undefined, placeholderData: keepPreviousData });
  const found = [...new Map((results.data?.pages.flatMap((page) => page.items) ?? []).map((item) => [item.id, item])).values()];
  const seen = [...new Set([...found.flatMap((item) => item.base_models), ...(base ? [base] : [])])].filter((value) => !bases.includes(value));
  if (seen.length) setBases([...bases, ...seen].sort());
  const open = (item: CivitaiLoraSummary) => {
    if (router.latestLocation.search.civitaiModel !== undefined) return;
    void navigate({ to: "/settings/$section", params: { section: "imageGeneration" }, search: { localImageView: "loras", civitaiModel: item.id }, state: { loraDetailPushed: true }, resetScroll: false });
  };
  const showNsfw = pure === "off";
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <SearchField className="min-w-0 flex-1 basis-56" value={input} onChange={setInput} aria-label={t("image.local.lora.search")} placeholder={t("image.local.lora.search")} />
        <Menu><MenuTrigger><Button variant="secondary" leading={<ArrowDownUp size={icon.sm} />} trailing={<ChevronDown size={icon.sm} />}>{t(sort === "Highest Rated" ? "image.local.lora.highestRated" : sort === "Most Downloaded" ? "image.local.lora.mostDownloaded" : "image.local.lora.newest")}</Button></MenuTrigger><MenuContent align="end">
          <MenuLabel>{t("image.local.lora.sort")}</MenuLabel>
          {([{ value: "Highest Rated", key: "image.local.lora.highestRated" }, { value: "Most Downloaded", key: "image.local.lora.mostDownloaded" }, { value: "Newest", key: "image.local.lora.newest" }] as const).map((item) => <MenuItem key={item.value} selected={sort === item.value} onSelect={() => setSort(item.value)}>{t(item.key)}</MenuItem>)}
          <MenuSeparator /><MenuLabel>{t("image.local.lora.period")}</MenuLabel>
          {(["AllTime", "Year", "Month", "Week", "Day"] as const).map((value) => <MenuItem key={value} selected={period === value} onSelect={() => setPeriod(value)}>{t(`image.local.lora.periods.${value}`)}</MenuItem>)}
        </MenuContent></Menu>
        <IconButton label={t("image.local.lora.token")} variant="secondary" onClick={() => setTokenOpen(true)}><KeyRound size={icon.md} /></IconButton>
      </div>
      <FilterBar aria-label={t("image.local.lora.base")} options={[{ value: "", label: t("image.local.lora.allBases") }, ...bases.map((value) => ({ value, label: value }))]} value={base} onChange={setBase} />
      {pure !== undefined && !showNsfw && <Text variant="hint">{t("image.local.lora.pureNote")}</Text>}
      {auth.isError && <FailureState error={auth.error} onRetry={() => void auth.refetch()} size="sm" />}
      {civitaiTokenNeeded(auth.data) && <TokenAlert saved={auth.data?.saved ?? false} onOpen={() => setTokenOpen(true)} />}
      {results.isPending ? <div role="status" aria-busy="true" aria-label={t("image.local.lora.loadingResults")} className="@container"><div className="grid grid-cols-2 gap-3 @xl:grid-cols-3 @4xl:grid-cols-4">{Array.from({ length: skeletonCards }, (_, index) => <MediaCardSkeleton key={index} />)}</div></div>
        : found.length === 0 && !results.isError ? <EmptyState title={t("image.local.lora.noResults")} description={t("image.local.lora.noResultsBody")} />
        : <div className="@container"><div className="grid grid-cols-2 gap-3 @xl:grid-cols-3 @4xl:grid-cols-4">{found.map((item) => <MediaCard key={item.id} title={item.name} image={item.preview_image && (item.preview_image.nsfw_level <= 1 || showNsfw) ? civitaiImage(item.preview_image.url, civitaiWidths.card) : null} description={[item.creator_username ? t("image.local.lora.by", { name: item.creator_username }) : null, t("image.local.lora.downloadCount", { count: item.download_count, value: compactNumber(item.download_count, i18n.language) })].filter(Boolean).join(" · ")} {...(item.base_models[0] ? { meta: item.base_models[0] } : {})} onClick={() => open(item)} />)}</div></div>}
      {results.isError && <FailureState error={results.error} onRetry={() => void (found.length ? results.fetchNextPage() : results.refetch())} />}
      {results.hasNextPage && !results.isError && <Button variant="ghost" disabled={results.isPlaceholderData} loading={results.isFetchingNextPage} onClick={() => void results.fetchNextPage()}>{t("tools.loadMore")}</Button>}
      <CivitaiTokenSheet open={tokenOpen} onClose={() => setTokenOpen(false)} />
    </div>
  );
}
