import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Button, CopyButton, LoadingState, Sheet, useCompactLayout } from "@lettuceai/crisp";
import type { SettingsModelSettingsLayer } from "@/api/generated/bindings";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { EditorRow } from "@/shared/ui/EditorRow";
import { FailureState } from "@/shared/ui/FailureState";
import { JinjaEditor } from "@/shared/ui/JinjaEditor";
import { useSettingsApi } from "./data";
import { effectiveField, readField, writeField, type ModelConfig } from "./model-config";

export function ModelTemplate({ path, config, defaults, onChange }: { path: string; config: ModelConfig; defaults: SettingsModelSettingsLayer | undefined; onChange: (config: ModelConfig) => void }) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const phone = useCompactLayout();
  const [mode, setMode] = useState<"view" | "edit" | null>(null);
  const { shown } = useClosingValue(mode);
  const [draft, setDraft] = useState("");
  const value = effectiveField(config, defaults, "chat", { store: "llama", key: "chat_template_override" });
  const own = readField(config, "chat", "llama", "chat_template_override");
  const template = useQuery({ queryKey: ["llama", "template", path], queryFn: () => api.call("llamaChatTemplate", { target: { type: "file", model: { uri: path } } }), enabled: mode === "view" && Boolean(path) });
  const edit = (source: string) => { setDraft(source); setMode("edit"); };
  const apply = () => {
    let next = writeField(config, "chat", "llama", "chat_template_override", draft.trim() ? draft : undefined);
    if (draft.trim()) next = writeField(next, "chat", "llama", "chat_template_preset", "custom");
    onChange(next); setMode(null);
  };
  const label = t("modelEditor.fields.llamaChatTemplateOverride");
  const code = shown === "edit" ? draft : template.data?.template ?? "";
  return <>
    <div className="px-4 py-3"><Button variant="ghost" size="sm" onClick={() => setMode("view")}>{t("modelEditor.templateView")}</Button></div>
    <EditorRow label={label} overridden={own.overridden} onReset={() => onChange(writeField(config, "chat", "llama", "chat_template_override", undefined))} resetLabel={t("modelEditor.resetField", { field: label })}>
      <Button variant="secondary" className="w-full" onClick={() => edit(typeof value === "string" ? value : template.data?.template ?? "")}>{t("modelEditor.templateEdit")}</Button>
    </EditorRow>
    <Sheet open={mode !== null} onClose={() => setMode(null)} title={t(shown === "edit" ? "modelEditor.fields.llamaChatTemplateOverride" : "modelEditor.templateTitle")} placement={phone ? "bottom" : "side"} size="lg" className="[&>.overscroll-contain]:flex [&>.overscroll-contain]:flex-1 [&>.overscroll-contain]:overflow-hidden" {...(phone ? { detents: [0.9] } : {})} footer={<div className="flex flex-wrap items-center justify-end gap-2">
      <CopyButton value={code} label={t("modelEditor.templateCopy")} copiedLabel={t("modelEditor.templateCopied")} variant="ghost" disabled={!code} />
      {shown === "edit" ? <><Button variant="ghost" onClick={() => setMode(null)}>{t("manage.cancel")}</Button><Button onClick={apply}>{t("modelEditor.templateApply")}</Button></> : <Button disabled={!template.data} onClick={() => edit(template.data?.template ?? "")}>{t("modelEditor.templateUseCustom")}</Button>}
    </div>}>
      {shown === "view" && template.isPending && <LoadingState />}
      {shown === "view" && template.isError && <FailureState error={template.error} onRetry={() => void template.refetch()} />}
      {(shown === "edit" || template.data) && <JinjaEditor label={shown === "edit" ? label : t("modelEditor.templateTitle")} value={code} readOnly={shown !== "edit"} onChange={setDraft} />}
    </Sheet>
  </>;
}
