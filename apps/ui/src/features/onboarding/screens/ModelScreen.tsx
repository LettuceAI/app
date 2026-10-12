import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Check, ChevronDown } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Field, icon, Input } from "@lettuceai/crisp";
import type { RemoteModelContract } from "@/api/generated/bindings";
import { ProviderMark } from "@/entities/provider";
import { ActionError } from "../components/ActionError";
import { ModelMark } from "../components/ModelMark";
import { ActionButton, ScreenTitle } from "../components/ui";
import { formatContext, formatPrice } from "../data/format";
import {
  activeFilterCount,
  defaultDisplayName,
  externalModelId,
  filterModels,
  modelName,
  priceOf,
  noFilters,
  acceptsImages,
  vendorNames,
  type ModelFilters,
} from "../data/models";
import { providerModelsQuery, useApi } from "../data/queries";
import { useDescriptor, useOnboarding } from "../flow/context";
import { useRouteName } from "../flow/route-name";
import { ModelFilterSheet } from "../sheets/ModelFilterSheet";
import { ModelPickerSheet } from "../sheets/ModelPickerSheet";

const noModels: readonly RemoteModelContract[] = [];

export function ModelScreen() {
  const { t, i18n } = useTranslation("onboarding");
  const api = useApi();
  const { state, dispatch, phone, saveModel } = useOnboarding();
  const descriptor = useDescriptor(state.providerKind);
  const routeName = useRouteName();
  const lists = descriptor?.lists_models ?? false;
  const accountId = state.account?.id ?? null;

  const modelsQuery = useQuery({
    ...providerModelsQuery(api, accountId ?? ""),
    enabled: lists && accountId !== null,
  });
  const models = modelsQuery.data ?? noModels;

  const [picker, setPicker] = useState(false);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [filtersKey, setFiltersKey] = useState(0);
  const [query, setQuery] = useState("");
  const [onlyFree, setOnlyFree] = useState(false);
  const [grid, setGrid] = useState(false);
  const [filters, setFilters] = useState<ModelFilters>(noFilters);

  const shown = useMemo(() => filterModels(models, { text: query, onlyFree, filters }), [models, query, onlyFree, filters]);
  const vendors = useMemo(() => vendorNames(models), [models]);
  const picked = state.model;
  const chosenId = externalModelId(state);

  const choose = (model: RemoteModelContract) => {
    dispatch({ type: "pickModel", model, displayName: defaultDisplayName(model) });
    setPicker(false);
    setQuery("");
  };
  const useId = (id: string) => {
    dispatch({ type: "pickManual", id });
    setPicker(false);
    setQuery("");
  };

  const mark = (modelId: string) => <ModelMark modelId={modelId} providerKind={state.providerKind} />;

  const modelField = lists ? (
    <Field label={t("model.model")}>
      <button
        type="button"
        onClick={() => setPicker(true)}
        className="flex min-h-12 w-full items-center gap-3 rounded-xl border border-line-2 bg-fill px-3.5 py-2 text-left transition-colors hover:border-line-3"
      >
        {chosenId && mark(chosenId)}
        <span className="min-w-0 flex-1">
          {chosenId ? (
            <>
              <span className="block truncate text-base text-fg">{picked ? modelName(picked) : chosenId}</span>
              <span className="block truncate font-mono text-sm text-fg-3">{picked ? picked.id : t("model.customId")}</span>
            </>
          ) : (
            <span className="block text-base text-fg-3">{t("model.choose", { count: models.length })}</span>
          )}
        </span>
        <ChevronDown size={icon.md} className="shrink-0 text-fg-3" />
      </button>
    </Field>
  ) : (
    <Field label={t("model.modelId")} hint={t("model.modelIdHint")}>
      <Input
        value={state.manualId}
        onChange={(event) => dispatch({ type: "patch", patch: { manualId: event.target.value, model: null } })}
        placeholder={t("model.modelIdPlaceholder")}
        className="font-mono"
        data-autofocus
      />
    </Field>
  );

  const form = (
    <div className="max-w-md space-y-5">
      {modelField}
      <Field label={t("model.displayName")} hint={t("model.displayNameHint")}>
        <Input
          value={state.displayName}
          onChange={(event) => dispatch({ type: "patch", patch: { displayName: event.target.value } })}
          placeholder={picked ? modelName(picked) : t("model.displayNamePlaceholder")}
        />
      </Field>
    </div>
  );

  const title = <ScreenTitle title={t("model.title")} lead={t("model.lead", { name: routeName })} />;
  const price = picked ? priceOf(picked) : null;

  const aside = (
    <div className="space-y-4">
      <div className="flex items-center gap-3 rounded-2xl border border-line bg-fill p-4">
        <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-fill-2">
          {descriptor && <ProviderMark kind={descriptor.kind} name={descriptor.display_name} />}
        </span>
        <span className="min-w-0 flex-1">
          <span className="block truncate text-base text-fg">
            {state.label || t("connect.form.namePlaceholder", { name: routeName })}
          </span>
          <span className="block truncate text-sm text-fg-3">{routeName}</span>
        </span>
        <span className="flex items-center gap-1.5 text-sm text-accent">
          <Check size={icon.sm} />
          {t("model.connected")}
        </span>
      </div>
      {picked ? (
        <div className="rounded-2xl border border-line bg-fill p-5">
          <div className="flex items-center gap-3">
            {mark(picked.id)}
            <span className="min-w-0 flex-1">
              <span className="block truncate text-lg font-semibold text-fg">{modelName(picked)}</span>
              <span className="block truncate font-mono text-sm text-fg-3">{picked.id}</span>
            </span>
          </div>
          <dl className="mt-5 grid grid-cols-2 gap-x-6 gap-y-4">
            {picked.context_length !== null && (
              <div>
                <dt className="text-xs text-fg-3">{t("model.context")}</dt>
                <dd className="mt-0.5 text-base text-fg">
                  {t("model.contextTokens", { value: formatContext(picked.context_length, i18n.language) })}
                </dd>
              </div>
            )}
            <div>
              <dt className="text-xs text-fg-3">{t("model.images")}</dt>
              <dd className="mt-0.5 text-base text-fg">{acceptsImages(picked) ? t("model.yes") : t("model.no")}</dd>
            </div>
            {price && (
              <div>
                <dt className="text-xs text-fg-3">{t("model.inputPerMillion")}</dt>
                <dd className="mt-0.5 text-base tabular-nums text-fg">{formatPrice(price.input, i18n.language)}</dd>
              </div>
            )}
            {price && (
              <div>
                <dt className="text-xs text-fg-3">{t("model.outputPerMillion")}</dt>
                <dd className="mt-0.5 text-base tabular-nums text-fg">{formatPrice(price.output, i18n.language)}</dd>
              </div>
            )}
          </dl>
        </div>
      ) : (
        <div className="flex h-48 flex-col items-center justify-center gap-1 rounded-2xl border border-dashed border-line-2 px-6 text-center">
          <p className="text-base text-fg-2">{t("model.emptyTitle")}</p>
          <p className="text-sm text-fg-3">{t("model.emptyBody")}</p>
        </div>
      )}
    </div>
  );

  const sheets = lists && (
    <>
      <ModelPickerSheet
        open={picker}
        onClose={() => setPicker(false)}
        phone={phone}
        routeName={routeName}
        providerKind={state.providerKind}
        loading={modelsQuery.isPending && accountId !== null}
        error={modelsQuery.error}
        onRetry={() => void modelsQuery.refetch()}
        total={models.length}
        shown={shown}
        query={query}
        onQuery={setQuery}
        showFree={state.providerKind === "openrouter"}
        onlyFree={onlyFree}
        onOnlyFree={setOnlyFree}
        grid={grid}
        onGrid={setGrid}
        activeFilters={activeFilterCount(filters)}
        onOpenFilters={() => {
          setFiltersKey((key) => key + 1);
          setFiltersOpen(true);
        }}
        selectedId={picked?.id ?? null}
        onChoose={choose}
        onUseId={useId}
      />
      <ModelFilterSheet
        key={filtersKey}
        open={filtersOpen}
        onClose={() => setFiltersOpen(false)}
        applied={filters}
        vendors={vendors}
        countFor={(draft) => filterModels(models, { text: query, onlyFree, filters: draft }).length}
        onApply={(next) => {
          setFilters(next);
          setFiltersOpen(false);
        }}
      />
    </>
  );

  if (phone) {
    return (
      <>
        {title}
        {form}
        <div className="mt-4 max-w-md">
          <ActionError error={saveModel.error} busy={saveModel.busy} onRetry={saveModel.run} />
        </div>
        {sheets}
      </>
    );
  }
  return (
    <div className="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)] items-start gap-12">
      <div>
        {title}
        {form}
        <div className="mt-8 max-w-md space-y-3">
          <ActionError error={saveModel.error} busy={saveModel.busy} onRetry={saveModel.run} />
          <ActionButton
            label={saveModel.busy ? t("actions.saving") : t("actions.useModel")}
            disabled={saveModel.busy || chosenId === ""}
            loading={saveModel.busy}
            block
            onClick={saveModel.run}
          />
        </div>
      </div>
      <div className="pt-2">{aside}</div>
      {sheets}
    </div>
  );
}
