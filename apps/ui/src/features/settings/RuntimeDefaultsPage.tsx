import { useState, type ReactNode } from "react";
import { useNavigate, useRouter, useLocation } from "@tanstack/react-router";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { FolderInput, FolderOpen, HardDrive, KeyRound, RotateCcw } from "lucide-react";
import {
  Badge,
  Button,
  IconButton,
  Input,
  ListGroup,
  ListRow,
  ListRowButton,
  LoadingState,
  Page,
  Select,
  Sheet,
  Switch,
  Text,
  fileSize,
  icon,
  toast,
} from "@lettuceai/crisp";
import type {
  LlamaDevice,
  ModelView,
  SettingsLlamaDefaultKvCacheType,
  SettingsModelSettingsLayer,
  SettingsView,
} from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { parseModelConfig, type ModelConfig } from "@/entities/model/model-config";
import { ModelEditorField } from "@/entities/model/ModelEditorField";
import type { FieldId } from "@/entities/model/model-editor-fields";
import type { EditorKind } from "@/entities/model/model-editor-sections";
import { EditorRow } from "@/shared/ui/EditorRow";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsAction, useSettingsApi, useSettingsLayout } from "./data";
import { runtimeFieldDefault } from "./model-editor-defaults";
import { ModelEditorColumns } from "./ModelEditorColumns";
import { FolderMoves } from "./model-browser/FolderMoves";
import { useFolderMoves } from "./model-browser/folder-move-data";
import { HfTokenSheet } from "./model-browser/HfTokenSheet";
import { hfAuthKey } from "./model-browser/data";
import { CivitaiTokenSheet } from "./image/CivitaiTokenSheet";
import { useCivitaiAuth } from "./image/local-data";

const llamaKeys = [
  "batch_size",
  "ubatch_size",
  "multi_gpu_enabled",
  "gpu_device_ids",
  "gpu_distribution_mode",
  "priority_vram_limit_bytes",
  "kv_placement",
  "main_gpu",
] as const;
const samplerKeys = ["repeat_penalty", "n_pen_range"] as const;
const kvCacheTypes = ["auto", "f16", "q8_0", "q4_0"] as const;
const distributionModes = ["balanced", "proportional", "priority"] as const;
const localKind: EditorKind = "local";
const defaultContextLength = 8192;

interface Draft {
  context: string;
  kv: SettingsLlamaDefaultKvCacheType | "";
  config: ModelConfig;
}

function draftOf(settings: SettingsView): Draft {
  return {
    context: String(settings.global.llama_default_context_length ?? ""),
    kv: settings.global.llama_default_kv_cache_type ?? "",
    config: parseModelConfig({
      chat_parameters: {},
      feature_parameters: {},
      llama_cpp: {
        ...Object.fromEntries(llamaKeys.map((key) => [key, settings.sampler_defaults.llama_cpp[key] ?? undefined])),
        sampler: Object.fromEntries(samplerKeys.map((key) => [key, settings.sampler_defaults.llama_cpp.sampler[key] ?? undefined])),
      },
      stable_diffusion: {},
    }),
  };
}

function layerFrom(base: SettingsModelSettingsLayer, config: ModelConfig): SettingsModelSettingsLayer {
  const layer = structuredClone(base);
  const llama = layer.llama_cpp as Record<string, unknown>;
  const sampler = layer.llama_cpp.sampler as Record<string, unknown>;
  const ownLlama = config.llama_cpp as Record<string, unknown>;
  const ownSampler = (config.llama_cpp.sampler ?? {}) as Record<string, unknown>;
  for (const key of llamaKeys) llama[key] = ownLlama[key] ?? null;
  for (const key of samplerKeys) sampler[key] = ownSampler[key] ?? null;
  return layer;
}

function same(a: Draft, b: Draft): boolean {
  return a.context === b.context && a.kv === b.kv && JSON.stringify(a.config.llama_cpp) === JSON.stringify(b.config.llama_cpp);
}

