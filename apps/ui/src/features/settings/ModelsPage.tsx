import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  Check,
  ChevronDown,
  Columns2,
  Copy,
  Cpu,
  Download,
  EthernetPort,
  Layers,
  List,
  Pencil,
  Plus,
  Search,
  Star,
  Trash2,
} from "lucide-react";
import {
  Badge,
  Button,
  confirm,
  Field,
  icon,
  IconButton,
  Input,
  ListGroup,
  ListRowButton,
  LoadingState,
  Menu,
  MenuContent,
  MenuItem,
  MenuTrigger,
  Page,
  Select,
  Sheet,
  Text,
} from "@lettuceai/crisp";
import type { ModelView, ProviderAccountView, RemoteModelContract } from "@/api/generated/bindings";
import { appStatusQuery } from "@/api/app-status-query";
import type { ApiClient } from "@/api/client";
import { queryKeys } from "@/api/query-keys";
import { defaultModelName, ModelMark, ProviderSelect, providerLogo, vendorOf } from "@/entities/provider";
import { minimalModelConfig } from "@/entities/model";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { FailureState } from "@/shared/ui/FailureState";
import { useProviderData, useSettingsAction, useSettingsApi, useSettingsLayout } from "./data";
import { ModelTools } from "./ModelTools";

const modelListViews = ["all", "provider", "providerColumns"] as const;
type ModelListView = (typeof modelListViews)[number];
const modelListViewKey = "modelsListView";

function useModelListView(api: ApiClient) {
  const queryClient = useQueryClient();
  const status = useQuery(appStatusQuery(api));
  const [chosen, setChosen] = useState<ModelListView | null>(null);
  const saved = modelListViews.find((view) => view === status.data?.ui_state[modelListViewKey]);
  const view = chosen ?? saved ?? "all";
  const choose = (next: ModelListView) => {
    setChosen(next);
    void api
      .call("appUiStateUpdate", { patch: { [modelListViewKey]: next } })
      .then(() => queryClient.invalidateQueries({ queryKey: queryKeys.app.status() }));
  };
  return [view, choose] as const;
}

function isLocalProvider(account: ProviderAccountView | undefined): boolean {
  return account?.provider_kind === "llamacpp";
}

