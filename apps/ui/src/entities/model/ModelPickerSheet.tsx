import type { ReactNode } from "react";
import { ArrowRight, Check, LayoutGrid, List, Search, SlidersHorizontal } from "lucide-react";
import { Trans, useTranslation } from "react-i18next";
import { Chip, cn, icon, Input, Sheet, Skeleton } from "@lettuceai/crisp";
import type { RemoteModelContract } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { ModelMark } from "./PickerModelMark";
import { PriceLine } from "./PriceLine";
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

function FreeBadge() {
  const { t } = useTranslation("onboarding");
  return (
    <span className="rounded-full bg-accent/15 px-2 py-0.5 text-2xs font-semibold text-accent">{t("model.free")}</span>
  );
}

const skeletonRows = 8;

function ListRow({ selected, onClick, leading, title, detail, meta, trailing }: { selected?: boolean; onClick: () => void; leading: ReactNode; title: ReactNode; detail: ReactNode; meta?: ReactNode; trailing?: ReactNode }) {
  return (
    <button
      type="button"
      aria-pressed={selected ?? false}
      onClick={onClick}
      className={cn("flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors hover:bg-fill", selected && "bg-fill")}
    >
      {leading}
      <span className="min-w-0 flex-1">
        <span className="block truncate text-base font-medium text-fg">{title}</span>
        <span className="block truncate font-mono text-xs text-fg-3">{detail}</span>
      </span>
      {meta && <span className="shrink-0 text-right text-xs leading-5 tabular-nums text-fg-3">{meta}</span>}
      {trailing}
    </button>
  );
}

function ListSkeleton() {
  return (
    <div aria-hidden className="divide-y divide-line overflow-hidden rounded-xl border border-line">
      {Array.from({ length: skeletonRows }, (_, index) => (
        <div key={index} className="flex items-center gap-3 px-3 py-2.5">
          <Skeleton className="h-9 w-9 shrink-0 rounded-lg" />
          <span className="min-w-0 flex-1 space-y-1.5">
            <Skeleton className="h-4 w-2/5" />
            <Skeleton className="h-3 w-3/5" />
          </span>
          <span className="flex shrink-0 flex-col items-end gap-1.5">
            <Skeleton className="h-3 w-24" />
            <Skeleton className="h-3 w-10" />
          </span>
        </div>
      ))}
    </div>
  );
}

export function ModelPickerSheet(props: ModelPickerSheetProps) {
  const { t, i18n } = useTranslation("onboarding");
  const { phone, shown, query, selectedId, providerKind } = props;
  const trimmed = query.trim();
  const useIdRow = trimmed && !shown.some((model) => model.id === trimmed) ? (
    <ListRow
      onClick={() => props.onUseId(trimmed)}
      leading={<span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-fill-2 text-fg-3"><ArrowRight size={icon.md} /></span>}
      title={<Trans t={t} i18nKey="model.picker.useId" values={{ id: trimmed }} components={{ mono: <span className="font-mono" /> }} />}
      detail={<span className="font-sans">{t("model.picker.useIdBody")}</span>}
    />
  ) : null;
  const context = (model: RemoteModelContract) =>
    model.context_length === null ? null : formatContext(model.context_length, i18n.language);

  return (
    <Sheet open={props.open} onClose={props.onClose} title={t("model.picker.title", { name: props.routeName })} size="md">
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
              "relative flex h-11 w-11 shrink-0 items-center justify-center rounded-xl border transition-colors",
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
            <div role="radiogroup" aria-label={t("model.picker.layout")} className="flex h-11 shrink-0 items-center rounded-xl border border-line-2 bg-fill p-1">
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
                    "flex h-full w-9 items-center justify-center rounded-lg transition-colors",
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
          <div className="flex flex-wrap gap-2">
            <Chip selected={props.onlyFree} aria-pressed={props.onlyFree} onClick={() => props.onOnlyFree(!props.onlyFree)}>{t("model.picker.onlyFree")}</Chip>
          </div>
        )}
        {props.loading ? (
          <div role="status" aria-label={t("model.picker.loading")}><ListSkeleton /></div>
        ) : props.error ? (
          <FailureState error={props.error} onRetry={props.onRetry} size="sm" />
        ) : !phone && props.grid ? (
          <div className="space-y-2">
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
          {useIdRow && <div className="overflow-hidden rounded-xl border border-line">{useIdRow}</div>}
          </div>
        ) : (
          (shown.length > 0 || useIdRow) && (
            <div className="divide-y divide-line overflow-hidden rounded-xl border border-line">
              {shown.map((model) => {
                const price = priceOf(model);
                const size = context(model);
                const selected = selectedId === model.id;
                return (
                  <ListRow
                    key={model.id}
                    selected={selected}
                    onClick={() => props.onChoose(model)}
                    leading={<ModelMark modelId={model.id} providerKind={providerKind} />}
                    title={
                      <span className="flex min-w-0 items-center gap-2">
                        <span className="truncate">{modelName(model)}</span>
                        {isFree(model) && <FreeBadge />}
                      </span>
                    }
                    detail={model.id}
                    meta={(price && !isFree(model)) || size ? (
                      <>
                        {price && !isFree(model) && <span className="block text-fg-2"><PriceLine model={model} /></span>}
                        {size && <span className="block">{t("model.contextSuffix", { value: size })}</span>}
                      </>
                    ) : undefined}
                    trailing={<Check size={icon.md} strokeWidth={2.5} className={cn("shrink-0 text-accent", !selected && "invisible")} />}
                  />
                );
              })}
              {useIdRow}
            </div>
          )
        )}
        {!props.loading && !props.error && shown.length === 0 && !trimmed && (
          <p className="px-2 py-6 text-center text-sm text-fg-3">{t("model.picker.none")}</p>
        )}
        <p className="text-sm text-fg-3">{t("model.picker.priceNote", { name: props.routeName })}</p>
      </div>
    </Sheet>
  );
}
