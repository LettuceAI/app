import { useState } from "react";
import { createPortal } from "react-dom";
import { FileUp, Layers, Sparkles, Trash2 } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Alert, Button, confirm, EmptyState, Field, icon, ListGroup, ListRowButton, Sheet, SkeletonRow, Tab, TabList, Tabs, Text, Textarea, toast, useLongPress } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import type { InstalledLora } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { ImageJob } from "./ImageJob";
import { CivitaiView } from "./CivitaiView";
import { useByteSize, useCleanupWarning, useImageAction } from "./local-data";

const keywordPreview = 4;

export function LoraLibraryView({ open, actionTarget, downloads = [] }: { open: boolean; actionTarget?: HTMLElement | null; downloads?: { id: string; title: string }[] }) {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation("common");
  const api = useSettingsApi();
  const action = useImageAction();
  const importing = useImageAction();
  const warn = useCleanupWarning();
  const size = useByteSize();
  const loras = useQuery({ queryKey: queryKeys.loras.all, queryFn: () => api.call("lorasList", { profile_id: null }), refetchOnMount: "always" });
  const [view, setView] = useState<"library" | "browse">("library");
  const [selected, setSelected] = useState<InstalledLora | null>(null);
  const [keywords, setKeywords] = useState("");
  const [browsed, setBrowsed] = useState(false);
  const [discoveries, setDiscoveries] = useState<{ id: string; title: string }[]>([]);
  const [wasOpen, setWasOpen] = useState(open);
  if (wasOpen !== open) {
    setWasOpen(open);
    if (!open) { setSelected(null); setView("library"); }
  }
  if (view === "browse" && !browsed) setBrowsed(true);
  const importLora = () => importing.mutate(async () => {
    const picked = await api.call("filesPickOpen", { kinds: ["any"], multiple: false });
    const source = picked.sources[0];
    if (!source) return;
    const added = await api.call("lorasImport", { source });
    toast.success(t("image.local.lora.imported", { name: added.filename }));
  });
  const choose = (lora: InstalledLora) => { setSelected(lora); setKeywords(lora.keywords.join("\n")); };
  const remove = async () => {
    if (!selected) return;
    const sure = await confirm({ title: t("tools.deleteLoraTitle", { name: selected.filename }), message: t("image.local.lora.deleteConfirm", { name: selected.filename }), confirmLabel: t("manage.delete"), cancelLabel: common("actions.cancel"), destructive: true });
    if (sure) action.mutate(async () => { const result = await api.call("lorasDelete", { path: selected.path }); toast.success(t("image.local.lora.deleted", { name: selected.filename })); warn(result.left_behind); setSelected(null); });
  };
  return (
    <div className="space-y-4">
      {actionTarget && createPortal(<Button variant="tonal" size="sm" leading={<FileUp size={icon.sm} />} loading={importing.isPending} onClick={importLora}>{t("image.local.lora.importFile")}</Button>, actionTarget)}
      <Tabs value={view} onChange={setView}><TabList aria-label={t("models.loras")}><Tab value="library" {...(loras.data ? { count: loras.data.loras.length } : {})}>{t("image.local.lora.library")}</Tab><Tab value="browse">{t("image.local.lora.browse")}</Tab></TabList></Tabs>
      {importing.isError && (isApiFailure(importing.error) && importing.error.code === "conflict" ? <Alert tone="danger" title={t("image.local.lora.importConflict")} /> : <FailureState error={importing.error} size="sm" />)}
      {downloads.map((download) => <ImageJob key={download.id} id={download.id} title={download.title} download />)}
      {discoveries.map((discovery) => <ImageJob key={discovery.id} id={discovery.id} title={discovery.title} />)}
      {browsed && <div className={view === "browse" ? undefined : "hidden"}><CivitaiView /></div>}
      {view === "library" && <>
        {!actionTarget && <Button loading={importing.isPending} onClick={importLora}>{t("manage.import")}</Button>}
        {loras.isError ? <FailureState error={loras.error} onRetry={() => void loras.refetch()} /> : !loras.data ? <div role="status" aria-busy="true" aria-label={t("image.local.lora.loading")}><ListGroup>{[0, 1, 2, 3].map((row) => <SkeletonRow key={row} />)}</ListGroup></div> : loras.data.loras.length === 0 ? <EmptyState icon={<Layers size={icon.xl} />} title={t("image.local.lora.empty")} description={t("image.local.lora.emptyBody")} action={<div className="flex flex-wrap justify-center gap-2"><Button variant="primary" onClick={() => setView("browse")}>{t("image.local.lora.browse")}</Button><Button variant="secondary" loading={importing.isPending} onClick={importLora}>{t("image.local.lora.importFile")}</Button></div>} /> : <ListGroup>{loras.data.loras.map((lora) => <LoraRow key={lora.path} lora={lora} onOpen={() => choose(lora)} />)}</ListGroup>}
      </>}
      <Sheet open={selected !== null} onClose={() => setSelected(null)} placement="auto" title={selected?.filename} description={selected ? `${size(selected.bytes_on_disk)} · ${selected.architecture ?? t("image.local.lora.unknownBase")}` : undefined} dismissible={!action.isPending} footer={<div className="flex justify-end gap-2"><Button variant="ghost" onClick={() => setSelected(null)}>{common("actions.cancel")}</Button><Button disabled={!selected || keywords === selected.keywords.join("\n")} loading={action.isPending} onClick={() => { if (selected) action.mutate(async () => { await api.call("lorasUpdateKeywords", { path: selected.path, profile_id: null, keywords: keywords.split("\n").map((word) => word.trim()).filter(Boolean) }); setSelected(null); }); }}>{t("manage.save")}</Button></div>}>
        {action.isError && (isApiFailure(action.error) && action.error.code === "conflict" ? <Alert tone="danger" title={t("image.local.lora.deleteBlockedTitle")} description={t("image.local.lora.deleteBlockedBody")} /> : <FailureState error={action.error} size="sm" />)}
        {selected && <div className="space-y-5">
          <Field label={t("image.local.lora.triggerWords")} hint={t("image.local.lora.keywordBody")}><Textarea rows={6} value={keywords} onChange={(event) => setKeywords(event.target.value)} aria-label={t("image.local.lora.keywords")} /></Field>
          <Button variant="secondary" leading={<Sparkles size={icon.sm} />} loading={action.isPending} onClick={() => action.mutate(async () => { const accepted = await api.call("loraKeywordsDiscover", { path: selected.path, profile_id: null, client_operation_id: crypto.randomUUID() }); const title = t("image.local.lora.discoverFor", { name: selected.filename }); setDiscoveries((current) => current.some((item) => item.id === accepted.job_id) ? current : [...current, { id: accepted.job_id, title }]); setSelected(null); })}>{t("image.local.lora.discover")}</Button>
          <Text variant="hint">{t("image.local.lora.discoverBody")}</Text>
          <ListGroup><ListRowButton icon={<Trash2 size={icon.lg} />} title={t("manage.delete")} description={t("image.local.lora.deleteBody")} tone="danger" chevron={false} onClick={() => void remove()} /></ListGroup>
        </div>}
      </Sheet>
    </div>
  );
}

function LoraRow({ lora, onOpen }: { lora: InstalledLora; onOpen: () => void }) {
  const { t } = useTranslation("settings");
  const size = useByteSize();
  const press = useLongPress(onOpen);
  const shown = lora.keywords.slice(0, keywordPreview);
  const keywords = lora.keywords.length ? t(lora.keywords.length > keywordPreview ? "image.local.lora.addsMore" : "image.local.lora.adds", { words: shown.join(", "), count: lora.keywords.length - keywordPreview }) : t(lora.keyword_source === "none" ? "image.local.lora.noKeywords" : "image.local.lora.alwaysActive");
  return <ListRowButton icon={<Layers size={icon.lg} />} title={lora.filename} description={`${size(lora.bytes_on_disk)} · ${lora.architecture ?? t("image.local.lora.unknownBase")} · ${keywords}`} {...press.handlers} onClick={press.guardClick(onOpen)} />;
}
