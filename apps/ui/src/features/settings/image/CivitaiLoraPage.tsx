import { useState } from "react";
import { Copy } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Badge, Button, Chip, compactNumber, EmptyState, icon, IconButton, ImageViewer, KeyValue, ListGroup, ListRow, Masonry, MediaTile, Page, Segmented, Select, Skeleton, SkeletonRow, Text } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import type { CivitaiFile, CivitaiImage, CivitaiModelDetail } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { CivitaiTokenSheet, TokenAlert } from "./CivitaiTokenSheet";
import { ImageJob } from "./ImageJob";
import { civitaiImage, civitaiModelUrl, civitaiTokenNeeded, civitaiWidths, loraDownloadKey, useByteSize, useCivitaiAuth, useCopyText, useImageAction, usePureMode } from "./local-data";

export type LoraDownloads = Record<string, { id: string; title: string }>;

const segmentedVersions = 3;
const stripImages = 6;
const heroMaxHeight = "70dvh";
const wrappingChip = "h-auto min-h-8 max-w-full whitespace-normal rounded-2xl py-1.5 text-left";
const descriptionPreview = 600;

const isSafetensors = (file: CivitaiFile) => file.name.toLowerCase().endsWith(".safetensors");

function stripHtml(value: string) {
  const withBreaks = value.replace(/<(?:br|\/p|\/div|\/li|\/h[1-6])[^>]*>/gi, "\n");
  const text = new DOMParser().parseFromString(withBreaks, "text/html").body.textContent ?? "";
  return text.split("\n").map((line) => line.trim()).join("\n").replace(/\n{3,}/g, "\n\n").trim();
}

const aspectOf = (image: CivitaiImage) => image.width > 0 && image.height > 0 ? `${image.width} / ${image.height}` : undefined;

export function CivitaiLoraPage({ id, downloads, onBack }: { id: number; downloads: LoraDownloads; onBack: () => void }) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const copy = useCopyText();
  const detail = useQuery({ queryKey: queryKeys.civitai.model(id), queryFn: () => api.call("civitaiModel", { model_id: id }) });
  const missing = detail.isError && isApiFailure(detail.error) && detail.error.code === "not_found";
  return (
    <Page title={detail.data?.name ?? (detail.isPending ? <Skeleton className="h-6 w-48" /> : t("image.local.lora.detailTitle"))} {...(detail.data?.creator_username ? { meta: t("image.local.lora.by", { name: detail.data.creator_username }) } : {})} onBack={onBack} backLabel={t("image.local.lora.browse")} actions={<IconButton label={t("image.local.lora.copyLink")} variant="ghost" onClick={() => copy(civitaiModelUrl(id))}><Copy size={icon.md} /></IconButton>}>
      <div className="@container">
        {missing ? <EmptyState title={t("image.local.lora.notFound")} description={t("image.local.lora.notFoundBody")} action={<Button variant="secondary" onClick={onBack}>{t("image.local.lora.browse")}</Button>} />
          : detail.isError ? <FailureState error={detail.error} onRetry={() => void detail.refetch()} />
          : !detail.data ? <LoraPageSkeleton />
          : <LoraDetail model={detail.data} downloads={downloads} />}
      </div>
    </Page>
  );
}

function LoraPageSkeleton() {
  const { t } = useTranslation("settings");
  return (
    <div role="status" aria-busy="true" aria-label={t("image.local.lora.loadingModel")} className="grid gap-7 @4xl:grid-cols-[minmax(0,1fr)_26rem] @4xl:items-start">
      <Skeleton shape="block" className="aspect-3/4 w-full max-w-md justify-self-center rounded-xl" />
      <div className="space-y-5">
        <Skeleton className="h-8 w-40" />
        <ListGroup>{[0, 1, 2].map((row) => <SkeletonRow key={row} />)}</ListGroup>
      </div>
    </div>
  );
}

