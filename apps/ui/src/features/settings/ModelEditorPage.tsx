import { AnimatePresence, motion } from "framer-motion";
import { Fragment, useRef, useState } from "react";
import { useNavigate, useParams } from "@tanstack/react-router";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { ChevronDown, Copy, Download, Ellipsis, LoaderCircle, RotateCcw, Star, Trash2 } from "lucide-react";
import { AppBar, Button, confirm, IconButton, Input, ListGroup, ListRowButton, LoadingState, Menu, MenuContent, MenuItem, MenuSeparator, MenuTrigger, Switch, Text, icon, inputSurface, motion as m, useCompactLayout } from "@lettuceai/crisp";
import type { ModelModality, ModelView, ProviderAccountView, ProviderDescriptorContract, RemoteModelContract, SettingsModelSettingsLayer } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { defaultModelName, ProviderMark, ProviderSelect, providerGroup } from "@/entities/provider";
import { minimalModelConfig } from "@/entities/model";
import { SelectMenuField } from "@/shared/ui/SelectMenuField";
import { EditorRow } from "@/shared/ui/EditorRow";
import { FailureState } from "@/shared/ui/FailureState";
import { useProviderData, useSettingsAction, useSettingsApi } from "./data";
import { changedCount, effectiveField, featureKeys, parseModelConfig, resetFeature, serializeModelConfig, writeField, type Layer, type ModelConfig } from "@/entities/model/model-config";
import { useLlamaContext } from "./model-editor-runtime-data";
import { runtimeFieldDefault } from "./model-editor-defaults";
import { editorFields, type FieldId } from "@/entities/model/model-editor-fields";
import { editorSections, type EditorKind } from "@/entities/model/model-editor-sections";
import { ModelEditorField } from "@/entities/model/ModelEditorField";
import { ModelEditorReveal } from "@/entities/model/ModelEditorReveal";
import { ModelComponentPicker, ModelFilePicker, type PickedModel } from "./ModelFilePicker";
import { LlamaRuntimeInfo, ModelLoras, OpenRouterEndpoints } from "./ModelEditorRuntime";
import { ModelTemplate } from "./ModelTemplate";
import { deleteModelWithUsage, exportModel } from "./model-actions";

export function ModelEditorPage() {
  const params = useParams({ strict: false });
  const modelId = "modelId" in params ? params.modelId : undefined;
  const api = useSettingsApi();
  const { accounts, catalog } = useProviderData();
  const model = useQuery({ queryKey: [...queryKeys.models.all, "detail", modelId], queryFn: () => api.call("modelGet", { model_id: modelId! }), enabled: Boolean(modelId) });
  const settings = useQuery({ queryKey: queryKeys.settings.detail(), queryFn: () => api.call("settingsGet") });
  if (accounts.isPending || (modelId && model.isPending)) return <LoadingState />;
  if (accounts.isError) return <FailureState error={accounts.error} onRetry={() => void accounts.refetch()} />;
  if (modelId && model.isError) return <FailureState error={model.error} onRetry={() => void model.refetch()} />;
  return <ModelEditor key={modelId ?? "new"} model={model.data ?? null} accounts={accounts.data} catalog={catalog.data ?? []} defaults={settings.data?.sampler_defaults} defaultsError={settings.isError ? settings.error : undefined} retryDefaults={() => void settings.refetch()} />;
}

interface Draft { accountId: string; externalId: string; name: string; inputs: ModelModality[]; outputs: ModelModality[]; config: ModelConfig; remote: RemoteModelContract | null }
function initialDraft(model: ModelView | null, accounts: ProviderAccountView[]): Draft {
  return { accountId: model?.provider_account_id ?? accounts[0]?.id ?? "", externalId: model?.external_model_id ?? "", name: model?.display_name ?? "", inputs: model?.input_scopes ?? ["text"], outputs: model?.output_scopes ?? ["text"], config: parseModelConfig(model?.config ?? minimalModelConfig), remote: null };
}

