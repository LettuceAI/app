import { LayoutGrid, List, Search, SlidersHorizontal } from "lucide-react";
import { Trans, useTranslation } from "react-i18next";
import { cn, icon, Input, LoadingState, Sheet, Switch } from "@lettuceai/crisp";
import type { RemoteModelContract } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { ModelMark } from "./PickerModelMark";
import { PriceLine } from "./PriceLine";
import { Row, Rows } from "@/shared/ui/SelectionRows";
import { formatContext } from "./format";
import { isFree, modelName, priceOf } from "./remote-models";

export interface ModelPickerSheetProps {
  open: boolean;
  onClose: () => void;
  phone: boolean;
  routeName: string;
  providerKind: string | null;
  loading: boolean;
  error: unknown;
  onRetry: () => void;
  total: number;
  shown: readonly RemoteModelContract[];
  query: string;
  onQuery: (value: string) => void;
  showFree: boolean;
  onlyFree: boolean;
  onOnlyFree: (value: boolean) => void;
  grid: boolean;
  onGrid: (value: boolean) => void;
  activeFilters: number;
  onOpenFilters: () => void;
  selectedId: string | null;
  onChoose: (model: RemoteModelContract) => void;
  onUseId: (id: string) => void;
}

const separator = " · ";

function FreeBadge() {
  const { t } = useTranslation("onboarding");
  return (
    <span className="rounded-full bg-accent/15 px-2 py-0.5 text-2xs font-semibold text-accent">{t("model.free")}</span>
  );
}

