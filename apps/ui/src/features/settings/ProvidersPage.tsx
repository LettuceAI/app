import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Cpu, EthernetPort, Plus, Volume2 } from "lucide-react";
import { Button, confirm, Field, Input, ListGroup, ListRow, ListRowButton, LoadingState, Page, Sheet, Switch, Tab, TabList, Tabs, Text, icon } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import type { AudioProviderConfiguration, AudioProviderView, ProviderAccountView, ProviderDescriptorContract } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { ProviderMark, ProviderSelect, providerGroup, providerGroups, providerLogo, type ProviderGroup } from "@/entities/provider";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { FailureState } from "@/shared/ui/FailureState";
import { CustomProviderFields } from "./CustomProviderFields";
import { customProviderDraft, customProviderConfig } from "./custom-provider";
import { useProviderData, useSettingsAction, useSettingsApi, useSettingsLayout } from "./data";

const audioKinds = ["elevenlabs", "gemini", "fish_tts", "fish_speech", "open_ai_compatible", "kokoro"] as const;
const appManagedKinds: ReadonlySet<string> = new Set(["llamacpp", "sdcpp"]);
type Editor = { audio: boolean; account?: ProviderAccountView; voice?: AudioProviderView };

export function ProvidersPage() {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const { phone, back } = useSettingsLayout();
  const { accounts: allAccounts, catalog: fullCatalog } = useProviderData();
  const accounts = { ...allAccounts, data: allAccounts.data?.filter((account) => !appManagedKinds.has(account.provider_kind)) };
  const catalog = { ...fullCatalog, data: fullCatalog.data?.filter((descriptor) => !appManagedKinds.has(descriptor.kind)) };
  const audio = useQuery({ queryKey: queryKeys.audioProviders.all, queryFn: () => api.call("audioProvidersList") });
  const models = useQuery({ queryKey: queryKeys.models.list(), queryFn: () => api.call("modelsList") });
  const { t: common } = useTranslation();
  const [tab, setTab] = useState<"llm" | "audio">("llm");
  const [editor, setEditor] = useState<Editor | null>(null);
  const closingEditor = useClosingValue(editor);
  const query = tab === "audio" ? audio : accounts;
  const modelCounts = new Map<string, number>();
  for (const model of models.data?.models ?? []) modelCounts.set(model.provider_account_id, (modelCounts.get(model.provider_account_id) ?? 0) + 1);
  const groupOf = (account: ProviderAccountView): ProviderGroup => {
    const descriptor = catalog.data?.find((d) => d.kind === account.provider_kind);
    return descriptor ? providerGroup(descriptor) : "custom";
  };
  return (
    <>
      <Page
        title={t("sections.providers.title")}
        {...back}
        padding="none"
        contentClassName={`pb-10 pt-1 space-y-4 ${phone ? "px-4" : "px-8"}`}
        actions={
          <Button
            variant="tonal"
            size="sm"
            disabled={tab === "llm" && !catalog.data?.length}
            leading={<Plus size={icon.sm} />}
            onClick={() => setEditor({ audio: tab === "audio" })}
          >
            {t("manage.add")}
          </Button>
        }
      >
        <Tabs value={tab} onChange={setTab}>
          <TabList aria-label={t("providers.type")}>
            <Tab value="llm" count={accounts.data?.length ?? 0}>{t("providers.language")}</Tab>
            <Tab value="audio" count={audio.data?.length ?? 0}>{t("providers.audio")}</Tab>
          </TabList>
        </Tabs>
        {tab === "llm" && catalog.isError && <FailureState error={catalog.error} onRetry={() => void catalog.refetch()} />}
        {query.isPending ? (
          <LoadingState />
        ) : query.isError ? (
          <FailureState error={query.error} onRetry={() => void query.refetch()} />
        ) : (
          tab === "llm" ? (
            <div className="@container space-y-7">
              {providerGroups.map((group) => {
                const members = (accounts.data ?? []).filter((p) => groupOf(p) === group);
                if (members.length === 0) return null;
                return (
                  <section key={group} aria-label={common(`providerSelect.groups.${group}`)} className="space-y-3">
                    <h3 className="flex items-center gap-2.5 px-1 text-fg-2">
                      <span className="text-sm font-medium">{common(`providerSelect.groups.${group}`)}</span>
                      <span className="text-xs tabular-nums text-fg-4" aria-hidden>{members.length}</span>
                      <span className="h-px flex-1 bg-line" aria-hidden />
                    </h3>
                    <div className="grid grid-cols-1 gap-2 @min-[53rem]:grid-cols-2">
                      {members.map((p) => {
                        const kind = catalog.data?.find((d) => d.kind === p.provider_kind);
                        const needsSetup = kind?.api_key === "required" && !p.api_key_set;
                        const address = p.base_url ?? kind?.default_endpoint ?? null;
                        const showAddress = address !== null && (group !== "cloud" || (p.base_url !== null && p.base_url !== kind?.default_endpoint));
                        const count = modelCounts.get(p.id) ?? 0;
                        return (
                          <ListGroup key={p.id} className="min-w-0 [&>div]:h-full [&>div>button]:h-full">
                            <ListRowButton
                              icon={
                                providerLogo(p.provider_kind) ? (
                                  <ProviderMark kind={p.provider_kind} name={p.label} />
                                ) : address ? (
                                  <EthernetPort size={icon.lg} />
                                ) : (
                                  <Cpu size={icon.lg} />
                                )
                              }
                              tone={needsSetup ? "warning" : address ? "info" : "accent"}
                              title={p.label}
                              description={[
                                kind?.display_name ?? p.provider_kind,
                                needsSetup ? t("providers.needsSetup") : t("providers.modelCount", { count }),
                                showAddress ? address : null,
                              ]
                                .filter(Boolean)
                                .join(" · ")}
                              onClick={() => setEditor({ audio: false, account: p })}
                            />
                          </ListGroup>
                        );
                      })}
                    </div>
                  </section>
                );
              })}
            </div>
          ) : (
          <ListGroup>
            {audio.data?.map((p) => (
                  <ListRowButton
                    key={p.id}
                    icon={
                      providerLogo(p.configuration.type) ? (
                        <ProviderMark kind={p.configuration.type} name={p.label} />
                      ) : (
                        <Volume2 size={icon.lg} />
                      )
                    }
                    tone="info"
                    title={p.label}
                    description={t(`providers.audioKinds.${p.configuration.type}`)}
                    onClick={() => setEditor({ audio: true, voice: p })}
                  />
                ))}
          </ListGroup>
          )
        )}
        {query.data?.length === 0 && <Text variant="hint">{t("providers.empty")}</Text>}
        {tab === "audio" && <Text variant="hint">{t("providers.voicesHint")}</Text>}
      </Page>
      {closingEditor.shown && (
        <ProviderEditor
          key={closingEditor.session}
          open={editor !== null}
          editor={closingEditor.shown}
          catalog={catalog.data ?? []}
          onClose={() => setEditor(null)}
        />
      )}
    </>
  );
}

