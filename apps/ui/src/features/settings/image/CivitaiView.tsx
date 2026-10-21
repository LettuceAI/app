import { useEffect, useState } from "react";
import { ArrowDownUp, ChevronDown, Copy, KeyRound } from "lucide-react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Alert, Badge, Button, Chip, EmptyState, Field, FilterBar, icon, IconButton, ImageViewer, InlineMarkdown, KeyValue, ListGroup, ListRow, LoadingState, MediaCard, MediaTile, Menu, MenuContent, MenuItem, MenuLabel, MenuSeparator, MenuTrigger, SearchField, SecretInput, Segmented, Sheet, Text } from "@lettuceai/crisp";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { ImageJob } from "./ImageJob";
import { useByteSize, useCopyText, useImageAction } from "./local-data";

export function CivitaiView() {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const action = useImageAction();
  const [input, setInput] = useState("");
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState("Highest Rated");
  const [period, setPeriod] = useState("AllTime");
  const [base, setBase] = useState("");
  const [detail, setDetail] = useState<number | null>(null);
  const [token, setToken] = useState("");
  const [tokenOpen, setTokenOpen] = useState(false);
  useEffect(() => {
    const timer = window.setTimeout(() => setQuery(input.trim()), 350);
    return () => window.clearTimeout(timer);
  }, [input]);
  const auth = useQuery({ queryKey: [...queryKeys.localImages.all, "civitai-auth"], queryFn: () => api.call("civitaiAuthStatus") });
  const results = useInfiniteQuery({ queryKey: [...queryKeys.localImages.all, "civitai", query, sort, period, base], initialPageParam: null as string | null, queryFn: ({ pageParam }) => api.call("civitaiSearch", { query: query || null, sort, period, base_models: base ? [base] : [], cursor: pageParam, limit: null }), getNextPageParam: (page) => page.next_cursor ?? undefined, enabled: detail === null });
  const found = results.data?.pages.flatMap((page) => page.items) ?? [];
  const bases = [...new Set([...found.flatMap((item) => item.base_models), ...(base ? [base] : [])])];
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
      {auth.isError && <FailureState error={auth.error} onRetry={() => void auth.refetch()} size="sm" />}
      {auth.data && !auth.data.valid && <Alert tone="warning" title={t("image.local.lora.token")} description={t("image.local.lora.tokenBody")} action={<Button variant="ghost" size="sm" onClick={() => setTokenOpen(true)}>{t("image.local.lora.token")}</Button>} />}
      {action.isError && <FailureState error={action.error} size="sm" />}
      {detail !== null && <CivitaiDetail key={detail} id={detail} onBack={() => setDetail(null)} />}
        {results.isPending ? <LoadingState /> : found.length === 0 && !results.isError ? <EmptyState title={t("tools.noResults")} /> : <div className="@container"><div className="grid grid-cols-2 gap-3 @xl:grid-cols-3 @4xl:grid-cols-4">{found.map((item) => <MediaCard key={item.id} title={item.name} image={item.preview_image && item.preview_image.nsfw_level <= 1 ? item.preview_image.url : null} description={[item.creator_username, ...item.base_models].filter(Boolean).join(" · ")} onClick={() => setDetail(item.id)} />)}</div></div>}
        {results.isError && <FailureState error={results.error} onRetry={() => void (found.length ? results.fetchNextPage() : results.refetch())} />}
        {results.hasNextPage && <Button variant="ghost" loading={results.isFetchingNextPage} onClick={() => void results.fetchNextPage()}>{t("tools.loadMore")}</Button>}
      <Sheet open={tokenOpen} onClose={() => { setTokenOpen(false); setToken(""); }} title={t("image.local.lora.token")} dismissible={!action.isPending}>
        <div className="space-y-3">
          <Field label={t("image.local.lora.token")} hint={t("image.local.lora.tokenPrivacy")}><SecretInput value={token} onChange={setToken} aria-label={t("image.local.lora.token")} /></Field>
          <div className="flex gap-2"><Button disabled={!token.trim() || action.isPending} onClick={() => action.mutate(async () => { await api.call("civitaiAuthSave", { token: token.trim() }); setToken(""); setTokenOpen(false); })}>{t("manage.save")}</Button>{auth.data?.saved && <Button variant="ghost" disabled={action.isPending} onClick={() => action.mutate(async () => { await api.call("civitaiAuthClear"); setToken(""); setTokenOpen(false); })}>{t("image.local.remove")}</Button>}</div>
          {action.isError && <FailureState error={action.error} size="sm" />}
        </div>
      </Sheet>
    </div>
  );
}