export function RuntimeDefaultsPage() {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const navigate = useNavigate();
  const router = useRouter();
  const pushed = useLocation({ select: (location) => location.state.runtimePushed });
  const { phone } = useSettingsLayout();
  const settings = useQuery({ queryKey: queryKeys.settings.detail(), queryFn: () => api.call("settingsGet") });
  const back = () => {
    if (pushed) router.history.back();
    else void navigate({ to: "/settings/$section", params: { section: "models" } });
  };
  return (
    <Page
      title={t("models.runtime")}
      onBack={back}
      backLabel={t("sections.models.title")}
      padding="none"
      contentClassName={`pt-1 ${phone ? "px-4 pb-[calc(6rem+var(--safe-bottom,0px))]" : "px-8 pb-10"}`}
    >
      {settings.isPending ? (
        <LoadingState />
      ) : settings.isError ? (
        <FailureState error={settings.error} onRetry={() => void settings.refetch()} />
      ) : (
        <RuntimeDefaults key={settings.data.revision} settings={settings.data} />
      )}
    </Page>
  );
}

function RuntimeDefaults({ settings }: { settings: SettingsView }) {
  const { t, i18n } = useTranslation("settings");
  const api = useSettingsApi();
  const client = useQueryClient();
  const action = useSettingsAction();
  const devices = useQuery({ queryKey: ["llama", "devices"], queryFn: () => api.call("llamaDevices") });
  const [baseline] = useState(() => draftOf(settings));
  const [draft, setDraft] = useState(baseline);
  const [invalid, setInvalid] = useState<Record<string, boolean>>({});
  const [reconfigure, setReconfigure] = useState<{ kind: "multi" | "single"; models: ModelView[]; gpu: LlamaDevice | undefined } | null>(null);
  const discrete = devices.data?.devices.filter((device) => device.device_type !== "IntegratedGpu") ?? [];
  const multiGpuAvailable = discrete.length >= 2;
  const llama = draft.config.llama_cpp;
  const multiGpu = multiGpuAvailable && llama.multi_gpu_enabled === true;
  const selected = llama.gpu_device_ids ?? [];
  const contextInvalid = draft.context !== "" && (!Number.isSafeInteger(Number(draft.context)) || Number(draft.context) < 1);
  const blocked = contextInvalid || Object.values(invalid).some(Boolean);
  const dirty = !same(draft, baseline);
  const setConfig = (config: ModelConfig) => setDraft((current) => ({ ...current, config }));
  const setLlama = (key: string, value: unknown) =>
    setConfig({ ...draft.config, llama_cpp: { ...draft.config.llama_cpp, [key]: value ?? undefined } });
  const field = (id: FieldId, options?: { value: string; label: string }[]) => (
    <ModelEditorField
      key={id}
      id={id}
      kind={localKind}
      layer="chat"
      config={draft.config}
      defaults={undefined}
      runtimeDefault={runtimeFieldDefault(localKind, id, draft.config, undefined, "chat")}
      multiGpuAvailable={multiGpuAvailable}
      options={options}
      onChange={setConfig}
      onFile={() => undefined}
      onInvalid={(key, value) => setInvalid((state) => ({ ...state, [key]: value }))}
    />
  );
  const save = () =>
    action.mutate(async () => {
      let view = settings;
      if (draft.context !== baseline.context || draft.kv !== baseline.kv)
        view = await api.call("settingsUpdate", {
          expected_revision: view.revision,
          patch: {
            section: "local_runtime",
            context_length: draft.context === "" ? null : Number(draft.context),
            kv_cache_type: draft.kv || null,
          },
        });
      if (JSON.stringify(draft.config.llama_cpp) !== JSON.stringify(baseline.config.llama_cpp))
        view = await api.call("settingsSamplerDefaultsUpdate", {
          expected_revision: view.revision,
          value: layerFrom(view.sampler_defaults, draft.config),
        });
      const multiBefore = baseline.config.llama_cpp.multi_gpu_enabled === true;
      const multiAfter = draft.config.llama_cpp.multi_gpu_enabled === true;
      if (multiBefore !== multiAfter && multiGpuAvailable) {
        const [models, accounts] = await Promise.all([api.call("modelsList"), api.call("providerAccountsList")]);
        const local = new Set(accounts.filter((account) => account.provider_kind === "llamacpp").map((account) => account.id));
        const own = models.models.filter((model) => model.kind === "chat" && local.has(model.provider_account_id));
        const pinned = draft.config.llama_cpp.main_gpu ?? selected[0] ?? discrete[0]?.index;
        if (own.length > 0)
          setReconfigure({ kind: multiAfter ? "multi" : "single", models: own, gpu: devices.data?.devices.find((device) => device.index === pinned) });
      }
      client.setQueryData(queryKeys.settings.detail(), view);
      await client.invalidateQueries({ queryKey: queryKeys.settings.all });
      toast.success(t("runtime.saved"));
    });
  const runReconfigure = () =>
    action.mutate(async () => {
      if (!reconfigure) return;
      for (const model of reconfigure.models) {
        const config = parseModelConfig(model.config);
        delete config.llama_cpp.gpu_layers;
        delete config.llama_cpp.multi_gpu_enabled;
        delete config.llama_cpp.single_gpu_device_id;
        if (reconfigure.kind === "single" && reconfigure.gpu) config.llama_cpp.single_gpu_device_id = reconfigure.gpu.index;
        await api.call("modelSave", {
          model: {
            id: model.id,
            provider_account_id: model.provider_account_id,
            external_model_id: model.external_model_id,
            display_name: model.display_name,
            kind: model.kind,
            config,
            input_scopes: model.input_scopes,
            output_scopes: model.output_scopes,
            remote_metadata: null,
          },
          expected_revision: model.revision,
          client_operation_id: crypto.randomUUID(),
        });
      }
      await client.invalidateQueries({ queryKey: queryKeys.models.all });
      toast.success(t("runtime.reconfigured", { count: reconfigure.models.length }));
      setReconfigure(null);
    });
  const deviceLabel = (device: LlamaDevice) => device.description || device.name || `GPU ${device.index}`;
  const selectedDevices = discrete.filter((device) => selected.includes(device.index));
  return (
    <div className="space-y-6">
      {action.isError && <FailureState error={action.error} size="sm" />}
      <ModelEditorColumns
        primary={
          <>
            <StorageSection />
            <TokensSection />
          </>
        }
        secondary={
          <>
            <Section label={t("runtime.llama")} hint={t("runtime.llamaHint")}>
              <EditorRow label={t("tools.context")} hint={t("tools.defaultHint")}>
                <Input
                  aria-label={t("tools.context")}
                  type="number"
                  min={1}
                  step={1}
                  value={draft.context}
                  placeholder={String(defaultContextLength)}
                  onChange={(event) => setDraft((current) => ({ ...current, context: event.target.value }))}
                />
              </EditorRow>
              <EditorRow label={t("tools.kv")} hint={t("runtime.kvHint")}>
                <Select
                  aria-label={t("tools.kv")}
                  value={draft.kv}
                  options={[
                    { value: "", label: t("tools.inherit") },
                    ...kvCacheTypes.map((value) => ({ value, label: value })),
                  ]}
                  onChange={(event) => setDraft((current) => ({ ...current, kv: event.target.value as Draft["kv"] }))}
                />
              </EditorRow>
              {field("llamaBatchSize")}
              {field("llamaUbatchSize")}
              {field("llamaRepeatPenalty")}
              {field("llamaNPenRange")}
            </Section>
            <ListGroup label={t("modelEditor.groups.multipleGpus")}>
              <EditorRow
                label={t("modelEditor.fields.llamaMultiGpuEnabled")}
                hint={multiGpuAvailable ? t("runtime.multiGpuHint") : t("runtime.multiGpuNeedsTwo")}
                inline
              >
                <Switch
                  aria-label={t("modelEditor.fields.llamaMultiGpuEnabled")}
                  disabled={!multiGpuAvailable}
                  checked={multiGpu}
                  onChange={(checked) => setLlama("multi_gpu_enabled", checked ? true : undefined)}
                />
              </EditorRow>
              {devices.isError && <FailureState error={devices.error} onRetry={() => void devices.refetch()} size="sm" />}
              {multiGpu && (
                <>
                  {discrete.map((device) => (
                    <EditorRow
                      key={device.index}
                      label={deviceLabel(device)}
                      hint={`${fileSize(device.memory_free, i18n.language)} / ${fileSize(device.memory_total, i18n.language)} · ${device.backend}`}
                      inline
                    >
                      <Switch
                        aria-label={deviceLabel(device)}
                        checked={selected.includes(device.index)}
                        onChange={(checked) => {
                          const next = checked
                            ? [...selected.filter((id) => id !== device.index), device.index].sort((a, b) => a - b)
                            : selected.filter((id) => id !== device.index);
                          setLlama("gpu_device_ids", next.length > 0 ? next : undefined);
                        }}
                      />
                    </EditorRow>
                  ))}
                  {selected.length === 1 && (
                    <Text variant="hint" className="px-4 pb-3 text-warning">
                      {t("runtime.pickTwo")}
                    </Text>
                  )}
                  {field(
                    "llamaGpuDistributionMode",
                    distributionModes.map((value) => ({ value, label: t(`modelEditor.options.llamaGpuDistributionMode.${value}`) })),
                  )}
                  {llama.gpu_distribution_mode === "priority" && field("llamaPriorityVramLimitBytes")}
                  {field("llamaKvPlacement")}
                  {llama.kv_placement === "pin" &&
                    selectedDevices.length > 0 &&
                    field(
                      "llamaMainGpu",
                      selectedDevices.map((device) => ({ value: String(device.index), label: deviceLabel(device) })),
                    )}
                  <Text variant="hint" className="px-4 pb-3">
                    {t("runtime.manualPerModel")}
                  </Text>
                </>
              )}
            </ListGroup>
          </>
        }
      />
      <div className="flex justify-end">
        <Button variant="primary" disabled={!dirty || blocked} loading={action.isPending && reconfigure === null} onClick={save}>
          {t("manage.save")}
        </Button>
      </div>
      <Sheet
        open={reconfigure !== null}
        dismissible={!action.isPending}
        onClose={() => setReconfigure(null)}
        title={t("runtime.reconfigureTitle")}
        description={
          reconfigure?.kind === "single"
            ? t("runtime.reconfigureSingle", { gpu: reconfigure.gpu ? deviceLabel(reconfigure.gpu) : "GPU" })
            : t("runtime.reconfigureMulti")
        }
        footer={
          <div className="flex items-center justify-end gap-2">
            <Button variant="ghost" disabled={action.isPending} onClick={() => setReconfigure(null)}>
              {t("runtime.reconfigureSkip")}
            </Button>
            <Button variant="primary" loading={action.isPending} onClick={runReconfigure}>
              {t("runtime.reconfigureConfirm", { count: reconfigure?.models.length ?? 0 })}
            </Button>
          </div>
        }
      >
        {reconfigure && (
          <ListGroup>
            {reconfigure.models.map((model) => (
              <ListRow key={model.id} title={model.display_name} description={model.external_model_id.split(/[\\/]/).pop()} />
            ))}
          </ListGroup>
        )}
      </Sheet>
    </div>
  );
}

