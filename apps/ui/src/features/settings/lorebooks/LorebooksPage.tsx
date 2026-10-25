import { useMemo, useState } from "react";
import { useQueries, useQuery } from "@tanstack/react-query";
import { useLocation, useNavigate, useRouter, useSearch } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import {
  Check,
  FileText,
  Globe,
  Hash,
  ListOrdered,
  PenLine,
  ScanSearch,
  Sparkles,
  Wand2,
} from "lucide-react";
import {
  Button,
  CrispStringsProvider,
  EmptyState,
  icon,
  ListGroup,
  ListRow,
  ListRowButton,
  NumberInput,
  Page,
  SearchField,
  Sheet,
  Skeleton,
  Slider,
  Tab,
  TabList,
  Tabs,
  Text,
} from "@lettuceai/crisp";
import type { PromptKind, SettingsLorebookGeneratorSelection } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { ModelMark } from "@/entities/provider";
import { promptsQuery } from "@/features/characters";
import { generatorDefaultsQuery } from "@/entities/lorebook";
import { FailureState } from "@/shared/ui/FailureState";
import { SliderValue } from "@/shared/ui/SliderValue";
import { SettingsControlRow } from "@/shared/ui/SettingsControlRow";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { useSettingsApi, useSettingsLayout } from "../data";
import { useLorebookSettings } from "./use-lorebook-settings";