function ModelEditor({ model, accounts, catalog, defaults, defaultsError, retryDefaults }: { model: ModelView | null; accounts: ProviderAccountView[]; catalog: ProviderDescriptorContract[]; defaults: SettingsModelSettingsLayer | undefined; defaultsError: unknown; retryDefaults: () => void }) {
  const { t, i18n } = useTranslation("settings");
  const api = useSettingsApi();
  const navigate = useNavigate();
  const client = useQueryClient();
  const phone = useCompactLayout();
  const scrollRef = useRef<HTMLDivElement>(null);
  const formRef = useRef<HTMLFormElement>(null);
  const action = useSettingsAction();
  const [baseline, setBaseline] = useState(() => initialDraft(model, accounts));
  const [draft, setDraft] = useState(baseline);
  const [savedModel, setSavedModel] = useState(model);
  const [section, setSection] = useState("overview");
  const [layer, setLayer] = useState<Layer>("chat");
  const [picker, setPicker] = useState(false);
  const [componentField, setComponentField] = useState<"text_encoder_path" | "vae_path" | "vision_encoder_path" | null>(null);
  const [resetVersion, setResetVersion] = useState(0);
  const [invalid, setInvalid] = useState<Record<string, boolean>>({});
  const [fileError, setFileError] = useState<unknown>(null);
  const [picking, setPicking] = useState(false);
  const [pickingField, setPickingField] = useState<FieldId | null>(null);
  const account = accounts.find((entry) => entry.id === draft.accountId);
  const descriptor = catalog.find((entry) => entry.kind === account?.provider_kind);
  const kind: EditorKind = account?.provider_kind === "llamacpp" ? "local" : account?.provider_kind === "sdcpp" || (savedModel?.kind === "image" && savedModel.provider_account_id === draft.accountId) ? "image" : account?.provider_kind === "ollama" ? "ollama" : "cloud";
  const localFile = kind === "local" || (kind === "image" && account?.provider_kind === "sdcpp");
  const externalLabel = localFile ? draft.externalId.split(/[\\/]/).at(-1) ?? draft.externalId : draft.externalId;
  const headerMeta = localFile ? account?.label : [account?.label, draft.externalId].filter(Boolean).join(" · ");
  const sections = editorSections(kind, account?.provider_kind === "openrouter").filter((item) => {
    if (kind === "cloud" && descriptor && item.id === "reasoning") return descriptor.reasoning !== "none" && draft.config.capabilities.reasoning !== "unsupported";
    if (kind === "cloud" && descriptor && item.id === "caching") return descriptor.prompt_caching !== "none" && draft.config.capabilities.prompt_cache !== "unsupported";
    if (kind === "image" && item.id === "loras") return draft.config.stable_diffusion.cpp?.supports_lora !== false;
    return true;
  });
  const contextInfo = useLlamaContext(draft.externalId, draft.config, defaults, kind === "local");
  const gpuUnavailable = kind === "local" && contextInfo.data?.supports_gpu_offload === false;
  const devices = useQuery({ queryKey: ["llama", "devices"], queryFn: () => api.call("llamaDevices"), enabled: kind === "local" && section === "gpu" });
  const imageOptions = useQuery({ queryKey: ["image", "capabilities", draft.accountId, draft.externalId], queryFn: () => api.call("imageCapabilities", { target: { type: "provider", provider_kind: account!.provider_kind, model: draft.externalId || null } }), enabled: kind === "image" && Boolean(account) });
  const upscalers = useQuery({ queryKey: ["sd", "upscalers"], queryFn: () => api.call("sdUpscalersList"), enabled: kind === "image" && section === "hires" });
  const imageProfiles = useQuery({ queryKey: ["image", "profiles"], queryFn: () => api.call("hfImageBundleProfiles"), enabled: kind === "image" });
  const imageProfile = imageProfiles.data?.profiles.find((profile) => profile.id === draft.config.stable_diffusion.cpp?.profile_id);
  const current = sections.find((item) => item.id === section);
  const activeLayer = current?.perFeature ? layer : "chat";
  const dirty = JSON.stringify(draft) !== JSON.stringify(baseline) || Object.values(invalid).some(Boolean);
  const update = (patch: Partial<Draft>) => setDraft((value) => ({ ...value, ...patch }));
  const setConfig = (config: ModelConfig) => update({ config });
  const back = () => { void navigate({ to: "/settings/$section", params: { section: "models" } }); };
  const leave = async () => {
    if (!dirty || await confirm({ title: t("modelEditor.leaveTitle"), message: t("modelEditor.leaveBody"), confirmLabel: t("modelEditor.discard"), cancelLabel: t("manage.cancel") })) back();
  };
  const save = () => {
    if (!formRef.current?.reportValidity()) return;
    action.mutate(async () => {
      const saved = await api.call("modelSave", {
        model: { id: savedModel?.id ?? null, provider_account_id: draft.accountId, external_model_id: draft.externalId.trim(), display_name: draft.name.trim() || defaultModelName(null, draft.externalId), kind: kind === "image" ? "image" : savedModel?.kind === "embedding" || savedModel?.kind === "speech" ? savedModel.kind : "chat", config: serializeModelConfig(draft.config), input_scopes: draft.inputs, output_scopes: draft.outputs, remote_metadata: draft.remote },
        expected_revision: savedModel?.revision ?? null, client_operation_id: crypto.randomUUID(),
      });
      setSavedModel(saved);
      const next = initialDraft(saved, accounts);
      setDraft(next); setBaseline(next); setInvalid({});
      client.setQueryData([...queryKeys.models.all, "detail", saved.id], saved);
      if (!model) await navigate({ to: "/settings/models/$modelId", params: { modelId: saved.id }, replace: true });
    });
  };
  const pickModel = async (picked: PickedModel) => {
    setPicking(true); setFileError(null);
    let config = structuredClone(draft.config);
    let inputs = draft.inputs; let outputs = draft.outputs;
    try {
      if (kind === "image") {
        const detected = await api.call("sdDetectModelFile", { source: { uri: picked.id } });
        if (!detected.exists) throw new Error(t("modelEditor.pickError"));
        const profile = detected.profile;
        const installed = picked.installed;
        config.stable_diffusion.cpp = {
          ...config.stable_diffusion.cpp,
          ...(profile ? { profile_id: profile.id, supports_text_to_image: profile.supports_text_to_image, supports_image_edit: profile.supports_image_edit, max_reference_images: profile.max_reference_images, requires_reference_image: profile.requires_reference_image, recommended_for_scenes: profile.recommended_for_scenes } : {}),
          ...(installed ? { profile_id: installed.profile_id, variant_id: installed.variant_id, runtime_release: installed.runtime_release, runtime_asset: installed.runtime_asset, runtime_backend: installed.runtime_backend, ...Object.fromEntries(installed.components.filter((component) => component.role !== "diffusion_model").map((component) => [`${component.role}_path`, component.path])) } : {}),
        };
        inputs = profile?.supports_image_edit ? ["text", "image"] : ["text"]; outputs = ["image"];
      } else if (picked.remote) {
        inputs = (picked.remote.input_modalities ?? ["text"]).filter((v): v is ModelModality => ["text", "image", "audio"].includes(v));
        outputs = (picked.remote.output_modalities ?? ["text"]).filter((v): v is ModelModality => ["text", "image", "audio"].includes(v));
      }
      update({ externalId: picked.id, name: draft.name || defaultModelName(picked.name, picked.id), remote: picked.remote ?? null, config, inputs, outputs });
    } catch (error) { setFileError(error); }
    finally { setPicking(false); }
  };
  const pickFieldFile = async (id: FieldId) => {
    if (id === "textEncoder" || id === "vae" || id === "visionEncoder") { setComponentField(editorFields[id].key); return; }
    setFileError(null); setPickingField(id);
    try {
      const files = await api.call("filesPickOpen", { kinds: ["any"], multiple: false });
      const source = files.sources[0];
      if (source) {
        if (source.uri.includes("://")) throw new Error(t("modelEditor.pickError"));
        const field = editorFields[id];
        setConfig(writeField(draft.config, "chat", field.store, field.key, source.uri));
      }
    } catch (error) { setFileError(error); }
    finally { setPickingField(null); }
  };
  const fieldOptions = (id: FieldId) => {
    if ((id === "llamaMainGpu" || id === "llamaSingleGpuDeviceId") && devices.data) return devices.data.devices.map((device) => ({ value: String(device.index), label: device.name }));
    const values = id === "sdSize" ? imageOptions.data?.sizes : id === "sdSampler" ? imageOptions.data?.samplers : id === "sdScheduler" ? imageOptions.data?.schedulers : id === "sdHiresUpscaler" ? upscalers.data?.hires_upscaler_names : undefined;
    return values?.length ? values.map((value) => ({ value, label: id === "sdSize" ? value.replace("x", "×") : value })) : undefined;
  };
  const runtimeDefault = (id: FieldId) => id === "llamaOffloadKqv" && contextInfo.data?.supports_gpu_offload === false ? false : runtimeFieldDefault(kind, id, draft.config, defaults, activeLayer, imageProfile) ?? (id === "sdSize" ? imageOptions.data?.default_size ?? undefined : undefined);
  const field = (id: FieldId) => <ModelEditorField loading={pickingField === id} kind={kind} runtimeDefault={runtimeDefault(id)} disabled={(pickingField !== null && editorFields[id].type === "file") || gpuUnavailable && ["llamaCpuLayers", "llamaMultiGpuEnabled", "llamaGpuDistributionMode", "llamaMainGpu", "llamaPriorityVramLimitBytes", "llamaSingleGpuDeviceId", "llamaOffloadKqv"].includes(id)} disabledOptions={gpuUnavailable ? id === "llamaKvPlacement" ? ["split", "pin"] : id === "llamaMtpPlacement" ? ["gpu"] : [] : []} options={fieldOptions(id)} key={`${resetVersion}.${activeLayer}.${id}`} id={id} config={draft.config} defaults={defaults} layer={activeLayer} onChange={setConfig} onFile={(key) => void pickFieldFile(key)} onInvalid={(key, value) => setInvalid((state) => ({ ...state, [key]: value }))} />;
  const supports = (id: FieldId) => {
    if (id === "llamaKvTypeK" || id === "llamaKvTypeV") return effectiveField(draft.config, defaults, "chat", editorFields.llamaKvType) === "split";
    if (kind === "image") {
      if (id === "sdNegativePrompt" && imageOptions.data?.negative_prompt === false) return false;
      if (["sdImageCfgScale", "sdDenoisingStrength", "sdAutoResizeRefImages", "sdIncreaseRefIndex"].includes(id) && draft.config.stable_diffusion.cpp?.supports_image_edit === false) return false;
    }
    if (kind !== "cloud" || !descriptor) return true;
    const key = editorFields[id].key;
    const caps = draft.config.capabilities.parameter_support as Record<string, string>;
    if (caps[key] === "unsupported") return false;
    const parameters = descriptor.parameters as unknown as Record<string, boolean>;
    return !(key in parameters) || parameters[key] !== false;
  };
  const overview = <div className="space-y-6">
    {accounts.length === 0 && <Text>{t("models.noProviders")}</Text>}
    <ListGroup>
      <EditorRow label={t("models.provider")}><ProviderSelect value={draft.accountId || null} compact={phone} aria-label={t("models.provider")} providers={accounts.map((entry) => {
        const entryDescriptor = catalog.find((item) => item.kind === entry.provider_kind);
        const group = ["llamacpp", "sdcpp"].includes(entry.provider_kind) ? "device" : entryDescriptor && providerGroup(entryDescriptor) === "cloud" ? "cloud" : "servers";
        return { id: entry.id, name: entry.label, kind: entry.provider_kind, group: t(`modelEditor.${group}`) };
      })} onChange={(id) => { if (id && id !== draft.accountId) { update({ accountId: id, externalId: "", remote: null }); setSection("overview"); } }} /></EditorRow>
      <EditorRow label={t(kind === "local" || kind === "image" ? "modelEditor.modelFile" : "modelEditor.model")} hint={externalLabel}>
        <button type="button" className={`${inputSurface} flex h-10 w-full items-center gap-2 px-3.5 text-left text-sm`} aria-busy={picking || undefined} disabled={!draft.accountId || picking} onClick={() => setPicker(true)}><span className="min-w-0 flex-1 truncate">{draft.externalId ? localFile ? externalLabel : draft.remote?.display_name ?? defaultModelName(null, draft.externalId) : t("modelEditor.chooseModel")}</span>{picking ? <LoaderCircle size={icon.sm} className="shrink-0 animate-spin text-fg-3" /> : <ChevronDown size={icon.sm} className="shrink-0 text-fg-3" />}</button>
      </EditorRow>
      <EditorRow label={t("modelEditor.name")} hint={t("modelEditor.nameHint")}><Input aria-label={t("modelEditor.name")} value={draft.name} placeholder={draft.externalId ? defaultModelName(null, draft.externalId) : t("modelEditor.name")} onChange={(event) => update({ name: event.target.value })} /></EditorRow>
    </ListGroup>
    {kind === "image" ? <ListGroup label={t("modelEditor.components")}><EditorRow label={t("modelEditor.componentProfile")} hint={t("modelEditor.detectedFile")}><SelectMenuField aria-label={t("modelEditor.componentProfile")} value={draft.config.stable_diffusion.cpp?.profile_id ?? ""} options={[{ value: "", label: t("modelEditor.customProfile") }, ...(imageProfiles.data?.profiles ?? []).map((profile) => ({ value: profile.id, label: profile.display_name })), ...(!imageProfile && draft.config.stable_diffusion.cpp?.profile_id ? [{ value: draft.config.stable_diffusion.cpp.profile_id, label: draft.config.stable_diffusion.cpp.profile_id }] : [])]} onChange={(id) => {
      const profile = imageProfiles.data?.profiles.find((entry) => entry.id === id);
      const cpp = { ...draft.config.stable_diffusion.cpp, profile_id: id || null };
      if (profile) Object.assign(cpp, { supports_text_to_image: profile.supports_text_to_image, supports_image_edit: profile.supports_image_edit, max_reference_images: profile.max_reference_images, requires_reference_image: profile.requires_reference_image, recommended_for_scenes: profile.recommended_for_scenes });
      setConfig({ ...draft.config, stable_diffusion: { ...draft.config.stable_diffusion, cpp } });
    }} /></EditorRow>{(["textEncoder", "vae", "visionEncoder"] as const).map((id) => {
      const spec = editorFields[id];
      const path = effectiveField(draft.config, defaults, "chat", spec);
      const role = spec.key.replace("_path", "");
      const required = !path && imageProfile?.required_roles.some((value) => value === role);
      return <EditorRow key={id} label={t(`modelEditor.fields.${id}`)} hint={typeof path === "string" ? <span className="font-mono">{path.split(/[\\/]/).at(-1)}</span> : required ? <span className="text-warning">{t("modelEditor.requiredComponent")}</span> : id === "visionEncoder" ? t("modelEditor.visionEncoderHint") : undefined}><Button variant={required ? "primary" : "secondary"} className="w-full" onClick={() => void pickFieldFile(id)}>{t(path ? "modelEditor.changeComponent" : "modelEditor.chooseComponent")}</Button></EditorRow>;
    })}<div className="px-4 py-3 text-xs text-fg-3">{[draft.config.stable_diffusion.cpp?.supports_text_to_image ? t("modelEditor.drawsText") : null, draft.config.stable_diffusion.cpp?.supports_image_edit ? draft.config.stable_diffusion.cpp.max_reference_images ? t("modelEditor.editsImages", { count: draft.config.stable_diffusion.cpp.max_reference_images }) : t("modelEditor.editsImagesUnknown") : null, draft.config.stable_diffusion.cpp?.supports_lora ? t("modelEditor.takesLoras") : null].filter(Boolean).join(" · ")}</div></ListGroup> : ["inputs", "outputs"].map((direction) => <ListGroup key={direction} label={t(`modelEditor.${direction}` as never)}>
      {(["text", "image", "audio"] as const).map((scope) => {
        const key = direction === "inputs" ? "inputs" : "outputs";
        return <EditorRow key={scope} label={t(`modelEditor.${scope}`)} hint={scope === "text" ? undefined : t(`modelEditor.scopeHints.${direction}.${scope}` as never)} inline><Switch aria-label={t("modelEditor.scopeLabel", { direction: t(`modelEditor.${key}`), scope: t(`modelEditor.${scope}`) })} checked={draft[key].includes(scope)} disabled={draft.config.capabilities[key === "inputs" ? "input_modalities" : "output_modalities"][scope] === "unsupported"} onChange={(checked) => update({ [key]: checked ? [...draft[key], scope] : draft[key].filter((value) => value !== scope), remote: null })} /></EditorRow>;
      })}
    </ListGroup>)}
    {kind === "local" && draft.externalId && <ListGroup label={t("modelEditor.onDevice")}><div className="px-4 py-3"><LlamaRuntimeInfo path={draft.externalId} config={draft.config} defaults={defaults} onChange={setConfig} /></div></ListGroup>}
  </div>;
  const featurePicker = <div className="flex flex-wrap items-center gap-3 rounded-xl border border-line bg-surface-1 px-4 py-3">
    <div className="min-w-0 flex-1 basis-48"><p className="text-sm font-medium">{t("modelEditor.settingsFor")}</p><p className="mt-0.5 text-xs text-fg-3">{t(layer === "chat" ? "modelEditor.chatHint" : "modelEditor.featureHint")}</p></div>
    <Menu>
      <MenuTrigger>
        <button type="button" aria-label={t("modelEditor.settingsFor")} className={`${inputSurface} flex h-10 w-full items-center gap-2 px-3.5 text-left text-sm @[36rem]:w-64`}>
          <span className="min-w-0 flex-1 truncate text-fg">{t(layer === "chat" ? "modelEditor.chat" : `modelEditor.features.${layer}`)}</span>
          {layer !== "chat" && changedCount(draft.config, layer) > 0 && <span className="shrink-0 text-xs text-accent">{t("modelEditor.changed", { count: changedCount(draft.config, layer) })}</span>}
          <ChevronDown size={icon.sm} className="shrink-0 text-fg-3" />
        </button>
      </MenuTrigger>
      <MenuContent align="end" className="w-72">
        <MenuItem selected={layer === "chat"} onSelect={() => setLayer("chat")}><span className="block">{t("modelEditor.chat")}</span><span className="block text-xs text-fg-3">{t("modelEditor.mainSettings")}</span></MenuItem>
        <MenuSeparator />
        {featureKeys.map((feature) => <MenuItem key={feature} selected={layer === feature} onSelect={() => setLayer(feature)}>
          <span className="block">{t(`modelEditor.features.${feature}`)}</span>
          <span className={`block text-xs ${changedCount(draft.config, feature) ? "text-accent" : "text-fg-3"}`}>{changedCount(draft.config, feature) ? t("modelEditor.changed", { count: changedCount(draft.config, feature) }) : t("modelEditor.sameAsChat")}</span>
        </MenuItem>)}
      </MenuContent>
    </Menu>
    {layer !== "chat" && changedCount(draft.config, layer) > 0 && <Button variant="ghost" size="sm" leading={<RotateCcw size={icon.sm} />} onClick={() => { setConfig(resetFeature(draft.config, layer)); setResetVersion((value) => value + 1); setInvalid((values) => Object.fromEntries(Object.entries(values).filter(([key]) => !key.startsWith(`${layer}.`)))); }}>{t("modelEditor.reset")}</Button>}
  </div>;
  const body = current ? <div className="space-y-6">
    {current.perFeature && featurePicker}
    {section === "gpu" && (gpuUnavailable ? <Text variant="hint">{t("modelEditor.gpuUnavailable")}</Text> : <LlamaRuntimeInfo path={draft.externalId} config={draft.config} defaults={defaults} onChange={setConfig} gpu splitOnly />)}
    {current.groups.map((group, index) => {
      if (section === "memory" && index === 0) return <LlamaRuntimeInfo key={index} path={draft.externalId} config={draft.config} defaults={defaults} onChange={setConfig} contextEditor />;
      const enabled = group.toggle ? effectiveField(draft.config, defaults, activeLayer, editorFields[group.toggle]) ?? runtimeDefault(group.toggle) : true;
      return <ListGroup key={index} label={group.label ? t(`modelEditor.groups.${group.label}` as never) : undefined}>
        {group.toggle && field(group.toggle)}
        <ModelEditorReveal open={!group.toggle || enabled === true || enabled === "enabled"}>
          {section === "gpu" && group.toggle && <LlamaRuntimeInfo path={draft.externalId} config={draft.config} defaults={defaults} onChange={setConfig} gpu devicesOnly />}
          {group.fields.filter(supports).map((id) => <Fragment key={id}>{id === "llamaGpuLayers" ? <LlamaRuntimeInfo path={draft.externalId} config={draft.config} defaults={defaults} onChange={setConfig} gpu layersOnly /> : id === "llamaChatTemplateOverride" ? <ModelTemplate path={draft.externalId} config={draft.config} defaults={defaults} onChange={setConfig} /> : field(id)}</Fragment>)}
        </ModelEditorReveal>
      </ListGroup>;
    })}
    {section === "routing" && <OpenRouterEndpoints externalId={draft.externalId} config={draft.config} defaults={defaults} onChange={setConfig} />}
    {section === "loras" && <ListGroup><div className="px-4 py-3"><ModelLoras config={draft.config} onChange={setConfig} /></div></ListGroup>}
  </div> : overview;
  const nav = [{ id: "overview", summary: draft.name || t("modelEditor.overviewSummary") }, ...sections.map((item) => {
    const valueOf = (id: FieldId) => effectiveField(draft.config, defaults, "chat", editorFields[id]) ?? runtimeFieldDefault(kind, id, draft.config, defaults, "chat", imageProfile);
    if (item.id === "routing") return { id: item.id, summary: draft.config.chat_parameters.openrouter?.pinned_provider || defaults?.chat_parameters.openrouter?.pinned_provider || t("modelEditor.automaticRouting") };
    if (item.id === "loras") return { id: item.id, summary: t("modelEditor.lorasCount", { count: draft.config.stable_diffusion.base_loras?.length ?? 0 }) };
    if (item.id === "prompt") return { id: item.id, summary: t(valueOf("sdNegativePrompt") ? "modelEditor.negativeSet" : "modelEditor.noNegative") };
    const summaryFields: Record<string, FieldId[]> = {
      generation: kind === "image" ? ["sdSize", "sdSteps"] : kind === "local" ? ["temperature", "llamaSamplerProfile"] : ["temperature", "maxOutputTokens"],
      memory: ["contextLength", "llamaKvType"], gpu: ["llamaGpuLayers"], performance: ["llamaThreads", "llamaBatchSize"],
      speculative: ["llamaMtpEnabled", "llamaDflashEnabled"], template: ["llamaChatTemplatePreset"], speed: ["sdCacheMode", "sdVaeTilingEnabled"],
    };
    const first = summaryFields[item.id] ?? item.groups.flatMap((group) => [...(group.toggle ? [group.toggle] : []), ...group.fields]).slice(0, 2);
    const summary = first.flatMap((id) => {
      const value = valueOf(id);
      if (id === "contextLength" && value == null && contextInfo.data?.recommended_context_length) return t("modelEditor.contextSummary", { count: contextInfo.data.recommended_context_length });
      if (id === "llamaThreads" && value == null) return t("modelEditor.autoThreads");
      if (id === "llamaChatTemplatePreset" && !value) return t("modelEditor.templateFromModel");
      if (value == null) return [];
      if (id === "maxOutputTokens" && typeof value === "number") return t("modelEditor.tokenValue", { value: new Intl.NumberFormat(i18n.language).format(value) });
      if (id === "promptCachingEnabled") return typeof value === "object" && "enabled" in value ? t(`modelEditor.options.promptCachingEnabled.${(value as { enabled: { retention: string } }).enabled.retention}` as never) : t("modelEditor.off");
      const option = `modelEditor.options.${id}.${String(value)}`;
      const display = i18n.exists(option, { ns: "settings" }) ? t(option as never) : value === "enabled" || value === "disabled" ? t(value === "enabled" ? "modelEditor.on" : "modelEditor.off") : typeof value === "boolean" ? t(value ? "modelEditor.on" : "modelEditor.off") : String(value);
      return `${t(`modelEditor.fields.${id}`)} ${display}`;
    }).join(" · ");
    return { id: item.id, summary: summary || t("modelEditor.automatic") };
  })];
  const navLabel = (id: string) => id === "overview" ? t("modelEditor.overview") : t(`modelEditor.sections.${id}` as never);
  const title = model ? draft.name || t("modelEditor.model") : t("models.new");
  const more = savedModel && <Menu><MenuTrigger><IconButton label={t("modelEditor.more")} disabled={action.isPending}><Ellipsis size={icon.md} /></IconButton></MenuTrigger><MenuContent align="end">
    <MenuItem icon={<Star size={icon.sm} />} onSelect={() => action.mutate(async () => { const list = await client.fetchQuery({ queryKey: queryKeys.models.list(), queryFn: () => api.call("modelsList") }); await api.call("modelDefaultSet", { model_id: savedModel.id, expected_revision: list.revision, client_operation_id: crypto.randomUUID() }); })}>{t("models.setDefault")}</MenuItem>
    <MenuItem icon={<Copy size={icon.sm} />} onSelect={() => action.mutate(async () => { await api.call("modelDuplicate", { model_id: savedModel.id, display_name: t("models.copyName", { name: savedModel.display_name }), expected_revision: savedModel.revision, client_operation_id: crypto.randomUUID() }); })}>{t("manage.duplicate")}</MenuItem>
    <MenuItem icon={<Download size={icon.sm} />} onSelect={() => action.mutate(() => exportModel(api, savedModel))}>{t("manage.export")}</MenuItem>
    <MenuSeparator /><MenuItem icon={<Trash2 size={icon.sm} />} danger onSelect={() => action.mutate(async () => { if (await deleteModelWithUsage(api, savedModel, t)) back(); })}>{t("manage.delete")}</MenuItem>
  </MenuContent></Menu>;
  return <div className="relative flex h-full min-h-0 flex-col overflow-hidden">
    <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto">
      <AppBar title={phone && section !== "overview" ? navLabel(section) : <span className="flex items-center gap-3">{account && <ProviderMark kind={account.provider_kind} name={account.label} />}{title}</span>} meta={section === "overview" || !phone ? headerMeta : undefined} scrollRef={scrollRef} onBack={phone && section !== "overview" ? () => setSection("overview") : () => void leave()} backLabel={phone && section !== "overview" ? title : t("sections.models.title")} actions={phone && section !== "overview" ? undefined : more || undefined} />
      <form ref={formRef} onSubmit={(event) => { event.preventDefault(); save(); }} className={phone ? "px-4 pb-[calc(7rem+var(--safe-bottom,0px))]" : "flex gap-8 px-8 pb-28"}>
        {!phone && <nav aria-label={t("modelEditor.nav")} className="sticky top-4 w-52 shrink-0 self-start"><ul className="space-y-0.5">{nav.map((item) => <li key={item.id}><button type="button" disabled={item.id !== "overview" && !draft.externalId} aria-current={section === item.id ? "page" : undefined} onClick={() => setSection(item.id)} className={`w-full rounded-lg px-3 py-2 text-left disabled:opacity-40 ${section === item.id ? "bg-fill-2 text-fg" : "text-fg-2 hover:bg-fill"}`}><span className="block text-sm font-medium">{navLabel(item.id)}</span><span className="block truncate text-xs text-fg-3">{item.summary}</span></button></li>)}</ul></nav>}
        <div className="@container min-w-0 max-w-2xl flex-1 space-y-4">
          {action.isError && <FailureState error={action.error} />}
          {defaultsError !== undefined && <FailureState error={defaultsError} onRetry={retryDefaults} size="sm" />}
          {fileError !== null && <FailureState error={fileError} size="sm" />}
          {imageOptions.isError && <FailureState error={imageOptions.error} onRetry={() => void imageOptions.refetch()} size="sm" />}
          {section === "hires" && upscalers.isError && <FailureState error={upscalers.error} onRetry={() => void upscalers.refetch()} size="sm" />}
          <AnimatePresence mode="wait" initial={false}>
            <motion.div key={section} initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} transition={m.quick} className="space-y-4">
          {body}
          {phone && section === "overview" && draft.externalId && <ListGroup label={t("modelEditor.settings")}>{nav.filter((item) => item.id !== "overview").map((item) => <ListRowButton key={item.id} title={navLabel(item.id)} description={item.summary} onClick={() => setSection(item.id)} />)}</ListGroup>}
            </motion.div>
          </AnimatePresence>
        </div>
      </form>
    </div>
    <AnimatePresence initial={false}>{(!savedModel || dirty) && <motion.div initial={{ y: 24, opacity: 0 }} animate={{ y: 0, opacity: 1 }} exit={{ y: 24, opacity: 0 }} transition={m.quick} className={`absolute inset-x-0 bottom-0 z-10 border-t border-line bg-transparent px-4 py-3 backdrop-blur ${phone ? "pb-[calc(0.75rem+var(--safe-bottom,0px))]" : "px-8"}`}>
      <div className="flex items-center gap-2">
        <span className={phone ? "sr-only" : "mr-auto text-sm text-fg-3"} role="status">{t("modelEditor.unsaved")}</span>
        <Button variant="ghost" className={phone ? "flex-1" : undefined} disabled={!dirty || action.isPending} onClick={() => { setDraft(structuredClone(baseline)); setInvalid({}); setResetVersion((value) => value + 1); }}>{t("modelEditor.discard")}</Button>
        <Button variant="primary" className={phone ? "flex-1" : undefined} loading={action.isPending} disabled={!draft.accountId || !draft.externalId.trim() || picking || pickingField !== null || Object.values(invalid).some(Boolean) || (Boolean(savedModel) && !dirty)} onClick={save}>{t(savedModel ? "manage.save" : "modelEditor.add")}</Button>
      </div>
      {Object.values(invalid).some(Boolean) && <p className="text-xs text-danger" role="alert">{t("modelEditor.invalid")}</p>}
    </motion.div>}</AnimatePresence>
    <ModelComponentPicker field={componentField} onClose={() => setComponentField(null)} onPick={(path) => { if (componentField) setConfig(writeField(draft.config, "chat", "cpp", componentField, path)); }} />
    <ModelFilePicker key={`${draft.accountId}-${kind}`} open={picker} providerKind={account?.provider_kind ?? null} providerName={account?.label ?? ""} selectedId={draft.externalId || null} accountId={draft.accountId} kind={kind} onClose={() => setPicker(false)} onPick={(picked) => void pickModel(picked)} />
  </div>;
}
