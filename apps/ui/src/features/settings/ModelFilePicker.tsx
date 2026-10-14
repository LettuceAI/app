import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { FileUp } from "lucide-react";
import { Button, ListGroup, ListRowButton, LoadingState, SearchField, Sheet, Text, fileSize, icon, useCompactLayout } from "@lettuceai/crisp";
import type { RemoteModelContract, SdInstalledModel } from "@/api/generated/bindings";
import { ModelPickerSheet } from "@/entities/model/ModelPickerSheet";
import { ModelFilterSheet } from "@/entities/model/ModelFilterSheet";
import { activeFilterCount, filterModels, noFilters, vendorNames, type ModelFilters } from "@/entities/model/remote-models";
import { queryKeys } from "@/api/query-keys";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "./data";
import type { EditorKind } from "@/entities/model/model-editor-sections";

export interface PickedModel { id: string; name: string | null; remote?: RemoteModelContract; installed?: SdInstalledModel }
export function ModelFilePicker({ open, accountId, kind, providerKind, providerName, selectedId, onClose, onPick }: {
  open: boolean; accountId: string; kind: EditorKind; providerKind: string | null; providerName: string; selectedId: string | null; onClose: () => void; onPick: (model: PickedModel) => void;
}) {
  const { t, i18n } = useTranslation("settings");
  const api = useSettingsApi();
  const phone = useCompactLayout();
  const [search, setSearch] = useState("");
  const [onlyFree, setOnlyFree] = useState(false);
  const [grid, setGrid] = useState(false);
  const [filters, setFilters] = useState<ModelFilters>(noFilters);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [filtersKey, setFiltersKey] = useState(0);
  const [error, setError] = useState<unknown>(null);
  const [picking, setPicking] = useState(false);
  const remoteKind = kind === "cloud" || kind === "ollama";
  const remote = useQuery({ queryKey: queryKeys.providers.models(accountId), queryFn: () => api.call("providerModels", { account_id: accountId }), enabled: remoteKind && Boolean(accountId) });
  const local = useQuery({ queryKey: queryKeys.localModels.all, queryFn: () => api.call("localModelsList"), enabled: open && kind === "local" });
  const images = useQuery({ queryKey: ["sd", "installed"], queryFn: () => api.call("sdModelsInstalled"), enabled: open && kind === "image" });
  const result = remoteKind ? remote : kind === "local" ? local : images;
  const choose = (value: PickedModel) => { onPick(value); onClose(); };
  const entries = remoteKind ? (remote.data ?? []).map((entry) => ({ id: entry.id, name: entry.display_name, remote: entry, detail: entry.id }))
    : kind === "local" ? (local.data?.files ?? []).filter((entry) => !entry.is_mmproj && !entry.is_mtp && !entry.is_dflash).map((entry) => ({ id: entry.path, name: entry.filename, detail: `${entry.quantization} · ${fileSize(entry.size, i18n.language)}` }))
    : (images.data?.models ?? []).map((entry) => ({ id: entry.model_path, name: entry.display_name, installed: entry, detail: fileSize(entry.component_bytes_on_disk, i18n.language) }));
  const shown = entries.filter((entry) => `${entry.name} ${entry.id}`.toLocaleLowerCase().includes(search.trim().toLocaleLowerCase()));
  const pickFile = async () => {
    setPicking(true); setError(null);
    try {
      const picked = await api.call("filesPickOpen", { kinds: ["any"], multiple: false });
      const source = picked.sources[0];
      if (!source) return;
      if (source.uri.includes("://")) throw new Error(t("modelEditor.pickError"));
      choose({ id: source.uri, name: source.uri.split(/[\\/]/).at(-1) ?? null });
    } catch (reason) { setError(reason); }
    finally { setPicking(false); }
  };
  if (remoteKind) {
    const models = remote.data ?? [];
    return <>
      <ModelPickerSheet open={open} onClose={onClose} phone={phone} routeName={providerName} providerKind={providerKind}
        loading={remote.isPending && Boolean(accountId)} error={remote.error} onRetry={() => void remote.refetch()}
        total={models.length} shown={filterModels(models, { text: search, onlyFree, filters })} query={search} onQuery={setSearch}
        showFree={providerKind === "openrouter"} onlyFree={onlyFree} onOnlyFree={setOnlyFree} grid={grid} onGrid={setGrid}
        activeFilters={activeFilterCount(filters)} onOpenFilters={() => { setFiltersKey((key) => key + 1); setFiltersOpen(true); }}
        selectedId={selectedId} onChoose={(model) => { choose({ id: model.id, name: model.display_name, remote: model }); setSearch(""); }}
        onUseId={(id) => { choose({ id, name: null }); setSearch(""); }} />
      <ModelFilterSheet key={filtersKey} open={filtersOpen} onClose={() => setFiltersOpen(false)} applied={filters} vendors={vendorNames(models)}
        countFor={(draft) => filterModels(models, { text: search, onlyFree, filters: draft }).length}
        onApply={(next) => { setFilters(next); setFiltersOpen(false); }} />
    </>;
  }
  return <Sheet open={open} onClose={onClose} placement={phone ? "bottom" : "center"} size="md" title={t(remoteKind ? "modelEditor.chooseModel" : "modelEditor.chooseFile")}>
    <div className="space-y-3">
      <SearchField value={search} onChange={setSearch} aria-label={t("modelEditor.search")} placeholder={t("modelEditor.search")} />
      {error !== null && <FailureState error={error} size="sm" />}
      {result.isFetching && <LoadingState />}
      {result.isError && <FailureState error={result.error} onRetry={() => void result.refetch()} />}
      <ListGroup>{shown.map((entry) => <ListRowButton key={entry.id} title={entry.name ?? entry.id} description={entry.detail} onClick={() => choose(entry)} />)}</ListGroup>
      {result.isSuccess && shown.length === 0 && <Text variant="hint">{t(remoteKind ? "models.noResults" : "modelEditor.emptyLibrary")}</Text>}
      <Button variant="ghost" size="sm" loading={picking} leading={<FileUp size={icon.sm} />} onClick={() => void pickFile()}>{t("modelEditor.browseFile")}</Button>
    </div>
  </Sheet>;
}