function CivitaiDetail({ id, onBack }: { id: number; onBack: () => void }) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const action = useImageAction();
  const copy = useCopyText();
  const size = useByteSize();
  const detail = useQuery({ queryKey: [...queryKeys.localImages.all, "civitai-model", id], queryFn: () => api.call("civitaiModel", { model_id: id }) });
  const installed = useQuery({ queryKey: queryKeys.loras.all, queryFn: () => api.call("lorasList", { profile_id: null }) });
  const [versionId, setVersion] = useState<number | null>(null);
  const [job, setJob] = useState<string | null>(null);
  const [revealed, setRevealed] = useState<Set<string>>(new Set());
  const [viewer, setViewer] = useState<number | null>(null);
  const version = detail.data?.versions.find((item) => item.id === versionId) ?? detail.data?.versions[0];
  const images = version?.images.filter((image) => image.nsfw_level <= 1 || revealed.has(image.url)) ?? [];
  return <Sheet open onClose={onBack} placement="auto" size="lg" title={detail.data?.name} description={detail.data?.creator_username ?? undefined} rightAction={<IconButton label={t("image.local.lora.copyLink")} variant="ghost" onClick={() => copy(`https://civitai.com/models/${id}`)}><Copy size={icon.md} /></IconButton>}><div className="space-y-5">
    {detail.isError ? <FailureState error={detail.error} onRetry={() => void detail.refetch()} /> : !detail.data ? <LoadingState /> : <>
      {detail.data.versions.length > 1 && <Segmented size="sm" aria-label={t("image.local.lora.version")} value={String(version?.id ?? "")} options={detail.data.versions.map((item) => ({ value: String(item.id), label: item.name }))} onChange={(value) => { setVersion(Number(value)); setViewer(null); }} />}
      {version && <KeyValue items={[{ label: t("image.local.lora.base"), value: version.base_model ?? t("image.local.lora.unknownBase") }, { label: t("image.local.lora.architecture"), value: version.architecture ?? t("image.local.lora.unknownBase") }]} />}
      <ListGroup label={t("image.local.bundle.files")}>
        {version?.files.filter((file) => file.name.toLowerCase().endsWith(".safetensors")).map((file) => {
          const present = installed.data?.loras.some((lora) => lora.filename.toLowerCase() === file.name.toLowerCase());
          return <ListRow key={file.id} title={<span className="font-mono text-sm">{file.name}</span>} description={[file.size_kb === null ? null : size(file.size_kb * 1024), file.fp].filter(Boolean).join(" · ")} trailing={present ? <Badge tone="accent">{t("image.local.lora.inLibrary")}</Badge> : <Button size="sm" variant="tonal" disabled={action.isPending || !installed.data} onClick={() => action.mutate(async () => { const accepted = await api.call("civitaiLoraDownload", { model_id: id, version_id: version.id, file_id: file.id }); setJob(accepted.job_id); })}>{t("image.local.catalog.download")}</Button>} />;
        })}
        {version && !version.files.some((file) => file.name.toLowerCase().endsWith(".safetensors")) && <ListRow title={t("image.local.lora.noFiles")} />}
      </ListGroup>
      {version && <div className="space-y-2"><Text variant="label">{t("image.local.lora.triggerWords")}</Text><div className="flex flex-wrap gap-1.5">{version.trained_words.map((word) => <Chip key={word} icon={<Copy size={icon.xs} />} onClick={() => copy(word)}>{word}</Chip>)}</div></div>}
      {detail.data.description && <InlineMarkdown>{detail.data.description}</InlineMarkdown>}
      {version && <div className="grid grid-cols-2 gap-2 @xl:grid-cols-3">{version.images.map((image) => image.nsfw_level > 1 && !revealed.has(image.url) ? <Button key={image.url} variant="secondary" onClick={() => setRevealed((current) => new Set([...current, image.url]))}>{t("image.local.lora.reveal")}</Button> : <MediaTile key={image.url} aspect="portrait" src={image.url} alt={detail.data.name} onClick={() => setViewer(images.findIndex((candidate) => candidate.url === image.url))} />)}</div>}
    </>}
    {installed.isError && <FailureState error={installed.error} onRetry={() => void installed.refetch()} size="sm" />}
    {job && <ImageJob id={job} title={detail.data?.name} />}
    {action.isError && <FailureState error={action.error} size="sm" />}
    <ImageViewer images={images.map((image) => ({ src: image.url, alt: detail.data?.name ?? t("image.local.lora.preview") }))} index={viewer} onIndexChange={setViewer} />
  </div></Sheet>;
}
