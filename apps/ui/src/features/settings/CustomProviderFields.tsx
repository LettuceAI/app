import { useTranslation } from "react-i18next";
import { Field, Input, ListGroup, ListRow, Select, Switch } from "@lettuceai/crisp";
import type { CustomProviderDraft } from "./custom-provider";

export function CustomProviderFields({ draft, onChange, anthropic, allowInvalidTls, onTlsChange }: {
  draft: CustomProviderDraft;
  onChange: (draft: CustomProviderDraft) => void;
  anthropic: boolean;
  allowInvalidTls: boolean;
  onTlsChange: (value: boolean) => void;
}) {
  const { t } = useTranslation("settings");
  const roleNames = { system: "system", user: "user", assistant: "assistant" };
  const update = <K extends keyof CustomProviderDraft>(key: K, value: CustomProviderDraft[K]) => onChange({ ...draft, [key]: value });
  return <div className="space-y-4">
    <Field label={t("customProvider.chatPath")}><Input value={draft.chatPath} onChange={(e) => update("chatPath", e.target.value)} /></Field>
    <Field label={t("customProvider.auth")}><Select value={draft.auth} options={(["bearer", "header", "query", "none"] as const).map((value) => ({ value, label: t(`customProvider.authModes.${value}`) }))} onChange={(e) => update("auth", e.target.value as CustomProviderDraft["auth"])} /></Field>
    {draft.auth === "header" && <Field label={t("customProvider.headerName")}><Input value={draft.headerName} onChange={(e) => update("headerName", e.target.value)} /></Field>}
    {draft.auth === "query" && <Field label={t("customProvider.queryName")}><Input value={draft.queryName} onChange={(e) => update("queryName", e.target.value)} /></Field>}
    {!anthropic && <Field label={t("customProvider.toolChoice")}><Select value={draft.toolChoice} options={(["auto", "required", "none", "omit", "passthrough"] as const).map((value) => ({ value, label: t(`customProvider.toolModes.${value}`) }))} onChange={(e) => update("toolChoice", e.target.value as CustomProviderDraft["toolChoice"])} /></Field>}
    <ListGroup><ListRow title={t("customProvider.fetchModels")} description={t("customProvider.fetchHint")} trailing={<Switch checked={draft.fetchModels} onChange={(value) => update("fetchModels", value)} aria-label={t("customProvider.fetchModels")} />} /></ListGroup>
    {draft.fetchModels && <>
      <Field label={t("customProvider.modelsPath")}><Input value={draft.modelsPath} onChange={(e) => update("modelsPath", e.target.value)} /></Field>
      <div className="grid grid-cols-2 gap-3">
        <Field label={t("customProvider.listPath")}><Input value={draft.listPath} onChange={(e) => update("listPath", e.target.value)} /></Field>
        <Field label={t("customProvider.idPath")}><Input value={draft.idPath} onChange={(e) => update("idPath", e.target.value)} /></Field>
        <Field label={t("customProvider.namePath")}><Input value={draft.namePath} onChange={(e) => update("namePath", e.target.value)} /></Field>
        <Field label={t("customProvider.descriptionPath")}><Input value={draft.descriptionPath} onChange={(e) => update("descriptionPath", e.target.value)} /></Field>
      </div>
      <Field label={t("customProvider.contextPath")} hint={t("customProvider.optionalPath")}><Input value={draft.contextPath} onChange={(e) => update("contextPath", e.target.value)} /></Field>
    </>}
    <Field label={t("customProvider.systemRole")} hint={t("customProvider.roleHint")}><Input value={draft.systemRole} placeholder={roleNames.system} onChange={(e) => update("systemRole", e.target.value)} /></Field>
    <div className="grid grid-cols-2 gap-3">
      <Field label={t("customProvider.userRole")}><Input value={draft.userRole} placeholder={roleNames.user} onChange={(e) => update("userRole", e.target.value)} /></Field>
      <Field label={t("customProvider.assistantRole")}><Input value={draft.assistantRole} placeholder={roleNames.assistant} onChange={(e) => update("assistantRole", e.target.value)} /></Field>
    </div>
    <ListGroup>
      <ListRow title={t("customProvider.supportsStreaming")} trailing={<Switch checked={draft.supportsStreaming} onChange={(value) => update("supportsStreaming", value)} aria-label={t("customProvider.supportsStreaming")} />} />
      <ListRow title={t("customProvider.mergeMessages")} trailing={<Switch checked={draft.mergeMessages} onChange={(value) => update("mergeMessages", value)} aria-label={t("customProvider.mergeMessages")} />} />
      <ListRow title={t("customProvider.templateKwargs")} description={t("customProvider.templateHint")} trailing={<Switch checked={draft.templateKwargs} onChange={(value) => update("templateKwargs", value)} aria-label={t("customProvider.templateKwargs")} />} />
      <ListRow title={t("customProvider.allowInvalidTls")} description={t("customProvider.tlsHint")} tone="warning" trailing={<Switch checked={allowInvalidTls} onChange={onTlsChange} aria-label={t("customProvider.allowInvalidTls")} />} />
    </ListGroup>
  </div>;
}