export function ModelPickerSheet(props: ModelPickerSheetProps) {
  const { t, i18n } = useTranslation("onboarding");
  const { phone, shown, query, selectedId, providerKind } = props;
  const trimmed = query.trim();
  const context = (model: RemoteModelContract) =>
    model.context_length === null ? null : formatContext(model.context_length, i18n.language);

  return (
    <Sheet open={props.open} onClose={props.onClose} title={t("model.picker.title", { name: props.routeName })} size="lg">
      <div className="space-y-3">
        <div className="flex items-center gap-2">
          <div className="min-w-0 flex-1">
            <Input
              value={query}
              onChange={(event) => props.onQuery(event.target.value)}
              placeholder={t("model.picker.search", { count: props.total })}
              leading={<Search size={icon.sm} />}
            />
          </div>
          <button
            type="button"
            aria-label={t("model.picker.filter")}
            onClick={props.onOpenFilters}
            className={cn(
              "relative flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border transition-colors",
              props.activeFilters ? "border-accent/50 bg-accent/15 text-accent" : "border-line-2 bg-fill text-fg-2 hover:bg-fill-2",
            )}
          >
            <SlidersHorizontal size={icon.md} />
            {props.activeFilters > 0 && (
              <span className="absolute -right-1 -top-1 flex h-4 min-w-4 items-center justify-center rounded-full bg-accent px-1 text-2xs font-bold text-on-accent">
                {props.activeFilters}
              </span>
            )}
          </button>
          {!phone && (
            <div role="radiogroup" aria-label={t("model.picker.layout")} className="flex h-10 shrink-0 items-center rounded-xl border border-line-2 bg-fill p-1">
              {(
                [
                  [false, List, "list"],
                  [true, LayoutGrid, "twoPerRow"],
                ] as const
              ).map(([value, Glyph, label]) => (
                <button
                  key={label}
                  type="button"
                  role="radio"
                  aria-checked={props.grid === value}
                  aria-label={t(`model.picker.${label}`)}
                  onClick={() => props.onGrid(value)}
                  className={cn(
                    "flex h-full w-8 items-center justify-center rounded-lg transition-colors",
                    props.grid === value ? "bg-fill-3 text-fg" : "text-fg-3 hover:text-fg-2",
                  )}
                >
                  <Glyph size={icon.md} />
                </button>
              ))}
            </div>
          )}
        </div>
        {props.showFree && (
          <label className="flex cursor-pointer items-center justify-between gap-3 rounded-xl border border-line bg-fill px-3.5 py-2.5">
            <span className="text-sm text-fg">{t("model.picker.onlyFree")}</span>
            <Switch checked={props.onlyFree} onChange={props.onOnlyFree} aria-label={t("model.picker.onlyFree")} />
          </label>
        )}
        {props.loading ? (
          <LoadingState label={t("model.picker.loading")} />
        ) : props.error ? (
          <FailureState error={props.error} onRetry={props.onRetry} size="sm" />
        ) : !phone && props.grid ? (
          <div className="grid grid-cols-2 gap-2">
            {shown.map((model) => {
              const price = priceOf(model);
              return (
                <button
                  key={model.id}
                  type="button"
                  aria-pressed={selectedId === model.id}
                  onClick={() => props.onChoose(model)}
                  className={cn(
                    "flex flex-col gap-3 rounded-xl border p-3.5 text-left transition-colors",
                    selectedId === model.id ? "border-accent bg-accent/10" : "border-line bg-fill hover:bg-fill-2",
                  )}
                >
                  <span className="flex items-center gap-3">
                    <ModelMark modelId={model.id} providerKind={providerKind} />
                    <span className="min-w-0 flex-1">
                      <span className="flex min-w-0 items-center gap-2 text-base text-fg">
                        <span className="truncate">{modelName(model)}</span>
                        {isFree(model) && <FreeBadge />}
                      </span>
                      <span className="block truncate font-mono text-xs text-fg-3">{model.id}</span>
                    </span>
                  </span>
                  <span className="flex items-center justify-between text-sm tabular-nums text-fg-3">
                    <span>{price && !isFree(model) ? <PriceLine model={model} /> : isFree(model) ? t("model.free") : ""}</span>
                    <span>{context(model)}</span>
                  </span>
                </button>
              );
            })}
          </div>
        ) : (
          <Rows>
            {shown.map((model) => {
              const price = priceOf(model);
              const size = context(model);
              return (
                <Row
                  key={model.id}
                  selected={selectedId === model.id}
                  onClick={() => props.onChoose(model)}
                  leading={<ModelMark modelId={model.id} providerKind={providerKind} />}
                  title={
                    <span className="flex min-w-0 items-center gap-2">
                      <span className="truncate">{modelName(model)}</span>
                      {isFree(model) && <FreeBadge />}
                    </span>
                  }
                  description={
                    phone ? (
                      <span className="block">
                        <span className="block truncate font-mono text-sm">{model.id}</span>
                        <span className="mt-0.5 block text-sm tabular-nums text-fg-3">
                          {price && !isFree(model) && <PriceLine model={model} />}
                          {price && !isFree(model) && size && separator}
                          {size && t("model.contextSuffix", { value: size })}
                        </span>
                      </span>
                    ) : (
                      <span className="font-mono text-sm">{model.id}</span>
                    )
                  }
                  trailing={
                    phone ? undefined : (
                      <span className="shrink-0 text-right text-sm tabular-nums text-fg-2">
                        {price && (
                          <>
                            <PriceLine model={model} />
                            <br />
                          </>
                        )}
                        <span className="text-fg-3">{size}</span>
                      </span>
                    )
                  }
                />
              );
            })}
          </Rows>
        )}
        {!props.loading && ((shown.length === 0 && !props.error && !trimmed) || trimmed) && (
          <Rows>
            {shown.length === 0 && !props.error && !trimmed && (
              <p className="px-2 py-6 text-center text-sm text-fg-3">{t("model.picker.none")}</p>
            )}
            {trimmed && !shown.some((model) => model.id === trimmed) && (
              <Row
                indicator="none"
                title={<Trans t={t} i18nKey="model.picker.useId" values={{ id: trimmed }} components={{ mono: <span className="font-mono" /> }} />}
                description={t("model.picker.useIdBody")}
                onClick={() => props.onUseId(trimmed)}
              />
            )}
          </Rows>
        )}
        <p className="text-sm text-fg-3">{t("model.picker.priceNote", { name: props.routeName })}</p>
      </div>
    </Sheet>
  );
}