function LoraDetail({ model, downloads }: { model: CivitaiModelDetail; downloads: LoraDownloads }) {
  const { t, i18n } = useTranslation("settings");
  const api = useSettingsApi();
  const action = useImageAction();
  const copy = useCopyText();
  const size = useByteSize();
  const auth = useCivitaiAuth();
  const pure = usePureMode();
  const installed = useQuery({ queryKey: queryKeys.loras.all, queryFn: () => api.call("lorasList", { profile_id: null }) });
  const [versionId, setVersionId] = useState<number | null>(null);
  const [heroUrl, setHeroUrl] = useState<string | null>(null);
  const [viewer, setViewer] = useState<number | null>(null);
  const [revealed, setRevealed] = useState<Set<string>>(new Set());
  const [expanded, setExpanded] = useState(false);
  const [tokenOpen, setTokenOpen] = useState(false);
  const version = model.versions.find((item) => item.id === versionId) ?? model.versions.find((item) => item.files.some(isSafetensors)) ?? model.versions[0];
  const showNsfw = pure === "off";
  const images = version?.images ?? [];
  const shown = images.filter((image) => image.nsfw_level <= 1 || showNsfw || revealed.has(image.url));
  const held = images.filter((image) => !shown.includes(image));
  const hero = shown.find((image) => image.url === heroUrl) ?? shown[0];
  const strip = shown.slice(0, stripImages);
  const more = shown.slice(stripImages);
  const reveal = (urls: string[]) => setRevealed((current) => new Set([...current, ...urls]));
  const choose = (value: string) => { setVersionId(Number(value)); setHeroUrl(null); setViewer(null); };
  const files = version?.files.filter(isSafetensors) ?? [];
  const running = (file: CivitaiFile) => version !== undefined && downloads[loraDownloadKey(version.id, file.id)] !== undefined;
  const jobs = version ? files.flatMap((file) => { const job = downloads[loraDownloadKey(version.id, file.id)]; return job ? [job] : []; }) : [];
  const description = model.description ? stripHtml(model.description) : "";
  const long = description.length > descriptionPreview;
  const date = version?.published_at ? new Date(version.published_at) : null;
  const facts = version ? [
    { label: t("image.local.lora.base"), value: version.base_model ?? t("image.local.lora.unknownBase") },
    { label: t("image.local.lora.architecture"), value: version.architecture ?? t("image.local.lora.unknownBase") },
    ...(date && !Number.isNaN(date.getTime()) ? [{ label: t("image.local.lora.published"), value: new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" }).format(date) }] : []),
    { label: t("image.local.lora.downloads"), value: compactNumber(model.download_count, i18n.language) },
    { label: t("image.local.lora.likes"), value: compactNumber(model.thumbs_up_count, i18n.language) },
  ] : [];
  const viewerIndex = (image: CivitaiImage) => setViewer(shown.indexOf(image));
  return (
    <div className="grid gap-7 @4xl:grid-cols-[minmax(0,1fr)_26rem] @4xl:items-start">
      <section aria-label={t("image.local.lora.gallery")} className="min-w-0 space-y-3 @4xl:col-start-1 @4xl:row-start-1">
        {images.length === 0 ? <EmptyState size="sm" title={t("image.local.lora.noImages")} />
          : shown.length === 0 ? <EmptyState size="sm" title={t("image.local.lora.previewsHidden")} description={t("image.local.lora.previewsHiddenBody")} action={<Button variant="secondary" onClick={() => reveal(held.map((image) => image.url))}>{t("image.local.lora.showImages")}</Button>} />
          : hero && <>
            <MediaTile src={civitaiImage(hero.url, civitaiWidths.hero)} alt={t("image.local.lora.imageAlt", { name: model.name, index: shown.indexOf(hero) + 1 })} className="mx-auto" style={{ aspectRatio: aspectOf(hero) ?? "3 / 4", maxWidth: `calc(${heroMaxHeight} * ${hero.width > 0 && hero.height > 0 ? hero.width / hero.height : 0.75})` }} onClick={() => viewerIndex(hero)} />
            {strip.length > 1 && <div className="flex gap-2 overflow-x-auto pb-1">{strip.map((image, index) => <div key={image.url} className="w-16 shrink-0"><MediaTile src={civitaiImage(image.url, civitaiWidths.card)} alt={t("image.local.lora.imageAlt", { name: model.name, index: index + 1 })} selected={image === hero} onClick={() => setHeroUrl(image.url)} /></div>)}</div>}
          </>}
      </section>
      <aside className="min-w-0 space-y-6 @4xl:sticky @4xl:top-(--page-bar-height) @4xl:col-start-2 @4xl:row-span-3 @4xl:row-start-1 @4xl:self-start">
        {model.versions.length > 1 && (model.versions.length <= segmentedVersions
          ? <Segmented size="sm" aria-label={t("image.local.lora.version")} value={String(version?.id ?? "")} options={model.versions.map((item) => ({ value: String(item.id), label: item.name }))} onChange={choose} />
          : <Select aria-label={t("image.local.lora.version")} value={String(version?.id ?? "")} options={model.versions.map((item) => ({ value: String(item.id), label: item.name }))} onChange={(event) => choose(event.target.value)} />)}
        {version && <KeyValue items={facts} />}
        {civitaiTokenNeeded(auth.data) && <TokenAlert saved={auth.data?.saved ?? false} onOpen={() => setTokenOpen(true)} />}
        <ListGroup label={t("image.local.bundle.files")}>
          {files.length === 0 ? <ListRow title={t("image.local.lora.noFiles")} />
            : files.map((file) => {
              const present = installed.data?.loras.some((lora) => lora.filename.toLowerCase() === file.name.toLowerCase());
              const busy = running(file);
              return <ListRow key={file.id} title={<span className="font-mono text-sm">{file.name}</span>} description={[file.size_kb === null ? null : size(file.size_kb * 1024), file.fp, file.primary ? t("image.local.lora.mainFile") : null, busy ? t("image.local.lora.downloading") : null].filter(Boolean).join(" · ")} trailing={present ? <Badge tone="accent">{t("image.local.lora.inLibrary")}</Badge> : <Button size="sm" variant="tonal" disabled={busy || action.isPending || !installed.data} onClick={() => { if (version) action.mutate(async () => { await api.call("civitaiLoraDownload", { model_id: model.id, version_id: version.id, file_id: file.id }); }); }}>{t("image.local.catalog.download")}</Button>} />;
            })}
        </ListGroup>
        {jobs.map((job) => <ImageJob key={job.id} id={job.id} title={job.title} download />)}
        {action.isError && <FailureState error={action.error} size="sm" />}
        {installed.isError && <FailureState error={installed.error} onRetry={() => void installed.refetch()} size="sm" />}
        {auth.isError && <FailureState error={auth.error} onRetry={() => void auth.refetch()} size="sm" />}
        {version && <div className="space-y-2">
          <div className="flex items-center justify-between gap-2"><Text variant="label">{t("image.local.lora.triggerWords")}</Text>{version.trained_words.length > 1 && <Button variant="ghost" size="sm" onClick={() => copy(version.trained_words.join(", "))}>{t("image.local.lora.copyAll")}</Button>}</div>
          {version.trained_words.length ? <div className="flex flex-wrap gap-1.5">{version.trained_words.map((word) => <Chip key={word} className={wrappingChip} icon={<Copy size={icon.xs} className="shrink-0" />} aria-label={t("image.local.lora.copyWord", { word })} onClick={() => copy(word)}>{word}</Chip>)}</div> : <Text variant="hint">{t("image.local.lora.noneListed")}</Text>}
        </div>}
        <div className="space-y-2">
          <Text variant="label">{t("image.local.lora.tags")}</Text>
          {model.tags.length ? <div className="flex flex-wrap gap-1.5">{model.tags.map((tag) => <Chip key={tag}>{tag}</Chip>)}</div> : <Text variant="hint">{t("image.local.lora.noneListed")}</Text>}
        </div>
      </aside>
      {description && <section className="min-w-0 space-y-2 @4xl:col-start-1 @4xl:row-start-2">
        <Text variant="label">{t("image.local.lora.description")}</Text>
        <Text variant="secondary" className={long && !expanded ? "line-clamp-6 whitespace-pre-line" : "whitespace-pre-line"}>{description}</Text>
        {long && <Button variant="ghost" size="sm" aria-expanded={expanded} onClick={() => setExpanded((current) => !current)}>{t(expanded ? "image.local.lora.showLess" : "image.local.lora.showMore")}</Button>}
      </section>}
      {(more.length > 0 || (held.length > 0 && shown.length > 0)) && <section aria-label={t("image.local.lora.moreImages")} className="min-w-0 space-y-2 @4xl:col-start-1 @4xl:row-start-3">
        <Text variant="label">{t("image.local.lora.moreImages")}</Text>
        <Masonry>
          {more.map((image) => <MediaTile key={image.url} src={civitaiImage(image.url, civitaiWidths.card)} alt={t("image.local.lora.imageAlt", { name: model.name, index: shown.indexOf(image) + 1 })} style={{ aspectRatio: aspectOf(image) ?? "3 / 4" }} onClick={() => viewerIndex(image)} />)}
          {shown.length > 0 && held.map((image) => <Button key={image.url} variant="secondary" block onClick={() => reveal([image.url])}>{t("image.local.lora.reveal")}</Button>)}
        </Masonry>
      </section>}
      <ImageViewer images={shown.map((image, index) => ({ src: civitaiImage(image.url, image.width), alt: t("image.local.lora.imageAlt", { name: model.name, index: index + 1 }) }))} index={viewer} onIndexChange={setViewer} />
      <CivitaiTokenSheet open={tokenOpen} onClose={() => setTokenOpen(false)} />
    </div>
  );
}
