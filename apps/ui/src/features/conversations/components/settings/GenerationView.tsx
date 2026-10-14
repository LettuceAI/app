import { Fragment, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { RotateCcw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, icon, ListGroup, LoadingState } from "@lettuceai/crisp";
import type { ProviderAccountView, SettingsModelSettingsLayer } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { ModelEditorField } from "@/entities/model/ModelEditorField";
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

  return (
    <div className="space-y-6 pb-4">
      <p className="text-sm text-fg-muted">{t("settings.generation.hint")}</p>
      {!model && <p className="text-sm text-fg-muted">{t("settings.generation.noModel")}</p>}
      {sections.map((section) => (
        <div key={section.id} className="space-y-3">
          <p className="text-xs font-semibold text-fg-3">{settingsText(`modelEditor.sections.${section.id}` as never)}</p>
          {section.groups.map((group, index) => {
            const toggled = group.toggle ? effectiveField(config, inherited, "chat", editorFields[group.toggle]) : true;
            return (
              <ListGroup key={index} label={group.label ? settingsText(`modelEditor.groups.${group.label}` as never) : undefined}>
                {group.toggle && <ModelEditorField kind={kind} runtimeDefault={undefined} id={group.toggle} config={config} defaults={inherited} layer="chat" onChange={setDraft} onFile={noop} onInvalid={noop} />}
                {(!group.toggle || toggled === true || toggled === "enabled") && (
                  <>
                    {group.fields.filter(supports).map((id) => (
                      <Fragment key={id}>
                        <ModelEditorField
                          kind={kind}
                          runtimeDefault={undefined}
                          id={id}
                          config={config}
                          defaults={inherited}
                          layer="chat"
                          onChange={setDraft}
                          onFile={noop}
                          onInvalid={(key, value) => setInvalid((state) => ({ ...state, [key]: value }))}
                        />
                      </Fragment>
                    ))}
                  </>
                )}
              </ListGroup>
            );
          })}
        </div>
      ))}
      <div className="sticky bottom-0 flex flex-wrap gap-2 border-t border-line bg-surface py-3">
        <Button variant="ghost" leading={<RotateCcw size={icon.sm} />} disabled={ctx.busy || count === 0} onClick={() => {
            setDraft(empty);
            setInvalid({});
          }}>{t("settings.generation.resetAll")}</Button>
        <div className="flex-1" />
        <Button variant="secondary" disabled={ctx.busy || !dirty} onClick={() => {
            setDraft(null);
            setInvalid({});
          }}>{t("settings.generation.discard")}</Button>
        <Button variant="primary" disabled={ctx.busy || !dirty || hasInvalid} onClick={() => void ctx.save({ model_settings: layerOf(config) }).then((ok) => {
            if (!ok) return;
            setDraft(null);
            setInvalid({});
          })}>{t("note.save")}</Button>
      </div>
    </div>
  );
}
