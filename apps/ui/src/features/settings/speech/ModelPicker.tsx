import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { AudioLines, Trash2 } from "lucide-react";
import { Badge, Button, confirm, EmptyState, fileSize, icon, IconButton, ListGroup, ListRow, SearchField, Segmented, Sheet, Skeleton, Text } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import { queryKeys } from "@/api/query-keys";
import type { WhisperInstalledModel } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { modelName, useSpeechAction } from "./data";

export function ModelPicker({ api, open, phone, models, onClose, onDownload, pending }: { api: ApiClient; open: boolean; phone: boolean; models: WhisperInstalledModel[]; onClose: () => void; onDownload: (id: string) => void; pending: ReadonlySet<string> }) {
  const { t } = useTranslation("settings", { keyPrefix: "speech" });
  const catalog = useQuery({ queryKey: [...queryKeys.speech.all, "catalog"], queryFn: () => api.call("whisperCatalog"), enabled: open });
  const action = useSpeechAction();
  const [show, setShow] = useState<"device" | "all" | "downloaded">("device");
  const [query, setQuery] = useState("");
  const downloaded = new Set(models.map((model) => model.id));
  const q = query.trim().toLowerCase();
  const catalogIds = new Set(catalog.data?.models.map((model) => model.id));
  const available = catalog.data ? [...catalog.data.models, ...models.filter((model) => !catalogIds.has(model.id)).map((model) => ({ ...model, installed: true, recommended: false, recommended_for_mobile: false, recommended_for_desktop: false }))] : undefined;
  const list = available?.filter((model) => (show === "all" || (show === "device" ? phone ? model.recommended_for_mobile : model.recommended_for_desktop : downloaded.has(model.id))) && (!q || `${model.id} ${model.filename}`.toLowerCase().includes(q)));
  const choose = (id: string) => action.mutate(() => api.call("whisperDictationModelSet", { model_id: id }), { onSuccess: onClose });
  const remove = async (id: string) => { if (await confirm({ title: t("deleteTitle", { name: modelName(id) }), message: t("deleteModelBody"), confirmLabel: t("delete"), cancelLabel: t("cancel"), destructive: true })) action.mutate(() => api.call("whisperDelete", { model_id: id })); };
  return <Sheet open={open} onClose={onClose} title={t("whisperModel")} description={t("modelPickerBody")} placement={phone ? "bottom" : "side"} size="lg" {...(phone ? { detents: [0.7, 0.95] } : {})}>
    <div className="space-y-4"><SearchField value={query} onChange={setQuery} placeholder={t("searchModels")} /><Segmented block size="sm" value={show} onChange={setShow} options={[{ value: "device", label: t(phone ? "forPhone" : "forDesktop") }, { value: "all", label: t("all") }, { value: "downloaded", label: t("downloaded") }]} />
      {catalog.isPending && <div className="space-y-3" role="status" aria-label={t("loading")}>{[0, 1, 2].map((index) => <Skeleton key={index} className="h-24 w-full" />)}</div>}
      {catalog.isError && <FailureState error={catalog.error} onRetry={() => void catalog.refetch()} />}
      {action.isError && <FailureState error={action.error} size="sm" />}
      {list?.length ? <ListGroup>{list.map((model) => <ListRow key={model.id} icon={<AudioLines size={icon.lg} />} tone={downloaded.has(model.id) ? "accent" : "neutral"} title={<span className="flex flex-wrap items-center gap-1.5">{modelName(model.id)}{model.quantized && <Badge>{t("quantized")}</Badge>}</span>} description={<span>{t("metadata", { left: t(model.english_only ? "englishOnly" : "multilingual"), right: fileSize(model.size_bytes) })}{!phone && <span className="block font-mono text-xs text-fg-3">{model.filename}</span>}</span>} trailing={models.some((item) => item.id === model.id && item.dictation) ? <span className="flex items-center gap-1"><Badge tone="accent" dot>{t("inUse")}</Badge><IconButton label={`${t("delete")} ${modelName(model.id)}`} size="sm" disabled={action.isPending} onClick={() => void remove(model.id)}><Trash2 size={icon.sm} /></IconButton></span> : pending.has(model.id) ? <Badge>{t("downloading")}</Badge> : downloaded.has(model.id) ? <span className="flex items-center gap-1"><IconButton label={`${t("delete")} ${modelName(model.id)}`} size="sm" disabled={action.isPending} onClick={() => void remove(model.id)}><Trash2 size={icon.sm} /></IconButton><Button size="sm" variant="tonal" disabled={action.isPending} onClick={() => choose(model.id)}>{t("use")}</Button></span> : <Button size="sm" variant="ghost" disabled={action.isPending || pending.has(model.id)} onClick={() => onDownload(model.id)}>{t("get")}</Button>} />)}</ListGroup> : catalog.isSuccess && <EmptyState icon={<AudioLines size={icon.xl} />} title={t("noModelsMatch")} description={t("noModelsMatchBody")} />}
      <Text variant="hint">{t("modelHint")}</Text>
    </div>
  </Sheet>;
}