export function ModelComponentPicker({ field, onClose, onPick }: { field: "text_encoder_path" | "vae_path" | "vision_encoder_path" | null; onClose: () => void; onPick: (path: string) => void }) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const { shown } = useClosingValue(field);
  const [error, setError] = useState<unknown>(null);
  const library = useQuery({ queryKey: ["sd", "components"], queryFn: () => api.call("sdComponentLibrary"), enabled: field !== null });
  const select = (path: string) => { onPick(path); onClose(); };
  const browse = async () => {
    try {
      const files = await api.call("filesPickOpen", { kinds: ["any"], multiple: false });
      const source = files.sources[0];
      if (source) {
        if (source.uri.includes("://")) throw new Error(t("modelEditor.pickError"));
        select(source.uri);
      }
    } catch (reason) { setError(reason); }
  };
  return <Sheet open={field !== null} onClose={onClose} title={t("modelEditor.chooseFile")}>
    <div className="space-y-3">
      {error !== null && <FailureState error={error} size="sm" />}
      {library.isFetching && <LoadingState />}
      {library.isError && <FailureState error={library.error} onRetry={() => void library.refetch()} />}
      <ListGroup>{library.data?.files.filter((file) => file.role === null || `${file.role}_path` === shown).map((file) => <ListRowButton key={file.path} title={file.filename} description={file.path} onClick={() => select(file.path)} />)}</ListGroup>
      <Button variant="secondary" onClick={() => void browse()}>{t("modelEditor.browseFile")}</Button>
    </div>
  </Sheet>;
}
