import { useState } from "react";
import { createPortal } from "react-dom";
import { FileUp, Layers, Sparkles, Trash2 } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Button, confirm, EmptyState, Field, icon, ListGroup, ListRowButton, LoadingState, Sheet, Tab, TabList, Tabs, Text, Textarea, useLongPress } from "@lettuceai/crisp";
import type { InstalledLora } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { ImageJob } from "./ImageJob";
import { CivitaiView } from "./CivitaiView";
import { useByteSize, useCleanupWarning, useImageAction } from "./local-data";

export function LoraLibraryView({ open, actionTarget }: { open: boolean; actionTarget?: HTMLElement | null }) {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation("common");
  const api = useSettingsApi();
  const action = useImageAction();
  const warn = useCleanupWarning();
  const loras = useQuery({ queryKey: queryKeys.loras.all, queryFn: () => api.call("lorasList", { profile_id: null }) });
  const [view, setView] = useState<"library" | "browse">("library");
  const [selected, setSelected] = useState<InstalledLora | null>(null);
  const [keywords, setKeywords] = useState("");
  const [job, setJob] = useState<string | null>(null);
  const [wasOpen, setWasOpen] = useState(open);
  if (wasOpen !== open) {
    setWasOpen(open);
    if (!open) { setSelected(null); setView("library"); }
  }
  const importLora = () => action.mutate(async () => {
    const picked = await api.call("filesPickOpen", { kinds: ["any"], multiple: false });
    if (picked.sources[0]) await api.call("lorasImport", { source: picked.sources[0] });
  });
  const choose = (lora: InstalledLora) => { setSelected(lora); setKeywords(lora.keywords.join("\n")); };
  const remove = async () => {
    if (!selected) return;
    const sure = await confirm({ title: t("tools.deleteLoraTitle", { name: selected.filename }), message: t("tools.deleteLora"), confirmLabel: t("manage.delete"), cancelLabel: common("actions.cancel"), destructive: true });
    if (sure) action.mutate(async () => { const result = await api.call("lorasDelete", { path: selected.path }); warn(result.left_behind); setSelected(null); });
  };
  return (
    <div className="space-y-4">
      {actionTarget && createPortal(<Button variant="tonal" size="sm" leading={<FileUp size={icon.sm} />} loading={action.isPending} onClick={importLora}>{t("image.local.lora.importFile")}</Button>, actionTarget)}
      <Tabs value={view} onChange={setView}><TabList aria-label={t("models.loras")}><Tab value="library">{t("image.local.lora.library")}</Tab><Tab value="browse">{t("image.local.lora.browse")}</Tab></TabList></Tabs>
      {action.isError && <FailureState error={action.error} size="sm" />}
      {job && <ImageJob id={job} title={t("image.local.lora.discover")} />}
      {view === "browse" && open ? <CivitaiView /> : <>
        {!actionTarget && <Button loading={action.isPending} onClick={importLora}>{t("manage.import")}</Button>}
        {loras.isError ? <FailureState error={loras.error} onRetry={() => void loras.refetch()} /> : !loras.data ? <LoadingState /> : loras.data.loras.length === 0 ? <EmptyState icon={<Layers size={icon.xl} />} title={t("tools.noLoras")} description={t("image.local.lora.emptyBody")} action={<div className="flex flex-wrap justify-center gap-2"><Button onClick={() => setView("browse")}>{t("image.local.lora.browse")}</Button><Button variant="secondary" onClick={importLora}>{t("image.local.lora.importFile")}</Button></div>} /> : <ListGroup>{loras.data.loras.map((lora) => <LoraRow key={lora.path} lora={lora} onOpen={() => choose(lora)} />)}</ListGroup>}
        <Text variant="hint">{t("image.local.lora.editorHint")}</Text>
      </>}
      <Sheet open={selected !== null} onClose={() => setSelected(null)} placement="auto" title={selected?.filename} dismissible={!action.isPending} footer={<div className="flex justify-end gap-2"><Button variant="ghost" onClick={() => setSelected(null)}>{common("actions.cancel")}</Button><Button disabled={!selected || keywords === selected.keywords.join("\n")} loading={action.isPending} onClick={() => { if (selected) action.mutate(async () => { await api.call("lorasUpdateKeywords", { path: selected.path, profile_id: null, keywords: keywords.split("\n").map((word) => word.trim()).filter(Boolean) }); setSelected(null); }); }}>{t("manage.save")}</Button></div>}>
        {action.isError && <FailureState error={action.error} size="sm" />}
        {selected && <div className="space-y-5">
          <Field label={t("image.local.lora.triggerWords")} hint={t("image.local.lora.keywordBody")}><Textarea rows={6} value={keywords} onChange={(event) => setKeywords(event.target.value)} aria-label={t("image.local.lora.keywords")} /></Field>
          <Button variant="secondary" leading={<Sparkles size={icon.sm} />} loading={action.isPending} onClick={() => action.mutate(async () => { const accepted = await api.call("loraKeywordsDiscover", { path: selected.path, profile_id: null, client_operation_id: crypto.randomUUID() }); setJob(accepted.job_id); setSelected(null); })}>{t("image.local.lora.discover")}</Button>
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
  const keywords = lora.keywords.length ? lora.keywords.join(", ") : t(lora.keyword_source === "none" ? "image.local.lora.noKeywords" : "image.local.lora.alwaysActive");
  return <ListRowButton icon={<Layers size={icon.lg} />} title={lora.filename} description={`${size(lora.bytes_on_disk)} · ${lora.architecture ?? t("image.local.lora.unknownBase")} · ${keywords}`} {...press.handlers} onClick={press.guardClick(onOpen)} />;
}
