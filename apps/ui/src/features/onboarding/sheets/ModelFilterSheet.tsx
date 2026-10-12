import { useState } from "react";
import { ArrowDownAZ, Coins, TextSearch } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, Chip, Sheet, Switch } from "@lettuceai/crisp";
import { modelVendors } from "@/entities/provider";
import { activeFilterCount, noFilters, type ModelFilters, type ModelSort } from "../data/models";

export interface ModelFilterSheetProps {
  open: boolean;
  onClose: () => void;
  applied: ModelFilters;
  vendors: readonly string[];
  countFor: (filters: ModelFilters) => number;
  onApply: (filters: ModelFilters) => void;
}

const sorts = [
  ["name", "sortName", ArrowDownAZ],
  ["price", "sortPrice", Coins],
  ["context", "sortContext", TextSearch],
] as const satisfies readonly (readonly [ModelSort, string, unknown])[];

export function ModelFilterSheet({ open, onClose, applied, vendors, countFor, onApply }: ModelFilterSheetProps) {
  const { t } = useTranslation("onboarding");
  const [draft, setDraft] = useState<ModelFilters>(applied);
  const shown = countFor(draft);

  return (
    <Sheet
      open={open}
      onClose={onClose}
      title={t("model.filters.title")}
      footer={
        <div className="flex gap-2">
          <Button variant="ghost" size="lg" disabled={activeFilterCount(draft) === 0} onClick={() => setDraft(noFilters)}>
            {t("model.filters.clear")}
          </Button>
          <Button variant="primary" size="lg" className="min-w-0 flex-1" onClick={() => onApply(draft)}>
            {t("model.filters.show", { count: shown })}
          </Button>
        </div>
      }
    >
      <div className="space-y-6">
        {vendors.length > 1 && (
          <section>
            <h3 className="mb-3 text-sm font-medium text-fg-2">{t("model.filters.madeBy")}</h3>
            <div className="flex flex-wrap gap-2">
              {vendors.map((vendor) => {
                const mark = modelVendors.find((candidate) => candidate.vendor === vendor);
                return (
                  <Chip
                    key={vendor}
                    selected={draft.vendors.includes(vendor)}
                    icon={mark ? <img src={mark.logo} alt="" className="h-3.5 w-3.5 object-contain" /> : undefined}
                    onClick={() =>
                      setDraft({
                        ...draft,
                        vendors: draft.vendors.includes(vendor)
                          ? draft.vendors.filter((entry) => entry !== vendor)
                          : [...draft.vendors, vendor],
                      })
                    }
                  >
                    {vendor}
                  </Chip>
                );
              })}
            </div>
          </section>
        )}
        <section>
          <h3 className="mb-1 text-sm font-medium text-fg-2">{t("model.filters.capabilities")}</h3>
          <label className="flex cursor-pointer items-center justify-between gap-3 py-2">
            <span>
              <span className="block text-base text-fg">{t("model.filters.imagesTitle")}</span>
              <span className="block text-sm text-fg-3">{t("model.filters.imagesBody")}</span>
            </span>
            <Switch
              checked={draft.imagesOnly}
              onChange={(value) => setDraft({ ...draft, imagesOnly: value })}
              aria-label={t("model.filters.imagesTitle")}
            />
          </label>
        </section>
        <section>
          <h3 className="mb-3 text-sm font-medium text-fg-2">{t("model.filters.sortBy")}</h3>
          <div className="flex flex-wrap gap-2">
            {sorts.map(([id, label, Glyph]) => (
              <Chip key={id} selected={draft.sort === id} icon={<Glyph size={14} />} onClick={() => setDraft({ ...draft, sort: id })}>
                {t(`model.filters.${label}`)}
              </Chip>
            ))}
          </div>
        </section>
      </div>
    </Sheet>
  );
}
