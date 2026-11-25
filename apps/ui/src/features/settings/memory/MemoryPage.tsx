import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useLocation, useNavigate, useRouter, useSearch } from "@tanstack/react-router";
import { Archive, BookOpen, Brain, Cpu, FileText, History, Layers, MemoryStick, Search, SlidersHorizontal, Sparkles, User, Users } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  Badge,
  Button,
  confirm,
  icon,
  ListGroup,
  ListRow,
  ListRowButton,
  LoadingState,
  Page,
  Segmented,
  Switch,
  toast,
} from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type {
  PromptKind,
  SettingsDynamicMemorySettings,
  SettingsPatch_Serialize,
  SettingsView,
} from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { MemoryDownloadSheet, useEmbeddingInstall } from "@/entities/embedding";
import { ModelChoiceSheet } from "@/entities/model";
import { promptsQuery } from "@/entities/prompt";
import { useChatModels } from "@/features/conversations";
import { FailureState } from "@/shared/ui/FailureState";
import { SettingsControlRow } from "@/shared/ui/SettingsControlRow";
import { useSettingsApi, useSettingsLayout } from "../data";
import { CommitNumber } from "./CommitNumber";
import { capacities, defaultCapacity } from "./presets";
import { MemoryPromptSheet } from "./MemoryPromptSheet";
import { ProfileRows, ScopedSettings, type MemorySection } from "./ScopedSettings";
import { useSettingsWrite } from "../use-settings-write";

const eidos = "lettuce_eidos_v5";
const summarizerPrompt = "dynamic_memory_summarizer";
const managerPrompt = "dynamic_memory_manager";

type Scope = "direct" | "group";
type View = MemorySection;

const views = ["saved", "recall", "summary", "advanced"] as const satisfies readonly View[];
const viewIcons = { saved: Archive, recall: Search, summary: BookOpen, advanced: SlidersHorizontal } as const;

function useMemoryNavigation() {
  const navigate = useNavigate();
  const router = useRouter();
  const view = useSearch({ from: "/settings/$section", select: (search) => search.memoryView ?? null });
  const scope: Scope = useSearch({ from: "/settings/$section", select: (search) => search.memoryScope ?? "direct" });
  const pushed = useLocation({ select: (location) => location.state.memoryPushed });
  const go = (next: View | null, nextScope: Scope, replace: boolean) =>
    void navigate({
      to: "/settings/$section",
      params: { section: "memory" },
      search: { ...(next ? { memoryView: next } : {}), ...(nextScope === "group" ? { memoryScope: "group" as const } : {}) },
      state: { memoryPushed: next !== null && !replace ? true : pushed === true },
      replace,
      resetScroll: !replace,
    });
  return {
    view,
    scope,
    setScope: (next: Scope) => go(view, next, true),
    open: (next: View) => go(next, scope, false),
    close: () => {
      if (pushed) router.history.back();
      else go(null, scope, true);
    },
  };
}
type Sheet = "model" | "summaryPrompt" | "managerPrompt" | null;

export function MemoryPage() {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const { back } = useSettingsLayout();
  const navigation = useMemoryNavigation();
  const settings = useQuery({
    queryKey: queryKeys.settings.detail(),
    queryFn: () => api.call("settingsGet"),
  });
  const status = useQuery({
    queryKey: queryKeys.embedding.status(),
    queryFn: () => api.call("embeddingStatus"),
  });
  let body;
  if (settings.isPending || status.isPending) body = <LoadingState />;
  else if (settings.isError)
    body = <FailureState error={settings.error} onRetry={() => void settings.refetch()} />;
  else if (status.isError)
    body = <FailureState error={status.error} onRetry={() => void status.refetch()} />;
  else
    body = (
      <MemoryForm
        view={settings.data}
        installed={status.data.some((model) => model.family === eidos)}
        navigation={navigation}
      />
    );
  return (
    <Page
      title={navigation.view ? t(`memory.pages.${navigation.view}.title`) : t("memory.title")}
      {...(navigation.view ? { onBack: navigation.close, backLabel: t("memory.title") } : back)}
      contentClassName="max-w-6xl"
    >
      {body}
    </Page>
  );
}