const promptKeys = ["planner", "writer", "refine", "coherence", "entry", "keyword"] as const;
const purposes: Record<(typeof promptKeys)[number], PromptKind> = {
  planner: "lorebook_generator_planner",
  writer: "lorebook_generator_writer",
  refine: "lorebook_generator_refine",
  coherence: "lorebook_generator_coherence",
  entry: "lorebook_entry_writer",
  keyword: "lorebook_keyword_generator",
};
const promptFields = {
  planner: "planner_prompt_id",
  writer: "writer_prompt_id",
  refine: "refine_prompt_id",
  coherence: "coherence_prompt_id",
} as const;
export function LorebooksPage() {
  const { t, i18n } = useTranslation("lorebook-create");
  const api = useSettingsApi();
  const { phone, back } = useSettingsLayout();
  const state = useLorebookSettings();
  const settings = state.view?.global;
  const search = useSearch({ from: "/settings/$section" });
  const tab = search.loreTab ?? "scanning";
  const picker = search.lorePicker;
  const overlay = useMemo(() => (picker ? { picker, tab } : null), [picker, tab]);
  const shown = useClosingValue(overlay).shown;
  const pickerTab = shown?.tab ?? tab;
  const navigate = useNavigate();
  const router = useRouter();
  const pushed = useLocation({ select: (location) => location.state.lorebookPanelPushed });
  const routeKey = useLocation({ select: (location) => location.state.__TSR_key });
  const [q, setQuery] = useState("");
  const defaults = useQuery(generatorDefaultsQuery(api));
  const models = useQuery({
    queryKey: queryKeys.models.list(),
    queryFn: () => api.call("modelsList"),
  });
  const accounts = useQuery({
    queryKey: queryKeys.providers.accounts(),
    queryFn: () => api.call("providerAccountsList"),
  });
  const prompts = useQueries({
    queries: promptKeys.map((key) => promptsQuery(api, purposes[key])),
  });
  const close = () => {
    if (!picker || router.state.location.state.__TSR_key !== routeKey) return;
    if (pushed) router.history.back();
    else
      void navigate({
        to: "/settings/$section",
        params: { section: "lorebooks" },
        search: { loreTab: tab },
        replace: true,
        resetScroll: false,
      });
  };
  const open = (lorePicker: string) => {
    setQuery("");
    void navigate({
      to: "/settings/$section",
      params: { section: "lorebooks" },
      search: { loreTab: tab, lorePicker },
      state: { lorebookPanelPushed: true },
      resetScroll: false,
    });
  };
  const promptKey = promptKeys.find((key) => key === shown?.picker);
  const promptQuery = promptKey ? prompts[promptKeys.indexOf(promptKey)] : undefined;
  const modelPicker = shown?.picker === "model";
  const modelId =
    tab === "entry"
      ? settings?.lorebook_entry_generator.model_profile_id
      : settings?.lorebook_generator.selection.model_profile_id;
  const selected = promptKey
    ? promptKey === "entry"
      ? settings?.lorebook_entry_generator.entry_prompt_id
      : promptKey === "keyword"
        ? settings?.lorebook_entry_generator.keyword_prompt_id
        : settings?.lorebook_generator.selection[promptFields[promptKey]]
    : pickerTab === "entry"
      ? settings?.lorebook_entry_generator.model_profile_id
      : settings?.lorebook_generator.selection.model_profile_id;
  const choose = async (id: string | null) => {
    let success = false;
    if (modelPicker)
      success = await state.update(
        pickerTab === "entry"
          ? { type: "entry", patch: { model_profile_id: id } }
          : { type: "selection", patch: { model_profile_id: id } },
      );
    else if (promptKey) {
      if (promptKey === "entry" || promptKey === "keyword")
        success = await state.update({
          type: "entry",
          patch: { [promptKey === "entry" ? "entry_prompt_id" : "keyword_prompt_id"]: id },
        });
      else
        success = await state.update({
          type: "selection",
          patch: { [promptFields[promptKey]]: id } as Partial<SettingsLorebookGeneratorSelection>,
        });
    }
    if (success) close();
  };
  const choices = (models.data?.models ?? [])
    .filter((model) => model.output_scopes.includes("text") && model.input_scopes.includes("text"))
    .flatMap((model) => {
      const account = accounts.data?.find((account) => account.id === model.provider_account_id);
      return account ? [{ model, account }] : [];
    });
  const selectedModel = choices.find(({ model }) => model.id === modelId);
  const modelRow = (
    <ListRowButton
      icon={
        selectedModel ? (
          <ModelMark
            name={selectedModel.model.display_name}
            modelId={selectedModel.model.external_model_id}
            providerKind={selectedModel.account.provider_kind}
          />
        ) : (
          <Globe size={icon.lg} />
        )
      }
      title={t("model")}
      description={
        models.isPending || accounts.isPending ? (
          <Skeleton className="h-4 w-32" />
        ) : selectedModel ? (
          `${selectedModel.account.label} · ${selectedModel.model.display_name}`
        ) : modelId ? (
          t("missingModel")
        ) : (
          t("settings.useDefault")
        )
      }
      onClick={() => open("model")}
    />
  );
  const promptRow = (key: (typeof promptKeys)[number]) => {
    const query = prompts[promptKeys.indexOf(key)];
    const id =
      key === "entry"
        ? settings?.lorebook_entry_generator.entry_prompt_id
        : key === "keyword"
          ? settings?.lorebook_entry_generator.keyword_prompt_id
          : settings?.lorebook_generator.selection[promptFields[key]];
    const selected = query?.data?.find((prompt) => prompt.id === id);
    return (
      <ListRowButton
        key={key}
        icon={
          key === "planner" ? (
            <ListOrdered size={icon.lg} />
          ) : key === "writer" ? (
            <PenLine size={icon.lg} />
          ) : key === "refine" ? (
            <Wand2 size={icon.lg} />
          ) : key === "coherence" ? (
            <Sparkles size={icon.lg} />
          ) : (
            <FileText size={icon.lg} />
          )
        }
        title={t(`settings.promptNames.${key}`)}
        description={`${id ? (selected?.name ?? t("settings.missingPrompt")) : t("settings.builtin")} · ${t(`settings.promptBodies.${key}`)}`}
        onClick={() => open(key)}
      />
    );
  };
  const format =
    settings &&
    (tab === "entry"
      ? settings.lorebook_entry_generator.structured_fallback_format
      : settings.lorebook_generator.structured_fallback_format);
  const formats = (
    <section className="space-y-2">
      <ListGroup label={t("settings.formatGroup")}>
        {(["json", "xml"] as const).map((value) => (
          <ListRowButton
            key={value}
            title={t(`settings.formats.${value}`)}
            description={t(`settings.formatBodies.${value}`)}
            chevron={false}
            trailing={
              format === value ? <Check size={icon.md} className="text-accent" /> : undefined
            }
            onClick={() =>
              void state.update(
                tab === "entry"
                  ? { type: "entry", patch: { structured_fallback_format: value } }
                  : { type: "generator", patch: { structured_fallback_format: value } },
              )
            }
          />
        ))}
      </ListGroup>
      <Text variant="hint">{t("settings.formatHint")}</Text>
    </section>
  );
  const filtered = choices.filter(
    ({ model, account }) =>
      !q ||
      `${model.display_name} ${account.label}`.toLocaleLowerCase().includes(q.toLocaleLowerCase()),
  );
  const custom = (promptQuery?.data ?? []).filter(
    (prompt) =>
      prompt.origin.type !== "built_in" &&
      (!q || prompt.name.toLocaleLowerCase().includes(q.toLocaleLowerCase())),
  );
  return (
    <CrispStringsProvider
      locale={i18n.language}
      strings={{ sheetClose: t("close"), searchFieldClear: t("clearSearch") }}
    >
      <Page
        title={t("settings.title")}
        meta={phone ? undefined : t("settings.meta")}
        {...back}
        contentClassName="max-w-3xl space-y-7"
        filters={
          <Tabs
            value={tab}
            onChange={(loreTab) =>
              void navigate({
                to: "/settings/$section",
                params: { section: "lorebooks" },
                search: { loreTab: loreTab as "scanning" | "lorebook" | "entry" },
                replace: true,
              })
            }
          >
            <TabList aria-label={t("settings.sections")}>
              {(["scanning", "lorebook", "entry"] as const).map((value) => (
                <Tab key={value} value={value}>
                  {t(`settings.tabs.${value}`)}
                </Tab>
              ))}
            </TabList>
          </Tabs>
        }
      >
        {state.error !== null && (
          <FailureState error={state.error} onRetry={state.retry} size="sm" />
        )}
        {!settings ? (
          <div role="status" aria-label={t("settings.loading")} className="space-y-5">
            <Skeleton className="h-32 w-full" />
            <Skeleton className="h-44 w-full" />
          </div>
        ) : tab === "scanning" ? (
          <section className="space-y-2">
            <ListGroup label={t("settings.matching")}>
              <SettingsControlRow
                phone={phone}
                icon={<ScanSearch size={icon.lg} />}
                title={t("settings.messages")}
                description={t("settings.messagesBody", { count: settings.lorebook_scan_depth })}
                control={
                  <div className={`flex items-center gap-3 ${phone ? "w-full" : "w-64"}`}>
                    <Slider
                      min={1}
                      max={20}
                      value={settings.lorebook_scan_depth}
                      onChange={(value) => void state.update({ type: "scan", value })}
                      className="min-w-0 flex-1"
                      aria-label={t("settings.messages")}
                    />
                    <SliderValue
                      value={settings.lorebook_scan_depth}
                      text={String(settings.lorebook_scan_depth)}
                      overridden
                      min={1}
                      max={20}
                      step={1}
                      label={t("settings.messages")}
                      onCommit={(value) => void state.update({ type: "scan", value })}
                    />
                  </div>
                }
              />
            </ListGroup>
            <Text variant="hint">{t("settings.matchingHint")}</Text>
          </section>
        ) : (
          <>
            <Text variant="secondary">
              {t(tab === "entry" ? "settings.entryBody" : "settings.writerBody")}
            </Text>
            <ListGroup label={t("model")}>{modelRow}</ListGroup>
            {[models, accounts].map(
              (query, index) =>
                query.isError && (
                  <FailureState
                    key={index}
                    error={query.error}
                    onRetry={() => void query.refetch()}
                    size="sm"
                  />
                ),
            )}
            {tab === "lorebook" && (
              <section className="space-y-2">
                <ListGroup label={t("size")}>
                  <SettingsControlRow
                    phone={phone}
                    icon={<ListOrdered size={icon.lg} />}
                    title={t("entriesToPlan")}
                    description={
                      defaults.data
                        ? t("settings.targetDefault", {
                            count:
                              settings.lorebook_generator.default_target_count ??
                              defaults.data.target_count,
                          })
                        : undefined
                    }
                    control={
                      defaults.data ? (
                        <div className={`flex items-center gap-3 ${phone ? "w-full" : "w-64"}`}>
                          <Slider
                            value={
                              settings.lorebook_generator.default_target_count ??
                              defaults.data.target_count
                            }
                            min={defaults.data.min_target_count}
                            max={defaults.data.max_target_count}
                            className="min-w-0 flex-1"
                            onChange={(default_target_count) =>
                              void state.update({
                                type: "generator",
                                patch: { default_target_count },
                              })
                            }
                            aria-label={t("entriesToPlan")}
                          />
                          <SliderValue
                            value={
                              settings.lorebook_generator.default_target_count ??
                              defaults.data.target_count
                            }
                            text={String(
                              settings.lorebook_generator.default_target_count ??
                                defaults.data.target_count,
                            )}
                            overridden
                            min={defaults.data.min_target_count}
                            max={defaults.data.max_target_count}
                            step={1}
                            label={t("entriesToPlan")}
                            onCommit={(default_target_count) =>
                              void state.update({
                                type: "generator",
                                patch: { default_target_count },
                              })
                            }
                          />
                        </div>
                      ) : (
                        <Skeleton className={`h-8 ${phone ? "w-full" : "w-64"}`} />
                      )
                    }
                  />
                  <ListRow
                    icon={<Hash size={icon.lg} />}
                    title={t("settings.replyLimit")}
                    description={t("settings.replyLimitBody")}
                    trailing={
                      defaults.data ? (
                        <NumberInput
                          className="w-32"
                          min={256}
                          max={32768}
                          step={128}
                          value={
                            settings.lorebook_generator.max_output_tokens ??
                            defaults.data.output_tokens
                          }
                          onChange={(max_output_tokens) => {
                            if (
                              max_output_tokens !==
                              (settings.lorebook_generator.max_output_tokens ??
                                defaults.data?.output_tokens)
                            )
                              void state.update({
                                type: "generator",
                                patch: { max_output_tokens },
                              });
                          }}
                          aria-label={t("settings.replyLimit")}
                        />
                      ) : (
                        <Skeleton className="h-10 w-32" />
                      )
                    }
                  />
                </ListGroup>
                <Text variant="hint">{t("settings.replyLimitHint")}</Text>
                {defaults.isError && (
                  <FailureState error={defaults.error} onRetry={() => void defaults.refetch()} />
                )}
              </section>
            )}
            {formats}
            <section className="space-y-2">
              <ListGroup label={t(tab === "entry" ? "settings.prompts" : "settings.stepPrompts")}>
                {(tab === "entry"
                  ? (["entry", "keyword"] as const)
                  : (["planner", "writer", "refine", "coherence"] as const)
                ).map(promptRow)}
              </ListGroup>
              <Text variant="hint">{t("settings.promptsHint")}</Text>
            </section>
          </>
        )}
        {shown && (
          <Sheet
            open={picker !== undefined}
            onClose={close}
            title={promptKey ? t(`settings.promptNames.${promptKey}`) : t("model")}
          >
            <div className="space-y-4">
              <SearchField
                value={q}
                onChange={setQuery}
                placeholder={t(modelPicker ? "settings.searchModels" : "settings.searchPrompts")}
                aria-label={t(modelPicker ? "settings.searchModels" : "settings.searchPrompts")}
              />
              {(modelPicker && (models.isPending || accounts.isPending)) ||
              promptQuery?.isPending ? (
                <Skeleton className="h-32 w-full" />
              ) : (
                <>
                  <ListGroup>
                    <ListRowButton
                      icon={<FileText size={icon.lg} />}
                      title={t(modelPicker ? "settings.useDefault" : "settings.builtin")}
                      chevron={false}
                      trailing={
                        selected === null ? (
                          <Check size={icon.md} className="text-accent" />
                        ) : undefined
                      }
                      onClick={() => void choose(null)}
                    />
                    {modelPicker
                      ? filtered.map(({ model, account }) => (
                          <ListRowButton
                            key={model.id}
                            icon={
                              <ModelMark
                                name={model.display_name}
                                modelId={model.external_model_id}
                                providerKind={account.provider_kind}
                              />
                            }
                            title={model.display_name}
                            description={
                              account.enabled ? account.label : t("settings.providerDisabled")
                            }
                            disabled={!account.enabled}
                            chevron={false}
                            trailing={
                              model.id === selected ? (
                                <Check size={icon.md} className="text-accent" />
                              ) : undefined
                            }
                            onClick={() => void choose(model.id)}
                          />
                        ))
                      : custom.map((prompt) => (
                          <ListRowButton
                            key={prompt.id}
                            title={prompt.name}
                            chevron={false}
                            trailing={
                              prompt.id === selected ? (
                                <Check size={icon.md} className="text-accent" />
                              ) : undefined
                            }
                            onClick={() => void choose(prompt.id)}
                          />
                        ))}
                  </ListGroup>
                  {(modelPicker ? filtered.length : custom.length) === 0 && q && (
                    <EmptyState title={t("noMatches")} />
                  )}
                  <Button
                    variant="ghost"
                    onClick={() =>
                      void navigate({
                        to: modelPicker ? "/settings/models/new" : "/settings/$section",
                        ...(modelPicker ? {} : { params: { section: "prompts" } }),
                        replace: true,
                      })
                    }
                  >
                    {t(modelPicker ? "settings.addModel" : "settings.managePrompts")}
                  </Button>
                </>
              )}
              {promptQuery?.isError && (
                <FailureState
                  error={promptQuery.error}
                  onRetry={() => void promptQuery.refetch()}
                />
              )}
            </div>
          </Sheet>
        )}
      </Page>
    </CrispStringsProvider>
  );
}
