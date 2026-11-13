import { useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Badge, cn, EmptyState, Page, Segmented, Text } from "@lettuceai/crisp";
import { CHANGELOG } from "./changelog";

type Filter = "all" | "new" | "fixed";

const TOTAL = CHANGELOG.reduce((sum, area) => sum + area.items.length, 0);

/**
 * Every change in 3.0 as plain text, for anyone who wants the whole list: grouped by
 * area, searchable, with New and Fixed marked. No backdrop, nothing animated.
 */
export function AllChanges({ phone, onBack }: { phone: boolean; onBack: () => void }) {
  const { t } = useTranslation("welcome-back");
  const main = useRef<HTMLElement>(null);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const areas = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return CHANGELOG.map((area) => ({
      area: area.area,
      items: area.items.filter((item) => (filter === "all" || item.kind === filter) && (!needle || item.text.toLowerCase().includes(needle))),
    })).filter((area) => area.items.length > 0);
  }, [query, filter]);
  const shown = areas.reduce((sum, area) => sum + area.items.length, 0);
  return (
    <div className="absolute inset-0 z-30 flex flex-col bg-surface">
      <Page
        mainRef={main}
        title={t("changes.title")}
        meta={shown === TOTAL ? t("changes.count", { count: TOTAL }) : t("changes.shown", { shown, total: TOTAL })}
        onBack={onBack}
        backLabel={t("changes.back")}
        searchValue={query}
        onSearchChange={setQuery}
        searchPlaceholder={t("changes.search")}
        filters={
          <Segmented
            size="sm"
            aria-label={t("changes.filter")}
            value={filter}
            onChange={setFilter}
            options={[
              { value: "all", label: t("changes.all") },
              { value: "new", label: t("changes.new") },
              { value: "fixed", label: t("changes.fixed") },
            ]}
          />
        }
        contentClassName={cn("mx-auto w-full max-w-3xl space-y-8", phone ? "pt-2" : "pt-4")}
      >
        {areas.length === 0 ? (
          <EmptyState size="sm" title={t("changes.emptyTitle")} description={t("changes.emptyBody")} />
        ) : (
          areas.map((area) => (
            <section key={area.area} className="space-y-2">
              <div className="flex items-baseline justify-between gap-3 border-b border-line pb-2">
                <Text variant="section">{area.area}</Text>
                <Text variant="caption">{area.items.length}</Text>
              </div>
              <ul className="space-y-1.5">
                {area.items.map((item, index) => (
                  <li key={index} className="flex items-start gap-2.5 text-sm leading-relaxed text-fg-2">
                    <Badge tone={item.kind === "new" ? "accent" : "neutral"} className="mt-0.5 w-12 shrink-0 justify-center">
                      {item.kind === "new" ? t("changes.new") : t("changes.fixed")}
                    </Badge>
                    <span>{item.text}</span>
                  </li>
                ))}
              </ul>
            </section>
          ))
        )}
      </Page>
    </div>
  );
}