function ProviderEditor({ open, editor, catalog, onClose }: { open: boolean; editor: Editor; catalog: ProviderDescriptorContract[]; onClose: () => void }) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const action = useSettingsAction();
  const queryClient = useQueryClient();
  const { account, voice, audio } = editor;
  const { phone } = useSettingsLayout();
  const { t: common } = useTranslation();
  const [kind, setKind] = useState(account?.provider_kind ?? voice?.configuration.type ?? (audio ? "elevenlabs" : catalog.find((p) => p.kind === "openrouter")?.kind ?? catalog[0]?.kind ?? ""));
  const isCustom = !audio && (kind === "custom" || kind === "custom-anthropic");
  const [custom, setCustom] = useState(() => customProviderDraft(kind, account?.config));
  const [allowInvalidTls, setAllowInvalidTls] = useState(account?.allow_invalid_tls ?? false);
  const config = isCustom ? customProviderConfig(custom) : account?.config ?? {};
  const [label, setLabel] = useState(account?.label ?? voice?.label ?? "");
  const [key, setKey] = useState("");
  const [host, setHost] = useState(account?.base_url ?? (voice && "base_url" in voice.configuration ? voice.configuration.base_url : "") ?? "");
  const [streaming, setStreaming] = useState(account?.streaming_enabled ?? true);
  const [verified, setVerified] = useState<boolean | null>(null);
  const [project, setProject] = useState(voice?.configuration.type === "gemini" ? voice.configuration.project_id ?? "" : "");
  const [location, setLocation] = useState(voice?.configuration.type === "gemini" ? voice.configuration.location : "us-central1");
  const descriptor = catalog.find((p) => p.kind === kind);
  const options = audio ? audioKinds.map((value) => ({ value, label: t(`providers.audioKinds.${value}`) })) : catalog.map((p) => ({ value: p.kind, label: p.display_name }));
  const needsKey = audio ? !["kokoro", "fish_speech"].includes(kind) && !voice?.has_api_key : (isCustom ? custom.auth !== "none" : descriptor?.api_key === "required") && !account?.api_key_set;
  const name = label.trim() || options.find((p) => p.value === kind)?.label || kind;
  const configuration = (): AudioProviderConfiguration => {
    switch (kind) {
      case "gemini": return { type: "gemini", project_id: project.trim() || null, location: location.trim() };
      case "fish_speech": case "open_ai_compatible": return { type: kind, base_url: host.trim() || null, request_path: voice && "request_path" in voice.configuration ? voice.configuration.request_path : null };
      case "kokoro": return { type: "kokoro", variant: voice?.configuration.type === "kokoro" ? voice.configuration.variant : null };
      case "fish_tts": return { type: "fish_tts" };
      default: return { type: "elevenlabs" };
    }
  };
  const save = () => action.mutate(async () => {
    if (audio) {
      if (voice) {
        if (key.trim()) {
          const status = await api.call("audioProviderCredentialStatus", { provider_id: voice.id });
          await api.call("audioProviderApiKeyRotate", { provider_id: voice.id, api_key: key.trim(), expected_generation: status.generation });
          setKey("");
        }
        await api.call("audioProviderUpdate", { provider_id: voice.id, expected_revision: voice.revision, label: name, configuration: configuration() });
      } else await api.call("audioProviderCreate", { label: name, draft: { configuration: configuration(), api_key: key.trim() || null }, client_operation_id: crypto.randomUUID() });
    } else await api.call("providerAccountSave", {
      account: { id: account?.id ?? null, provider_kind: kind, label: name, base_url: host.trim() || null, enabled: account?.enabled ?? true, streaming_enabled: isCustom ? custom.supportsStreaming : streaming, allow_invalid_tls: allowInvalidTls, config },
      api_key: isCustom && custom.auth === "none" ? null : key.trim() || null, clear_api_key: false, expected_revision: account?.revision ?? null, client_operation_id: crypto.randomUUID(),
    });
    onClose();
  });
  const deleteAccount = (target: ProviderAccountView, withModels: boolean) =>
    api.call("providerAccountDelete", { account_id: target.id, expected_revision: target.revision, delete_models: withModels, client_operation_id: crypto.randomUUID() });
  const remove = () => action.mutate(async () => {
    if (voice) await api.call("audioProviderDelete", { provider_id: voice.id, expected_revision: voice.revision });
    if (account) {
      try {
        await deleteAccount(account, false);
      } catch (error) {
        if (!isApiFailure(error) || error.details?.type !== "provider_models_in_use") throw error;
        const models = await queryClient.fetchQuery({ queryKey: queryKeys.models.list(), queryFn: () => api.call("modelsList") });
        const names = error.details.models.map((id) => models.models.find((model) => model.id === id)?.display_name ?? id);
        const sure = await confirm({
          title: t("providers.deleteWithModelsTitle", { name: account.label }),
          message: t("providers.deleteWithModelsBody", { count: names.length, names: names.join(", ") }),
          confirmLabel: t("providers.deleteWithModels", { count: names.length }),
          cancelLabel: t("manage.cancel"),
          destructive: true,
        });
        if (!sure) return;
        await deleteAccount(account, true);
      }
    }
    onClose();
  });
  const changedEndpoint = Boolean(account && (host.trim() !== (account.base_url ?? "") || allowInvalidTls !== account.allow_invalid_tls || (isCustom && JSON.stringify(custom) !== JSON.stringify(customProviderDraft(kind, account.config)))));
  const customIncomplete = isCustom && (!host.trim() || !custom.chatPath.trim() || (custom.fetchModels && (!custom.modelsPath.trim() || !custom.listPath.trim() || !custom.idPath.trim())) || (custom.auth === "header" && !custom.headerName.trim()) || (custom.auth === "query" && !custom.queryName.trim()));
  const verify = () => action.mutate(async () => {
    if (audio) {
      const valid = await api.call("audioProviderVerify", voice && !key.trim() ? { type: "saved", provider_id: voice.id } : { type: "draft", draft: { configuration: configuration(), api_key: key.trim() || null } });
      setVerified(valid);
    } else {
      await api.call("providerVerify", account && !key.trim() ? { type: "saved", account_id: account.id } : { type: "draft", draft: { provider_kind: kind, base_url: host.trim() || null, api_key: key.trim() || null, config } });
      setVerified(true);
    }
  });
  const askDelete = async () => {
    const sure = await confirm({
      title: t("providers.deleteTitle", { name }),
      message: t("providers.deleteWarning"),
      confirmLabel: t("manage.confirmDelete"),
      cancelLabel: t("manage.cancel"),
      destructive: true,
    });
    if (sure) remove();
  };
  const providerOptions = options.map((option) => {
    const provider = catalog.find((p) => p.kind === option.value);
    return {
      id: option.value,
      name: option.label,
      kind: option.value,
      ...(!audio && provider ? { group: common(`providerSelect.groups.${providerGroup(provider)}`) } : {}),
    };
  });
  const showKey = (!audio || kind !== "kokoro") && (!isCustom || custom.auth !== "none");
  const showUrl = !audio || kind === "fish_speech" || kind === "open_ai_compatible";
  return (
    <Sheet
      open={open}
      placement={phone ? "bottom" : "side"}
      onClose={() => { if (!action.isPending) onClose(); }}
      title={t(account || voice ? "providers.edit" : "providers.new")}
      footer={
        <div className="flex justify-end gap-2">
          <Button variant="ghost" disabled={action.isPending} onClick={onClose}>{t("manage.cancel")}</Button>
          <Button
            variant="primary"
            loading={action.isPending}
            disabled={!kind || (needsKey && !key.trim()) || customIncomplete}
            onClick={save}
          >
            {t(account || voice ? "manage.save" : "providers.add")}
          </Button>
        </div>
      }
    >
      <div className="space-y-4">
        {action.isError && <FailureState error={action.error} size="sm" />}
        <Field label={t("providers.type")}>
          <ProviderSelect
            providers={providerOptions}
            value={kind}
            compact={phone}
            aria-label={t("providers.type")}
            disabled={Boolean(account || voice)}
            onChange={(value) => {
              if (value) {
                setKind(value);
                setCustom(customProviderDraft(value));
                setVerified(null);
              }
            }}
          />
        </Field>
        <Field label={t("providers.label")} hint={t("providers.labelHint")}>
          <Input value={label} onChange={(e) => setLabel(e.target.value)} placeholder={options.find((p) => p.value === kind)?.label} />
        </Field>
        {showKey && (
          <Field label={t("providers.key")} hint={account?.api_key_set || voice?.has_api_key ? t("providers.keepKey") : undefined}>
            <Input
              type="password"
              autoComplete="new-password"
              value={key}
              onChange={(e) => { setKey(e.target.value); setVerified(null); }}
              placeholder={t("providers.keyPlaceholder")}
            />
          </Field>
        )}
        {showUrl && (
          <Field label={t("providers.url")} hint={isCustom ? t("customProvider.urlRequired") : t("providers.urlHint")}>
            <Input
              value={host}
              disabled={!audio && descriptor?.endpoint_editable === false}
              onChange={(e) => { setHost(e.target.value); setVerified(null); }}
              placeholder={descriptor?.default_endpoint ?? "https://"}
            />
          </Field>
        )}
        {audio && kind === "gemini" && (
          <>
            <Field label={t("providers.project")}><Input value={project} onChange={(e) => setProject(e.target.value)} /></Field>
            <Field label={t("providers.location")}><Input value={location} onChange={(e) => setLocation(e.target.value)} /></Field>
          </>
        )}
        {isCustom && (
          <CustomProviderFields
            draft={custom}
            onChange={(next) => { setCustom(next); setVerified(null); }}
            anthropic={kind === "custom-anthropic"}
            allowInvalidTls={allowInvalidTls}
            onTlsChange={setAllowInvalidTls}
          />
        )}
        {!audio && !isCustom && (
          <ListGroup>
            <ListRow
              title={t("providers.stream")}
              description={t("providers.streamHint")}
              trailing={<Switch checked={streaming} onChange={setStreaming} aria-label={t("providers.stream")} />}
            />
          </ListGroup>
        )}
        {verified === false && <Text>{t("providers.verifyFailed")}</Text>}
        {changedEndpoint && !key.trim() && <Text variant="hint">{t("providers.saveBeforeVerify")}</Text>}
        <div className="flex justify-between gap-2">
          {(account || voice) && (
            <Button variant="ghost" disabled={action.isPending} onClick={() => void askDelete()}>{t("manage.delete")}</Button>
          )}
          {(audio || descriptor?.verifies_key) && (
            <Button variant="ghost" disabled={action.isPending || (changedEndpoint && !key.trim())} onClick={verify}>
              {t(verified ? "providers.verified" : "providers.verify")}
            </Button>
          )}
        </div>
      </div>
    </Sheet>
  );
}