export function ModelsPage() {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation("common");
  const api = useSettingsApi();
  const { phone, back } = useSettingsLayout();
  const { accounts } = useProviderData();
  const models = useQuery({ queryKey: queryKeys.models.list(), queryFn: () => api.call("modelsList") });
  const action = useSettingsAction();
  const [group, setGroup] = useModelListView(api);
  const [search, setSearch] = useState("");
  const [collapsedSearchProviders, setCollapsedSearchProviders] = useState<Set<string>>(() => new Set());
  const [collapsedProviders, setCollapsedProviders] = useState<Set<string>>(() => new Set());
  const [menu, setSelectedMenu] = useState<ModelView | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const setMenu = (value: ModelView | null) => {
    setMenuOpen(value !== null);
    if (value) setSelectedMenu(value);
  };
  const [editor, setEditor] = useState<ModelView | "new" | null>(null);
  const [tool, setTool] = useState<"browse" | "runtime" | "loras" | null>(null);
  const closingEditor = useClosingValue(editor);
  const closingTool = useClosingValue(tool);
  const provider = (model: ModelView) => accounts.data?.find((p) => p.id === model.provider_account_id);
  const query = search.trim().toLocaleLowerCase();
  const visibleModels = (models.data?.models ?? []).filter((model) =>
    [model.display_name, model.external_model_id, provider(model)?.label ?? ""].some((value) =>
      value.toLocaleLowerCase().includes(query),
    ),
  );
  const groups = new Map<string, ModelView[]>();
  for (const model of visibleModels) {
    groups.set(model.provider_account_id, [...(groups.get(model.provider_account_id) ?? []), model]);
  }
  const openMenu = (model: ModelView) => {
    action.reset();
    setMenu(model);
  };
  const row = (model: ModelView, grouped = false) => {
    const account = provider(model);
    const local = isLocalProvider(account);
    const isDefault = model.id === models.data?.default_model_id;
    const parts = [
      ...(grouped ? [] : [account?.label ?? model.provider_account_id]),
      modelFileLabel(model),
      ...modelCapabilities(model).map((capability) => t(`models.capability.${capability}`)),
    ];
    const hasMark = account && (vendorOf(model.external_model_id) || providerLogo(account.provider_kind));
    return (
      <ListRowButton
        key={model.id}
        icon={
          hasMark ? (
            <ModelMark modelId={model.external_model_id} providerKind={account.provider_kind} name={model.display_name} />
          ) : local ? (
            <Cpu size={icon.lg} />
          ) : (
            <EthernetPort size={icon.lg} />
          )
        }
        tone={local ? "accent" : "info"}
        title={model.display_name}
        description={
          <span className="flex min-w-0 flex-wrap items-center gap-x-1.5">
            {parts.map((part, index) => (
              <span
                key={`${index}-${part}`}
                className={index === (grouped ? 0 : 1) ? "min-w-0 truncate font-mono text-xs" : undefined}
              >
                {index > 0 && <span className="mr-1.5 text-fg-4 before:content-['·']" aria-hidden />}
                {part}
              </span>
            ))}
          </span>
        }
        trailing={
          isDefault ? (
            <Badge tone="accent" dot>
              {t("models.default")}
            </Badge>
          ) : undefined
        }
        chevron={!isDefault}
        onClick={() => openMenu(model)}
        onContextMenu={(event) => {
          event.preventDefault();
          openMenu(model);
        }}
      />
    );
  };
  const run = (fn: () => Promise<unknown>) =>
    action.mutate(async () => {
      await fn();
      setMenu(null);
    });
  const confirmDelete = (model: ModelView) => {
    void (async () => {
      const sure = await confirm({
        title: t("models.deleteTitle", { name: model.display_name }),
        message: t("models.deleteWarning"),
        confirmLabel: t("manage.confirmDelete"),
        cancelLabel: common("actions.cancel"),
        destructive: true,
      });
      if (!sure) return;
      run(() =>
        api.call("modelDelete", {
          model_id: model.id,
          expected_revision: model.revision,
          client_operation_id: crypto.randomUUID(),
        }),
      );
    })();
  };
  const viewIcon =
    group === "providerColumns" ? (
      <Columns2 size={icon.lg} />
    ) : group === "provider" ? (
      <Layers size={icon.lg} />
    ) : (
      <List size={icon.lg} />
    );
  const viewTitle = group === "providerColumns" ? "models.providerColumns" : group === "provider" ? "models.byProvider" : "models.allModels";
  return (
    <>
      <Page
        title={t("sections.models.title")}
        meta={models.data ? String(models.data.models.length) : undefined}
        {...back}
        padding="none"
        contentClassName={`pb-10 pt-1 space-y-6 ${phone ? "px-4" : "px-8"}`}
        actions={
          <div className="flex items-center gap-1">
            {modelTools.map((tool) =>
              phone ? (
                <IconButton key={tool.id} label={t(`models.${tool.id}`)} variant="ghost" onClick={() => setTool(tool.id)}>
                  {tool.icon}
                </IconButton>
              ) : (
                <Button key={tool.id} variant="ghost" size="sm" leading={tool.icon} title={t(`models.${tool.id}Hint`)} onClick={() => setTool(tool.id)}>
                  {t(`models.${tool.id}Short`)}
                </Button>
              ),
            )}
            <Menu>
              <MenuTrigger>
                <IconButton label={t("models.view")} title={t(viewTitle)} variant="ghost">
                  {viewIcon}
                </IconButton>
              </MenuTrigger>
              <MenuContent align="end">
                <MenuItem icon={<List size={icon.md} />} selected={group === "all"} onSelect={() => setGroup("all")}>
                  {t("models.allModels")}
                </MenuItem>
                <MenuItem
                  icon={<Layers size={icon.md} />}
                  selected={group === "provider"}
                  onSelect={() => setGroup("provider")}
                >
                  {t("models.byProvider")}
                </MenuItem>
                <MenuItem
                  icon={<Columns2 size={icon.md} />}
                  selected={group === "providerColumns"}
                  onSelect={() => setGroup("providerColumns")}
                >
                  {t("models.providerColumns")}
                </MenuItem>
              </MenuContent>
            </Menu>
            <Button variant="tonal" size="sm" className="ml-1" leading={<Plus size={icon.sm} />} onClick={() => setEditor("new")}>
              {t("manage.add")}
            </Button>
          </div>
        }
      >
        <div className="space-y-2">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <Text variant="section">{t("models.yours")}</Text>
            <div className="order-last w-full min-w-0 sm:order-none sm:ml-auto sm:w-auto sm:max-w-sm sm:flex-1">
              <Input
                type="search"
                aria-label={t("models.search")}
                placeholder={t("models.search")}
                leading={<Search size={icon.sm} />}
                value={search}
                onChange={(event) => {
                  setSearch(event.target.value);
                  setCollapsedSearchProviders(new Set());
                }}
              />
            </div>
          </div>
          {models.isPending ? (
            <LoadingState />
          ) : models.isError ? (
            <FailureState error={models.error} onRetry={() => void models.refetch()} />
          ) : models.data.models.length === 0 ? (
            <Text variant="hint">{t("models.empty")}</Text>
          ) : visibleModels.length === 0 ? (
            <Text variant="hint" role="status">
              {t("models.noResults")}
            </Text>
          ) : group === "all" ? (
            <ListGroup>{visibleModels.map((model) => row(model))}</ListGroup>
          ) : (
            <div className="@container">
              <div
                className={
                  group === "providerColumns"
                    ? `gap-4 ${phone ? "columns-1" : "columns-2"}`
                    : "grid grid-flow-row-dense grid-cols-1 items-start gap-x-4 gap-y-7 @min-[53rem]:grid-cols-2 @min-[80rem]:grid-cols-3"
                }
              >
                {[...groups].map(([id, list]) => {
                  const account = accounts.data?.find((p) => p.id === id);
                  const label = account?.label ?? id;
                  const collapsed = (query ? collapsedSearchProviders : collapsedProviders).has(id);
                  const listId = `provider-models-${id}`;
                  const sectionClass =
                    group === "providerColumns"
                      ? "flow-root break-inside-avoid pb-7 space-y-3"
                      : `min-w-0 space-y-3 ${list.length >= 2 ? "@min-[53rem]:col-span-2" : ""} ${list.length >= 3 ? "@min-[80rem]:col-span-3" : ""}`;
                  const listClass =
                    group === "providerColumns"
                      ? "min-w-0"
                      : "min-w-0 [&>div]:grid [&>div]:grid-cols-[repeat(auto-fit,minmax(min(100%,26rem),1fr))] [&>div]:gap-px [&>div]:divide-y-0 [&>div]:bg-line";
                  return (
                    <section key={id} aria-label={label} className={sectionClass}>
                      <h3>
                        <button
                          type="button"
                          aria-label={label}
                          aria-expanded={!collapsed}
                          aria-controls={listId}
                          className="flex min-h-9 w-full items-center gap-2.5 rounded-md px-1 text-left text-fg-2 hover:text-fg focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
                          onClick={() =>
                            (query ? setCollapsedSearchProviders : setCollapsedProviders)((current) => {
                              const next = new Set(current);
                              if (next.has(id)) next.delete(id);
                              else next.add(id);
                              return next;
                            })
                          }
                        >
                          <ChevronDown
                            size={icon.sm}
                            aria-hidden
                            className={`shrink-0 ${collapsed ? "-rotate-90" : ""}`}
                          />
                          <span className="text-sm font-medium">{label}</span>
                          <span className="text-xs tabular-nums text-fg-4" aria-hidden>
                            {list.length}
                          </span>
                          <span className="h-px flex-1 bg-line" aria-hidden />
                        </button>
                      </h3>
                      <div id={listId} hidden={collapsed}>
                        <ListGroup className={listClass}>
                          {list.map((model) =>
                            group === "providerColumns" ? (
                              row(model, true)
                            ) : (
                              <div key={model.id} className="min-w-0 bg-surface-1 [&>button]:h-full">
                                {row(model, true)}
                              </div>
                            ),
                          )}
                        </ListGroup>
                      </div>
                    </section>
                  );
                })}
              </div>
            </div>
          )}
        </div>
      </Page>
      <Sheet
        open={menuOpen}
        onClose={() => {
          if (!action.isPending) setMenu(null);
        }}
        title={menu?.display_name}
        description={menu?.external_model_id}
      >
        {action.isError && <FailureState error={action.error} size="sm" />}
        {menu && (
          <ListGroup>
            <ListRowButton
              disabled={action.isPending}
              icon={<Pencil size={icon.lg} />}
              title={t("manage.edit")}
              description={t("models.editHint")}
              onClick={() => {
                setEditor(menu);
                setMenu(null);
              }}
            />
            <ListRowButton
              disabled={action.isPending}
              icon={<Star size={icon.lg} />}
              tone="accent"
              title={t(menu.id === models.data?.default_model_id ? "models.alreadyDefault" : "models.setDefault")}
              description={t("models.defaultHint")}
              chevron={false}
              trailing={
                menu.id === models.data?.default_model_id ? <Check size={icon.md} className="text-accent" /> : undefined
              }
              onClick={() => {
                if (menu.id === models.data?.default_model_id) {
                  setMenu(null);
                  return;
                }
                run(() =>
                  api.call("modelDefaultSet", {
                    model_id: menu.id,
                    expected_revision: models.data!.revision,
                    client_operation_id: crypto.randomUUID(),
                  }),
                );
              }}
            />
            <ListRowButton
              disabled={action.isPending}
              icon={<Download size={icon.lg} />}
              title={t("manage.export")}
              description={t("models.exportHint")}
              chevron={false}
              onClick={() =>
                run(async () => {
                  const picked = await api.call("filesPickSave", {
                    suggested_name: `${menu.display_name.replace(/[/\\]/g, "_")}.uec`,
                    kind: "any",
                  });
                  if (picked.target) await api.call("modelExport", { id: menu.id, target: picked.target });
                })
              }
            />
            <ListRowButton
              disabled={action.isPending}
              icon={<Copy size={icon.lg} />}
              title={t("manage.duplicate")}
              description={t("models.duplicateHint")}
              chevron={false}
              onClick={() =>
                run(() =>
                  api.call("modelDuplicate", {
                    model_id: menu.id,
                    display_name: t("models.copyName", { name: menu.display_name }),
                    expected_revision: menu.revision,
                    client_operation_id: crypto.randomUUID(),
                  }),
                )
              }
            />
            <ListRowButton
              disabled={action.isPending}
              icon={<Trash2 size={icon.lg} />}
              tone="danger"
              title={t("manage.delete")}
              description={t("models.deleteHint")}
              chevron={false}
              onClick={() => confirmDelete(menu)}
            />
          </ListGroup>
        )}
      </Sheet>
      {closingEditor.shown && (
        <ModelEditor
          key={closingEditor.session}
          open={editor !== null}
          model={closingEditor.shown === "new" ? null : closingEditor.shown}
          accounts={accounts.data ?? []}
          onClose={() => setEditor(null)}
        />
      )}
      {closingTool.shown && (
        <ModelTools key={closingTool.session} open={tool !== null} tool={closingTool.shown} onClose={() => setTool(null)} />
      )}
    </>
  );
}