function StorageSection() {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const client = useQueryClient();
  const action = useSettingsAction();
  const folderMoves = useFolderMoves();
  const dir = useQuery({
    queryKey: [...queryKeys.localModels.all, "directory"],
    queryFn: () => api.call("localModelsDirGet"),
  });
  const [pending, setPending] = useState<string | null>(null);
  const busy = dir.isPending || dir.isError || action.isPending || folderMoves.active;
  const apply = (path: string, move: boolean) =>
    action.mutate(async () => {
      const accepted = await api.call("localModelsDirSet", {
        path,
        move_existing: move,
        client_operation_id: crypto.randomUUID(),
      });
      folderMoves.admit(accepted.job_id);
      setPending(null);
      await client.invalidateQueries({ queryKey: queryKeys.localModels.all });
    });
  const choose = (path: string) => {
    if (!dir.data || path === dir.data.path) return;
    if (dir.data.model_count > 0) setPending(path);
    else apply(path, false);
  };
  const pick = () =>
    action.mutate(async () => {
      const chosen = await api.call("filesPickDirectory");
      if (chosen.path) choose(chosen.path);
    });
  return (
    <>
      <Section label={t("runtime.storage")} hint={t("runtime.storageHint")}>
        {dir.isError ? (
          <FailureState error={dir.error} onRetry={() => void dir.refetch()} size="sm" />
        ) : (
          <ListRow
            icon={<HardDrive size={icon.lg} />}
            tone="info"
            title={
              <span className="flex items-center gap-2">
                {t("hf.folder")}
                {dir.data && <Badge>{t(dir.data.is_custom ? "runtime.folderCustom" : "runtime.folderDefault")}</Badge>}
              </span>
            }
            description={<span className="block truncate font-mono text-xs" title={dir.data?.path}>{dir.data?.path}</span>}
            trailing={
              <span className="flex items-center gap-1">
                {dir.data?.is_custom && (
                  <IconButton label={t("runtime.folderReset")} title={t("runtime.folderReset")} variant="ghost" disabled={busy} onClick={() => choose(dir.data!.default_path)}>
                    <RotateCcw size={icon.sm} />
                  </IconButton>
                )}
                <Button size="sm" variant="secondary" leading={<FolderOpen size={icon.sm} />} disabled={busy} onClick={pick}>
                  {t("runtime.folderChange")}
                </Button>
              </span>
            }
          />
        )}
      </Section>
      <FolderMoves state={folderMoves} />
      {action.isError && <FailureState error={action.error} size="sm" />}
      <Sheet
        open={pending !== null}
        dismissible={!action.isPending}
        onClose={() => setPending(null)}
        title={t("runtime.moveTitle")}
        description={dir.data ? t("runtime.moveCount", { count: dir.data.model_count }) : undefined}
      >
        <ListGroup>
          <ListRowButton
            icon={<FolderInput size={icon.lg} />}
            tone="accent"
            title={t("runtime.moveConfirm")}
            description={t("runtime.moveConfirmHint")}
            chevron={false}
            disabled={action.isPending}
            onClick={() => pending && apply(pending, true)}
          />
          <ListRowButton
            icon={<FolderOpen size={icon.lg} />}
            title={t("runtime.moveKeep")}
            description={t("runtime.moveKeepHint")}
            chevron={false}
            disabled={action.isPending}
            onClick={() => pending && apply(pending, false)}
          />
        </ListGroup>
      </Sheet>
    </>
  );
}

