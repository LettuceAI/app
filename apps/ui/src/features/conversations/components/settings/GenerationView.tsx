import { useState } from "react";
import { createPortal } from "react-dom";
import { useQuery } from "@tanstack/react-query";
import { RotateCcw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, icon, ListGroup, LoadingState } from "@lettuceai/crisp";
import type { ProviderAccountView, SettingsModelSettingsLayer } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { ModelMark } from "@/entities/model/PickerModelMark";
import { ModelEditorField } from "@/entities/model/ModelEditorField";
import { ModelEditorReveal } from "@/entities/model/ModelEditorReveal";
import { effectiveField, layerOf, mergeLayers, overrideCount, parseModelConfig, type ModelConfig } from "@/entities/model/model-config";
import { editorFields, type FieldId } from "@/entities/model/model-editor-fields";
import { editorSections, type EditorKind } from "@/entities/model/model-editor-sections";
import { FailureState } from "@/shared/ui/FailureState";
import { accountsQuery, modelsQuery } from "../../data/queries";
import type { PanelCtx } from "./context";

const sectionsFor: Record<EditorKind, readonly string[]> = {
  local: ["generation", "memory", "performance"],
  ollama: ["generation", "server"],
  cloud: ["generation", "reasoning", "caching"],
  image: [],
};

function kindOf(account: ProviderAccountView | undefined): EditorKind {
  return account?.provider_kind === "llamacpp" ? "local" : account?.provider_kind === "ollama" ? "ollama" : "cloud";
}

const noop = () => undefined;

const emptyLayer = { chat_parameters: {}, llama_cpp: {}, stable_diffusion: {} } as unknown as SettingsModelSettingsLayer;

export function GenerationView({ ctx }: { ctx: PanelCtx }) {
  const { t } = useTranslation("conversations");
  const { t: settingsText } = useTranslation("settings");
  const models = useQuery(modelsQuery(ctx.api));
  const accounts = useQuery(accountsQuery(ctx.api));
  const app = useQuery({ queryKey: queryKeys.settings.detail(), queryFn: () => ctx.api.call("settingsGet") });
  const own = ctx.settings ? (ctx.settings.model_settings ?? emptyLayer) : null;
  const [draft, setDraft] = useState<ModelConfig | null>(null);
  const [invalid, setInvalid] = useState<Record<string, boolean>>({});

  const failed = models.error ?? accounts.error ?? app.error;
  if (failed) return <FailureState error={failed} onRetry={() => void Promise.all([models.refetch(), accounts.refetch(), app.refetch()])} />;
  if (!models.data || !accounts.data || !app.data || !own) return <LoadingState />;

  const model = models.data.models.find((entry) => entry.id === ctx.settings?.model.id);
  const modelConfig = model ? parseModelConfig(model.config) : null;
  const account = accounts.data.find((entry) => entry.id === model?.provider_account_id);
  const kind = kindOf(account);
  const saved = parseModelConfig({ ...(modelConfig ?? {}), chat_parameters: own.chat_parameters, llama_cpp: own.llama_cpp, stable_diffusion: own.stable_diffusion, feature_parameters: {} });
  const config = draft ?? saved;
  const inherited = mergeLayers(app.data.sampler_defaults, modelConfig ? layerOf(modelConfig) : undefined);
  const dirty = JSON.stringify(layerOf(config)) !== JSON.stringify(layerOf(saved));
  const count = overrideCount(layerOf(config));
  const hasInvalid = Object.values(invalid).some(Boolean);
  const supports = (id: FieldId) => {
    if (id === "llamaKvTypeK" || id === "llamaKvTypeV") return effectiveField(config, inherited, "chat", editorFields.llamaKvType) === "split";
    if (kind !== "cloud" || !modelConfig) return true;
    const support = modelConfig.capabilities.parameter_support as Record<string, string>;
    return support[editorFields[id].key as string] !== "unsupported";
  };
  const sections = editorSections(kind, false).filter((section) => sectionsFor[kind].includes(section.id));
  const empty = parseModelConfig({ ...(modelConfig ?? {}), chat_parameters: {}, llama_cpp: {}, stable_diffusion: {}, feature_parameters: {} });
  const field = (id: FieldId) => <ModelEditorField
    key={id}
    kind={kind}
    runtimeDefault={undefined}
    id={id}
    config={config}
    defaults={inherited}
    layer="chat"
    compact
    disabled={ctx.busy}
    onChange={setDraft}
    onFile={noop}
    onInvalid={(key, value) => setInvalid((state) => ({ ...state, [key]: value }))}
  />;

  return (
    <div className="@container space-y-6 pb-2">
      <div className="space-y-3">
        <div className="flex min-w-0 items-center justify-between gap-3">
          <div className="flex min-w-0 flex-1 items-center gap-3">
            {model && <ModelMark modelId={model.external_model_id} providerKind={account?.provider_kind ?? null} />}
            <div className="min-w-0">
              {model && <p className="truncate text-sm font-medium text-fg" title={model.display_name}>{model.display_name}</p>}
              <p className="text-xs text-fg-3">{t("settings.generation.scope")}</p>
            </div>
          </div>
          <Button variant="ghost" size="sm" leading={<RotateCcw size={icon.sm} />} disabled={ctx.busy || count === 0} onClick={() => {
            setDraft(empty);
            setInvalid({});
          }}>{t("settings.generation.resetAll")}</Button>
        </div>
        <p className="text-sm text-fg-3">{t(model ? "settings.generation.hint" : "settings.generation.noModel")}</p>
      </div>
      <fieldset disabled={ctx.busy} className="min-w-0 space-y-6">
        {sections.map((section) => (
          <div key={section.id} className="space-y-5">
            {section.groups.map((group, index) => {
              const fields = group.fields.filter(supports);
              if (!group.toggle && fields.length === 0) return null;
              const toggled = group.toggle ? effectiveField(config, inherited, "chat", editorFields[group.toggle]) : true;
              return (
                <ListGroup key={index} label={settingsText((group.label ? `modelEditor.groups.${group.label}` : `modelEditor.sections.${section.id}`) as never)}>
                  {group.toggle && field(group.toggle)}
                  {group.toggle ? <ModelEditorReveal open={toggled === true || toggled === "enabled"}>{fields.map(field)}</ModelEditorReveal> : fields.map(field)}
                </ListGroup>
              );
            })}
          </div>
        ))}
      </fieldset>
      {ctx.footerTarget && createPortal(<div className="flex min-h-8 items-center gap-2">
        <p className={dirty ? "sr-only @sm:not-sr-only @sm:mr-auto @sm:min-w-0 @sm:flex-1 @sm:truncate @sm:text-sm @sm:text-fg-3" : "text-xs text-fg-3"} role="status">{dirty ? settingsText("modelEditor.unsaved") : count > 0 ? t("settings.generation.count", { count }) : t("settings.generation.inherited")}</p>
        {dirty && <>
          <Button variant="ghost" className="flex-1 @sm:flex-none" disabled={ctx.busy} onClick={() => {
            setDraft(null);
            setInvalid({});
          }}>{t("settings.generation.discard")}</Button>
          <Button variant="primary" className="flex-1 @sm:flex-none" loading={ctx.busy} disabled={ctx.busy || hasInvalid} onClick={() => void ctx.save({ model_settings: layerOf(config) }).then((ok) => {
            if (!ok) return;
            setDraft(null);
            setInvalid({});
          })}>{t("note.save")}</Button>
        </>}
      </div>, ctx.footerTarget)}
    </div>
  );
}