function MemoryForm({ view, installed, navigation }: { view: SettingsView; installed: boolean; navigation: ReturnType<typeof useMemoryNavigation> }) {
  const { t, i18n } = useTranslation("settings");
  const { t: common } = useTranslation("common");
  const { t: characters } = useTranslation("characters");
  const api = useSettingsApi();
  const { phone } = useSettingsLayout();
  const queryClient = useQueryClient();
  const write = useSettingsWrite(api);
  const chat = useChatModels(api);
  const { scope, setScope } = navigation;
  const [sheet, setSheet] = useState<Sheet>(null);
  const [download, setDownload] = useState<"enable" | "install" | null>(null);
  const install = useEmbeddingInstall(api, false);
  const finishing = useRef(false);
  const { global } = view;
  const dynamic = global.dynamic_memory;
  const group = global.group_dynamic_memory;
  const scoped = scope === "direct" ? dynamic : group;

  const writeDynamic = (
    change: (current: SettingsDynamicMemorySettings) => SettingsDynamicMemorySettings,
  ) =>
    write((latest) => ({ section: "dynamic_memory", value: change(latest.global.dynamic_memory) }));
  const writeGroup = (
    change: (
      group: SettingsDynamicMemorySettings | null,
      dynamic: SettingsDynamicMemorySettings,
    ) => SettingsDynamicMemorySettings | null,
  ) =>
    write((latest) => ({
      section: "group_dynamic_memory",
      value: change(latest.global.group_dynamic_memory, latest.global.dynamic_memory),
    }));
  const writeGroupSettings = (
    change: (current: SettingsDynamicMemorySettings) => SettingsDynamicMemorySettings,
  ) =>
    write((latest) => {
      const current = latest.global.group_dynamic_memory;
      return current
        ? {
            section: "group_dynamic_memory",
            value: { ...change(current), enabled: latest.global.dynamic_memory.enabled },
          }
        : null;
    });
  const writeScoped = (
    change: (current: SettingsDynamicMemorySettings) => SettingsDynamicMemorySettings,
  ) => (scope === "direct" ? writeDynamic(change) : writeGroupSettings(change));
  const setEnabled = async (enabled: boolean) => {
    await writeDynamic((current) => ({ ...current, enabled }));
    await write((latest) => {
      const current = latest.global.group_dynamic_memory;
      return current && current.enabled !== enabled
        ? { section: "group_dynamic_memory", value: { ...current, enabled } }
        : null;
    });
  };

  const toggleDynamic = (next: boolean) => {
    if (!next) {
      void setEnabled(false);
      return;
    }
    if (installed) {
      void setEnabled(true);
      return;
    }
    install.reset();
    setDownload("enable");
  };

  useEffect(() => {
    if (download === null || install.state.phase !== "done" || finishing.current) return;
    finishing.current = true;
    const mode = download;
    void (async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.embedding.status() });
      if (mode === "enable") await setEnabled(true);
      setDownload(null);
      install.reset();
      finishing.current = false;
    })();
  });

  const closeDownload = () => {
    setDownload(null);
    install.reset();
  };

  const removeModel = async () => {
    const sure = await confirm({
      title: t("memory.embedding.removeTitle"),
      message: t("memory.embedding.removeBody"),
      confirmLabel: t("memory.embedding.removeConfirm"),
      cancelLabel: common("actions.cancel"),
      destructive: true,
    });
    if (!sure) return;
    try {
      await api.call("embeddingRemove", { family: eidos });
      await setEnabled(false);
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    }
    await Promise.all(
      [queryKeys.embedding.all, queryKeys.settings.all].map((queryKey) =>
        queryClient.invalidateQueries({ queryKey }),
      ),
    );
  };

  const modelId = view.dynamic_memory_model_profile_id;
  const defaultName = chat.find(view.default_model_profile_id)?.name;
  const modelsLoaded = chat.models.length > 0;
  const modelDescription = modelId
    ? (chat.find(modelId)?.name ??
      (modelsLoaded ? t("memory.summarization.modelMissing") : undefined))
    : defaultName
      ? t("memory.summarization.appDefaultNamed", { name: defaultName })
      : t("memory.summarization.appDefault");
  const prompts = global.dynamic_memory_prompts;
  const writePrompts = (patch: Partial<typeof prompts>) =>
    void write((latest) => ({
      section: "dynamic_memory_prompts",
      value: { ...latest.global.dynamic_memory_prompts, ...patch },
    }));
  const tokens = view.device.embedding_max_tokens ?? defaultCapacity;
  const capacity = new Intl.NumberFormat(i18n.language).format(tokens);
  const general = (value: number): SettingsPatch_Serialize => ({
    section: "general",
    pure_mode: null,
    analytics_enabled: null,
    update_checks_enabled: null,
    developer_mode_enabled: null,
    auto_download_character_card_avatars: null,
    message_context_window: value,
    lorebook_scan_depth: null,
  });

  const scopeBar = (
    <>
      <Segmented<Scope>
        block
        aria-label={t("memory.dynamic.title")}
        value={scope}
        onChange={setScope}
        options={[
          {
            value: "direct",
            label: t("memory.scope.direct"),
            icon: <User size={icon.sm} />,
          },
          {
            value: "group",
            label: t("memory.scope.group"),
            icon: <Users size={icon.sm} />,
          },
        ]}
      />

      {scope === "group" && (
        <ListGroup>
          <ListRow
            title={t("memory.scope.same")}
            description={t(group ? "memory.scope.sameOff" : "memory.scope.sameOn")}
            trailing={
              <Switch
                checked={group === null}
                onChange={(same) =>
                  void writeGroup((_current, latest) => (same ? null : { ...latest }))
                }
                aria-label={t("memory.scope.same")}
              />
            }
          />
        </ListGroup>
      )}
    </>
  );

  if (navigation.view) {
    return (
      <div className="@container">
        <div className="space-y-7">
          {scopeBar}
          {scoped && <ScopedSettings section={navigation.view} value={scoped} onChange={writeScoped} />}
        </div>
      </div>
    );
  }

  return (
    <>
      <div className="@container">
        <div className="grid grid-cols-1 gap-7 @4xl:grid-cols-2 @4xl:items-start @4xl:gap-x-10">
          <div className="min-w-0 space-y-7">
            <ListGroup label={t("memory.manual.group")}>
              <ListRow
                icon={<History size={icon.lg} />}
                title={t("memory.manual.window")}
                description={t("memory.manual.windowBody")}
                trailing={
                  <CommitNumber
                    value={global.message_context_window}
                    min={1}
                    max={1000}
                    onCommit={(next) => write(() => general(next))}
                  />
                }
              />
            </ListGroup>

            <ListGroup data-tour-id="dynmem-enable" label={t("memory.dynamic.group")}>
              <ListRow
                icon={<Brain size={icon.lg} />}
                title={t("memory.dynamic.title")}
                description={t("memory.dynamic.body")}
                trailing={
                  <Switch
                    checked={dynamic.enabled}
                    onChange={toggleDynamic}
                    aria-label={t("memory.dynamic.title")}
                  />
                }
              />
              {dynamic.enabled && <ProfileRows value={dynamic} onChange={writeDynamic} tour />}
            </ListGroup>

            {dynamic.enabled && (
              <>
                <ListGroup label={t("memory.scope.group")}>
                  <ListRow
                    title={t("memory.scope.same")}
                    description={t(group ? "memory.scope.sameOff" : "memory.scope.sameOn")}
                    trailing={
                      <Switch
                        checked={group === null}
                        onChange={(same) =>
                          void writeGroup((_current, latest) => (same ? null : { ...latest }))
                        }
                        aria-label={t("memory.scope.same")}
                      />
                    }
                  />
                  {group && <ProfileRows value={group} onChange={writeGroupSettings} />}
                </ListGroup>

                <ListGroup label={t("memory.pages.group")}>
                  {views.map((page) => {
                    const Icon = viewIcons[page];
                    return (
                      <ListRowButton
                        key={page}
                        icon={<Icon size={icon.lg} />}
                        title={t(`memory.pages.${page}.title`)}
                        description={t(`memory.pages.${page}.body`)}
                        onClick={() => navigation.open(page)}
                      />
                    );
                  })}
                </ListGroup>
              </>
            )}

          </div>
          {dynamic.enabled && (
            <div className="min-w-0 space-y-7">
              <ListGroup label={t("memory.summarization.group")}>
                <ListRowButton
                  icon={<Cpu size={icon.lg} />}
                  title={t("memory.summarization.model")}
                  description={modelDescription}
                  onClick={() => setSheet("model")}
                />
                <PromptRow
                  api={api}
                  kind={summarizerPrompt}
                  title={t("memory.summarization.summaryPrompt")}
                  id={prompts.summarizer_prompt_id}
                  onOpen={() => setSheet("summaryPrompt")}
                />
                <PromptRow
                  api={api}
                  kind={managerPrompt}
                  title={t("memory.summarization.managerPrompt")}
                  id={prompts.manager_prompt_id}
                  onOpen={() => setSheet("managerPrompt")}
                />
                <ListRow
                  icon={<Sparkles size={icon.lg} />}
                  title={t("memory.summarization.sampler")}
                  description={t(
                    global.dynamic_memory_llama_sampler_overwrite_enabled
                      ? "memory.summarization.samplerOn"
                      : "memory.summarization.samplerOff",
                  )}
                  trailing={
                    <Switch
                      checked={global.dynamic_memory_llama_sampler_overwrite_enabled}
                      onChange={(enabled) =>
                        void write(() => ({ section: "dynamic_memory_sampler", enabled }))
                      }
                      aria-label={t("memory.summarization.sampler")}
                    />
                  }
                />
              </ListGroup>

              <ListGroup data-tour-id="dynmem-embedding" label={t("memory.embedding.group")}>
                <ListRow
                  icon={<Layers size={icon.lg} />}
                  title={
                    <span className="flex flex-wrap items-center gap-x-2 gap-y-1">
                      {t("memory.embedding.name")}{" "}
                      {installed ? (
                        <Badge tone="accent">{t("memory.embedding.installed")}</Badge>
                      ) : (
                        <Badge>{t("memory.embedding.missing")}</Badge>
                      )}
                    </span>
                  }
                  description={
                    installed
                      ? t("memory.embedding.body", { capacity })
                      : t("memory.embedding.missingBody")
                  }
                  trailing={
                    installed ? (
                      <Button variant="ghost" size="sm" onClick={() => void removeModel()}>
                        {t("memory.embedding.remove")}
                      </Button>
                    ) : (
                      <Button
                        variant="tonal"
                        size="sm"
                        onClick={() => {
                          install.reset();
                          setDownload("install");
                        }}
                      >
                        {t("memory.embedding.download")}
                      </Button>
                    )
                  }
                />
                {installed && (
                  <ListRow
                    icon={<MemoryStick size={icon.lg} />}
                    title={t("memory.embedding.keepLoaded")}
                    description={t("memory.embedding.keepLoadedBody")}
                    trailing={
                      <Switch
                        checked={view.device.embedding_keep_model_loaded}
                        onChange={(keep) =>
                          void write(() => ({ section: "device_onnx", model: "eidos", keep_model_loaded: keep }))
                        }
                        aria-label={t("memory.embedding.keepLoadedLabel")}
                      />
                    }
                  />
                )}
                {installed && (
                  <SettingsControlRow
                    phone={phone}
                    title={t("memory.embedding.capacity")}
                    description={t("memory.embedding.capacityBody")}
                    control={
                      <Segmented<string>
                        size="sm"
                        block={phone}
                        aria-label={t("memory.embedding.capacity")}
                        value={String(tokens)}
                        onChange={(next) =>
                          void write((latest) => ({
                            section: "device_embedding",
                            model_version: latest.device.embedding_model_version,
                            max_tokens: Number(next),
                            keep_model_loaded: latest.device.embedding_keep_model_loaded,
                          }))
                        }
                        options={capacities.map((value) => ({
                          value: String(value),
                          label: new Intl.NumberFormat(i18n.language, {
                            useGrouping: false,
                          }).format(value),
                        }))}
                      />
                    }
                  />
                )}
              </ListGroup>
            </div>
          )}
        </div>
      </div>

      <ModelChoiceSheet
        api={api}
        open={sheet === "model"}
        title={characters("pickers.model")}
        value={modelId}
        requirement={{ kind: "chat" }}
        nullChoice={{ title: characters("details.modelDefault"), description: characters("pickers.modelDefaultBody") }}
        onChange={(id) =>
          void write(() => ({
            section: "selections",
            default_model: null,
            default_prompt: null,
            group_speaker_model: null,
            dynamic_memory_model: id ? { type: "set", id } : { type: "reset" },
          }))
        }
        onClose={() => setSheet(null)}
      />
      <MemoryPromptSheet
        api={api}
        kind={summarizerPrompt}
        title={t("memory.summarization.summaryPrompt")}
        open={sheet === "summaryPrompt"}
        value={prompts.summarizer_prompt_id}
        onChange={(id) => writePrompts({ summarizer_prompt_id: id })}
        onClose={() => setSheet(null)}
      />
      <MemoryPromptSheet
        api={api}
        kind={managerPrompt}
        title={t("memory.summarization.managerPrompt")}
        open={sheet === "managerPrompt"}
        value={prompts.manager_prompt_id}
        onChange={(id) => writePrompts({ manager_prompt_id: id })}
        onClose={() => setSheet(null)}
      />
      <MemoryDownloadSheet
        open={download !== null}
        onClose={closeDownload}
        install={install.state}
        onStart={() => void install.start()}
        onManual={closeDownload}
        mode={download === "install" ? "install" : "enable"}
      />
    </>
  );
}

function PromptRow({
  api,
  kind,
  title,
  id,
  onOpen,
}: {
  api: ApiClient;
  kind: PromptKind;
  title: string;
  id: string | null;
  onOpen: () => void;
}) {
  const { t } = useTranslation("settings");
  const prompts = useQuery(promptsQuery(api, kind));
  const description =
    id === null
      ? t("memory.summarization.builtIn")
      : prompts.data
        ? (prompts.data.find((prompt) => prompt.id === id)?.name ??
          t("memory.summarization.promptMissing"))
        : undefined;
  return (
    <ListRowButton
      icon={<FileText size={icon.lg} />}
      title={title}
      description={description}
      onClick={onOpen}
    />
  );
}