function TokensSection() {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const hf = useQuery({ queryKey: hfAuthKey, queryFn: () => api.call("hfAuthStatus") });
  const civitai = useCivitaiAuth();
  const [open, setOpen] = useState<"hf" | "civitai" | null>(null);
  const hfStatus = hf.data
    ? hf.data.type === "valid"
      ? t("hf.tokenValid", { name: hf.data.username })
      : hf.data.type === "invalid"
        ? t("hf.tokenRefused")
        : hf.data.type === "unknown"
          ? t("hf.tokenUnknown")
          : t("runtime.tokenMissing")
    : undefined;
  const civitaiStatus = civitai.data
    ? civitai.data.valid
      ? t("runtime.tokenSaved")
      : civitai.data.saved
        ? t(civitai.data.error_kind === "unverified" ? "image.local.lora.tokenUnverified" : "image.local.lora.tokenInvalid")
        : t("image.local.lora.tokenMissing")
    : undefined;
  const hfSaved = hf.data !== undefined && hf.data.type !== "missing";
  return (
    <>
      <Section label={t("runtime.huggingFace")} hint={t("hf.tokenSheetHint")}>
        <ListRowButton
          icon={<KeyRound size={icon.lg} />}
          tone="info"
          title={t("hf.token")}
          description={hf.isError ? t("hf.tokenUnknown") : hfStatus}
          trailing={<Text variant="hint">{t(hfSaved ? "image.local.lora.tokenReplace" : "image.local.lora.tokenAdd")}</Text>}
          onClick={() => setOpen("hf")}
        />
      </Section>
      <Section label={t("runtime.civitai")} hint={t("image.local.lora.tokenBody")}>
        <ListRowButton
          icon={<KeyRound size={icon.lg} />}
          tone="info"
          title={t("image.local.lora.token")}
          description={civitai.isError ? t("hf.tokenUnknown") : civitaiStatus}
          trailing={<Text variant="hint">{t(civitai.data?.saved ? "image.local.lora.tokenReplace" : "image.local.lora.tokenAdd")}</Text>}
          onClick={() => setOpen("civitai")}
        />
      </Section>
      <HfTokenSheet open={open === "hf"} onClose={() => setOpen(null)} auth={hf} />
      <CivitaiTokenSheet open={open === "civitai"} onClose={() => setOpen(null)} />
    </>
  );
}

function Section({ label, hint, children }: { label: string; hint: string; children: ReactNode }) {
  return (
    <section className="space-y-2">
      <div className="space-y-0.5 px-1">
        <Text variant="section">{label}</Text>
        <Text variant="hint">{hint}</Text>
      </div>
      <ListGroup>{children}</ListGroup>
    </section>
  );
}