const modelTools = [
  { id: "browse", icon: <Download size={icon.sm} /> },
  { id: "runtime", icon: <Cpu size={icon.sm} /> },
  { id: "loras", icon: <Layers size={icon.sm} /> },
] as const;

type ModelCapability = "vision" | "audio" | "imageGeneration";

/** What a model can do beyond text, for the line under its name. */
function modelCapabilities(model: ModelView): ModelCapability[] {
  if (model.kind === "image") return ["imageGeneration"];
  const capabilities: ModelCapability[] = [];
  if (model.kind === "chat" && model.input_scopes.includes("image")) capabilities.push("vision");
  if (model.input_scopes.includes("audio") || model.output_scopes.includes("audio")) capabilities.push("audio");
  return capabilities;
}

/** The model id, or only the file name when the id is a path to a local file. */
function modelFileLabel(model: ModelView): string {
  const id = model.external_model_id;
  return /[\\/]/.test(id) ? (id.split(/[\\/]/).pop() ?? id) : id;
}

function record(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

type Scope = "text" | "image" | "audio";

function scopesOf(modalities: string[] | null | undefined): Scope[] {
  return (modalities ?? ["text"]).filter((m): m is Scope => ["text", "image", "audio"].includes(m));
}

function ModelEditor({
  open,
  model,
  accounts,
  onClose,
}: {
  open: boolean;
  model: ModelView | null;
  accounts: ProviderAccountView[];
  onClose: () => void;
}) {
  const { phone } = useSettingsLayout();
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const action = useSettingsAction();
  const [accountId, setAccountId] = useState(model?.provider_account_id ?? accounts[0]?.id ?? "");
  const [externalId, setExternalId] = useState(model?.external_model_id ?? "");
  const [name, setName] = useState(model?.display_name ?? "");
  const local = isLocalProvider(accounts.find((p) => p.id === accountId));
  const [threads, setThreads] = useState(String(record(model?.config.llama_cpp).threads ?? ""));
  const [gpuLayers, setGpuLayers] = useState(String(record(model?.config.llama_cpp).gpu_layers ?? ""));
  const [remote, setRemote] = useState<RemoteModelContract | null>(null);
  const [temperature, setTemperature] = useState(String(record(model?.config.chat_parameters).temperature ?? ""));
  const available = useQuery({
    queryKey: queryKeys.providers.models(accountId),
    queryFn: () => api.call("providerModels", { account_id: accountId }),
    enabled: Boolean(accountId),
  });
  const save = () =>
    action.mutate(async () => {
      const config: Record<string, unknown> = structuredClone(model?.config ?? minimalModelConfig);
      config.chat_parameters = {
        ...record(record(config).chat_parameters),
        temperature: temperature === "" ? null : Number(temperature),
      };
      if (local) {
        config.llama_cpp = {
          ...record(config.llama_cpp),
          threads: threads === "" ? null : Number(threads),
          gpu_layers: gpuLayers === "" ? null : Number(gpuLayers),
        };
      }
      await api.call("modelSave", {
        model: {
          id: model?.id ?? null,
          provider_account_id: accountId,
          external_model_id: externalId.trim(),
          display_name: name.trim() || externalId.trim(),
          kind: model?.kind ?? "chat",
          config,
          input_scopes: remote ? scopesOf(remote.input_modalities) : (model?.input_scopes ?? ["text"]),
          output_scopes: remote ? scopesOf(remote.output_modalities) : (model?.output_scopes ?? ["text"]),
          remote_metadata: remote,
        },
        expected_revision: model?.revision ?? null,
        client_operation_id: crypto.randomUUID(),
      });
      onClose();
    });
  const invalid =
    !accountId ||
    !externalId.trim() ||
    (temperature !== "" && (!Number.isFinite(Number(temperature)) || Number(temperature) < 0));
  return (
    <Sheet
      open={open}
      onClose={() => {
        if (!action.isPending) onClose();
      }}
      title={t(model ? "models.edit" : "models.new")}
      footer={
        <div className="flex justify-end gap-2">
          <Button variant="ghost" disabled={action.isPending} onClick={onClose}>
            {t("manage.cancel")}
          </Button>
          <Button loading={action.isPending} disabled={invalid} onClick={save}>
            {t("manage.save")}
          </Button>
        </div>
      }
    >
      <div className="space-y-4">
        {action.isError && <FailureState error={action.error} size="sm" />}
        {accounts.length === 0 && <Text>{t("models.noProviders")}</Text>}
        <Field label={t("models.provider")}>
          <ProviderSelect
            value={accountId || null}
            compact={phone}
            aria-label={t("models.provider")}
            providers={accounts.map((p) => ({
              id: p.id,
              name: p.label,
              kind: p.provider_kind,
              ...(p.base_url ? { meta: p.base_url } : {}),
            }))}
            onChange={(value) => {
              if (value) {
                setAccountId(value);
                setRemote(null);
                setExternalId("");
              }
            }}
          />
        </Field>
        {available.isFetching && <LoadingState />}
        {available.isError && (
          <FailureState error={available.error} size="sm" onRetry={() => void available.refetch()} />
        )}
        {available.data && (
          <Field label={t("models.catalog")}>
            <Select
              value={remote?.id ?? ""}
              options={[
                { value: "", label: t("models.manual") },
                ...available.data.map((p) => ({ value: p.id, label: p.display_name ?? p.id })),
              ]}
              onChange={(e) => {
                const selected = available.data.find((p) => p.id === e.target.value) ?? null;
                setRemote(selected);
                if (selected) {
                  setExternalId(selected.id);
                  setName(defaultModelName(selected.display_name, selected.id));
                }
              }}
            />
          </Field>
        )}
        <Field label={t("models.id")}>
          <Input
            value={externalId}
            onChange={(e) => {
              setExternalId(e.target.value);
              setRemote(null);
            }}
          />
        </Field>
        <Field label={t("providers.label")}>
          <Input value={name} onChange={(e) => setName(e.target.value)} />
        </Field>
        {local && (
          <>
            <Field label={t("models.threads")}>
              <Input type="number" min={1} step={1} value={threads} onChange={(e) => setThreads(e.target.value)} />
            </Field>
            <Field label={t("models.gpuLayers")}>
              <Input type="number" min={0} step={1} value={gpuLayers} onChange={(e) => setGpuLayers(e.target.value)} />
            </Field>
          </>
        )}
        <Field label={t("models.temperature")} hint={t("models.temperatureHint")}>
          <Input
            type="number"
            min={0}
            step={0.1}
            value={temperature}
            onChange={(e) => setTemperature(e.target.value)}
          />
        </Field>
      </div>
    </Sheet>
  );
}
